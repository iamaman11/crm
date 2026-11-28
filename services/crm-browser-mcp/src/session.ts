import { DurableObject } from "cloudflare:workers";
import { history, limits, type BrowserContext, type Page } from "@cloudflare/playwright";
import { DEFAULT_BROWSER_URL, browserUrl, isPermittedBrowserRequest, redact, safePagePath } from "./browser/security";
import { siteAccessTargets, siteAccessForRequest, siteAccessCredentialsReady, siteRequestDispositionForTargets, isTrustedSiteNavigationRequest } from "./site-access";
import type { Command, Env, Target } from "./contracts";
import type { BrowserDevice } from "./browser-device";
import { readVitals, type WebVitals } from "./browser-performance";
import { AXE_SOURCE } from "./axe-source.generated";
import { MAX_VISUAL_PNG_BYTES, comparePngPixels, pngReceipt, visualThreshold } from "./browser-visual";
import { navigatePage } from "./page-navigation";
import { boundedWaitTimeout, boundedRunTimeout, compareBrowserAssertion, operationTimeout, safeAssertionValue, validateScenarioSteps } from "./browser-checks";
import { validatedUpload, transferDownload, MAX_TRANSFER_BYTES } from "./browser-files";
import { WorkspaceFileTransfers, MAX_WORKSPACE_FILE_BYTES, MAX_WORKSPACE_CHUNK_BYTES } from "./browser-transfers";
import { readSafeBrowserHistory, readSafeBrowserLimits, isOriginalPageMissing } from "./browser-recovery";
import { openBrowser, restoreBrowser, closeBrowser, ensureBrowserContext, rememberSelectedPageIdentity, type BrowserLifecycleDependencies, type BrowserLifecycleState } from "./browser/lifecycle";
import { BROWSER_RUNTIME_EPOCH, requireRuntimeEpoch } from "./runtime-epoch";
import { executeCdpCommands, validateCdpCommands } from "./browser-cdp";
import { recordingTargets, recordingHar } from "./browser-recordings";
import { completedHandoff, handoffIdentifier, handoffLiveViewUrl, isProviderHandoffActive,
  validateHandoffRequest, type HumanHandoffRecord } from "./browser-handoff";

type Result = { ok: true; data: unknown; image?: string } | { ok: false; error: string };

// One DO = one authenticated operator's browser session. The MCP transport stays stateless.
// Only in-memory browser handles are ephemeral. Official Browser Run session IDs
// are persisted in DO storage and reattached after eviction; expired sessions
// are not silently replaced until an explicit browser_open.
export class CrmBrowserSession extends DurableObject<Env> {
  private readonly lifecycle: BrowserLifecycleState = {
    browser: null,
    context: null,
    page: null,
    browserSessionId: null,
    recordingEnabled: false,
    recoveredPageMissing: false,
    tabIds: new Map<Page, string>(),
  };
  private readonly transfers = new WorkspaceFileTransfers();
  // Only the current DO session owns an optional screenshot captured at a
  // failed scenario step. Large image bytes are retrieved separately.
  private lastFailureScreenshot: { image: string; capturedPath: string } | null = null;
  private nextOperation: Promise<void> = Promise.resolve();
  private pendingOperations = 0;
  private acceptingDialog = false;
  private ownerDirectedSiteNavigationOrigin: string | null = null;
  // The original Playwright context remains the sole CDP owner while the
  // human controls the page. Completion events are checkpointed to DO SQLite.
  private handoffCdp: Awaited<ReturnType<BrowserContext["newCDPSession"]>> | null = null;
  private consoleLog: Array<{ level: string; text: string }> = [];
  private requestStartedAt = new WeakMap<object, number>();
  private requestLog: Array<{
    method: string; path: string; status: number;
    resourceType?: string; durationMs?: number | null; failure?: string;
  }> = [];

  private target(target: Target) {
    if (!this.lifecycle.page) throw new Error("CRM_BROWSER_NOT_OPEN");
    if (target.frameSelector !== undefined && target.frameIndex !== undefined) {
      throw new Error("BROWSER_FRAME_TARGET_CONFLICT");
    }
    if (target.frameSelector !== undefined &&
        (!target.frameSelector || target.frameSelector.length > 512)) {
      throw new Error("BROWSER_FRAME_SELECTOR_INVALID");
    }
    const scope = target.frameSelector !== undefined ?
      this.lifecycle.page.frameLocator(target.frameSelector) :
      target.frameIndex === undefined ? this.lifecycle.page : this.lifecycle.page.frames()[target.frameIndex];
    if (!scope) throw new Error("BROWSER_FRAME_NOT_FOUND");
    if (target.kind === "role") {
      return scope.getByRole(target.value as Parameters<Page["getByRole"]>[0], {
        name: target.name, exact: true,
      });
    }
    if (target.kind === "label") return scope.getByLabel(target.value, { exact: true });
    if (target.kind === "text") return scope.getByText(target.value, { exact: true });
    return scope.locator(target.value);
  }

  // Both Open and navigate use one Playwright page owner. Service access, if
  // configured, is an optional origin-bound transport; interactive login is normal.
  private siteAccess() {
    return siteAccessTargets([
      {
        origin: this.env.SITE_ACCESS_ORIGIN,
        clientId: this.env.SITE_ACCESS_CLIENT_ID,
        clientSecret: this.env.SITE_ACCESS_CLIENT_SECRET,
      },
      {
        origin: this.env.SITE_ACCESS_PREVIEW_1_ORIGIN,
        clientId: this.env.SITE_ACCESS_PREVIEW_1_CLIENT_ID,
        clientSecret: this.env.SITE_ACCESS_PREVIEW_1_CLIENT_SECRET,
        requiredCredentials: true,
      },
      {
        origin: this.env.SITE_ACCESS_PREVIEW_2_ORIGIN,
        clientId: this.env.SITE_ACCESS_PREVIEW_2_CLIENT_ID,
        clientSecret: this.env.SITE_ACCESS_PREVIEW_2_CLIENT_SECRET,
        requiredCredentials: true,
      },
    ]);
  }

  private async navigateTo(destination: string, runDeadline?: number) {
    const page = this.lifecycle.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");
    const access = this.siteAccess();
    if (!siteAccessCredentialsReady(destination, access)) {
      throw new Error("BROWSER_SITE_ACCESS_CREDENTIALS_REQUIRED");
    }
    const target = siteAccessForRequest(destination, access);
    return navigatePage(
      page, destination, origin => { this.ownerDirectedSiteNavigationOrigin = origin; },
      operationTimeout(25_000, runDeadline),
      target?.serviceCredentials ? target.origin : null,
    );
  }

  private lifecycleDependencies(): BrowserLifecycleDependencies {
    return {
      state: this.lifecycle,
      binding: this.env.BROWSER,
      storage: this.ctx.storage,
      installSafetyRouting: context => this.installSafetyRouting(context),
      attachPage: page => this.attachPage(page),
      navigateTo: destination => this.navigateTo(destination),
      validateDestination: destination => {
        if (!siteAccessCredentialsReady(destination, this.siteAccess())) {
          throw new Error("BROWSER_SITE_ACCESS_CREDENTIALS_REQUIRED");
        }
      },
      beforeClose: async () => {
        try { await this.handoffCdp?.detach(); } catch { /* Browser already lost. */ }
        this.handoffCdp = null;
      },
      afterClose: () => {
        this.lastFailureScreenshot = null;
        this.acceptingDialog = false;
        this.ownerDirectedSiteNavigationOrigin = null;
        this.requestLog = [];
        this.consoleLog = [];
        this.transfers.clear();
      },
    };
  }

  private async rememberSelectedPageIdentity() {
    await rememberSelectedPageIdentity(this.lifecycle, this.ctx.storage);
  }

  private async restore() {
    await restoreBrowser(this.lifecycleDependencies());
  }

  private async installSafetyRouting(context: BrowserContext) {
    const siteAccess = this.siteAccess();
    context.on("page", page => { this.attachPage(page); });
      await context.route("**/*", async route => {
        const request = route.request();
        if (!isPermittedBrowserRequest(request.url())) {
          await route.abort("blockedbyclient");
          return;
        }
        let initiator = "";
        let trustedMainNavigation = false;
        try {
          const frame = request.frame();
          initiator = frame.url();
          trustedMainNavigation = isTrustedSiteNavigationRequest(
            request.url(), this.ownerDirectedSiteNavigationOrigin,
            request.isNavigationRequest(), frame === this.lifecycle.page?.mainFrame(),
          );
        } catch { /* Service-worker requests have no frame; never authorize. */ }
        const disposition = siteRequestDispositionForTargets(
          request.url(), initiator, trustedMainNavigation, siteAccess,
        );
        if (disposition === "block") {
          // Browser-managed CRM cookies can survive earlier authenticated
          // navigation; never allow a foreign page to trigger their use.
          await route.abort("blockedbyclient");
          return;
        }
        if (disposition === "public") {
          // Foreign sites, their assets, third-party CDNs and SSO are public.
          await route.continue();
          return;
        }
        const target = siteAccessForRequest(request.url(), siteAccess);
        if (!target?.serviceCredentials || !target.clientId || !target.clientSecret) {
          await route.abort("blockedbyclient");
          return;
        }
        // Accepted native Browser Run transport for this TEST CRM: route.fetch →
        // route.fulfill loses upstream response headers/MIME in this provider,
        // so Chromium must continue the original request. Playwright may forward
        // overridden headers across redirects; this residual risk is accepted
        // only for isolated test data and MUST NOT be reused for production or
        // real customer data. A production-safe redirect-scoped native transport
        // remains a separate prerequisite (see issue #357).
        await route.continue({
          headers: {
            ...request.headers(),
            "CF-Access-Client-Id": target.clientId,
            "CF-Access-Client-Secret": target.clientSecret,
          },
        });
      });
  }

  private attachPage(page: Page) {
    if (this.lifecycle.tabIds.has(page)) return;
    this.lifecycle.tabIds.set(page, crypto.randomUUID());
    page.on("close", () => {
      this.lifecycle.tabIds.delete(page);
      if (this.lifecycle.page === page) {
        this.lifecycle.page = this.lifecycle.context?.pages().find(p => !p.isClosed()) ?? null;
      }
    });
      page.on("console", message => {
        this.consoleLog.push({ level: message.type(), text: redact(message.text()) });
        if (this.consoleLog.length > 80) this.consoleLog.shift();
      });
      page.on("pageerror", error => {
        this.consoleLog.push({ level: "pageerror", text: redact(error.message) });
        if (this.consoleLog.length > 80) this.consoleLog.shift();
      });
      page.on("request", request => {
        this.requestStartedAt.set(request, Date.now());
      });
      page.on("response", response => {
        const url = response.url();
        this.requestLog.push({
          method: response.request().method(),
          path: safePagePath(url),
          status: response.status(),
          resourceType: response.request().resourceType(),
          durationMs: (() => {
            const started = this.requestStartedAt.get(response.request());
            return started === undefined ? null : Math.max(0, Date.now() - started);
          })(),
        });
        if (this.requestLog.length > 80) this.requestLog.shift();
      });
      page.on("requestfailed", request => {
        this.requestLog.push({
          method: request.method(), path: safePagePath(request.url()), status: 0,
          resourceType: request.resourceType(),
          failure: redact(request.failure()?.errorText ?? "request_failed"),
        });
        if (this.requestLog.length > 80) this.requestLog.shift();
      });
      page.on("dialog", async dialog => {
        const accept = this.acceptingDialog;
        this.acceptingDialog = false;
        if (accept) await dialog.accept();
        else await dialog.dismiss();
      });
  }

  private async open(path: string = DEFAULT_BROWSER_URL, recording: boolean = false,
      requestedDevice?: BrowserDevice) {
    return openBrowser(this.lifecycleDependencies(), path, recording, requestedDevice);
  }

  private async persistHandoffCompletion(record: HumanHandoffRecord, event: unknown) {
    // The human may click Done after Close; do not resurrect a stale handoff.
    const currentId = await this.ctx.storage.get<string>("browser_session_id");
    const active = await this.ctx.storage.get<HumanHandoffRecord>("browser_handoff");
    if (currentId !== record.sessionId ||
        !active || active.handoffId !== record.handoffId ||
        active.status !== "waiting_human") return;
    await this.ctx.storage.put("browser_handoff", completedHandoff(active, event));
  }

  private listenForHandoffCompletion(
    cdp: Awaited<ReturnType<BrowserContext["newCDPSession"]>>,
    record: HumanHandoffRecord,
  ) {
    this.handoffCdp = cdp;
    // Cloudflare.* events are supported at runtime, but Playwright's
    // arrow-method typings do not accept additional CDP event overloads.
    const listen = cdp.once as unknown as
      (event: string, callback: (value: unknown) => void) => void;
    listen.call(cdp, "Cloudflare.handoffComplete", event => {
      this.ctx.waitUntil(this.persistHandoffCompletion(record, event).finally(async () => {
        try { await cdp.detach(); } catch { /* Browser may have ended. */ }
        if (this.handoffCdp === cdp) this.handoffCdp = null;
      }));
    });
  }

  private async structuredHandoff(command: Command) {
    const existing = await this.ctx.storage.get<HumanHandoffRecord>("browser_handoff");
    const isStatus = command.handoffAction === "status";
    if (!isStatus && command.handoffAction !== "start") {
      throw new Error("BROWSER_HANDOFF_ACTION_INVALID");
    }
    if (isStatus && !existing) return { state: "none" };
    if (isStatus && existing?.status !== "waiting_human") {
      return { state: existing?.status, handoffId: existing?.handoffId };
    }
    // Validate before acquiring any CDP handles or minting an interactive
    // Live View URL; malformed handoffs cannot affect the existing session.
    const checked = isStatus ? undefined : validateHandoffRequest(
      command.handoffInstructions, command.handoffTimeoutMs,
    );
    const page = this.lifecycle.page;
    if (!page || page.isClosed() || !this.lifecycle.browserSessionId) {
      return isStatus ? { state: "needs_reopen", handoff: "unconfirmed" } :
        (() => { throw new Error("BROWSER_HANDOFF_REQUIRES_OPEN_PAGE"); })();
    }
    if (!this.lifecycle.context) throw new Error("BROWSER_HANDOFF_CONTEXT_MISSING");
    if (!isStatus && existing &&
        (existing.status === "waiting_human" || existing.status === "unconfirmed")) {
      throw new Error("BROWSER_HANDOFF_ALREADY_PENDING");
    }
    const cdp = this.handoffCdp ?? await this.lifecycle.context.newCDPSession(page);
    const newCdp = this.handoffCdp !== cdp;
    const send = cdp.send.bind(cdp) as unknown as
      (method: string, params?: Record<string, unknown>) => Promise<unknown>;
    try {
      if (isStatus) {
        if (existing?.sessionId !== this.lifecycle.browserSessionId) {
          return { state: "needs_reopen", handoff: "unconfirmed" };
        }
        if (newCdp) this.listenForHandoffCompletion(cdp, existing!);
        const provider = isProviderHandoffActive(await send("Cloudflare.getHandoffState"));
        if (provider === null) throw new Error("BROWSER_HANDOFF_STATE_UNAVAILABLE");
        const updated = await this.ctx.storage.get<HumanHandoffRecord>("browser_handoff");
        if (updated && updated.status !== "waiting_human") {
          return { state: updated.status, handoffId: updated.handoffId };
        }
        if (provider === false) {
          // No event observed: provider may have completed while DO was
          // evicted. Do not invent Done/Failed or silently release the lock.
          const unconfirmed = { ...existing!, status: "unconfirmed" as const };
          await this.ctx.storage.put("browser_handoff", unconfirmed);
          return { state: "unconfirmed", handoffId: existing!.handoffId };
        }
        const view = handoffLiveViewUrl(await send("Cloudflare.getLiveView", {
          mode: "tab", expiresInMs: 300_000,
        }));
        return { state: "waiting_human", handoffId: existing!.handoffId,
          liveViewUrl: view, expiresInSeconds: 300 };
      }

      if (!checked) throw new Error("BROWSER_HANDOFF_ACTION_INVALID");
      if (isProviderHandoffActive(await send("Cloudflare.getHandoffState")) !== false) {
        throw new Error("BROWSER_HANDOFF_PROVIDER_ALREADY_ACTIVE_OR_UNAVAILABLE");
      }
      const url = handoffLiveViewUrl(await send("Cloudflare.getLiveView", {
        mode: "tab", expiresInMs: 300_000,
      }));
      // Persist the intent BEFORE requesting a potentially irreversible
      // handoff. A lost ACK must never silently allow other browser actions.
      const startedAt = Date.now();
      const pending: HumanHandoffRecord = {
        sessionId: this.lifecycle.browserSessionId, handoffId: "pending",
        status: "waiting_human", startedAt,
        deadline: startedAt + checked.timeoutMs,
      };
      await this.ctx.storage.put("browser_handoff", pending);
      let earlyOutcome: unknown = null;
      let record: HumanHandoffRecord | null = null;
      const listen = cdp.once as unknown as
        (event: string, callback: (value: unknown) => void) => void;
      listen.call(cdp, "Cloudflare.handoffComplete", event => {
        if (!record) { earlyOutcome = event; return; }
        this.ctx.waitUntil(this.persistHandoffCompletion(record, event).finally(async () => {
          try { await cdp.detach(); } catch { /* Browser may have ended. */ }
          if (this.handoffCdp === cdp) this.handoffCdp = null;
        }));
      });
      const handoff = await send("Cloudflare.handoff", {
        instructions: checked.instructions, timeout: checked.timeoutMs,
      });
      record = { ...pending, handoffId: handoffIdentifier(handoff) };
      await this.ctx.storage.put("browser_handoff", record);
      this.handoffCdp = cdp;
      if (earlyOutcome !== null) {
        await this.persistHandoffCompletion(record, earlyOutcome);
      }
      const current = await this.ctx.storage.get<HumanHandoffRecord>("browser_handoff");
      return { state: current?.status ?? "unconfirmed", handoffId: record.handoffId,
        liveViewUrl: url, expiresInSeconds: 300 };
    } catch (error) {
      if (newCdp && !this.handoffCdp) {
        try { await cdp.detach(); } catch { /* Detached already. */ }
      }
      throw error;
    }
  }

  private async close() {
    await closeBrowser(this.lifecycleDependencies());
  }

  private async execute(command: Command, runDeadline?: number): Promise<{ data: unknown; image?: string }> {
    // The lightweight probe must NOT launch, restore or mutate a browser.
    if (command.op === "runtime") return { data: { epoch: BROWSER_RUNTIME_EPOCH } };
    const activeHandoff = await this.ctx.storage.get<HumanHandoffRecord>("browser_handoff");
    if ((activeHandoff?.status === "waiting_human" ||
         activeHandoff?.status === "unconfirmed") &&
        !["handoff", "status", "close", "live_view"].includes(command.op)) {
      // No agent actions may race an active human operator, including Open,
      // arbitrary CDP, tab changes, file uploads and background scenarios.
      throw new Error("BROWSER_HANDOFF_WAITING_FOR_HUMAN");
    }
    if (command.op === "open") return {
      data: await this.open(command.url, command.recording ?? false, command.device),
    };
    if (command.op === "close") {
      await this.close();
      return { data: { state: "closed" } };
    }
    let needsReopen = false;
    try {
      await this.restore();
    } catch (error) {
      if ((command.op !== "status" &&
           !(command.op === "handoff" && command.handoffAction === "status")) ||
          !isOriginalPageMissing(error)) throw error;
      needsReopen = true;
    }
    if (command.op === "recording") {
      // Recordings are finalized ONLY after Close. Reading them must never
      // acquire a new browser or create a new page/context.
      if (command.recordingAction === "chunk") {
        return { data: this.transfers.readDownload(
          command.transferId ?? "", command.offset ?? -1, command.chunkBytes,
        ) };
      }
      if (command.recordingAction === "discard") {
        return { data: this.transfers.discard(command.transferId ?? "") };
      }
      if (this.lifecycle.browserSessionId) throw new Error("BROWSER_RECORDING_REQUIRES_CLOSED_SESSION");
      const sessionId = await this.ctx.storage.get<string>("last_recording_session_id");
      if (!sessionId) throw new Error("BROWSER_RECORDING_NOT_FOUND");
      const token = this.env.CF_BROWSER_RECORDING_READ_TOKEN;
      if (!token) throw new Error("BROWSER_RECORDING_READ_TOKEN_MISSING");
      if (command.recordingAction === "targets") {
        return { data: {
          sessionId, targets: await recordingTargets(this.env.CF_BROWSER_ACCOUNT_ID, token, sessionId),
        } };
      }
      if (command.recordingAction === "har") {
        if (!command.recordingTarget) throw new Error("BROWSER_HAR_TARGET_REQUIRED");
        const har = await recordingHar(
          this.env.CF_BROWSER_ACCOUNT_ID, token, sessionId, command.recordingTarget,
        );
        const buffer = Buffer.from(JSON.stringify(har));
        const receipt = this.transfers.storeDownload(
          "browser-" + sessionId.slice(0, 8) + ".har", "application/json",
          buffer.toString("base64"),
        );
        return { data: {
          action: "har", sanitized: true, format: "HAR 1.2",
          entryCount: har.log.entries.length, ...receipt,
          ...(receipt.bytes <= MAX_WORKSPACE_CHUNK_BYTES ?
            { base64: buffer.toString("base64") } : {}),
        } };
      }
      throw new Error("BROWSER_RECORDING_UNKNOWN_ACTION");
    }
    if (command.op === "status") return {
      data: {
        state: needsReopen ? "needs_reopen" :
          this.lifecycle.page && !this.lifecycle.page.isClosed() ? "open" : "closed",
        path: this.lifecycle.page && !this.lifecycle.page.isClosed() ? safePagePath(this.lifecycle.page.url()) : null,
        recording: this.lifecycle.recordingEnabled,
        device: await this.ctx.storage.get<BrowserDevice>("browser_device") ?? null,
        humanHandoff: activeHandoff?.status ?? null,
        lastRecordedSessionId: await this.ctx.storage.get<string>("last_recording_session_id") ?? null,
        // Official @cloudflare/playwright /v1/limits readback. No Browser Run
        // acquire, no connection, no full provider session IDs in MCP output.
        providerLimits: await readSafeBrowserLimits(() => limits(this.env.BROWSER)),
        providerHistory: await readSafeBrowserHistory(() => history(this.env.BROWSER)),
      },
    };
    if (command.op === "handoff") {
      if (command.handoffAction === "status" && needsReopen) {
        return { data: { state: "needs_reopen", handoff: "unconfirmed" } };
      }
      return { data: await this.structuredHandoff(command) };
    }
    if (command.op === "tabs") {
      if (!this.lifecycle.browser) throw new Error("CRM_BROWSER_NOT_OPEN");
      if (command.tabAction === "new") {
        const context = this.lifecycle.context ?? await ensureBrowserContext(
          this.lifecycleDependencies(),
          await this.ctx.storage.get<BrowserDevice>("browser_device") ?? "desktop",
        );
        if (context.pages().filter(p => !p.isClosed()).length >= 12) {
          throw new Error("BROWSER_TAB_LIMIT");
        }
        // A new tab uses the same site-neutral context and optional access policy.
        const url = command.url ? browserUrl(command.url, this.lifecycle.page?.url() || DEFAULT_BROWSER_URL) : null;
        const page = await context.newPage();
        this.attachPage(page);
        this.lifecycle.page = page;
        if (url) await this.navigateTo(url);
      } else if (command.tabAction === "select" || command.tabAction === "close") {
        const matched = [...this.lifecycle.tabIds.entries()].find(([page, id]) =>
          id === command.tabId && !page.isClosed(),
        );
        if (!matched) throw new Error("BROWSER_TAB_NOT_FOUND");
        const [page] = matched;
        if (command.tabAction === "select") {
          this.lifecycle.page = page;
          await page.bringToFront();
        } else {
          await page.close();
          this.lifecycle.tabIds.delete(page);
          if (this.lifecycle.page === page) {
            this.lifecycle.page = this.lifecycle.context?.pages().find(p => !p.isClosed()) ?? null;
          }
        }
      } else if (command.tabAction !== "list") {
        throw new Error("BROWSER_UNKNOWN_TAB_ACTION");
      }
      // A tab switch/new/close changes the active CDP target. Persist the
      // exact replacement identity before reporting a successful command.
      if (command.tabAction !== "list") await this.rememberSelectedPageIdentity();
      const tabs = await Promise.all(
        (this.lifecycle.context?.pages() ?? []).filter(p => !p.isClosed()).map(async (page) => {
          this.attachPage(page);
          return {
            tabId: this.lifecycle.tabIds.get(page),
            active: page === this.lifecycle.page,
            path: safePagePath(page.url()),
            title: await page.title(),
          };
        }),
      );
      return { data: { tabs, count: tabs.length } };
    }
    const page = this.lifecycle.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");

    switch (command.op) {
      case "visual": {
        const action=command.visualAction;
        if (action==="discard") {
          await this.ctx.storage.delete("visual_baseline");
          return {data:{discarded:true}};
        }
        if (action!=="capture" && action!=="compare")
          throw new Error("BROWSER_VISUAL_ACTION_INVALID");
        const threshold=visualThreshold(command.visualThreshold);
        const png=Buffer.from(await page.screenshot({fullPage:false}));
        const receipt=pngReceipt(png);
        if(action==="capture") {
          await this.ctx.storage.put("visual_baseline",
            {base64:png.toString("base64"),sha256:receipt.sha256,bytes:receipt.bytes});
          return {data:{baseline:true,path:safePagePath(page.url()),...receipt}};
        }
        const baseline=await this.ctx.storage.get<{base64:string;sha256:string;bytes:number}>("visual_baseline");
        if(!baseline) throw new Error("BROWSER_VISUAL_BASELINE_MISSING");
        if(!baseline.bytes || baseline.bytes>MAX_VISUAL_PNG_BYTES ||
            Buffer.byteLength(baseline.base64,"base64")!==baseline.bytes)
          throw new Error("BROWSER_VISUAL_BASELINE_INVALID");
        const result=await comparePngPixels(this.lifecycle.context!,baseline.base64,png.toString("base64"),threshold);
        return {data:{...result,path:safePagePath(page.url()),
          baselineSha256:baseline.sha256,currentSha256:receipt.sha256,
          note:"Pixel comparison of actual browser viewport PNGs, max channel delta > 24. Difference is measured in an isolated blank tab; no second browser."}};
      }
      case "execute": {
        // Universal CDP on the SAME active Playwright Page. No second Browser,
        // MCP Worker, CDP connection manager or worker-side script evaluator.
        // The helper validates the whole batch before the first mutation.
        const commands = command.cdpCommands ?? [];
        // Validate the entire batch before obtaining a CDP handle; in
        // particular Cloudflare.handoff must use the typed DO handoff owner.
        validateCdpCommands(commands);
        const cdp = await page.context().newCDPSession(page);
        // Playwright's typed Protocol.CommandParameters enumerates known CDP
        // methods at compile time. This ONE boundary supports new runtime CDP
        // methods; browser-cdp.ts validates the dynamic method and JSON params.
        // No `any`, private SDK APIs or alternative browser engine.
        const dynamicSend = cdp.send.bind(cdp) as unknown as
          (method: string, params?: Record<string, unknown>) => Promise<unknown>;
        try {
          return {
            data: await executeCdpCommands(
              commands, dynamicSend, command.timeoutMs ?? 90_000, redact,
            ),
          };
        } finally {
          try { await cdp.detach(); } catch { /* Already disconnected. */ }
        }
      }
      case "inspect": {
        if (command.inspection === "network") {
          return {
            data: {
              path: safePagePath(page.url()), events: this.requestLog.slice(-80),
              timing_note: "Elapsed request-to-response-event ms (approximate); not a browser HAR waterfall.",
            },
          };
        }
        if (command.inspection === "performance") {
          const metrics = await page.evaluate(() => {
            const n = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
            const paints = performance.getEntriesByType("paint")
              .map(item => ({ name: item.name, startTimeMs: Math.round(item.startTime) }));
            return {
              navigationType: n?.type ?? null,
              timeToFirstByteMs: n ? Math.round(n.responseStart - n.requestStart) : null,
              domContentLoadedMs: n ? Math.round(n.domContentLoadedEventEnd - n.startTime) : null,
              loadEventMs: n && n.loadEventEnd ? Math.round(n.loadEventEnd - n.startTime) : null,
              transferSizeBytes: n?.transferSize ?? null,
              resourceEntries: performance.getEntriesByType("resource").length,
              paintEntries: paints,
            };
          });
          return {
            data: {
              path: safePagePath(page.url()),metrics,
              webVitals: readVitals(await page.evaluate(() =>
                (window as Window & {__mcpVitals?:WebVitals}).__mcpVitals)),
              note:"Per-page provisional PerformanceObserver values. LCP/CLS and INP candidate are not finalized CrUX field metrics; unsupported/unobserved values are null.",
            },
          };
        }
        if (command.inspection === "accessibility") {
          const audit = await page.evaluate(() => {
            const problems: Array<{ rule: string; selector: string }> = [];
            const describe = (element: Element) =>
              element.tagName.toLowerCase() + (element.id ? "#" + element.id : "");
            for (const image of document.querySelectorAll("img:not([alt])")) {
              problems.push({ rule: "img_missing_alt", selector: describe(image) });
              if (problems.length >= 100) break;
            }
            for (const field of document.querySelectorAll("input:not([type=hidden]),select,textarea")) {
              if (problems.length >= 100) break;
              const id = field.getAttribute("id");
              const hasLabel = field.hasAttribute("aria-label") ||
                field.hasAttribute("aria-labelledby") ||
                Boolean(id && document.querySelector("label[for='" + CSS.escape(id) + "']")) ||
                Boolean(field.closest("label"));
              if (!hasLabel) problems.push({ rule: "form_field_missing_label", selector: describe(field) });
            }
            for (const button of document.querySelectorAll("button")) {
              if (problems.length >= 100) break;
              if (!button.textContent?.trim() &&
                  !button.getAttribute("aria-label")?.trim() &&
                  !button.getAttribute("aria-labelledby")?.trim()) {
                problems.push({ rule: "button_missing_name", selector: describe(button) });
              }
            }
            if (!document.title.trim()) problems.push({ rule: "document_missing_title", selector: "html" });
            const images = document.querySelectorAll("img").length;
            const fields = document.querySelectorAll("input,select,textarea").length;
            return { issues: problems, scanned: { images, fields }, complete: problems.length < 100 };
          });
          return {
            data: {
              path: safePagePath(page.url()), ...audit,
              note: "Limited DOM heuristics, not a WCAG conformance audit.",
            },
          };
        }
        if (command.inspection === "axe") {
          // This is the original axe.min.js bytes from the exact npm lockfile.
          // Build-time text generation prevents Wrangler/esbuild from rewriting
          // axeFunction.toString() with an unavailable __name() closure.
          // Playwright evaluation bypasses page inline-script CSP without a CDN.
          await page.evaluate(AXE_SOURCE);
          const audit=await page.evaluate(async () => {
            type AxeViolation={id:string;impact?:string|null;description?:string;
              nodes:Array<{target:string[]}>};
            type AxeReport={violations:AxeViolation[];
              passes:unknown[];incomplete:unknown[];inapplicable:unknown[]};
            const runner=(window as Window & {axe?: {
              run:(document:Document,options:unknown)=>Promise<AxeReport>;
            }}).axe;
            if(!runner) throw new Error("BROWSER_AXE_INJECTION_FAILED");
            const report=await runner.run(document,{
              runOnly:{type:"tag",values:["wcag2a","wcag2aa","wcag21a","wcag21aa","wcag22aa","best-practice"]},
            });
            return {engine:"axe-core",violationsTotal:report.violations.length,
              violations:report.violations.slice(0,50).map(item=>({
                id:item.id,impact:item.impact??null,
                description:(item.description??"").slice(0,200),
                affectedNodes:item.nodes.length,
                targets:item.nodes.slice(0,4).map(node=>node.target.slice(0,3)
                  .map(selector=>String(selector).slice(0,180))),
              })),
              passes:report.passes.length,incomplete:report.incomplete.length,
              inapplicable:report.inapplicable.length,
              truncated:report.violations.length>50};
          });
          return {data:{path:safePagePath(page.url()),...audit,
            note:"Actual axe-core automated rules, not full WCAG certification. Some cross-origin frames may require separate audit."}};
        }
        if (command.inspection === "pdf") {
          const result = Buffer.from(await page.pdf({
            format: "A4", printBackground: true,
          }));
          if (result.byteLength > MAX_TRANSFER_BYTES) {
            throw new Error("BROWSER_PDF_TOO_LARGE");
          }
          return {
            data: {
              path: safePagePath(page.url()), mimeType: "application/pdf",
              bytes: result.byteLength, base64: result.toString("base64"),
            },
          };
        }
        throw new Error("BROWSER_INSPECTION_UNKNOWN_KIND");
      }
      case "viewport": {
        if (!Number.isInteger(command.width) || !Number.isInteger(command.height) ||
            (command.width ?? 0) < 320 || (command.width ?? 0) > 2560 ||
            (command.height ?? 0) < 320 || (command.height ?? 0) > 2560) {
          throw new Error("BROWSER_VIEWPORT_OUT_OF_RANGE");
        }
        await page.setViewportSize({ width: command.width!, height: command.height! });
        return {
          data: { viewport: page.viewportSize(), path: safePagePath(page.url()) },
        };
      }
      case "file": {
        // A single file transfer lives only in this DO's memory. If Cloudflare
        // evicts the DO mid-transfer, subsequent calls fail, never return
        // corrupted or invented bytes. Browser state stays owned by Playwright.
        if (command.fileAction === "upload_begin") {
          return { data: this.transfers.beginUpload(command.fileName ?? "", command.mimeType ?? "") };
        }
        if (command.fileAction === "upload_chunk") {
          return { data: this.transfers.appendUpload(command.transferId ?? "", command.base64 ?? "") };
        }
        if (command.fileAction === "discard") {
          return { data: this.transfers.discard(command.transferId ?? "") };
        }
        if (command.fileAction === "download_chunk") {
          return { data: this.transfers.readDownload(
            command.transferId ?? "", command.offset ?? -1, command.chunkBytes,
          ) };
        }
        if (command.fileAction === "upload_commit") {
          if (!command.target) throw new Error("BROWSER_FILE_TARGET_REQUIRED");
          const transferId = command.transferId ?? "";
          const file = this.transfers.preparedUpload(transferId, command.expectedSha256 ?? "");
          await this.target(command.target).setInputFiles({
            name: file.name, mimeType: file.mimeType, buffer: file.buffer,
          }, { timeout: 15_000 });
          this.transfers.finishUpload(transferId);
          return { data: {
            action: "upload", completed: true, fileName: file.name,
            mimeType: file.mimeType, bytes: file.bytes, sha256: file.sha256,
            path: safePagePath(page.url()),
          } };
        }
        if (!command.target) throw new Error("BROWSER_FILE_TARGET_REQUIRED");
        const locator = this.target(command.target);
        if (command.fileAction === "download_link" || command.fileAction === "download") {
          // Browser-side fetch is intentionally same-origin, using the
          // actual page's cookies/CORS and Playwright request routing.
          // Unlike a browser download event, response bytes are verified.
          // No Worker-side remote URL fetch and no speculative re-click.
          const response = await locator.evaluate(async (element, maxBytes) => {
            const link = element.closest("a[href]");
            if (!(link instanceof HTMLAnchorElement)) throw new Error("BROWSER_FILE_LINK_REQUIRED");
            const url = new URL(link.href, location.href);
            if (!["http:", "https:"].includes(url.protocol) ||
                url.origin !== location.origin) {
              throw new Error("BROWSER_FILE_LINK_CROSS_ORIGIN_UNSUPPORTED");
            }
            const downloaded = await fetch(url.href, {
              credentials: "same-origin", mode: "same-origin", redirect: "follow",
            });
            if (!downloaded.ok || new URL(downloaded.url).origin !== location.origin) {
              throw new Error("BROWSER_DOWNLOAD_HTTP_FAILED");
            }
            const length = downloaded.headers.get("content-length");
            if (length && Number(length) > maxBytes) throw new Error("BROWSER_DOWNLOAD_TOO_LARGE");
            if (!downloaded.body) throw new Error("BROWSER_DOWNLOAD_BODY_UNAVAILABLE");
            const reader = downloaded.body.getReader();
            const chunks: Uint8Array[] = [];
            let size = 0;
            try {
              for (;;) {
                const { done, value } = await reader.read();
                if (done) break;
                size += value.byteLength;
                if (size > maxBytes) throw new Error("BROWSER_DOWNLOAD_TOO_LARGE");
                chunks.push(value);
              }
            } finally { await reader.cancel().catch(() => {}); }
            if (!size) throw new Error("BROWSER_DOWNLOAD_CONTENT_UNVERIFIED");
            const bytes = new Uint8Array(size);
            let offset = 0;
            for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
            let binary = "";
            for (let i = 0; i < size; i += 8192) {
              binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
            }
            return {
              name: url.pathname.split("/").pop() || "download",
              mimeType: downloaded.headers.get("content-type") ?? "application/octet-stream",
              base64: btoa(binary),
            };
          }, MAX_WORKSPACE_FILE_BYTES, { timeout: 45_000 });
          if (command.fileAction === "download") {
            // Browser Run/CDP exposes a download event, but the stream is empty.
            // Use the same supported browser-side, same-origin link fetch as
            // download_link; never replay HTTP via a Worker or invent bytes.
            // Legacy inline downloads remain capped at 1 MB.
            const bytes = Buffer.from(response.base64, "base64");
            return { data: {
              action: "download", completed: true, mimeType: response.mimeType,
              ...transferDownload(response.name, [bytes]),
            } };
          }
          const receipt = this.transfers.storeDownload(response.name, response.mimeType, response.base64);
          return { data: {
            action: "download_link", completed: true,
            ...receipt,
            // Small files are one-call results. Larger verified files are
            // returned in bounded chunks using this same transferId.
            ...(receipt.bytes <= MAX_WORKSPACE_CHUNK_BYTES ?
              { base64: response.base64 } : {}),
          } };
        }
        if (command.fileAction === "upload") {
          const file = validatedUpload(
            command.fileName ?? "", command.mimeType ?? "", command.base64 ?? "",
          );
          await locator.setInputFiles(file, { timeout: 15_000 });
          return {
            data: {
              action: "upload", fileName: file.name, mimeType: file.mimeType,
              bytes: file.buffer.byteLength, completed: true,
              path: safePagePath(page.url()),
            },
          };
        }
        throw new Error("BROWSER_FILE_UNKNOWN_ACTION");
      }
      case "frames":
        return {
          data: {
            path: safePagePath(page.url()),
            frames: page.frames().slice(0, 50).map((frame, index) => ({
              frameIndex: index,
              name: frame.name(),
              path: safePagePath(frame.url()),
              isMain: frame === page.mainFrame(),
            })),
          },
        };
      case "snapshot": {
        if (command.frameIndex !== undefined && command.frameSelector !== undefined) {
          throw new Error("BROWSER_FRAME_TARGET_CONFLICT");
        }
        if (command.frameSelector !== undefined) {
          if (!command.frameSelector || command.frameSelector.length > 512) {
            throw new Error("BROWSER_FRAME_SELECTOR_INVALID");
          }
          return {
            data: {
              path: safePagePath(page.url()),
              frameSelector: command.frameSelector,
              title: await page.title(),
              accessibility: (await page.frameLocator(command.frameSelector)
                .locator("body").ariaSnapshot()).slice(0, 30_000),
            },
          };
        }
        const frame = command.frameIndex === undefined ? page.mainFrame() :
          page.frames()[command.frameIndex];
        if (!frame) throw new Error("BROWSER_FRAME_NOT_FOUND");
        return {
          data: {
            path: safePagePath(frame.url()),
            frameIndex: page.frames().indexOf(frame),
            title: await page.title(),
            accessibility: (await frame.locator("body").ariaSnapshot()).slice(0, 30000),
          },
        };
      }
      case "screenshot": {
        if (command.screenshotSource === "last_failure") {
          if (!this.lastFailureScreenshot) throw new Error("BROWSER_FAILURE_SCREENSHOT_NOT_AVAILABLE");
          return {
            data: { path: this.lastFailureScreenshot.capturedPath,
              mimeType: "image/png", source: "last_failure" },
            image: this.lastFailureScreenshot.image,
          };
        }
        const png = await page.screenshot({ fullPage: true });
        return {
          data: { path: safePagePath(page.url()), mimeType: "image/png", source: "current" },
          image: Buffer.from(png).toString("base64"),
        };
      }
      case "console":
        return { data: { messages: this.consoleLog.slice(-80), responses: this.requestLog.slice(-80) } };
      case "live_view": {
        if (!this.lifecycle.browser) throw new Error("BROWSER_SESSION_NOT_AVAILABLE");
        // The URL contains a signed bearer credential. Return it only through
        // the owner-authenticated MCP call; never log or store it.
        const view = await this.env.BROWSER.getLiveView(this.lifecycle.browser.sessionId(), {
          mode: "tab",
          expiresInMs: 60_000,
          guardrails: { mode: "readonly" },
        });
        return { data: { url: view.devtoolsFrontendUrl, readonly: true, expires_in_seconds: 60 } };
      }
      case "wait": {
        const timeout = operationTimeout(boundedWaitTimeout(command.timeoutMs), runDeadline);
        if (command.target) {
          await this.target(command.target).waitFor({
            state: command.state ?? "visible",
            timeout,
          });
        } else if (command.url) {
          // Playwright owns the wait; no arbitrary sleeps or polling loops.
          await page.waitForURL(command.url, { timeout });
        } else {
          await page.waitForLoadState("domcontentloaded", { timeout });
        }
        return {
          data: {
            completed: true,
            condition: command.target ? "element" : command.url ? "url" : "domcontentloaded",
            state: command.state ?? (command.target ? "visible" : "ready"),
            path: safePagePath(page.url()),
          },
        };
      }
      case "assert": {
        if (!command.kind || command.expected === undefined) {
          throw new Error("BROWSER_ASSERT_MISSING_CONDITION");
        }
        let actual: string | number | boolean;
        if (command.kind === "url") actual = safePagePath(page.url());
        else if (command.kind === "title") actual = await page.title();
        else {
          if (!command.target) throw new Error("BROWSER_ASSERT_TARGET_REQUIRED");
          const locator = this.target(command.target);
          if (command.kind === "visible") actual = await locator.isVisible();
          else if (command.kind === "hidden") actual = !(await locator.isVisible());
          else if (command.kind === "count") actual = await locator.count();
          else if (command.kind === "text") actual = (await locator.textContent()) ?? "";
          else throw new Error("BROWSER_ASSERT_UNKNOWN_KIND");
        }
        const result = compareBrowserAssertion({
          kind: command.kind, actual, expected: command.expected, match: command.match,
        });
        return {
          data: {
            ...result,
            expected: safeAssertionValue(result.expected),
            actual: safeAssertionValue(result.actual),
            path: safePagePath(page.url()),
          },
        };
      }
      case "run": {
        const steps = command.steps;
        validateScenarioSteps(steps, url => {
          browserUrl(url, page.url() || DEFAULT_BROWSER_URL);
        });
        this.lastFailureScreenshot = null;
        const startedAt = Date.now();
        const deadline = startedAt + boundedRunTimeout(command.timeoutMs);
        const results: Array<{ index: number; op: string; passed: boolean; elapsedMs: number; data?: unknown; error?: string; outcome?: string }> = [];
        let passed = true;
        for (const [index, step] of steps!.entries()) {
          const stepStartedAt = Date.now();
          try {
            operationTimeout(1, deadline);
            const response = await this.execute(step, deadline);
            // A late provider ACK must never count as PASS.
            operationTimeout(1, deadline);
            const data = response.data as { passed?: boolean };
            const ok = data.passed !== false;
            // Request diagnostics live in browser_diagnostics. Never repeat
            // up to eight HTTP receipts for every scenario step or error.
            const receipt = data && typeof data === "object" ?
              Object.fromEntries(Object.entries(data).filter(([key]) => key !== "latest_requests")) : data;
            results.push({ index, op: step.op, passed: ok, elapsedMs: Date.now() - stepStartedAt, data: receipt });
            if (!ok) { passed = false; break; }
          } catch (error) {
            passed = false;
            results.push({
              index, op: step.op, passed: false, elapsedMs: Date.now() - stepStartedAt,
              error: redact(error instanceof Error ? error.message : error),
              outcome: step.op === "assert" || step.op === "wait" ? "not_completed" : "uncertain",
            });
            break;
          }
        }
        const report = { passed, steps: results, total: steps!.length, executed: results.length, elapsedMs: Date.now() - startedAt };
        if (!passed && command.screenshotOnFailure) {
          try {
            const png = await page.screenshot({ fullPage: true, timeout: 5_000 });
            if (png.byteLength <= 2_000_000) {
              this.lastFailureScreenshot = {
                image: Buffer.from(png).toString("base64"),
                capturedPath: safePagePath(page.url()),
              };
            }
          } catch { /* The failure report remains authoritative. */ }
          return { data: { ...report, failureScreenshot: this.lastFailureScreenshot ?
            "available_via_browser_screenshot" : "unavailable" } };
        }
        return { data: report };
      }
      case "navigate": {
        const destination = browserUrl(command.url ?? "/", page.url() || DEFAULT_BROWSER_URL);
        return { data: await this.navigateTo(destination, runDeadline) };
      }
      case "reload":
        await page.reload({ waitUntil: "domcontentloaded", timeout: operationTimeout(25_000, runDeadline) });
        return { data: { path: safePagePath(page.url()) } };
      case "back":
        await page.goBack({ waitUntil: "domcontentloaded", timeout: operationTimeout(25_000, runDeadline) });
        return { data: { path: safePagePath(page.url()) } };
      case "forward":
        await page.goForward({ waitUntil: "domcontentloaded", timeout: operationTimeout(25_000, runDeadline) });
        return { data: { path: safePagePath(page.url()) } };
      case "scroll":
        await page.mouse.wheel(command.x ?? 0, command.y ?? 650);
        return { data: { action: "scroll", completed: true } };
      case "press":
        // A target-scoped press focuses the actual element with native
        // Playwright. Without a target, retain the ordinary page keyboard.
        if (command.target) {
          await this.target(command.target).press(command.key ?? "", {
            timeout: operationTimeout(12_000, runDeadline),
          });
        } else {
          await page.keyboard.press(command.key ?? "");
        }
        return { data: { action: "press", completed: true, targeted: !!command.target } };
      case "session": {
        // Optional site convention: read /api/session as the CURRENT browser
        // origin, with ordinary cookies or configured service identity.
        // Non-JSON/404 responses are honest results, not a fabricated session.
        const result = await page.evaluate(async () => {
          const response = await fetch("/api/session", { credentials: "same-origin" });
          const contentType = response.headers.get("content-type") ?? "";
          if (!contentType.includes("json")) {
            return { status: response.status, body: null, contentType };
          }
          return { status: response.status, body: await response.json() };
        });
        return { data: result };
      }
      case "click":
      case "double_click":
      case "fill":
      case "select":
      case "check":
      case "uncheck":
      case "hover":
      case "drag_to":
      case "type": {
        if (!command.target) throw new Error("CRM_BROWSER_TARGET_REQUIRED");
        if (command.followPopup &&
            command.op !== "click" && command.op !== "double_click") {
          throw new Error("BROWSER_POPUP_REQUIRES_CLICK");
        }
        const locator = this.target(command.target);
        let followedPopup: Page | null = null;
        this.acceptingDialog = command.acceptDialog === true;
        try {
          if (command.followPopup) {
            // Listen before clicking so a fast window.open is never missed.
            // The native Playwright event identifies exactly the new page.
            const [popup] = await Promise.all([
              page.waitForEvent("popup", { timeout: operationTimeout(12_000, runDeadline) }),
              command.op === "click"
                ? locator.click({ timeout: operationTimeout(12_000, runDeadline) })
                : locator.dblclick({ timeout: operationTimeout(12_000, runDeadline) }),
            ]);
            this.attachPage(popup);
            this.lifecycle.page = popup;
            await this.rememberSelectedPageIdentity();
            followedPopup = popup;
          } else if (command.op === "click") await locator.click({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "double_click") await locator.dblclick({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "fill") await locator.fill(command.text ?? "", { timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "select") await locator.selectOption(command.value ?? "", { timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "check") await locator.check({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "uncheck") await locator.uncheck({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "hover") await locator.hover({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "drag_to") {
            if (!command.destination) throw new Error("BROWSER_DRAG_DESTINATION_REQUIRED");
            // Official Playwright performs a real HTML5 drag gesture. The
            // page owns drag/drop events; do not synthesize DOM events or CDP.
            await locator.dragTo(this.target(command.destination), {
              timeout: operationTimeout(12_000, runDeadline),
            });
          }
          else if (command.op === "type") await locator.pressSequentially(command.text ?? "", { timeout: operationTimeout(12_000, runDeadline) });
        } finally {
          this.acceptingDialog = false;
        }
        return {
          data: {
            action: command.op, completed: true,
            path: safePagePath(followedPopup?.url() ?? page.url()),
            ...(followedPopup ? { followedPopup: true, tabId: this.lifecycle.tabIds.get(followedPopup) } : {}),
            // Request IDs and full data values can be inspected separately through Cloudflare.
            latest_requests: this.requestLog.slice(-8),
          },
        };
      }
      default:
        throw new Error("CRM_BROWSER_UNKNOWN_OPERATION");
    }
  }

  override async fetch(request: Request): Promise<Response> {
    if (request.method !== "POST" || new URL(request.url).pathname !== "/internal") {
      return new Response("Not found", { status: 404 });
    }
    // Commands for one owner share one browser and must not interleave across
    // await points. A bounded FIFO serializes tab changes and all UI actions.
    if (this.pendingOperations >= 8) {
      return Response.json({ ok: false, error: "BROWSER_BUSY" }, { status: 429 });
    }
    this.pendingOperations++;
    const previous = this.nextOperation;
    let release!: () => void;
    this.nextOperation = new Promise<void>(resolve => { release = resolve; });
    await previous;
    let result: Result;
    try {
      const command = await request.json() as Command;
      if (command.op !== "runtime") requireRuntimeEpoch(command.expectedRuntimeEpoch);
      const { data, image } = await this.execute(command);
      result = { ok: true, data, ...(image ? { image } : {}) };
    } catch (error) {
      result = { ok: false, error: redact(error instanceof Error ? error.message : error) };
    } finally {
      this.pendingOperations--;
      release();
    }
    return Response.json(result);
  }
}

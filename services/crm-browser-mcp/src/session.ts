import { DurableObject } from "cloudflare:workers";
import { acquire, connect, type Browser, type BrowserContext, type Page } from "@cloudflare/playwright";
import { CRM_ORIGIN, browserUrl, crmCredentialsAllowed, isCrmRequest, isPermittedBrowserRequest, redact, requireCrmAgentIdentity, safePagePath } from "./security";
import { navigatePage } from "./page-navigation";
import { boundedWaitTimeout, boundedRunTimeout, compareBrowserAssertion, operationTimeout, safeAssertionValue, validateScenarioSteps, type BrowserAssertionKind, type TextMatch, type WaitState, type ScenarioOperation } from "./browser-checks";
import { validatedUpload, transferDownload, MAX_TRANSFER_BYTES } from "./browser-files";
import { recoverExistingBrowserSession } from "./browser-recovery";
import { BROWSER_RUNTIME_EPOCH, requireRuntimeEpoch } from "./runtime-epoch";

export interface Env {
  BROWSER: BrowserRun;
  SESSIONS: DurableObjectNamespace<CrmBrowserSession>;
  CRM_BROWSER_CLIENT_ID: string;
  CRM_BROWSER_CLIENT_SECRET: string;
  CRM_MCP_OWNER_EMAIL: string;
  CRM_MCP_ACCESS_AUD: string;
}

export type Target = {
  kind: "css" | "role" | "text" | "label";
  value: string;
  name?: string;
  frameIndex?: number;
};

export type Command = {
  op: ScenarioOperation | "open" | "close" | "status" | "session" | "tabs" | "frames" | "file" | "inspect" | "viewport" | "snapshot" | "screenshot" | "console" | "live_view" | "run" | "runtime";
  expectedRuntimeEpoch?: string; // Worker-supplied internal compatibility fence.
  workspaceId?: string; // MCP routing only; stripped before sending to this DO.
  url?: string;
  target?: Target;
  text?: string;
  key?: string;
  checked?: boolean;
  x?: number;
  y?: number;
  value?: string;
  acceptDialog?: boolean;
  kind?: BrowserAssertionKind;
  expected?: string | number | boolean;
  match?: TextMatch;
  state?: WaitState;
  timeoutMs?: number;
  steps?: Command[];
  tabAction?: "list" | "new" | "select" | "close";
  tabId?: string;
  frameIndex?: number;
  fileAction?: "upload" | "download";
  fileName?: string;
  mimeType?: string;
  base64?: string;
  inspection?: "network" | "performance" | "accessibility" | "pdf";
  width?: number;
  height?: number;
  recording?: boolean;
  screenshotOnFailure?: boolean;
};

type Result = { ok: true; data: unknown; image?: string } | { ok: false; error: string };

// One DO = one authenticated operator's browser session. The MCP transport stays stateless.
// Only in-memory browser handles are ephemeral. Official Browser Run session IDs
// are persisted in DO storage and reattached after eviction; expired sessions
// are not silently replaced until an explicit browser_open.
export class CrmBrowserSession extends DurableObject<Env> {
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private page: Page | null = null;
  private browserSessionId: string | null = null;
  private recordingEnabled = false;
  private tabIds = new Map<Page, string>();
  private nextOperation: Promise<void> = Promise.resolve();
  private pendingOperations = 0;
  private acceptingDialog = false;
  private ownerDirectedCrmNavigation = false;
  private consoleLog: Array<{ level: string; text: string }> = [];
  private requestStartedAt = new WeakMap<object, number>();
  private requestLog: Array<{
    method: string; path: string; status: number;
    resourceType?: string; durationMs?: number | null; failure?: string;
  }> = [];

  private target(target: Target) {
    if (!this.page) throw new Error("CRM_BROWSER_NOT_OPEN");
    const scope = target.frameIndex === undefined ? this.page :
      this.page.frames()[target.frameIndex];
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

  // Both browser_open(url) and browser_action(navigate) must really navigate
  // an existing session. This is the single owner of privileged CRM navigation.
  private async navigateTo(destination: string, runDeadline?: number) {
    const page = this.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");
    requireCrmAgentIdentity(
      destination, this.env.CRM_BROWSER_CLIENT_ID, this.env.CRM_BROWSER_CLIENT_SECRET,
    );
    return navigatePage(
      page, destination, enabled => { this.ownerDirectedCrmNavigation = enabled; },
      operationTimeout(25_000, runDeadline),
    );
  }

  // The only browser owner. The DO stores only an opaque Browser Run session
  // ID; cookies, tabs and browser processes remain in the official runtime.
  private async restore() {
    if (this.browser?.isConnected()) return;
    this.browser = null;
    this.context = null;
    this.page = null;
    this.tabIds.clear();
    const sessionId = await this.ctx.storage.get<string>("browser_session_id");
    const recovered = await recoverExistingBrowserSession(
      sessionId,
      async id => this.env.BROWSER.getSession(id),
      async id => connect(this.env.BROWSER, id),
    );
    if (recovered.kind === "missing") return;
    if (recovered.kind === "expired") {
      // Delete only a conclusively closed provider session; a lookup outage
      // or reconnect failure leaves persisted identity available for retry.
      // Only an explicit browser_open may acquire a new Browser Run.
      await this.ctx.storage.delete("browser_session_id");
      await this.ctx.storage.delete("browser_recording");
      this.browserSessionId = null;
      this.recordingEnabled = false;
      return;
    }
    const { browser } = recovered;
    this.browser = browser;
    this.browserSessionId = recovered.sessionId;
    this.recordingEnabled = await this.ctx.storage.get<boolean>("browser_recording") ?? false;
    const contexts = browser.contexts();
    this.context = contexts.find(c => c.pages().some(p => !p.isClosed())) ??
      contexts[0] ?? null;
    if (this.context) {
      await this.installSafetyRouting(this.context);
      for (const page of this.context.pages()) this.attachPage(page);
      this.page = this.context.pages().find(p => !p.isClosed()) ?? null;
    }
  }

  private async installSafetyRouting(context: BrowserContext) {
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
          trustedMainNavigation = this.ownerDirectedCrmNavigation &&
            request.isNavigationRequest() && frame === this.page?.mainFrame();
        } catch { /* Service-worker requests have no frame; never authorize. */ }
        const hasAgentCredentials = Boolean(
          this.env.CRM_BROWSER_CLIENT_ID && this.env.CRM_BROWSER_CLIENT_SECRET,
        );
        if (!hasAgentCredentials ||
            !crmCredentialsAllowed(request.url(), initiator, trustedMainNavigation)) {
          // Includes foreign sites, their assets, third-party CDNs and SSO.
          await route.continue();
          return;
        }
        // Playwright route.continue({headers}) propagates overridden headers to
        // every redirect, including foreign origins. This is NOT safe.
        // route.fetch(maxRedirects:0) keeps the signed credentials on exactly
        // the first CRM request and fulfills the response to the browser.
        // A 3xx is followed in the browser WITHOUT privileged request headers.
        const response = await route.fetch({
          headers: {
            ...request.headers(),
            "CF-Access-Client-Id": this.env.CRM_BROWSER_CLIENT_ID,
            "CF-Access-Client-Secret": this.env.CRM_BROWSER_CLIENT_SECRET,
          },
          maxRedirects: 0,
          timeout: 25_000,
        });
        await route.fulfill({ response });
      });
  }

  private attachPage(page: Page) {
    if (this.tabIds.has(page)) return;
    this.tabIds.set(page, crypto.randomUUID());
    page.on("close", () => {
      this.tabIds.delete(page);
      if (this.page === page) {
        this.page = this.context?.pages().find(p => !p.isClosed()) ?? null;
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

  private async open(path: string = "/", recording: boolean = false) {
    const destination = browserUrl(path);
    requireCrmAgentIdentity(
      destination, this.env.CRM_BROWSER_CLIENT_ID, this.env.CRM_BROWSER_CLIENT_SECRET,
    );
    await this.restore();
    if (recording && this.browser && !this.recordingEnabled) {
      throw new Error("BROWSER_RECORDING_REQUIRES_NEW_SESSION");
    }
    if (this.page && !this.page.isClosed()) {
      if (this.page.url() === destination) {
        return { state: "already_open", path: safePagePath(this.page.url()) };
      }
      return { state: "navigated", ...(await this.navigateTo(destination)) };
    }
    let created = false;
    if (!this.browser) {
      // Official acquisition + explicit connection allows DO eviction recovery.
      // Keep-alive is idle time, not an automatic browser polling loop.
      const { sessionId } = await acquire(this.env.BROWSER, {
        keep_alive: 600_000, recording,
      });
      try {
        this.browser = await connect(this.env.BROWSER, sessionId);
        this.browserSessionId = sessionId;
        await this.ctx.storage.put("browser_session_id", sessionId);
        await this.ctx.storage.put("browser_recording", recording);
        this.recordingEnabled = recording;
        created = true;
      } catch (error) {
        await this.env.BROWSER.closeSession(sessionId);
        throw error;
      }
    }
    try {
      if (!this.context) {
        this.context = await this.browser.newContext({
          acceptDownloads: true,
          serviceWorkers: "block",
        });
        await this.installSafetyRouting(this.context);
      }
      this.page = await this.context.newPage();
      this.attachPage(this.page);
      return { state: "open", ...(await this.navigateTo(destination)) };
    } catch (error) {
      if (created) await this.close();
      throw error;
    }
  }

  private async close() {
    const sessionId = this.browserSessionId ??
      await this.ctx.storage.get<string>("browser_session_id");
    let failure: unknown = null;
    // Disconnecting the Playwright client is best-effort. Always attempt
    // termination of the exact Browser Run session to avoid billable orphans.
    try {
      await this.browser?.close();
    } catch (error) {
      failure = error;
    }
    try {
      if (sessionId) {
        await this.env.BROWSER.closeSession(sessionId);
        if (this.recordingEnabled) {
          // The provider recording becomes inspectable only after closure.
          await this.ctx.storage.put("last_recording_session_id", sessionId);
        }
      }
      await this.ctx.storage.delete("browser_session_id");
      await this.ctx.storage.delete("browser_recording");
    } catch (error) {
      failure ??= error;
      // Keep the persisted ID on failure so a later owner request can retry
      // termination. Never create a replacement session implicitly.
    } finally {
      this.browser = null;
      this.browserSessionId = null;
      this.recordingEnabled = false;
      this.context = null;
      this.page = null;
      this.tabIds.clear();
      this.acceptingDialog = false;
      this.ownerDirectedCrmNavigation = false;
      this.requestLog = [];
      this.consoleLog = [];
    }
    if (failure) throw failure;
  }

  private async execute(command: Command, runDeadline?: number): Promise<{ data: unknown; image?: string }> {
    // The lightweight probe must NOT launch, restore or mutate a browser.
    if (command.op === "runtime") return { data: { epoch: BROWSER_RUNTIME_EPOCH } };
    if (command.op === "open") return {
      data: await this.open(command.url, command.recording ?? false),
    };
    if (command.op === "close") {
      await this.close();
      return { data: { state: "closed" } };
    }
    await this.restore();
    if (command.op === "status") return {
      data: {
        state: this.page && !this.page.isClosed() ? "open" : "closed",
        path: this.page && !this.page.isClosed() ? safePagePath(this.page.url()) : null,
        recording: this.recordingEnabled,
        lastRecordedSessionId: await this.ctx.storage.get<string>("last_recording_session_id") ?? null,
      },
    };
    if (command.op === "tabs") {
      if (!this.browser) throw new Error("CRM_BROWSER_NOT_OPEN");
      if (command.tabAction === "new") {
        if (!this.context) {
          this.context = await this.browser.newContext({
            acceptDownloads: true, serviceWorkers: "block",
          });
          await this.installSafetyRouting(this.context);
        }
        if (this.context.pages().filter(p => !p.isClosed()).length >= 12) {
          throw new Error("BROWSER_TAB_LIMIT");
        }
        // A new tab belongs to the existing single browser context. Its
        // requests inherit precisely the same CRM origin/security route.
        const url = command.url ? browserUrl(command.url, CRM_ORIGIN + "/") : null;
        if (url) requireCrmAgentIdentity(
          url, this.env.CRM_BROWSER_CLIENT_ID, this.env.CRM_BROWSER_CLIENT_SECRET,
        );
        const page = await this.context.newPage();
        this.attachPage(page);
        this.page = page;
        if (url) await this.navigateTo(url);
      } else if (command.tabAction === "select" || command.tabAction === "close") {
        const matched = [...this.tabIds.entries()].find(([page, id]) =>
          id === command.tabId && !page.isClosed(),
        );
        if (!matched) throw new Error("BROWSER_TAB_NOT_FOUND");
        const [page] = matched;
        if (command.tabAction === "select") {
          this.page = page;
          await page.bringToFront();
        } else {
          await page.close();
          this.tabIds.delete(page);
          if (this.page === page) {
            this.page = this.context?.pages().find(p => !p.isClosed()) ?? null;
          }
        }
      } else if (command.tabAction !== "list") {
        throw new Error("BROWSER_UNKNOWN_TAB_ACTION");
      }
      const tabs = await Promise.all(
        (this.context?.pages() ?? []).filter(p => !p.isClosed()).map(async (page) => {
          this.attachPage(page);
          return {
            tabId: this.tabIds.get(page),
            active: page === this.page,
            path: safePagePath(page.url()),
            title: await page.title(),
          };
        }),
      );
      return { data: { tabs, count: tabs.length } };
    }
    const page = this.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");

    switch (command.op) {
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
            data: { path: safePagePath(page.url()), metrics, note: "Navigation and paint timing only; not Core Web Vitals." },
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
        if (!command.target) throw new Error("BROWSER_FILE_TARGET_REQUIRED");
        const locator = this.target(command.target);
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
        if (command.fileAction === "download") {
          const awaited = page.waitForEvent("download", { timeout: 20_000 });
          await locator.click({ timeout: 15_000 });
          const download = await awaited;
          // The event fires at download start. createReadStream() already
          // throws for a failed/canceled download under the Playwright contract.
          // Avoid another indefinite download.failure() wait before reading.
          const stream = await download.createReadStream();
          const chunks: Uint8Array[] = [];
          let size = 0;
          for await (const chunk of stream) {
            const bytes = Buffer.from(chunk);
            size += bytes.byteLength;
            if (size > MAX_TRANSFER_BYTES) {
              throw new Error("BROWSER_DOWNLOAD_TOO_LARGE");
            }
            chunks.push(bytes);
          }
          return {
            data: {
              action: "download", completed: true,
              ...transferDownload(download.suggestedFilename(), chunks),
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
        const png = await page.screenshot({ fullPage: true });
        return {
          data: { path: safePagePath(page.url()), mimeType: "image/png" },
          image: Buffer.from(png).toString("base64"),
        };
      }
      case "console":
        return { data: { messages: this.consoleLog.slice(-80), responses: this.requestLog.slice(-80) } };
      case "live_view": {
        if (!this.browser) throw new Error("BROWSER_SESSION_NOT_AVAILABLE");
        // The URL contains a signed bearer credential. Return it only through
        // the owner-authenticated MCP call; never log or store it.
        const view = await this.env.BROWSER.getLiveView(this.browser.sessionId(), {
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
          const destination = browserUrl(url, page.url() || CRM_ORIGIN);
          requireCrmAgentIdentity(
            destination, this.env.CRM_BROWSER_CLIENT_ID, this.env.CRM_BROWSER_CLIENT_SECRET,
          );
        });
        const startedAt = Date.now();
        const deadline = startedAt + boundedRunTimeout(command.timeoutMs);
        const results: Array<{ index: number; op: string; passed: boolean; data?: unknown; error?: string; outcome?: string }> = [];
        let passed = true;
        for (const [index, step] of steps!.entries()) {
          try {
            operationTimeout(1, deadline);
            const response = await this.execute(step, deadline);
            // A late provider ACK must never count as PASS.
            operationTimeout(1, deadline);
            const data = response.data as { passed?: boolean };
            const ok = data.passed !== false;
            results.push({ index, op: step.op, passed: ok, data });
            if (!ok) { passed = false; break; }
          } catch (error) {
            passed = false;
            results.push({
              index, op: step.op, passed: false,
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
              return { data: report, image: Buffer.from(png).toString("base64") };
            }
          } catch { /* The failure report remains authoritative. */ }
        }
        return { data: report };
      }
      case "navigate": {
        const destination = browserUrl(command.url ?? "/", page.url() || CRM_ORIGIN);
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
        // Pinned Cloudflare Playwright keyboard.press has no timeout option.
        // The run checks the remaining budget on return; never race a live key event.
        await page.keyboard.press(command.key ?? "");
        return { data: { action: "press", completed: true } };
      case "session": {
        if (!isCrmRequest(page.url())) throw new Error("CRM_BROWSER_NOT_ON_CRM");
        // Uses the CRM's own current Access identity, not an override header.
        const result = await page.evaluate(async () => {
          const response = await fetch("/api/session", { credentials: "same-origin" });
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
      case "type": {
        if (!command.target) throw new Error("CRM_BROWSER_TARGET_REQUIRED");
        const locator = this.target(command.target);
        this.acceptingDialog = command.acceptDialog === true;
        try {
          if (command.op === "click") await locator.click({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "double_click") await locator.dblclick({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "fill") await locator.fill(command.text ?? "", { timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "select") await locator.selectOption(command.value ?? "", { timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "check") await locator.check({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "uncheck") await locator.uncheck({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "hover") await locator.hover({ timeout: operationTimeout(12_000, runDeadline) });
          else if (command.op === "type") await locator.pressSequentially(command.text ?? "", { timeout: operationTimeout(12_000, runDeadline) });
        } finally {
          this.acceptingDialog = false;
        }
        return {
          data: {
            action: command.op, completed: true, path: safePagePath(page.url()),
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

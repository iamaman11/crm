import { DurableObject } from "cloudflare:workers";
import { history, limits, type Page } from "@cloudflare/playwright";
import { DEFAULT_BROWSER_URL, redact, safePagePath } from "./browser/security";
import { siteAccessCredentialsReady } from "./site-access";
import type { Command, Env, BrowserCommandResult } from "./contracts";
import type { BrowserDevice } from "./browser-device";
import { WorkspaceFileTransfers } from "./browser-transfers";
import { readSafeBrowserHistory, readSafeBrowserLimits, isOriginalPageMissing } from "./browser-recovery";
import { openBrowser, restoreBrowser, closeBrowser, tabsBrowser, rememberSelectedPageIdentity,
  type BrowserLifecycleDependencies, type BrowserLifecycleState } from "./browser/lifecycle";
import { navigateTo, configuredSiteAccess, installSafetyRouting, navigationCommand, type NavigationState } from "./browser/navigation";
import { attachInspection, inspectionCommand, type InspectionState } from "./browser/inspection";
import { attachDialogHandling, interactionCommand, type InteractionState } from "./browser/interactions";
import { artifactCommand, recordingCommand } from "./browser/artifacts";
import { structuredHandoff, closeHandoff, liveView, type HandoffState } from "./browser/handoff";
import { BROWSER_RUNTIME_EPOCH, requireRuntimeEpoch } from "./runtime-epoch";
import type { HumanHandoffRecord } from "./browser-handoff";

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
  private nextOperation: Promise<void> = Promise.resolve();
  private pendingOperations = 0;
  private readonly navigation: NavigationState = { ownerDirectedSiteNavigationOrigin: null };
  private readonly interactions: InteractionState = { acceptingDialog: false, lastFailureScreenshot: null };
  private readonly handoff: HandoffState = { cdp: null };
  private readonly inspection: InspectionState = { consoleLog: [], requestStartedAt: new WeakMap(), requestLog: [] };

  private async navigateTo(destination: string, runDeadline?: number) {
    return navigateTo(this.lifecycle.page, this.navigation, this.env, destination, runDeadline);
  }

  private lifecycleDependencies(): BrowserLifecycleDependencies {
    return {
      state: this.lifecycle,
      binding: this.env.BROWSER,
      storage: this.ctx.storage,
      installSafetyRouting: context => installSafetyRouting(context, this.env, this.navigation, this.lifecycle, page => this.attachPage(page)),
      attachPage: page => this.attachPage(page),
      navigateTo: destination => this.navigateTo(destination),
      validateDestination: destination => {
        if (!siteAccessCredentialsReady(destination, configuredSiteAccess(this.env))) {
          throw new Error("BROWSER_SITE_ACCESS_CREDENTIALS_REQUIRED");
        }
      },
      beforeClose: () => closeHandoff(this.handoff),
      afterClose: () => {
        this.interactions.lastFailureScreenshot = null;
        this.interactions.acceptingDialog = false;
        this.navigation.ownerDirectedSiteNavigationOrigin = null;
        this.inspection.requestLog = [];
        this.inspection.consoleLog = [];
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

  private attachPage(page: Page) {
    if (this.lifecycle.tabIds.has(page)) return;
    this.lifecycle.tabIds.set(page, crypto.randomUUID());
    page.on("close", () => {
      this.lifecycle.tabIds.delete(page);
      if (this.lifecycle.page === page) {
        this.lifecycle.page = this.lifecycle.context?.pages().find(p => !p.isClosed()) ?? null;
      }
    });
    attachInspection(page, this.inspection);
    attachDialogHandling(page, this.interactions);
  }

  private async open(path: string = DEFAULT_BROWSER_URL, recording: boolean = false,
      requestedDevice?: BrowserDevice) {
    return openBrowser(this.lifecycleDependencies(), path, recording, requestedDevice);
  }

  private async close() {
    await closeBrowser(this.lifecycleDependencies());
  }

  private async execute(command: Command, runDeadline?: number): Promise<BrowserCommandResult> {
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
      return recordingCommand(command, this.env, this.ctx.storage, this.transfers, this.lifecycle.browserSessionId);
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
      return { data: await structuredHandoff(command, this.ctx, this.lifecycle, this.handoff) };
    }
    if (command.op === "tabs") return tabsBrowser(this.lifecycleDependencies(), command);
    const page = this.lifecycle.page;
    if (!page || page.isClosed()) throw new Error("CRM_BROWSER_NOT_OPEN");

    switch (command.op) {
      case "navigate": case "reload": case "back": case "forward":
        return navigationCommand(page, command, (destination, deadline) => this.navigateTo(destination, deadline), runDeadline);
      case "inspect": case "frames": case "snapshot": case "screenshot": case "console": case "session":
        return inspectionCommand(page, command, this.inspection, this.interactions.lastFailureScreenshot);
      case "visual": case "file":
        return artifactCommand(page, command, this.ctx.storage, this.transfers, this.lifecycle.context);
      case "live_view":
        return liveView(this.lifecycle, this.env.BROWSER);
      case "execute": case "viewport": case "wait": case "assert": case "run":
      case "scroll": case "press": case "click": case "double_click": case "fill": case "select":
      case "check": case "uncheck": case "hover": case "drag_to": case "type":
        return interactionCommand(page, command, {
          state: this.interactions,
          execute: (step, deadline) => this.execute(step, deadline),
          selectPopup: async popup => {
            this.attachPage(popup);
            this.lifecycle.page = popup;
            await this.rememberSelectedPageIdentity();
          },
          tabId: popup => this.lifecycle.tabIds.get(popup),
          latestRequests: () => this.inspection.requestLog.slice(-8),
        }, runDeadline);
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

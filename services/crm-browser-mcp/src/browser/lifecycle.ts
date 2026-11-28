// Lifecycle operations act only on the state owned by CrmBrowserSession.
// No second session manager, Durable Object, browser or workspace registry.
import { acquire, connect, type Browser, type BrowserContext, type Page } from "@cloudflare/playwright";
import { DEFAULT_BROWSER_URL, browserUrl, isPermittedBrowserRequest, safePagePath } from "./security.ts";
import { chooseDevice, deviceOptions, type BrowserDevice } from "../browser-device.ts";
import { WEB_VITALS_INIT } from "../browser-performance.ts";
import type { Command, BrowserCommandResult } from "../contracts";
import {
  classifyBrowserAcquireFailure, recoverExistingBrowserSession, selectRestoredPublicPage,
  parsePageTargetInfo, restoreForExplicitOpen, type BrowserPageIdentity,
} from "../browser-recovery.ts";

export interface BrowserLifecycleState {
  browser: Browser | null;
  context: BrowserContext | null;
  page: Page | null;
  browserSessionId: string | null;
  recordingEnabled: boolean;
  recoveredPageMissing: boolean;
  tabIds: Map<Page, string>;
}

export interface BrowserLifecycleDependencies {
  state: BrowserLifecycleState;
  binding: BrowserRun;
  storage: DurableObjectStorage;
  installSafetyRouting(context: BrowserContext): Promise<void>;
  attachPage(page: Page): void;
  navigateTo(destination: string): Promise<{ http_status: number | null; path: string; title: string }>;
  validateDestination(destination: string): void;
  beforeClose(): Promise<void>;
  afterClose(): void;
}

/** The pinned SDK implementation recognizes persistent:true at runtime, while
 * WorkersConnectOptions.d.ts omits it. Keep the structural subtype local to the
 * one Durable Object that owns Browser Run connections; do not weaken typing. */
function persistentConnectOptions(sessionId: string): { sessionId: string; persistent: true } {
  return { sessionId, persistent: true };
}

/** Read the exact Chromium target via native Playwright CDPSession,
 * never by URL/title. Default context is represented explicitly by the
 * provider-identity parser, not confused with an incognito context. */
async function readPageIdentity(page: Page): Promise<BrowserPageIdentity> {
  try {
    const cdp = await page.context().newCDPSession(page);
    try {
      const response: unknown = await cdp.send("Target.getTargetInfo");
      const identity = parsePageTargetInfo(response);
      if (!identity) throw new Error("BROWSER_PAGE_IDENTITY_UNAVAILABLE");
      return identity;
    } finally {
      try { await cdp.detach(); } catch { /* Best-effort read-only CDP cleanup. */ }
    }
  } catch {
    throw new Error("BROWSER_PAGE_IDENTITY_UNAVAILABLE");
  }
}

export async function rememberSelectedPageIdentity(state: BrowserLifecycleState, storage: DurableObjectStorage) {
  const page = state.page;
  if (!page || page.isClosed()) {
    await storage.delete("browser_page_identity");
    return;
  }
  await storage.put("browser_page_identity", await readPageIdentity(page));
}

// The only browser owner. The DO stores only an opaque Browser Run session
// ID; cookies, tabs and browser processes remain in the official runtime.
export async function restoreBrowser(deps: BrowserLifecycleDependencies) {
  const { state, binding, storage } = deps;
  // A connected provider does NOT prove that the original page survived.
  // Block browser_open/status/action after detecting a lost incognito page;
  // otherwise browser_open would silently create a replacement tab.
  if (state.recoveredPageMissing) throw new Error("BROWSER_SESSION_PAGE_NOT_RESTORED");
  if (state.browser?.isConnected()) return;
  state.browser = null;
  state.context = null;
  state.page = null;
  state.tabIds.clear();
  const sessionId = await storage.get<string>("browser_session_id");
  const recovered = await recoverExistingBrowserSession<Browser>(
    sessionId,
    async id => {
      const session = await binding.getSession(id);
      // Cloudflare BrowserRunSession is an SDK-specific shape, not a
      // structural {status:string}. Normalize only the observable status;
      // never invent a closed state for an unknown provider response.
      if (!session) return null;
      const status = (session as unknown as { status?: unknown }).status;
      return { status };
    },
    // The provider may retain the process after the controlling DO isolate
    // restarts. Use persistent CDP connections so browser contexts and tabs
    // are eligible for reattachment instead of client-scoped disposal.
    async id => connect(binding, persistentConnectOptions(id)),
  );
  if (recovered.kind === "missing") return;
  if (recovered.kind === "expired") {
    // Delete only a conclusively closed provider session; a lookup outage
    // or reconnect failure leaves persisted identity available for retry.
    // Only an explicit browser_open may acquire a new Browser Run.
    await storage.delete("browser_session_id");
    await storage.delete("browser_recording");
    await storage.delete("browser_page_identity");
    state.browserSessionId = null;
    state.recordingEnabled = false;
    return;
  }
  const { browser } = recovered;
  state.browser = browser;
  state.browserSessionId = recovered.sessionId;
  state.recordingEnabled = await storage.get<boolean>("browser_recording") ?? false;
  // Native CDP may retain a process while a disposable device context dies.
  // Only reattach to the exact original page target, never a random page.
  const storedPageIdentity: unknown = await storage.get("browser_page_identity");
  const restored = await selectRestoredPublicPage(
    browser.contexts(), context => context.pages(), isPermittedBrowserRequest,
    storedPageIdentity, page => readPageIdentity(page),
  );
  if (!restored) {
    state.recoveredPageMissing = true;
    throw new Error("BROWSER_SESSION_PAGE_NOT_RESTORED");
  }
  state.context = restored.context;
  // Do not authorize any page actions until the shared request policy is
  // reinstalled on the recovered context.
  state.recoveredPageMissing = true;
  try {
    await deps.installSafetyRouting(state.context);
    for (const page of state.context.pages()) deps.attachPage(page);
    state.page = restored.page;
    state.recoveredPageMissing = false;
  } catch {
    state.context = null;
    state.page = null;
    throw new Error("BROWSER_SESSION_REATTACH_FAILED");
  }
}

/** Both Open and new-tab use the same context initialization path. */
export async function ensureBrowserContext(deps: BrowserLifecycleDependencies, device: BrowserDevice): Promise<BrowserContext> {
  const { state, storage } = deps;
  if (!state.context) {
    if (!state.browser) throw new Error("CRM_BROWSER_NOT_OPEN");
    // Desktop uses Chromium's persistent default context; device emulation
    // uses the same workspace's official Playwright context.
    state.context = device === "desktop"
      ? (state.browser.contexts()[0] ?? null)
      : await state.browser.newContext({
          ...deviceOptions(device),
          acceptDownloads: true, serviceWorkers: "block",
        });
    if (!state.context) throw new Error("BROWSER_DEFAULT_CONTEXT_UNAVAILABLE");
    await state.context.addInitScript({content: WEB_VITALS_INIT});
    await deps.installSafetyRouting(state.context);
    await storage.put("browser_device", device);
  }
  return state.context;
}

export async function openBrowser(deps: BrowserLifecycleDependencies, path: string = DEFAULT_BROWSER_URL,
    recording: boolean = false, requestedDevice?: BrowserDevice) {
  const { state, binding, storage } = deps;
  const destination = browserUrl(path);
  deps.validateDestination(destination);
  const activeDevice = await storage.get<BrowserDevice>("browser_device") ?? null;
  const device = chooseDevice(requestedDevice, activeDevice);
  // The user explicitly requested Open: replace a lost browser only after
  // closing the original provider session. Status and other tools never
  // acquire a substitute browser.
  const restarted = await restoreForExplicitOpen(
    () => restoreBrowser(deps), () => closeBrowser(deps),
  );
  if (recording && state.browser && !state.recordingEnabled) {
    throw new Error("BROWSER_RECORDING_REQUIRES_NEW_SESSION");
  }
  if (state.page && !state.page.isClosed()) {
    if (state.page.url() === destination) {
      return { state: "already_open", path: safePagePath(state.page.url()), device };
    }
    return { state: "navigated", ...(await deps.navigateTo(destination)) };
  }
  let created = false;
  if (!state.browser) {
    // Official acquisition + explicit connection allows DO eviction recovery.
    // Keep-alive is idle time, not an automatic browser polling loop.
    // A failed acquire must not start a second browser via automatic retry.
    // Give the owner one stable, machine-readable error without echoing
    // the raw provider message or falsely attributing its account plan.
    let sessionId: string;
    try {
      ({ sessionId } = await acquire(binding, {
        keep_alive: 600_000, recording,
      }));
    } catch (error) {
      throw classifyBrowserAcquireFailure(error);
    }
    try {
      state.browser = await connect(binding, persistentConnectOptions(sessionId));
      state.browserSessionId = sessionId;
      await storage.put("browser_session_id", sessionId);
      await storage.put("browser_recording", recording);
      state.recordingEnabled = recording;
      created = true;
    } catch (error) {
      await binding.closeSession(sessionId);
      throw error;
    }
  }
  try {
    const context = await ensureBrowserContext(deps, device);
    state.page = await context.newPage();
    deps.attachPage(state.page);
    await rememberSelectedPageIdentity(state, storage);
    return { state: "open", device, ...(restarted ? { restarted: true } : {}), ...(await deps.navigateTo(destination)) };
  } catch (error) {
    if (created) await closeBrowser(deps);
    throw error;
  }
}

export async function closeBrowser(deps: BrowserLifecycleDependencies) {
  const { state, binding, storage } = deps;
  await deps.beforeClose();
  const sessionId = state.browserSessionId ??
    await storage.get<string>("browser_session_id");
  let failure: unknown = null;
  // Disconnecting the Playwright client is best-effort. Always attempt
  // termination of the exact Browser Run session to avoid billable orphans.
  try {
    await state.browser?.close();
  } catch {
    // A disconnected Playwright client cannot override a successful native
    // Browser Run closeSession. The provider closure is authoritative.
  }
  try {
    if (sessionId) {
      await binding.closeSession(sessionId);
      if (state.recordingEnabled) {
        // The provider recording becomes inspectable only after closure.
        await storage.put("last_recording_session_id", sessionId);
      }
    }
    await storage.delete("browser_session_id");
    await storage.delete("browser_recording");
    await storage.delete("browser_page_identity");
    await storage.delete("browser_handoff");
    await storage.delete("browser_device");
    await storage.delete("visual_baseline");
  } catch (error) {
    failure ??= error;
    // Keep the persisted ID on failure so a later owner request can retry
    // termination. Never create a replacement session implicitly.
  } finally {
    state.browser = null;
    state.browserSessionId = null;
    state.recordingEnabled = false;
    state.recoveredPageMissing = false;
    state.context = null;
    state.page = null;
    state.tabIds.clear();
    deps.afterClose();
  }
  if (failure) throw failure;
}

export async function tabsBrowser(deps: BrowserLifecycleDependencies, command: Command): Promise<BrowserCommandResult> {
  const { state, storage } = deps;
  if (!state.browser) throw new Error("CRM_BROWSER_NOT_OPEN");
  if (command.tabAction === "new") {
    const context = state.context ?? await ensureBrowserContext(
      deps,
      await storage.get<BrowserDevice>("browser_device") ?? "desktop",
    );
    if (context.pages().filter(p => !p.isClosed()).length >= 12) {
      throw new Error("BROWSER_TAB_LIMIT");
    }
    // A new tab uses the same site-neutral context and optional access policy.
    const url = command.url ? browserUrl(command.url, state.page?.url() || DEFAULT_BROWSER_URL) : null;
    const page = await context.newPage();
    deps.attachPage(page);
    state.page = page;
    if (url) await deps.navigateTo(url);
  } else if (command.tabAction === "select" || command.tabAction === "close") {
    const matched = [...state.tabIds.entries()].find(([page, id]) =>
      id === command.tabId && !page.isClosed(),
    );
    if (!matched) throw new Error("BROWSER_TAB_NOT_FOUND");
    const [page] = matched;
    if (command.tabAction === "select") {
      state.page = page;
      await page.bringToFront();
    } else {
      await page.close();
      state.tabIds.delete(page);
      if (state.page === page) {
        state.page = state.context?.pages().find(p => !p.isClosed()) ?? null;
      }
    }
  } else if (command.tabAction !== "list") {
    throw new Error("BROWSER_UNKNOWN_TAB_ACTION");
  }
  // A tab switch/new/close changes the active CDP target. Persist the
  // exact replacement identity before reporting a successful command.
  if (command.tabAction !== "list") await rememberSelectedPageIdentity(state, storage);
  const tabs = await Promise.all(
    (state.context?.pages() ?? []).filter(p => !p.isClosed()).map(async (page) => {
      deps.attachPage(page);
      return {
        tabId: state.tabIds.get(page),
        active: page === state.page,
        path: safePagePath(page.url()),
        title: await page.title(),
      };
    }),
  );
  return { data: { tabs, count: tabs.length } };
}

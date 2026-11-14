import type { LimitsResponse, ClosedSession } from "@cloudflare/playwright";
/** A single testable recovery decision used by the real Durable Object.
 * Only the official Cloudflare Browser Run session may be reattached.
 * Never acquire a replacement browser as a side-effect of status or restore. */
export type ExistingSession = { status?: unknown } | null;

export type RecoveryOutcome<Browser> =
  | { kind: "missing" }
  | { kind: "expired" }
  | { kind: "connected"; sessionId: string; browser: Browser };

export async function recoverExistingBrowserSession<Browser>(
  sessionId: string | undefined,
  getSession: (sessionId: string) => Promise<ExistingSession>,
  reconnect: (sessionId: string) => Promise<Browser>,
): Promise<RecoveryOutcome<Browser>> {
  if (!sessionId) return { kind: "missing" };
  // If lookup fails, propagate the error and preserve the stored session ID.
  // An API outage is not proof of closure and must not create a second browser.
  const session = await getSession(sessionId);
  const status = typeof session?.status === "string" ? session.status.toLowerCase() : undefined;
  if (!session || status === "closed" || status === "closing" || status === "expired") {
    return { kind: "expired" };
  }
  try {
    return { kind: "connected", sessionId, browser: await reconnect(sessionId) };
  } catch {
    // Session might still be alive. Do not erase the stored ID or acquire a
    // replacement, and never return an ambiguous success or raw credentials.
    throw new Error("BROWSER_SESSION_RECONNECT_FAILED");
  }
}


/** Provider errors are not safe to echo: upstream messages may contain URLs or
 * credentials. Preserve only the distinction confirmed by the provider text.
 * Never infer account plan/billing state from a generic HTTP 429. */
export function classifyBrowserAcquireFailure(cause: unknown): Error {
  const message = cause instanceof Error ? cause.message : String(cause);
  if (/browser time limit exceeded for today/i.test(message)) {
    return new Error("BROWSER_PROVIDER_DAILY_LIMIT_EXCEEDED");
  }
  if (/\b429\b/i.test(message) &&
      /rate limit exceeded|too many requests/i.test(message)) {
    return new Error("BROWSER_PROVIDER_RATE_LIMITED");
  }
  return new Error("BROWSER_PROVIDER_ACQUIRE_FAILED");
}


/** Read-only Cloudflare Browser Run account limit readback, never acquisition.
 * The provider returns session IDs, but the MCP may only reveal a count.
 * A failed/unsupported limits endpoint is NOT evidence of available capacity. */
export async function readSafeBrowserLimits(
  readLimits: () => Promise<LimitsResponse>,
): Promise<{
  state: "available" | "unavailable";
  activeSessionCount?: number;
  allowedBrowserAcquisitions?: number;
  maxConcurrentSessions?: number;
  timeUntilNextAllowedBrowserAcquisition?: number;
}> {
  try {
    const result = await readLimits();
    const values = [
      result.activeSessions?.length,
      result.allowedBrowserAcquisitions,
      result.maxConcurrentSessions,
      result.timeUntilNextAllowedBrowserAcquisition,
    ];
    if (!values.every(value =>
      typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    )) return { state: "unavailable" };
    return {
      state: "available",
      activeSessionCount: result.activeSessions.length,
      allowedBrowserAcquisitions: result.allowedBrowserAcquisitions,
      maxConcurrentSessions: result.maxConcurrentSessions,
      timeUntilNextAllowedBrowserAcquisition: result.timeUntilNextAllowedBrowserAcquisition,
    };
  } catch {
    // Do not echo provider messages or expose a misleading zero-capacity
    // readback. Browser status itself remains usable if this API is down.
    return { state: "unavailable" };
  }
}


/** Account-wide recent Browser Run history, sanitized. This is a sample of
 * closed sessions, NOT a Cloudflare usage/billing counter. No session IDs
 * or arbitrary provider close-reason text may leave this boundary. */
export async function readSafeBrowserHistory(
  readHistory: () => Promise<ClosedSession[]>,
  nowMs: number = Date.now(),
): Promise<{
  state: "available" | "unavailable";
  recordsReturned?: number;
  recordsExamined?: number;
  closedTodayUtc?: number;
  observedDurationMsTodayUtc?: number;
  normalClosuresTodayUtc?: number;
  idleClosuresTodayUtc?: number;
  partial?: boolean;
}> {
  try {
    const records = await readHistory();
    if (!Array.isArray(records) || !Number.isFinite(nowMs)) {
      return { state: "unavailable" };
    }
    const sample = records.slice(0, 200);
    const startOfUtcDay = Math.floor(nowMs / 86_400_000) * 86_400_000;
    let closedTodayUtc = 0;
    let observedDurationMsTodayUtc = 0;
    let normalClosuresTodayUtc = 0;
    let idleClosuresTodayUtc = 0;
    for (const entry of sample) {
      if (!entry || !Number.isFinite(entry.startTime) ||
          !Number.isFinite(entry.endTime) || entry.endTime < entry.startTime ||
          entry.endTime < startOfUtcDay || entry.endTime > nowMs) continue;
      closedTodayUtc++;
      // Count only the portion of each recorded session in the current UTC
      // day, never infer missing sessions or treat this as billed usage.
      observedDurationMsTodayUtc +=
        entry.endTime - Math.max(entry.startTime, startOfUtcDay);
      if (entry.closeReasonText === "NormalClosure") normalClosuresTodayUtc++;
      else if (entry.closeReasonText === "BrowserIdle") idleClosuresTodayUtc++;
    }
    return {
      state: "available",
      recordsReturned: records.length,
      recordsExamined: sample.length,
      closedTodayUtc, observedDurationMsTodayUtc,
      normalClosuresTodayUtc, idleClosuresTodayUtc,
      partial: records.length > sample.length,
    };
  } catch {
    // Provider errors and raw account session identifiers are never echoed.
    return { state: "unavailable" };
  }
}


/** A lost tab is an expected lifecycle event, not a reason to make the
 * operator issue a separate Close. Only an explicit Open may replace it. */
export function isOriginalPageMissing(error: unknown): boolean {
  return error instanceof Error &&
    error.message === "BROWSER_SESSION_PAGE_NOT_RESTORED";
}

/** Reopen is a user command: terminate the old Browser Run before acquiring
 * another. All other provider errors remain visible, with no extra acquire. */
export async function restoreForExplicitOpen(
  restore: () => Promise<void>,
  close: () => Promise<void>,
): Promise<boolean> {
  try {
    await restore();
    return false;
  } catch (error) {
    if (!isOriginalPageMissing(error)) throw error;
    await close();
    return true;
  }
}


/** Chromium exposes both the page target and its incognito BrowserContext ID
 * through the public Playwright CDP session. URL equality alone is never an
 * identity proof: a default-context tab or a different workspace may have the
 * same public URL. Keep both opaque IDs only inside this workspace's DO. */
export type BrowserPageIdentity = Readonly<{
  targetId: string;
  browserContextId: string;
}>;

export function validBrowserPageIdentity(value: unknown): BrowserPageIdentity | null {
  if (!value || typeof value !== "object") return null;
  const info = value as { targetId?: unknown; browserContextId?: unknown };
  if (typeof info.targetId !== "string" ||
      typeof info.browserContextId !== "string") return null;
  if (!info.targetId || !info.browserContextId ||
      info.targetId.length > 128 || info.browserContextId.length > 128) return null;
  return { targetId: info.targetId, browserContextId: info.browserContextId };
}

/** Target.getTargetInfo is a read-only, official CDP operation performed
 * against the page's own Playwright CDPSession. Default contexts have no
 * browserContextId: do not treat them as an isolated original context. */
export function parsePageTargetInfo(value: unknown): BrowserPageIdentity | null {
  if (!value || typeof value !== "object") return null;
  const targetInfo = (value as { targetInfo?: unknown }).targetInfo;
  if (!targetInfo || typeof targetInfo !== "object") return null;
  const info = targetInfo as { type?: unknown };
  if (info.type !== "page") return null;
  return validBrowserPageIdentity(targetInfo);
}

/** Reconnect only to the exact previously selected Chromium page target AND
 * incognito BrowserContext. A different public tab, even with the same URL,
 * is not a recovered workspace. No new context/tab/browser is created. */
export async function selectRestoredPublicPage<
  Context,
  Page extends { isClosed(): boolean; url(): string },
>(
  contexts: readonly Context[],
  pagesOf: (context: Context) => readonly Page[],
  isPublicRequest: (url: string) => boolean,
  expectedIdentity: unknown,
  readIdentity: (page: Page) => Promise<BrowserPageIdentity>,
): Promise<{ context: Context; page: Page } | null> {
  const expected = validBrowserPageIdentity(expectedIdentity);
  if (!expected) return null;
  for (const context of contexts) {
    for (const page of pagesOf(context)) {
      try {
        if (page.isClosed()) continue;
        const url = page.url();
        if (!/^https?:\/\//i.test(url) || !isPublicRequest(url)) continue;
        const actual = await readIdentity(page);
        if (actual.targetId !== expected.targetId ||
            actual.browserContextId !== expected.browserContextId) continue;
        return { context, page };
      } catch { /* Detached pages or CDP errors cannot prove identity. */ }
    }
  }
  return null;
}

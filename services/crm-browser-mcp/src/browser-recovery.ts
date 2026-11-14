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

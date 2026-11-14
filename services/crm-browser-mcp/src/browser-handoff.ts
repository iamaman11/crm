/** Cloudflare Browser Run native structured handoff contract. Browser, CDP
 * sessions, event lifecycle and Durable Object storage are owned exclusively
 * by CrmBrowserSession. This module only validates typed provider messages. */
export type HumanHandoffStatus =
  "waiting_human" | "completed" | "failed" | "timed_out" | "unconfirmed";
export type HumanHandoffRecord = {
  sessionId: string;
  handoffId: string;
  status: HumanHandoffStatus;
  startedAt: number;
  deadline: number;
};
export const HANDOFF_MIN_MS = 30_000;
export const HANDOFF_MAX_MS = 1_800_000;
export const HANDOFF_DEFAULT_MS = 600_000;

export function validateHandoffRequest(instructions: unknown, timeoutMs: unknown): {
  instructions: string;
  timeoutMs: number;
} {
  if (typeof instructions !== "string" || instructions.trim().length < 5 ||
      instructions.length > 1_000) throw new Error("BROWSER_HANDOFF_INSTRUCTIONS_INVALID");
  const timeout = timeoutMs === undefined ? HANDOFF_DEFAULT_MS : timeoutMs;
  if (typeof timeout !== "number" || !Number.isInteger(timeout) ||
      timeout < HANDOFF_MIN_MS || timeout > HANDOFF_MAX_MS) {
    throw new Error("BROWSER_HANDOFF_TIMEOUT_INVALID");
  }
  return { instructions: instructions.trim(), timeoutMs: timeout };
}

export function handoffIdentifier(value: unknown): string {
  if (typeof value !== "object" || value === null) {
    throw new Error("BROWSER_HANDOFF_PROVIDER_INVALID");
  }
  const id = (value as {handoffId?: unknown}).handoffId;
  if (typeof id !== "string" || !/^[\w-]{1,128}$/.test(id)) {
    throw new Error("BROWSER_HANDOFF_PROVIDER_INVALID");
  }
  return id;
}

export function handoffLiveViewUrl(value: unknown): string {
  const raw = value && typeof value === "object" &&
    (value as {devtoolsFrontendUrl?: unknown}).devtoolsFrontendUrl;
  if (typeof raw !== "string" || raw.length > 8_192) {
    throw new Error("BROWSER_HANDOFF_LIVE_VIEW_INVALID");
  }
  let parsed: URL;
  try { parsed = new URL(raw); } catch {
    throw new Error("BROWSER_HANDOFF_LIVE_VIEW_INVALID");
  }
  if (parsed.protocol !== "https:" || parsed.username || parsed.password ||
      parsed.hostname !== "live.browser.run") {
    throw new Error("BROWSER_HANDOFF_LIVE_VIEW_INVALID");
  }
  return raw;
}

export function completedHandoff(
  record: HumanHandoffRecord, result: unknown, now: number = Date.now(),
): HumanHandoffRecord {
  if (!result || typeof result !== "object") {
    return { ...record, status: "unconfirmed" };
  }
  const event = result as { success?: unknown; reason?: unknown };
  if (event.success === true) return { ...record, status: "completed" };
  if (event.success !== false) return { ...record, status: "unconfirmed" };
  const reason = typeof event.reason === "string" ? event.reason.toLowerCase() : "";
  return {
    ...record,
    status: /timeout|timed.?out/.test(reason) || now >= record.deadline ?
      "timed_out" : "failed",
  };
}

export function isProviderHandoffActive(result: unknown): boolean | null {
  if (!result || typeof result !== "object") return null;
  const active = (result as {active?: unknown}).active;
  return typeof active === "boolean" ? active : null;
}

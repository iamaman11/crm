/** Code compatibility fence between MCP HTTP Worker and its long-lived Durable
 * Object isolate. Change this value with EVERY MCP src/ TypeScript change.
 * A DO still running previous code must not execute new Worker commands. */
export const BROWSER_RUNTIME_EPOCH = "browser-mcp-20261114-fence-10" as const;

export function requireRuntimeEpoch(actual: string | undefined): void {
  if (actual !== BROWSER_RUNTIME_EPOCH) throw new Error("BROWSER_RUNTIME_EPOCH_MISMATCH");
}

export function isVerifiedRuntimeProbe(input: unknown): boolean {
  if (typeof input !== "object" || input === null) return false;
  const data = input as { ok?: unknown; data?: { epoch?: unknown } };
  return data.ok === true && data.data?.epoch === BROWSER_RUNTIME_EPOCH;
}


/** Stale DOs must never execute new page actions. However their old session
 * must be safely closable, or Browser Run resources can remain billable.
 * This read-only handshake is used ONLY for an explicit owner-authorized
 * "close" command, never for any other operation or a blind retry. */
export function cleanupRuntimeEpoch(probe: unknown): string {
  if (typeof probe !== "object" || probe === null) return BROWSER_RUNTIME_EPOCH;
  const response = probe as { ok?: unknown; data?: { epoch?: unknown } };
  const value = response.ok === true ? response.data?.epoch : null;
  if (typeof value !== "string" ||
      !/^browser-mcp-[0-9]{8}-fence-[1-9][0-9]*$/.test(value)) {
    return BROWSER_RUNTIME_EPOCH;
  }
  return value;
}

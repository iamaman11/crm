/** Code compatibility fence between MCP HTTP Worker and its long-lived Durable
 * Object isolate. Change this value with EVERY MCP src/ TypeScript change.
 * A DO still running previous code must not execute new Worker commands. */
export const BROWSER_RUNTIME_EPOCH = "browser-mcp-20261114-fence-6" as const;

export function requireRuntimeEpoch(actual: string | undefined): void {
  if (actual !== BROWSER_RUNTIME_EPOCH) throw new Error("BROWSER_RUNTIME_EPOCH_MISMATCH");
}

export function isVerifiedRuntimeProbe(input: unknown): boolean {
  if (typeof input !== "object" || input === null) return false;
  const data = input as { ok?: unknown; data?: { epoch?: unknown } };
  return data.ok === true && data.data?.epoch === BROWSER_RUNTIME_EPOCH;
}

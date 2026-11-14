/** Universal CDP commands on the SAME Playwright Page and Browser Run owner.
 * This is a protocol adapter, not another browser/session manager or an
 * arbitrary Worker-side JavaScript interpreter. */
export type CdpCommand = {
  method: string;
  params?: Record<string, unknown>;
};

export type CdpStepResult = {
  index: number;
  method: string;
  passed: boolean;
  elapsedMs: number;
  result?: unknown;
  error?: string;
  outcome?: "uncertain";
};

const CDP_METHOD = /^[A-Za-z][A-Za-z0-9_]*\.[A-Za-z][A-Za-z0-9_]*$/;
const MAX_INPUT_CHARS = 256_000;
const MAX_OUTPUT_CHARS = 256_000;

export function validateCdpCommands(commands: readonly CdpCommand[] | undefined): void {
  if (!commands?.length || commands.length > 128) {
    throw new Error("BROWSER_CDP_COMMAND_COUNT_INVALID");
  }
  for (const command of commands) {
    if (!command || typeof command.method !== "string" ||
        command.method.length > 128 || !CDP_METHOD.test(command.method) ||
        (command.params !== undefined &&
          (command.params === null || typeof command.params !== "object" ||
            Array.isArray(command.params)))) {
      throw new Error("BROWSER_CDP_INVALID_COMMAND");
    }
  }
  let serialized: string | undefined;
  try { serialized = JSON.stringify(commands); } catch { /* A non-JSON input is invalid. */ }
  if (!serialized || serialized.length > MAX_INPUT_CHARS) {
    throw new Error("BROWSER_CDP_INPUT_TOO_LARGE_OR_INVALID");
  }
}

export async function executeCdpCommands(
  commands: readonly CdpCommand[],
  send: (method: string, params?: Record<string, unknown>) => Promise<unknown>,
  timeoutMs: number = 90_000,
  redactError: (error: unknown) => string = error => String(error),
  now: () => number = Date.now,
): Promise<{
  passed: boolean;
  total: number;
  executed: number;
  elapsedMs: number;
  results: CdpStepResult[];
}> {
  // Validate the entire sequence BEFORE issuing any CDP command.
  validateCdpCommands(commands);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1_000 || timeoutMs > 120_000) {
    throw new Error("BROWSER_CDP_TIMEOUT_INVALID");
  }
  const startedAt = now();
  const deadline = startedAt + timeoutMs;
  const results: CdpStepResult[] = [];
  let serializedOutputLength = 0;
  for (const [index, command] of commands.entries()) {
    const started = now();
    if (started >= deadline) {
      results.push({
        index, method: command.method, passed: false, elapsedMs: 0,
        error: "BROWSER_CDP_DEADLINE_EXCEEDED",
      });
      break;
    }
    try {
      // The official Playwright CDP session owns every call. Do not race a
      // live mutation against a timer and release the DO's command queue.
      const response = await send(command.method, command.params);
      if (now() >= deadline) throw new Error("BROWSER_CDP_DEADLINE_EXCEEDED");
      const encoded = JSON.stringify(response ?? null);
      if (!encoded || encoded.length + serializedOutputLength > MAX_OUTPUT_CHARS) {
        throw new Error("BROWSER_CDP_RESULT_TOO_LARGE");
      }
      serializedOutputLength += encoded.length;
      results.push({
        index, method: command.method, passed: true,
        elapsedMs: Math.max(0, now() - started), result: response ?? null,
      });
    } catch (error) {
      // The CDP call may have reached Chromium before the response failed.
      // Report uncertainty, never implicitly replay side-effecting commands.
      results.push({
        index, method: command.method, passed: false,
        elapsedMs: Math.max(0, now() - started),
        error: redactError(error instanceof Error ? error.message : error).slice(0, 1200),
        outcome: "uncertain",
      });
      break;
    }
  }
  return {
    passed: results.length === commands.length && results.every(x => x.passed),
    total: commands.length,
    executed: results.length,
    elapsedMs: Math.max(0, now() - startedAt),
    results,
  };
}

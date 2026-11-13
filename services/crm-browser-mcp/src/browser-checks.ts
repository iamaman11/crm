// Shared deterministic browser assertions. The real Page and browser remain
// exclusively owned by the Durable Object; this module has no browser state.
export type BrowserAssertionKind =
  "url" | "title" | "visible" | "hidden" | "text" | "count";
export type TextMatch = "exact" | "contains";
export type WaitState = "visible" | "hidden" | "attached" | "detached";

export function boundedWaitTimeout(ms: number | undefined): number {
  if (ms === undefined) return 10_000;
  if (!Number.isInteger(ms) || ms < 100 || ms > 30_000) {
    throw new Error("BROWSER_WAIT_TIMEOUT_OUT_OF_RANGE");
  }
  return ms;
}

export type BrowserAssertion = {
  kind: BrowserAssertionKind;
  actual: string | number | boolean;
  expected: string | number | boolean;
  match?: TextMatch;
};

export function compareBrowserAssertion(input: BrowserAssertion) {
  const { kind, actual, expected, match = "exact" } = input;
  if (typeof actual !== typeof expected) {
    throw new Error("BROWSER_ASSERT_TYPE_MISMATCH");
  }
  if (match === "contains" && typeof actual !== "string") {
    throw new Error("BROWSER_ASSERT_CONTAINS_REQUIRES_TEXT");
  }
  const passed = match === "contains"
    ? (actual as string).includes(expected as string)
    : actual === expected;
  return { passed, kind, match, expected, actual };
}

// Input values of an assertion may contain secrets; never place raw form
// values or arbitrary credential-bearing URLs into persistent test reports.
export function safeAssertionValue(value: string | number | boolean): string | number | boolean {
  if (typeof value !== "string") return value;
  return value.slice(0, 512).replace(/([?&](?:token|key|code|secret|password|session)=)[^&#\s]+/gi, "$1[REDACTED]");
}

/** One canonical list for the published scenario schema and DO validation. */
export const SCENARIO_OPERATIONS = [
  "click", "double_click", "fill", "type", "select", "check",
  "uncheck", "hover", "press", "scroll", "navigate", "reload",
  "back", "forward", "wait", "assert",
] as const;
export type ScenarioOperation = (typeof SCENARIO_OPERATIONS)[number];
const scenarioSet: ReadonlySet<string> = new Set(SCENARIO_OPERATIONS);
type ScenarioStep = {
  op: string;
  target?: unknown;
  kind?: unknown;
  expected?: unknown;
  url?: unknown;
  key?: unknown;
};
const targetOperations: ReadonlySet<string> = new Set([
  "click", "double_click", "fill", "type", "select", "check", "uncheck", "hover",
]);
export function validateScenarioSteps(steps: readonly ScenarioStep[] | undefined): void {
  if (!steps || steps.length === 0 || steps.length > 12) {
    throw new Error("BROWSER_RUN_STEPS_OUT_OF_RANGE");
  }
  // Preflight ALL steps before any side effects. An unsafe late step must not
  // cause the earlier clicks/submit to happen and only then fail.
  for (const step of steps) {
    if (!scenarioSet.has(step.op)) throw new Error("BROWSER_RUN_UNSAFE_STEP");
    if (targetOperations.has(step.op) && !step.target) {
      throw new Error("BROWSER_RUN_TARGET_REQUIRED");
    }
    if (step.op === "assert" &&
        (!["url", "title", "visible", "hidden", "text", "count"].includes(String(step.kind)) ||
          step.expected === undefined ||
          (!["url", "title"].includes(String(step.kind)) && !step.target))) {
      throw new Error("BROWSER_RUN_INVALID_ASSERTION");
    }
    if (step.op === "navigate" && (typeof step.url !== "string" || !step.url)) {
      throw new Error("BROWSER_RUN_URL_REQUIRED");
    }
    if (step.op === "press" && (typeof step.key !== "string" || !step.key)) {
      throw new Error("BROWSER_RUN_KEY_REQUIRED");
    }
  }
}
export function boundedRunTimeout(ms: number | undefined): number {
  if (ms === undefined) return 90_000;
  if (!Number.isInteger(ms) || ms < 1_000 || ms > 120_000) {
    throw new Error("BROWSER_RUN_TIMEOUT_OUT_OF_RANGE");
  }
  return ms;
}
/** Never race an unabortable page action against a timer: that would release
 * the DO operation lock while a remote click might still be executing. */
export function operationTimeout(defaultMs: number, deadline?: number, now = Date.now()): number {
  if (deadline === undefined) return defaultMs;
  const remaining = Math.ceil(deadline - now);
  if (remaining <= 0) throw new Error("BROWSER_RUN_DEADLINE_EXCEEDED");
  return Math.min(defaultMs, remaining);
}
export function isFailedVerdict(data: unknown): boolean {
  return typeof data === "object" && data !== null &&
    "passed" in data && data.passed === false;
}

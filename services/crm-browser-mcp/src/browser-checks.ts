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

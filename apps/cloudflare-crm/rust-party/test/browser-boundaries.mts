import assert from "node:assert/strict";
import type { APIResponse, Response } from "playwright";
import { string } from "../../scripts/json-boundary.mts";

export type BrowserSql = (query: string, ...params: (string | number)[]) => Promise<ReadonlyArray<Record<string, unknown>>>;

export type BrowserFailureScenario = "typed-browser" | "published-smoke";

const SAFE_ERROR_NAMES = new Set([
  "AssertionError", "Error", "RangeError", "TimeoutError", "TypeError", "UnknownError",
]);

/** Preserve only the failure class and an allowlisted test source frame. */
export function safeBrowserFailure(
  error: unknown,
  scenario: BrowserFailureScenario,
): Error {
  const sourceFile = `${scenario}.mts`;
  const candidateName = error instanceof Error ? error.name : "UnknownError";
  const name = SAFE_ERROR_NAMES.has(candidateName) ? candidateName : "UnknownError";
  const stack = error instanceof Error ? error.stack ?? "" : "";
  const framePattern = new RegExp(`^\\s+at .*[/\\\\]${scenario}\\.mts:(\\d+):(\\d+)\\)?$`);
  const frame = stack.split("\n").slice(1)
    .map(line => framePattern.exec(line))
    .find((match): match is RegExpExecArray => match !== null);
  const safe = new Error(`${scenario} failed`);
  safe.name = name;
  safe.stack = `${name}: ${scenario} failed${frame ? `\n    at ${sourceFile}:${frame[1]}:${frame[2]}` : ""}`;
  return safe;
}

// The query is selected by the test, while D1 results are an external boundary.
export function browserFirstId(sql: BrowserSql, tenant: string) {
  return async (table: string, column: string, value: string): Promise<string> => {
    const rows = await sql("SELECT id FROM " + table + " WHERE tenant=? AND json_extract(state,?)=?",
      tenant, "$." + column, value);
    assert.equal(rows.length, 1);
    return string(rows[0]?.id);
  };
}

// Query strings may contain privacy probes; only compare the RPC pathname.
export function matchesRpcPath(response: APIResponse | Response, path: string): boolean {
  return new URL(response.url()).pathname===path;
}

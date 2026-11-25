import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import type { APIResponse, Response } from "playwright";
import { parseFixtureConnectCode } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { string } from "../../scripts/json-boundary.mts";
import type { RpcEvidence } from "./rpc-browser-evidence.mts";

export type BrowserSql = (query: string, ...params: (string | number)[]) => Promise<ReadonlyArray<Record<string, unknown>>>;

const EXPECTED_RESULT = {
  "signed-readback": "The signed version endpoint reports the candidate SHA",
  "browser-setup": "Start Chromium with the expected test identity",
  "navigation": "Render the expected application route",
  "reload": "Reload the React shell and restore the route",
  "scenario": "Pass the browser and D1 assertions at the reported test line",
  "party-create": "Save one Party and verify its D1 evidence",
  "party-reload": "Read the saved Party after a reload",
  "account-create": "Save one Account and verify its D1 evidence",
  "viewer-denial": "Deny the viewer write and leave D1 unchanged",
  "tenant-isolation": "Show only the second test tenant's data",
  "anonymous-denial": "Stop the anonymous request at Cloudflare Access",
  "correlation": "Match browser requests with the expected source and D1 evidence",
  "cleanup": "Restore the test role and close the browser",
} as const;

export type BrowserFailurePhase = keyof typeof EXPECTED_RESULT;
type BrowserFailureContext = {
  scenario: "typed-browser" | "published-smoke" | "live";
  phase: BrowserFailurePhase;
  sourceSha: string;
  directory: string;
  steps?: ReadonlyArray<string>;
  requests?: ReadonlyArray<RpcEvidence>;
};

const SAFE_ERROR_NAMES = new Set([
  "AssertionError", "Error", "RangeError", "TimeoutError", "TypeError", "UnknownError",
]);

// Failure output deliberately excludes response paths and payloads, browser
// content, history, and audit rows. The successful release proof is separate.
export function reportBrowserFailure(
  error: unknown,
  { scenario, phase, sourceSha, directory, steps = [], requests = [] }: BrowserFailureContext,
): Error {
  const sourceFile = `${scenario}.mts`;
  const candidateName = error instanceof Error ? error.name : "UnknownError";
  const name = SAFE_ERROR_NAMES.has(candidateName) ? candidateName : "UnknownError";
  const stack = error instanceof Error ? error.stack ?? "" : "";
  const framePattern = new RegExp(`^\\s+at .*[/\\\\]${scenario}\\.mts:(\\d+):(\\d+)\\)?$`);
  const frame = stack.split("\n").slice(1)
    .map(line => framePattern.exec(line))
    .find((match): match is RegExpExecArray => match !== null);
  const message = `${scenario} failed during ${phase} after checkpoint ${steps.length}`;
  const safe = new Error(message);
  safe.name = name;
  safe.stack = `${name}: ${message}${frame ? `\n    at ${sourceFile}:${frame[1]}:${frame[2]}` : ""}`;

  const recentRequests = requests.slice(-8).map(request => {
    // An unverified response can still show that a request failed, but its
    // correlation headers cannot establish which Worker handled it.
    const correlated = request.source_sha === sourceSha && request.runtime === "native-workers-rs";
    return {
      status: Number.isInteger(request.status) && request.status >= 100 && request.status <= 599
        ? request.status : null,
      request_id: correlated && /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(request.request_id ?? "")
        ? request.request_id : null,
      connect_code: correlated && typeof parseFixtureConnectCode(request.error_code ?? "") === "number"
        ? request.error_code : null,
    };
  });
  try {
    writeFileSync(resolve(directory, "failure.json"), JSON.stringify({
      result: "FAIL",
      source_sha: /^[a-f0-9]{40}$/.test(sourceSha) ? sourceSha : null,
      scenario, phase, error: name,
      expected: EXPECTED_RESULT[phase],
      checkpoint: steps.length,
      last_completed: steps.at(-1) ?? null,
      location: frame ? `${sourceFile}:${frame[1]}:${frame[2]}` : null,
      recent_requests: recentRequests,
    }, null, 2) + "\n");
  } catch {
    console.error("Browser failure evidence could not be saved");
  }
  return safe;
}

export async function withBrowserCleanup<T>(run: () => Promise<T>, cleanup: () => Promise<void>): Promise<T> {
  let completed = false;
  try {
    const result = await run();
    completed = true;
    return result;
  } finally {
    try {
      await cleanup();
    } catch (error) {
      if (completed) throw error;
      console.error("Browser cleanup failed after an earlier failure");
    }
  }
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

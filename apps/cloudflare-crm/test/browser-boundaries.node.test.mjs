import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmdirSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { reportBrowserFailure, withBrowserCleanup } from "../rust-party/test/browser-boundaries.mts";
import { captureBrowserRpcEvidence } from "../rust-party/test/rpc-browser-evidence.mts";

const sourceSha = "0123456789abcdef0123456789abcdef01234567";
const requestId = "561679ff-b6e2-4c5b-bdf4-428278f2e547";

for (const scenario of ["typed-browser", "published-smoke"]) {
  test(`${scenario} writes only safe failure evidence, including an actionable checkpoint`, t => {
    const directory = mkdtempSync(join(tmpdir(), "crm-browser-failure-"));
    const file = join(directory, "failure.json");
    t.after(() => {
      unlinkSync(file);
      rmdirSync(directory);
    });
    const marker = "private-crm-form-sentinel";
    const error = new Error(`Playwright matcher output ${marker}`, {cause: new Error(marker)});
    error.name = "AssertionError";
    const frame = scenario === "typed-browser"
      ? "C:\\crm\\apps\\typed-browser.mts:173:12"
      : "file:///crm/apps/published-smoke.mts:173:12";
    error.stack = `AssertionError: ${marker}\n    at expectPage (node_modules/playwright/test.js:12:3)\n    at scenario (${frame})`;
    error.matcherResult = { actual: marker };

    const capture = captureBrowserRpcEvidence();
    const record = (status, headers) => capture.recordResponse({
      url: () => `https://example.test/crm.test.v1.TestService/${marker}?secret=${marker}#${marker}`,
      status: () => status, headers: () => headers,
    });
    const verified = {"x-crm-runtime": "native-workers-rs", "x-crm-source-sha": sourceSha};
    record(403, {...verified, "x-request-id": requestId, "x-crm-error-code": "permission_denied"});
    record(500, {...verified, "x-request-id": marker, "x-crm-error-code": marker});
    record(503, {...verified, "x-request-id": marker, "x-crm-error-code": "constructor"});
    record(404, {...verified, "x-crm-source-sha": marker, "x-request-id": requestId});
    assert.equal(capture.requests[1].error_code, marker);

    const safe = reportBrowserFailure(error, {
      scenario, directory, sourceSha, phase: "viewer-denial",
      steps: ["signed-access", "account-create"], requests: capture.requests,
    });
    assert.equal(safe.name, "AssertionError");
    assert.equal(safe.message, `${scenario} failed during viewer-denial after checkpoint 2`);
    assert.ok(safe.stack.endsWith(`at ${scenario}.mts:173:12`));
    assert.doesNotMatch(safe.stack, /private-crm-form-sentinel|node_modules|matcherResult/);
    assert.equal(safe.cause, undefined);
    const serialized = readFileSync(file, "utf8");
    assert.doesNotMatch(serialized, /private-crm-form-sentinel|constructor/);
    assert.deepEqual(readdirSync(directory), ["failure.json"]);
    assert.deepEqual(JSON.parse(serialized), {
      result: "FAIL", source_sha: sourceSha, scenario, phase: "viewer-denial", error: "AssertionError",
      expected: "Deny the viewer write and leave D1 unchanged",
      checkpoint: 2, last_completed: "account-create", location: `${scenario}.mts:173:12`,
      recent_requests: [
        {status: 403, request_id: requestId, connect_code: "permission_denied"},
        {status: 500, request_id: null, connect_code: null},
        {status: 503, request_id: null, connect_code: null},
        {status: 404, request_id: null, connect_code: null},
      ],
    });
  });
}

test("browser cleanup preserves the first failure and still fails on a sole cleanup error", async t => {
  const log = t.mock.method(console, "error", () => {});
  const primary = new Error("private-primary");
  const cleanup = new Error("private-restore");
  const failCleanup = async () => {throw cleanup;};
  await assert.rejects(withBrowserCleanup(async () => {throw primary;}, failCleanup), error => error === primary);
  assert.deepEqual(log.mock.calls.map(call => call.arguments),
    [["Browser cleanup failed after an earlier failure"]]);
  await assert.rejects(withBrowserCleanup(async () => undefined, failCleanup), error => error === cleanup);
});

test("failure evidence I/O cannot replace or expose the browser error", t => {
  const log = t.mock.method(console, "error", () => {});
  const directory = mkdtempSync(join(tmpdir(), "crm-browser-failure-"));
  t.after(() => rmdirSync(directory));
  const error = new Error("private text");
  error.name = "private label";
  const safe = reportBrowserFailure(error, {
    scenario: "live", phase: "signed-readback", sourceSha, directory: join(directory, "missing"),
  });
  assert.equal(safe.name, "UnknownError");
  assert.equal(safe.message, "live failed during signed-readback after checkpoint 0");
  assert.doesNotMatch(safe.stack, /private/);
  assert.deepEqual(log.mock.calls.map(call => call.arguments),
    [["Browser failure evidence could not be saved"]]);
});

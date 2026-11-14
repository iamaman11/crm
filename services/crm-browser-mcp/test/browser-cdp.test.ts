import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { executeCdpCommands, validateCdpCommands } from "../src/browser-cdp.ts";

test("CDP commands use one connection in the declared order and return real results", async () => {
  const calls: string[] = [];
  const result = await executeCdpCommands([
    { method: "Runtime.evaluate", params: { expression: "2 + 2", returnByValue: true } },
    { method: "DOM.getDocument", params: { depth: 1 } },
  ], async (method, params) => {
    calls.push(method);
    assert.ok(params);
    return method === "Runtime.evaluate" ? { result: { value: 4 } } : { root: { nodeId: 1 } };
  });
  assert.equal(result.passed, true);
  assert.equal(result.total, 2);
  assert.equal(result.executed, 2);
  assert.deepEqual(calls, ["Runtime.evaluate", "DOM.getDocument"]);
  assert.deepEqual(result.results[0]?.result, { result: { value: 4 } });
  assert.deepEqual(result.results[1]?.result, { root: { nodeId: 1 } });
});

test("CDP preflight rejects a malformed late method BEFORE earlier mutations", async () => {
  let calls = 0;
  await assert.rejects(executeCdpCommands([
    { method: "Input.dispatchMouseEvent", params: { type: "mousePressed", x: 3, y: 7 } },
    { method: "Bad Method", params: {} },
  ], async () => { calls++; return {}; }), /BROWSER_CDP_INVALID_COMMAND/);
  assert.equal(calls, 0);
});

test("CDP reports a failed browser response precisely once, never replays a click", async () => {
  const calls: string[] = [];
  const report = await executeCdpCommands([
    { method: "Input.dispatchMouseEvent", params: { type: "mousePressed", x: 1, y: 2 } },
    { method: "Network.enable" },
    { method: "Page.navigate", params: { url: "https://example.org" } },
  ], async method => {
    calls.push(method);
    if (method === "Network.enable") throw new Error("CDP target closed");
    return { ok: true };
  }, 90_000, input => String(input));
  assert.equal(report.passed, false);
  assert.equal(report.executed, 2);
  assert.deepEqual(calls, ["Input.dispatchMouseEvent", "Network.enable"]);
  assert.equal(report.results[0]?.passed, true);
  assert.equal(report.results[1]?.passed, false);
  assert.equal(report.results[1]?.outcome, "uncertain");
  assert.match(report.results[1]?.error ?? "", /CDP target closed/);
});

test("bounded CDP output never returns an enormous raw DOM payload", async () => {
  const report = await executeCdpCommands([
    { method: "Runtime.evaluate", params: { expression: "document.documentElement.outerHTML" } },
  ], async () => ({ result: "x".repeat(300_000) }));
  assert.equal(report.passed, false);
  assert.equal(report.results[0]?.error, "BROWSER_CDP_RESULT_TOO_LARGE");
  assert.equal(report.results[0]?.outcome, "uncertain");
});

test("a late successful CDP acknowledgement cannot count as PASS", async () => {
  const values = [0, 10, 1100, 1200];
  const time = () => values.shift() ?? 1200;
  const report = await executeCdpCommands([
    { method: "Page.reload" },
    { method: "DOM.getDocument" },
  ], async () => ({ ok: true }), 1000, input => String(input), time);
  assert.equal(report.passed, false);
  assert.equal(report.executed, 1);
  assert.equal(report.results[0]?.outcome, "uncertain");
  assert.match(report.results[0]?.error ?? "", /BROWSER_CDP_DEADLINE_EXCEEDED/);
});

test("CDP validates count, JSON-serializable payload and total input budget", () => {
  for (const invalid of [
    undefined, [], Array.from({ length: 129 }, () => ({ method: "Page.reload" })),
    [{ method: "No spaces allowed" }], [{ method: "Page.reload", params: [] }],
    [{ method: "Page.reload", params: null }],
    [{ method: "Runtime.evaluate", params: { expression: "x".repeat(300_000) } }],
  ]) {
    assert.throws(() => validateCdpCommands(invalid as never));
  }
});

test("published MCP and DO share the SAME Playwright Page with a single CDPSession", () => {
  const server = readFileSync(new URL("../src/index.ts", import.meta.url), "utf8");
  const session = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  assert.match(server, /registerTool\("browser_execute"/);
  assert.match(server, /op: "execute", workspaceId, cdpCommands: commands/);
  assert.match(session, /case "execute":/);
  assert.match(session, /const cdp = await page\.context\(\)\.newCDPSession\(page\)/);
  assert.match(session, /executeCdpCommands\(/);
  assert.match(session, /await cdp\.detach\(\)/);
  assert.doesNotMatch(session, /import.*agents\/browser/);
});

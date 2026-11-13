import test from "node:test";
import assert from "node:assert/strict";
import { SCENARIO_OPERATIONS, validateScenarioSteps, boundedRunTimeout, operationTimeout, isFailedVerdict, boundedWaitTimeout, compareBrowserAssertion, safeAssertionValue } from "../src/browser-checks.ts";

test("wait timeout is bounded and has a single stable default", () => {
  assert.equal(boundedWaitTimeout(undefined), 10_000);
  for (const ms of [100, 12_000, 30_000]) assert.equal(boundedWaitTimeout(ms), ms);
  for (const ms of [-1, 0, 99, 30_001, Infinity, NaN, 1.5]) {
    assert.throws(() => boundedWaitTimeout(ms), /OUT_OF_RANGE/);
  }
});

test("assertions are typed and return machine-readable pass/fail without side effects", () => {
  assert.deepEqual(compareBrowserAssertion({kind:"visible",actual:true,expected:true}), {
    passed:true, kind:"visible", match:"exact", actual:true, expected:true,
  });
  assert.equal(compareBrowserAssertion({kind:"text",actual:"CRM ready",expected:"CRM",match:"contains"}).passed,true);
  assert.equal(compareBrowserAssertion({kind:"count",actual:4,expected:3}).passed,false);
  assert.equal(compareBrowserAssertion({kind:"url",actual:"/clients",expected:"/deals"}).passed,false);
  assert.throws(() => compareBrowserAssertion({kind:"count",actual:1,expected:"1"}), /TYPE_MISMATCH/);
  assert.throws(() => compareBrowserAssertion({kind:"count",actual:1,expected:1,match:"contains"}), /REQUIRES_TEXT/);
});

test("test reports omit obvious URL credential values", () => {
  assert.equal(safeAssertionValue("/a?token=abc&code=secret"), "/a?token=[REDACTED]&code=[REDACTED]");
  assert.equal(safeAssertionValue(true), true);
  assert.equal(safeAssertionValue(9), 9);
});

test("scenario preflight refuses any forbidden or malformed step before first browser mutation", () => {
  assert.ok(SCENARIO_OPERATIONS.includes("assert"));
  assert.doesNotThrow(() => validateScenarioSteps([
    {op:"click",target:{kind:"role",value:"button",name:"Submit"}},
    {op:"assert",kind:"title",expected:"done"},
  ]));
  for (const invalid of [
    [{op:"click",target:{}},{op:"close"}],
    [{op:"click",target:{}},{op:"run"}],
    [{op:"fill"}],
    [{op:"assert",kind:"text",expected:"x"}],
    [{op:"assert",kind:"title"}],
    [{op:"navigate"}],
    [{op:"press"}],
    [{op:"status"}],
  ]) assert.throws(() => validateScenarioSteps(invalid), /BROWSER_RUN_/);
  assert.throws(() => validateScenarioSteps([]), /OUT_OF_RANGE/);
});
test("run budget clamps operation timeouts without starting concurrent actions", () => {
  assert.equal(boundedRunTimeout(undefined),90_000);
  assert.equal(boundedRunTimeout(1_000),1_000);
  assert.equal(boundedRunTimeout(120_000),120_000);
  for (const bad of [0,999,120_001,NaN,Infinity,1000.5]) assert.throws(
    () => boundedRunTimeout(bad), /OUT_OF_RANGE/,
  );
  assert.equal(operationTimeout(12_000,15_000,10_000),5_000);
  assert.equal(operationTimeout(12_000,15_000,14_999),1);
  assert.throws(()=>operationTimeout(12_000,15_000,15_000),/DEADLINE_EXCEEDED/);
});
test("negative verdicts are MCP errors, not a successful tool response", () => {
  assert.equal(isFailedVerdict({passed:false,steps:[]}),true);
  assert.equal(isFailedVerdict({passed:true}),false);
  assert.equal(isFailedVerdict({completed:true}),false);
  assert.equal(isFailedVerdict(null),false);
});

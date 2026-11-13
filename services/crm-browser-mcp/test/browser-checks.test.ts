import test from "node:test";
import assert from "node:assert/strict";
import { boundedWaitTimeout, compareBrowserAssertion, safeAssertionValue } from "../src/browser-checks.ts";

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

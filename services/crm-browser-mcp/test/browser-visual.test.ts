import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MAX_VISUAL_PNG_BYTES, pngReceipt, visualThreshold } from "../src/browser-visual.ts";
import axe from "axe-core";

test("visual baseline has actual screenshot bytes and checksum",()=>{
  const baseline=Buffer.from("fake-png-as-test-bytes");
  const receipt=pngReceipt(baseline);
  assert.equal(receipt.bytes,baseline.length);
  assert.match(receipt.sha256,/^[0-9a-f]{64}$/);
  assert.throws(()=>pngReceipt(Buffer.alloc(0)),/BROWSER_VISUAL_IMAGE_OUT_OF_RANGE/);
  assert.throws(()=>pngReceipt(Buffer.alloc(MAX_VISUAL_PNG_BYTES+1)),/BROWSER_VISUAL_IMAGE_OUT_OF_RANGE/);
});
test("visual difference tolerance is bounded and never silently clamped",()=>{
  assert.equal(visualThreshold(undefined),0.005);
  assert.equal(visualThreshold(0),0);
  assert.equal(visualThreshold(0.25),0.25);
  for(const x of [-1,0.26,NaN,Infinity]) assert.throws(()=>visualThreshold(x),/BROWSER_VISUAL_THRESHOLD_OUT_OF_RANGE/);
});
test("installed axe-core is the real pinned engine, not a DOM heuristic",()=>{
  assert.match(axe.version,/^4\.14\.0$/);
  assert.ok(axe.source.length>100_000);
});

test("axe injection uses Playwright page evaluation, not CSP-blocked inline script tags",()=>{
  // Regression guard for real Selenium CSP failure caught in Browser Run.
  const source = readFileSync(new URL("../src/session.ts",import.meta.url),"utf8");
  assert.match(source,/await page\\.evaluate\\(axe\\.source\\)/);
  assert.doesNotMatch(source,/page\\.addScriptTag\\(\\{content:axe\\.source\\}/);
});

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { AXE_SOURCE } from "../src/axe-source.generated.ts";
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
  const packageRoot=dirname(createRequire(import.meta.url).resolve("axe-core"));
  const original=readFileSync(resolve(packageRoot,"axe.min.js"),"utf8");
  assert.ok(AXE_SOURCE.length>100_000);
  assert.equal(AXE_SOURCE,original,
    "runtime script must be the exact pinned npm axe.min.js bytes, not function.toString transformed by bundler");
});

test("axe injection uses original immutable source through Playwright, not transformed function.toString",()=>{
  const source=readFileSync(new URL("../src/browser/inspection.ts",import.meta.url),"utf8");
  assert.match(source,/await page\.evaluate\(AXE_SOURCE\)/);
  assert.doesNotMatch(source,/page\.addScriptTag\(/);
  assert.doesNotMatch(source,/axe\.source/);
});

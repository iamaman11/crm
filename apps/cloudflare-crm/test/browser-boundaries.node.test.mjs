import assert from "node:assert/strict";
import test from "node:test";
import { safeBrowserFailure } from "../rust-party/test/browser-boundaries.mts";

test("browser failure keeps only scenario, safe error name, and allowlisted source frame", () => {
  const marker = "private-crm-form-sentinel";
  const error = new Error(`Playwright matcher output ${marker}`);
  error.name = "AssertionError";
  error.stack = `AssertionError: Playwright matcher output ${marker}\n` +
    `  matcherResult: actual page text ${marker}\n` +
    "    at expectPage (node_modules/playwright/test.js:12:3)\n" +
    "    at typedBrowser (C:\\crm\\apps\\cloudflare-crm\\rust-party\\test\\typed-browser.mts:3359:7)";
  error.cause = new Error(`nested cause ${marker}`);

  const safe = safeBrowserFailure(error, "typed-browser");

  assert.equal(safe.name, "AssertionError");
  assert.equal(safe.message, "typed-browser failed");
  assert.match(safe.stack, /typed-browser\.mts:3359:7/);
  assert.doesNotMatch(safe.stack, /private-crm-form-sentinel|matcherResult|node_modules/);
  assert.equal(safe.cause, undefined);
});

test("unknown errors cannot inject names or raw text into published-smoke diagnostics", () => {
  const marker = "private-published-form-sentinel";
  const error = new Error(`raw matcher output ${marker}`);
  error.name = `AssertionError ${marker}`;
  error.stack = `AssertionError ${marker}\n` +
    `  matcherResult: ${marker}\n` +
    "    at publishedSmoke (file:///crm/apps/cloudflare-crm/e2e/published-smoke.mts:240:9)";

  const safe = safeBrowserFailure(error, "published-smoke");

  assert.equal(safe.name, "UnknownError");
  assert.equal(safe.message, "published-smoke failed");
  assert.match(safe.stack, /published-smoke\.mts:240:9/);
  assert.doesNotMatch(safe.stack, /private-published-form-sentinel|matcherResult/);
  assert.equal(safe.cause, undefined);
});

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { validateScenarioSteps } from "../src/browser-checks.ts";

test("a click that follows a popup is valid inside an ordinary E2E run", () => {
  assert.doesNotThrow(() => validateScenarioSteps([
    { op: "click", target: { kind: "role", value: "link", name: "Click Here" }, followPopup: true },
    { op: "assert", kind: "title", expected: "New Window" },
  ]));
});

test("scenario preflight rejects popup follow on non-click before earlier side effects", () => {
  assert.throws(() => validateScenarioSteps([
    { op: "fill", target: { kind: "css", value: "#username" }, text: "must-not-execute" },
    { op: "hover", target: { kind: "css", value: "#link" }, followPopup: true },
  ]), /BROWSER_RUN_INVALID_POPUP_FOLLOW/);
  assert.throws(() => validateScenarioSteps([
    { op: "click", target: { kind: "css", value: "#link" }, followPopup: false },
  ]), /BROWSER_RUN_INVALID_POPUP_FOLLOW/);
});

test("scenario preflight rejects conflicting iframe addressing", () => {
  assert.throws(() => validateScenarioSteps([
    { op: "click", target: { kind: "css", value: "button" } },
    { op: "fill", target: {
      kind: "css", value: "body", frameIndex: 1, frameSelector: "iframe#editor",
    }},
  ]), /BROWSER_RUN_FRAME_TARGET_CONFLICT/);
  assert.doesNotThrow(() => validateScenarioSteps([
    { op: "fill", target: { kind: "css", value: "body", frameSelector: "iframe#editor" }},
  ]));
});

test("MCP exposes stable iframe selectors and opt-in popup following in existing tools", () => {
  const code = readFileSync(new URL("../src/tools.ts", import.meta.url), "utf8");
  const session = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  const interactions = readFileSync(new URL("../src/browser/interactions.ts", import.meta.url), "utf8");
  const inspection = readFileSync(new URL("../src/browser/inspection.ts", import.meta.url), "utf8");
  assert.match(code, /const frameSelector = z\.string\(\)\.min\(1\)\.max\(512\)\.optional\(\)/);
  assert.match(code, /frameIndex, frameSelector/);
  assert.match(code, /followPopup: z\.boolean\(\)\.optional\(\)/);
  assert.match(code, /followPopup: params\.followPopup/);
  assert.match(interactions, /page\.frameLocator\(target\.frameSelector\)/);
  assert.match(inspection, /page\.frameLocator\(command\.frameSelector\)/);
  assert.match(interactions, /page\.waitForEvent\("popup"/);
  assert.match(interactions, /await Promise\.all\(\[/);
  assert.match(session, /this\.lifecycle\.page = popup;/);
  assert.match(session, /await this\.rememberSelectedPageIdentity\(\)/);
  assert.match(interactions, /followedPopup: true, tabId: deps\.tabId\(followedPopup\)/);
  assert.doesNotMatch(session, /puppeteer\.connect|agents\/browser/);
});

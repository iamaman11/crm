import test from "node:test";
import assert from "node:assert/strict";
import { completedHandoff, handoffIdentifier, handoffLiveViewUrl,
  isProviderHandoffActive, validateHandoffRequest, type HumanHandoffRecord,
} from "../src/browser-handoff.ts";

const fixture: HumanHandoffRecord = {
  sessionId: "original-live-browser", handoffId: "handoff-123",
  status: "waiting_human", startedAt: 1_000, deadline: 600_000,
};

test("human handoff validates all options before acquiring CDP or pausing automation", () => {
  assert.deepEqual(validateHandoffRequest("Please sign in to the account",undefined), {
    instructions: "Please sign in to the account", timeoutMs: 600_000,
  });
  for (const instructions of ["", "a", "    ", "x".repeat(1_001), null]) {
    assert.throws(() => validateHandoffRequest(instructions, 90_000),
      /BROWSER_HANDOFF_INSTRUCTIONS_INVALID/);
  }
  for (const timeout of [0, 29_999, 1_800_001, 123.4, "60000", null]) {
    assert.throws(() => validateHandoffRequest("Complete login", timeout),
      /BROWSER_HANDOFF_TIMEOUT_INVALID/);
  }
  assert.equal(validateHandoffRequest("Complete login",1_800_000).timeoutMs,1_800_000);
});

test("native Cloudflare handoff must expose a bounded opaque ID, never fabricated success", () => {
  assert.equal(handoffIdentifier({handoffId:"provider-handoff_123"}),"provider-handoff_123");
  for (const value of [{},{handoffId:""}, {handoffId:24}, {handoffId:"../../../x"},
    {handoffId:"f".repeat(129)}, null]) {
    assert.throws(() => handoffIdentifier(value), /BROWSER_HANDOFF_PROVIDER_INVALID/);
  }
  assert.equal(isProviderHandoffActive({active:true}),true);
  assert.equal(isProviderHandoffActive({active:false}),false);
  assert.equal(isProviderHandoffActive({active:"false"}),null);
  assert.equal(isProviderHandoffActive({}),null);
});

test("signed Live View links must be HTTPS Cloudflare-hosted, never a third-party URL", () => {
  const signed = "https://live.browser.run/ui/inspector?jwt=confidential";
  assert.equal(handoffLiveViewUrl({devtoolsFrontendUrl:signed}),signed);
  for (const value of [{devtoolsFrontendUrl:"https://evil.example/?jwt=secret"},
    {devtoolsFrontendUrl:"http://live.browser.run/ui?jwt=x"},
    {devtoolsFrontendUrl:"https://name:pass@live.browser.run/"},
    {devtoolsFrontendUrl:"javascript:alert(1)"},
    {devtoolsFrontendUrl:123},
    null]) {
    assert.throws(() => handoffLiveViewUrl(value),
      /BROWSER_HANDOFF_LIVE_VIEW_INVALID/);
  }
});

test("Done, Failed, timeout and uncertain remain separate; no replay of human actions", () => {
  assert.equal(completedHandoff(fixture,{success:true},20_000).status,"completed");
  assert.equal(completedHandoff(fixture,{success:false,reason:"human cancelled"},20_000).status,"failed");
  assert.equal(completedHandoff(fixture,{success:false,reason:"timeout"},20_000).status,"timed_out");
  assert.equal(completedHandoff(fixture,{success:false},800_000).status,"timed_out");
  assert.equal(completedHandoff(fixture,{},20_000).status,"unconfirmed");
  assert.equal(completedHandoff(fixture,null,20_000).status,"unconfirmed");
  assert.equal(fixture.status,"waiting_human","pure classification never mutates persisted input");
});

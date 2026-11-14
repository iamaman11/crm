import test from "node:test";
import assert from "node:assert/strict";
import { classifyBrowserAcquireFailure, recoverExistingBrowserSession } from "../src/browser-recovery.ts";

const id = "provider-session-123";
test("no stored ID means closed, with no provider call or new browser", async () => {
  let calls = 0;
  const result = await recoverExistingBrowserSession<string>(undefined,
    async () => { calls++; throw new Error("should not lookup"); },
    async () => { calls++; throw new Error("should not connect"); });
  assert.deepEqual(result, { kind: "missing" });
  assert.equal(calls, 0);
});

test("definitive terminal provider sessions are not reconnected", async () => {
  for (const state of [null, {status:"closed"},{status:"closing"},
    {status:"EXPIRED"},{status:"CLOSED"}]) {
    let connectCalls = 0;
    const result = await recoverExistingBrowserSession(id,
      async key => { assert.equal(key,id); return state; },
      async () => { connectCalls++; throw new Error("never connect terminal session"); });
    assert.deepEqual(result,{kind:"expired"});
    assert.equal(connectCalls,0);
  }
});

test("a live session reattaches to the exact original provider ID", async () => {
  const calls: string[] = [];
  const browser = {id:"original-provider-browser"};
  const result = await recoverExistingBrowserSession(id,
    async key => { calls.push("lookup:"+key); return {status:"active"}; },
    async key => { calls.push("connect:"+key); return browser; });
  assert.deepEqual(result,{kind:"connected",sessionId:id,browser});
  assert.deepEqual(calls,["lookup:"+id,"connect:"+id]);
});

test("provider status outage is not mistaken for closure or a fresh acquire", async () => {
  let connectCalls=0;
  await assert.rejects(recoverExistingBrowserSession(id,
    async () => { throw new Error("provider unavailable"); },
    async () => { connectCalls++; return "unexpected"; }), /provider unavailable/);
  assert.equal(connectCalls,0);
});

test("failed reconnect does not silently replace or forget a still-live session", async () => {
  let attempts=0;
  await assert.rejects(recoverExistingBrowserSession(id,
    async () => ({status:"active"}),
    async () => { attempts++; throw new Error("sensitive connection error"); }),
    err => err instanceof Error && err.message==="BROWSER_SESSION_RECONNECT_FAILED");
  assert.equal(attempts,1);
});

test("unrecognized provider status uses reconnect rather than deleting a recoverable session", async () => {
  const x=await recoverExistingBrowserSession(id,async()=>({status:"RUNNING"}),async()=>42);
  assert.deepEqual(x,{kind:"connected",sessionId:id,browser:42});
});


test("new browser rate limits return a stable code without guessing plan or repeating acquire", () => {
  const throttled = new Error("Unable to create new browser: code: 429: message: Rate limit exceeded");
  assert.equal(classifyBrowserAcquireFailure(throttled).message,"BROWSER_PROVIDER_RATE_LIMITED");
  assert.equal(classifyBrowserAcquireFailure("HTTP 429 Too many requests").message,"BROWSER_PROVIDER_RATE_LIMITED");
  assert.equal(classifyBrowserAcquireFailure(
    new Error("Unable to create new browser: code: 429: message: Browser time limit exceeded for today"),
  ).message, "BROWSER_PROVIDER_DAILY_LIMIT_EXCEEDED");
});

test("unknown browser creation errors remain fail-closed and redact provider details", () => {
  for (const error of [new Error("secret=please-do-not-log, 401"),
    "something bad",null,undefined]) {
    assert.equal(classifyBrowserAcquireFailure(error).message,
      "BROWSER_PROVIDER_ACQUIRE_FAILED");
  }
  assert.notEqual(classifyBrowserAcquireFailure(new Error("429 quota")).message,
    "BROWSER_PROVIDER_RATE_LIMITED",
    "a bare status code is insufficient to prove a rate limit cause");
});

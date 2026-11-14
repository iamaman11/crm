import test from "node:test";
import assert from "node:assert/strict";
import { classifyBrowserAcquireFailure, readSafeBrowserLimits, recoverExistingBrowserSession } from "../src/browser-recovery.ts";

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

test("provider limits can be inspected without acquiring a browser or exposing session IDs", async () => {
  let reads=0;
  const result=await readSafeBrowserLimits(async () => {
    reads++;
    return {
      activeSessions: [{id:"SENSITIVE_BROWSER_SESSION_A"}, {id:"SENSITIVE_BROWSER_SESSION_B"}],
      allowedBrowserAcquisitions: 0, maxConcurrentSessions: 3,
      timeUntilNextAllowedBrowserAcquisition: 19,
    };
  });
  assert.equal(reads,1);
  assert.deepEqual(result,{
    state:"available", activeSessionCount:2, allowedBrowserAcquisitions:0,
    maxConcurrentSessions:3, timeUntilNextAllowedBrowserAcquisition:19,
  });
  assert.equal(JSON.stringify(result).includes("SENSITIVE_BROWSER_SESSION"),false);
});

test("provider limits read errors are unavailable, never falsely reported as zero", async () => {
  const result=await readSafeBrowserLimits(async () => {
    throw new Error("Authorization: Bearer SECRET Browser Run 429");
  });
  assert.deepEqual(result,{state:"unavailable"});
});

test("malformed provider limits do not become trusted account capacity", async () => {
  for (const bad of [
    {activeSessions:[],allowedBrowserAcquisitions:-1,maxConcurrentSessions:3,timeUntilNextAllowedBrowserAcquisition:0},
    {activeSessions:[],allowedBrowserAcquisitions:0,maxConcurrentSessions:2,timeUntilNextAllowedBrowserAcquisition:Infinity},
    {activeSessions:null,allowedBrowserAcquisitions:0,maxConcurrentSessions:2,timeUntilNextAllowedBrowserAcquisition:0},
  ]) {
    const result=await readSafeBrowserLimits(async()=>bad as never);
    assert.deepEqual(result,{state:"unavailable"});
  }
});

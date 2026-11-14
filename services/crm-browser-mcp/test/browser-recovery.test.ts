import test from "node:test";
import assert from "node:assert/strict";
import { classifyBrowserAcquireFailure, readSafeBrowserHistory, readSafeBrowserLimits, recoverExistingBrowserSession, selectRestoredPublicPage } from "../src/browser-recovery.ts";

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


test("history exposes UTC usage clues without exposing Browser Run session IDs", async () => {
  const day=Date.UTC(2026,10,14,0,0,0);
  const records=[
    {sessionId:"SECRET-CLOSED-A",startTime:day+10_000,endTime:day+70_000,
      closeReason:1,closeReasonText:"NormalClosure"},
    {sessionId:"SECRET-CLOSED-B",startTime:day-20_000,endTime:day+20_000,
      closeReason:2,closeReasonText:"BrowserIdle"},
  ];
  const result=await readSafeBrowserHistory(async()=>records,day+80_000);
  assert.deepEqual(result,{
    state:"available",recordsReturned:2,recordsExamined:2,
    closedTodayUtc:2,observedDurationMsTodayUtc:80_000,
    normalClosuresTodayUtc:1,idleClosuresTodayUtc:1,partial:false,
  });
  assert.equal(JSON.stringify(result).includes("SECRET"),false);
});

test("history provider outage or malformed result remains unavailable, never a fake zero", async () => {
  assert.deepEqual(await readSafeBrowserHistory(async()=>{
    throw new Error("Authorization: Bearer SENSITIVE sessionId=ABC");
  }),{state:"unavailable"});
  assert.deepEqual(await readSafeBrowserHistory(async()=>null as never),
    {state:"unavailable"});
});

test("invalid rows and data outside the current UTC day are not counted as current use", async () => {
  const day=Date.UTC(2026,10,14,0,0,0);
  const rows=[
    {sessionId:"old",startTime:day-80_000,endTime:day-20_000,
      closeReason:1,closeReasonText:"NormalClosure"},
    {sessionId:"invalid",startTime:day+5_000,endTime:day+1_000,
      closeReason:2,closeReasonText:"BrowserIdle"},
  ];
  const result=await readSafeBrowserHistory(async()=>rows,day+100_000);
  assert.equal(result.state,"available");
  assert.equal(result.closedTodayUtc,0);
  assert.equal(result.observedDurationMsTodayUtc,0);
});


test("provider session reconnect without an original public page fails closed", () => {
  const page = (url: string, closed = false) => ({
    isClosed: () => closed,
    url: () => url,
  });
  const contexts = [
    { id: "default", pages: [page("about:blank")] },
    { id: "disconnected", pages: [page("chrome://newtab"), page("http://localhost/")] },
    { id: "empty", pages: [] as ReturnType<typeof page>[] },
  ];
  const selected = selectRestoredPublicPage(
    contexts, c => c.pages, url => url.startsWith("https://"),
  );
  assert.equal(selected, null,
    "active Chrome alone must not be reported as an open browser workspace");
  assert.deepEqual(contexts.map(c => c.pages.map(p => p.url())), [
    ["about:blank"], ["chrome://newtab", "http://localhost/"], [],
  ], "must not silently create a substitute page");
});

test("recovery selects only a live public tab and preserves its exact context", () => {
  const closed = {isClosed:()=>true, url:()=> "https://closed.example.org"};
  const blocked = {isClosed:()=>false, url:()=> "http://127.0.0.1/"};
  const publicPage = {isClosed:()=>false, url:()=> "https://www.selenium.dev/selenium/web/web-form.html"};
  const contexts = [
    {id:"unsafe",pages:[closed,blocked]},
    {id:"original",pages:[publicPage]},
  ];
  const recovered = selectRestoredPublicPage(
    contexts, c => c.pages, url => !url.includes("127.0.0.1"),
  );
  assert.equal(recovered?.context,contexts[1]);
  assert.equal(recovered?.page,publicPage);
  assert.equal(recovered?.page.url(),publicPage.url());
});

test("detached pages and invalid URL classifiers fail closed", () => {
  const contexts = [{pages:[
    {isClosed:()=>false,url:()=>{throw new Error("page detached");}},
    {isClosed:()=>false,url:()=> "https://example.org"},
  ]}];
  const result = selectRestoredPublicPage(
    contexts,c=>c.pages,()=>{throw new Error("network policy unavailable");},
  );
  assert.equal(result,null);
});

test("production restore refuses to recreate a tab after lost page and still permits explicit Close", async () => {
  const { readFileSync } = await import("node:fs");
  const code = readFileSync(new URL("../src/session.ts",import.meta.url),"utf8");
  assert.match(code, /if \(this\.recoveredPageMissing\) throw new Error\("BROWSER_SESSION_PAGE_NOT_RESTORED"\)/);
  assert.match(code, /if \(!restored\) \{\s*this\.recoveredPageMissing = true;\s*throw new Error\("BROWSER_SESSION_PAGE_NOT_RESTORED"\)/);
  assert.match(code, /private async open\([\s\S]*?await this\.restore\(\);[\s\S]*?this\.browser\.newContext/);
  assert.match(code, /private async close\([\s\S]*?this\.recoveredPageMissing = false/);
});

import test from "node:test";
import assert from "node:assert/strict";
import { classifyBrowserAcquireFailure, readSafeBrowserHistory, readSafeBrowserLimits, recoverExistingBrowserSession, selectRestoredPublicPage, parsePageTargetInfo, validBrowserPageIdentity, isOriginalPageMissing, restoreForExplicitOpen } from "../src/browser-recovery.ts";

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


const original = { targetId: "target-original", browserContextId: "incognito-original" };

test("official CDP Target.getTargetInfo must prove a page in an incognito context", () => {
  assert.deepEqual(parsePageTargetInfo({
    targetInfo: { type: "page", ...original },
  }), original);
  for (const bad of [
    null, {}, {targetInfo: null}, {targetInfo:{type:"service_worker",...original}},
    {targetInfo:{type:"page",targetId:"target-original"}},
    {targetInfo:{type:"page",browserContextId:"incognito-original"}},
    {targetInfo:{type:"page",targetId:"",browserContextId:"incognito-original"}},
    {targetInfo:{type:"page",targetId:"target-original",browserContextId:""}},
    {targetInfo:{type:"page",targetId:123,browserContextId:"incognito-original"}},
  ]) {
    assert.equal(parsePageTargetInfo(bad),null);
  }
  assert.equal(validBrowserPageIdentity({targetId:"a"}),null);
  assert.equal(validBrowserPageIdentity({targetId:"a",browserContextId:""}),null);
});

test("provider reconnect without the original live public page fails closed", async () => {
  const page = (url: string, closed = false) => ({
    isClosed: () => closed, url: () => url, identity: original,
  });
  const contexts = [
    {id:"default",pages:[page("about:blank")]},
    {id:"disconnected",pages:[page("chrome://newtab"),page("http://localhost/")]},
    {id:"empty",pages:[] as ReturnType<typeof page>[]},
  ];
  let reads=0;
  const restored=await selectRestoredPublicPage(
    contexts,c=>c.pages,url=>url.startsWith("https://"),
    original,async p=>{ reads++;return p.identity; },
  );
  assert.equal(restored,null);
  assert.equal(reads,0,"never inspect CDP identity of blocked/non-HTTP targets");
  assert.deepEqual(contexts.map(c=>c.pages.map(p=>p.url())),[
    ["about:blank"],["chrome://newtab","http://localhost/"],[],
  ],"never create a replacement browser tab");
});

test("recovery selects ONLY the original target in the original incognito context", async () => {
  const publicURL="https://www.selenium.dev/selenium/web/web-form.html";
  const page = (id: string, ctx: string) => ({
    isClosed:()=>false,url:()=>publicURL,
    identity:{targetId:id,browserContextId:ctx},
  });
  const contexts=[
    {id:"default",pages:[page("target-original","another-context")]},
    {id:"different-target",pages:[page("different-target","incognito-original")]},
    {id:"original",pages:[page("target-original","incognito-original")]},
  ];
  const selected=await selectRestoredPublicPage(
    contexts,c=>c.pages,()=>true,original,async p=>p.identity,
  );
  assert.equal(selected?.context,contexts[2]);
  assert.equal(selected?.page,contexts[2].pages[0]);
});

test("identical URL cannot impersonate the original lost tab or browser context", async () => {
  const url="https://example.org/same-url";
  const contexts=[
    {pages:[{
      isClosed:()=>false,url:()=>url,
      identity:{targetId:"different-tab",browserContextId:"incognito-original"},
    }]},
    {pages:[{
      isClosed:()=>false,url:()=>url,
      identity:{targetId:"target-original",browserContextId:"different-context"},
    }]},
  ];
  const selected=await selectRestoredPublicPage(
    contexts,c=>c.pages,()=>true,original,async p=>p.identity,
  );
  assert.equal(selected,null);
});

test("legacy/missing/invalid stored identity cannot silently adopt a public tab", async () => {
  const page={isClosed:()=>false,url:()=> "https://example.org",identity:original};
  for (const bad of [undefined,null,{}, {targetId:"target-original"}]) {
    let cdpReads=0;
    const selected=await selectRestoredPublicPage(
      [{pages:[page]}],c=>c.pages,()=>true,bad,
      async p=>{cdpReads++;return p.identity;},
    );
    assert.equal(selected,null);
    assert.equal(cdpReads,0,"no page inspection without a durable identity");
  }
});

test("closed, detached, blocked or uninspectable page cannot be recovered", async () => {
  const contexts=[{pages:[
    {isClosed:()=>true,url:()=>"https://example.org",identity:original},
    {isClosed:()=>false,url:()=>{throw Error("detached");},identity:original},
    {isClosed:()=>false,url:()=>"https://example.org",identity:original},
  ]}];
  assert.equal(await selectRestoredPublicPage(
    contexts,c=>c.pages,()=>{throw Error("policy unavailable");},original,
    async p=>p.identity,
  ),null);
  assert.equal(await selectRestoredPublicPage(
    contexts,c=>c.pages,()=>true,original,
    async()=>{throw Error("CDP unavailable");},
  ),null);
});

test("real DO saves CDP target+incognito identity before reporting Open and tab changes", async () => {
  const {readFileSync}=await import("node:fs");
  const code=readFileSync(new URL("../src/session.ts",import.meta.url),"utf8");
  assert.match(code,/newCDPSession\(page\)/,"read via official Playwright CDP session");
  assert.match(code,/cdp\.send\("Target\.getTargetInfo"\)/);
  assert.match(code,/this\.ctx\.storage\.put\("browser_page_identity", await this\.readPageIdentity\(page\)\)/);
  assert.match(code,/const storedPageIdentity: unknown = await this\.ctx\.storage\.get\("browser_page_identity"\)/);
  assert.match(code,/storedPageIdentity, page => this\.readPageIdentity\(page\)/);
  assert.match(code,/await this\.rememberSelectedPageIdentity\(\);\s*return \{ state: "open"/);
  assert.match(code,/if \(command\.tabAction !== "list"\) await this\.rememberSelectedPageIdentity\(\)/);
  assert.match(code,/if \(this\.recoveredPageMissing\) throw new Error\("BROWSER_SESSION_PAGE_NOT_RESTORED"\)/);
  assert.match(code,/if \(!restored\) \{\s*this\.recoveredPageMissing = true;\s*throw new Error\("BROWSER_SESSION_PAGE_NOT_RESTORED"\)/);
  assert.match(code,/private async close\([\s\S]*?this\.ctx\.storage\.delete\("browser_page_identity"\)/);
  assert.match(code,/private async close\([\s\S]*?this\.recoveredPageMissing = false/);
});

test("explicit Open works normally when the original browser is healthy", async () => {
  const calls: string[] = [];
  const restarted = await restoreForExplicitOpen(
    async () => { calls.push("restore"); },
    async () => { calls.push("close"); },
  );
  assert.equal(restarted, false);
  assert.deepEqual(calls, ["restore"]);
});

test("explicit Open closes the lost browser once before a new acquire", async () => {
  const calls: string[] = [];
  const restarted = await restoreForExplicitOpen(
    async () => { calls.push("restore"); throw new Error("BROWSER_SESSION_PAGE_NOT_RESTORED"); },
    async () => { calls.push("provider-close"); },
  );
  assert.equal(restarted, true);
  assert.deepEqual(calls, ["restore", "provider-close"]);
  assert.equal(isOriginalPageMissing(new Error("BROWSER_SESSION_PAGE_NOT_RESTORED")), true);
});

test("a failed provider Close forbids creating a replacement browser", async () => {
  const calls: string[] = [];
  await assert.rejects(
    restoreForExplicitOpen(
      async () => { calls.push("restore"); throw new Error("BROWSER_SESSION_PAGE_NOT_RESTORED"); },
      async () => { calls.push("provider-close"); throw new Error("CLOSE_FAILED"); },
    ), /CLOSE_FAILED/,
  );
  assert.deepEqual(calls, ["restore","provider-close"]);
});

test("cloud outages and other failures must never trigger implicit Close or acquire", async () => {
  for (const msg of ["BROWSER_SESSION_RECONNECT_FAILED","BROWSER_PROVIDER_RATE_LIMITED","unexpected failure"]) {
    let closes = 0;
    await assert.rejects(
      restoreForExplicitOpen(
        async () => { throw new Error(msg); },
        async () => { closes++; },
      ), new RegExp(msg),
    );
    assert.equal(closes, 0);
  }
});

test("real DO exposes recoverable Status and one-step explicit Open", async () => {
  const { readFileSync } = await import("node:fs");
  const code = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  assert.match(code, /const restarted = await restoreForExplicitOpen\(/);
  assert.match(code, /\(\) => this\.restore\(\), \(\) => this\.close\(\)/);
  assert.match(code, /state: needsReopen \? "needs_reopen"/);
  assert.match(code, /if \(command\.op !== "status" \|\| !isOriginalPageMissing\(error\)\) throw error/);
  assert.match(code, /\(restarted \? \{ restarted: true \} : \{\}\)/);
});

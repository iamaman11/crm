import test from "node:test";
import assert from "node:assert/strict";
import {verifyAnonymousDenial,verifyTracePrivacySettings,classifySignedSessionReadback,
  isRetryableReadbackTransportError,waitForExactSourceSha} from "./release-readback.mjs";

const endpoint="https://example.test/api/version";

const expectedSha="a".repeat(40);
const versionRun=(sequence,overrides={})=>{
  const calls=[],events=[],delays=[];
  const result=waitForExactSourceSha(endpoint,expectedSha,{
    request:async(url,options)=>{
      calls.push({url,options});
      const item=sequence[calls.length-1];
      if(item instanceof Error)throw item;
      if(item instanceof Response)return item;
      const status=item?.status??200;
      const body=typeof item==="string"?{source:item}:{source:item?.source};
      return new Response(JSON.stringify(body),{status,headers:{"cf-ray":"test-ray"}});
    },
    pause:async ms=>{delays.push(ms);},
    log:event=>events.push(event),
    maxAttempts:10,intervalMs:5,timeoutMs:100,deadlineMs:1000,
    ...overrides
  });
  return {result,calls,events,delays};
};

test("exact source readback retries a reset and requires three consecutive signed GETs",async()=>{
  const x=versionRun([reset(),expectedSha,expectedSha,expectedSha]);
  await x.result;
  assert.equal(x.calls.length,4);
  assert.deepEqual(x.delays,[5,5,5]);
  for(const call of x.calls){
    assert.equal(call.url,endpoint);
    assert.equal(call.options.method,"GET");
    assert.equal(call.options.redirect,"manual");
    assert.equal(call.options.cache,"no-store");
    assert.ok(call.options.signal instanceof AbortSignal);
  }
  assert.deepEqual(x.events.map(e=>e.result),["RETRY","PASS"]);
});
test("exact source readback resets its stability count after stale and edge responses",async()=>{
  const x=versionRun([
    "b".repeat(40),expectedSha,
    new Response(null,{status:404,headers:{"cf-ray":"edge-not-ready"}}),
    expectedSha,expectedSha,expectedSha
  ]);
  await x.result;
  assert.equal(x.calls.length,6);
  assert.equal(x.events.at(-1).result,"PASS");
  assert.equal(x.events.at(-1).attempts,6);
});
test("exact source readback retries transient server failures but never accepts them",async()=>{
  const x=versionRun([
    new Response(null,{status:503}),
    expectedSha,expectedSha,expectedSha
  ]);
  await x.result;
  assert.equal(x.calls.length,4);
});
test("exact source readback fails immediately on invalid Access authorization",async()=>{
  const x=versionRun([new Response(null,{status:403}),expectedSha,expectedSha,expectedSha]);
  await assert.rejects(x.result,/CRM_E2E_VERSION_ACCESS_HTTP_403/);
  assert.equal(x.calls.length,1);
});
test("exact source readback never retries an invalid request or accepts a stale SHA",async()=>{
  const x=versionRun([new TypeError("invalid URL"),expectedSha,expectedSha,expectedSha]);
  await assert.rejects(x.result,/invalid URL/);
  assert.equal(x.calls.length,1);
});
test("exact source readback bounds persistent transport failure",async()=>{
  const x=versionRun([reset(),reset(),reset()],{maxAttempts:3});
  await assert.rejects(x.result,/transport=ECONNRESET/);
  assert.equal(x.calls.length,3);
  assert.deepEqual(x.delays,[5,5]);
});
test("readback transport classifier recognizes only known transient failures",()=>{
  assert.equal(isRetryableReadbackTransportError(reset()),true);
  for(const name of ["AbortError","TimeoutError"])
    assert.equal(isRetryableReadbackTransportError(Object.assign(new Error(name),{name})),true);
  assert.equal(isRetryableReadbackTransportError(new TypeError("invalid URL")),false);
  assert.equal(isRetryableReadbackTransportError(new Error("application failure")),false);
});
const reset=()=>Object.assign(new TypeError("fetch failed"),{
  cause:Object.assign(new Error("read ECONNRESET"),{code:"ECONNRESET"})
});
const run=async sequence=>{
  const calls=[],events=[],delays=[];
  const result=verifyAnonymousDenial(endpoint,{
    request:async(url,options)=>{
      calls.push({url,options});
      const entry=sequence[calls.length-1];
      if(entry instanceof Error)throw entry;
      return new Response(null,{status:entry,headers:{"cf-ray":"test-ray"}});
    },
    pause:async ms=>{delays.push(ms);},
    log:event=>events.push(event)
  });
  return {result,calls,events,delays};
};

test("anonymous GET retries a reset and requires actual denial",async()=>{
  const x=await run([reset(),403]);
  await x.result;
  assert.equal(x.calls.length,2);
  assert.deepEqual(x.delays,[1000]);
  for(const call of x.calls){
    assert.equal(call.url,endpoint);
    assert.equal(call.options.method,"GET");
    assert.equal(call.options.redirect,"manual");
    assert.equal(call.options.headers.connection,"close");
  }
  assert.deepEqual(x.events.map(e=>e.result),["RETRY","PASS"]);
});
test("explicit Access redirect passes",async()=>{
  const x=await run([302]);
  await x.result;
  assert.equal(x.calls.length,1);
});
test("anonymous success fails immediately without retry",async()=>{
  const x=await run([200,403]);
  await assert.rejects(x.result,/HTTP 200/);
  assert.equal(x.calls.length,1);
  assert.equal(x.events.length,0);
});
test("transient edge failure is retried but requires actual denial",async()=>{
  for(const status of [404,429,502,503,504,524]){
    const x=await run([status,403]);
    await x.result;
    assert.equal(x.calls.length,2);
    assert.deepEqual(x.delays,[1000]);
    assert.deepEqual(x.events.map(e=>e.result),["RETRY","PASS"]);
  }
});
test("delayed edge propagation still requires a positive Access denial",async()=>{
  const x=await run([...Array(8).fill(404),403]);
  await x.result;
  assert.equal(x.calls.length,9);
  assert.equal(x.delays.length,8);
  assert.equal(x.events.at(-1).result,"PASS");
});
test("persistent edge failure never passes the bounded readback",async()=>{
  for(const status of [404,429,502,503,504,524]){
    const x=await run([...Array(40).fill(status),403]);
    await assert.rejects(x.result,new RegExp("HTTP "+status));
    assert.equal(x.calls.length,40);
    assert.equal(x.delays.length,39);
    assert.deepEqual(x.delays.slice(0,4),[1000,2000,3000,3500]);
    assert.ok(x.events.every(e=>e.result==="RETRY"));
  }
});
test("transport retry is bounded and does not turn failure into denial",async()=>{
  const x=await run([reset(),reset(),reset(),403]);
  await assert.rejects(x.result,/fetch failed/);
  assert.equal(x.calls.length,3);
  assert.deepEqual(x.delays,[1000,2000]);
  assert.ok(x.events.every(e=>e.result==="RETRY"));
});
test("unexpected error fails without replay",async()=>{
  const x=await run([new TypeError("invalid URL"),403]);
  await assert.rejects(x.result,/invalid URL/);
  assert.equal(x.calls.length,1);
});

test("signed E2E session accepts only the exact deployed source and admin identity",()=>{
  const expectedSha="3f28905bf3a157762ae35ded7bf2dde6043c2bc2";
  const session={authenticated:true,role:"admin"};
  assert.deepEqual(classifySignedSessionReadback({
    status:200,source:expectedSha,session,expectedSha
  }),{kind:"pass"});
  assert.deepEqual(classifySignedSessionReadback({
    status:404,source:null,session:null,expectedSha
  }),{kind:"retry",reason:"edge_not_ready"});
});
test("signed E2E session retries an authenticated response from an older Worker version",()=>{
  const expectedSha="3f28905bf3a157762ae35ded7bf2dde6043c2bc2";
  assert.deepEqual(classifySignedSessionReadback({
    status:200,source:"ffc3f1a1477d40c50cc415d6701afc2fe4e29725",
    session:{authenticated:true,role:"admin"},expectedSha
  }),{kind:"retry",reason:"stale_worker_version"});
});
test("signed E2E session never retries invalid authorization or missing source provenance",()=>{
  const expectedSha="3f28905bf3a157762ae35ded7bf2dde6043c2bc2";
  for(const session of [
    {authenticated:false,role:"admin"},
    {authenticated:true,role:"viewer"},
    {authenticated:true,role:""}
  ])assert.throws(()=>classifySignedSessionReadback({
    status:200,source:"ffc3f1a1477d40c50cc415d6701afc2fe4e29725",session,expectedSha
  }),/CRM_SIGNED_E2E_ACCESS_SESSION_INVALID/);
  assert.throws(()=>classifySignedSessionReadback({
    status:200,source:null,session:{authenticated:true,role:"admin"},expectedSha
  }),/CRM_SIGNED_E2E_ACCESS_SOURCE_INVALID/);
  assert.throws(()=>classifySignedSessionReadback({
    status:200,source:"not-a-sha",session:{authenticated:true,role:"admin"},expectedSha
  }),/CRM_SIGNED_E2E_ACCESS_SOURCE_INVALID/);
  assert.throws(()=>classifySignedSessionReadback({
    status:404,source:expectedSha,session:null,expectedSha
  }),/CRM_SIGNED_E2E_ACCESS_HTTP_404/);
  for(const status of [401,403,500])
    assert.throws(()=>classifySignedSessionReadback({
      status,source:null,session:null,expectedSha
    }),new RegExp("CRM_SIGNED_E2E_ACCESS_HTTP_"+status));
});
test("Cloudflare tracing requires enabled logs, traces and query redaction",()=>{
  const safe={enabled:true,redact_query_string:true,
    logs:{enabled:true,head_sampling_rate:1,invocation_logs:false},
    traces:{enabled:true,head_sampling_rate:1}};
  assert.deepEqual(verifyTracePrivacySettings(safe),{
    traces_enabled:true,query_string_redacted:true,logs_enabled:true,
    invocation_logs_enabled:false,trace_sampling_rate:1});
  for(const unsafe of [
    null,{}, {...safe,enabled:false}, {...safe,redact_query_string:false},
    {...safe,redact_query_string:undefined}, {...safe,logs:{enabled:false}},
    {...safe,logs:{enabled:true,head_sampling_rate:1}},
    {...safe,logs:{enabled:true,head_sampling_rate:1,invocation_logs:true}},
    {...safe,traces:{enabled:false,head_sampling_rate:1}},
    {...safe,traces:{enabled:true,head_sampling_rate:0}},
    {...safe,traces:{enabled:true}}
  ])assert.throws(()=>verifyTracePrivacySettings(unsafe),/CRM_TRACE_PRIVACY_SETTINGS_UNSAFE/);
});

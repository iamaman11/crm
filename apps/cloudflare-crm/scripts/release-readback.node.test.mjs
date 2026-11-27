import test from "node:test";
import assert from "node:assert/strict";
import {verifyAnonymousDenial,verifyTracePrivacySettings,classifySignedSessionReadback,
  isRetryableReadbackTransportError,waitForExactSourceSha,waitForNativeTelemetry,
  releaseVersionAffinityKey} from "./release-readback.mts";

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

test("release version affinity key is unique to one GitHub run attempt",()=>{
  assert.equal(releaseVersionAffinityKey("46020391985","1"),"crm-release-46020391985-1");
  assert.notEqual(releaseVersionAffinityKey("46020391985","1"),
    releaseVersionAffinityKey("46020391985","2"));
  assert.throws(()=>releaseVersionAffinityKey("run","1"),/CRM_RELEASE_VERSION_AFFINITY_KEY_INVALID/);
  assert.throws(()=>releaseVersionAffinityKey("46020391985","0"),/CRM_RELEASE_VERSION_AFFINITY_KEY_INVALID/);
});

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
test("exact source readback preserves the release version-affinity key on every retry",async()=>{
  const key=releaseVersionAffinityKey("46020391985","1");
  const x=versionRun([expectedSha,expectedSha,expectedSha],{
    headers:{"Cloudflare-Workers-Version-Key":key}
  });
  await x.result;
  assert.equal(x.calls.length,3);
  for(const call of x.calls)
    assert.equal(call.options.headers["Cloudflare-Workers-Version-Key"],key);
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

const telemetryRequest={request_id:"receipt-1",path:"/crm.example.v1.Owner/Create",status:200};
const traceId="a".repeat(32),otherTraceId="b".repeat(32),liveVersion="version-1";
const requestEvent=(overrides={})=>({
  source:{event:"crm.native.request",request_id:telemetryRequest.request_id,
    operation:telemetryRequest.path,status:200},
  $metadata:{traceId},$workers:{scriptVersion:{id:liveVersion}},...overrides
});
const dbSpan=(id=traceId)=>({source:{traceId:id,db:{system:"sqlite"}},$metadata:{type:"span"}});
function telemetryRun(snapshots,{deadlineMs=120000}={}){
  let clock=0,calls=0;
  const events=[];
  const result=waitForNativeTelemetry({
    requests:new Map([[telemetryRequest.request_id,telemetryRequest]]),
    mutationIds:new Set([telemetryRequest.request_id]),
    probes:[{value:"synthetic-secret"}],liveVersion,deadlineMs,
    readSnapshot:async deadline=>{
      assert.equal(deadline,deadlineMs);
      assert.ok(clock<deadline);
      clock+=400;
      return snapshots(calls++);
    },
    pause:async ms=>{assert.ok(ms<=2000);clock+=ms;},
    now:()=>clock,log:entry=>events.push(entry)
  });
  return {result,events,calls:()=>calls,elapsed:()=>clock};
}
test("late native trace after former 30-round cutoff is accepted within the existing deadline",async()=>{
  const x=telemetryRun(i=>i<35?[]:[requestEvent(),dbSpan()]);
  assert.deepEqual(await x.result,{verifiedD1:1});
  assert.equal(x.calls(),36);
  assert.ok(x.elapsed()>72000&&x.elapsed()<120000);
});
test("native request and D1 span may arrive in either order without another polling phase",async()=>{
  for(const sequence of [[dbSpan(),requestEvent()],[requestEvent(),dbSpan()]]){
    const x=telemetryRun(i=>[sequence[i]]);
    assert.deepEqual(await x.result,{verifiedD1:1});
    assert.equal(x.calls(),2);
  }
});
test("wrong version or mismatched operation/status/error cannot substitute for the exact native request",async()=>{
  const wrong=[
    requestEvent({$workers:{scriptVersion:{id:"old"}}}),
    requestEvent({source:{...requestEvent().source,operation:"/other"}}),
    requestEvent({source:{...requestEvent().source,status:403}}),
    requestEvent({source:{...requestEvent().source,error_code:"wrong"}})
  ];
  const x=telemetryRun(()=>[...wrong,dbSpan()],{deadlineMs:7200});
  await assert.rejects(x.result,/Missing native Cloudflare traces/);
  assert.equal(x.calls(),3);
});
test("unrelated D1 spans cannot satisfy the exact signed mutation",async()=>{
  const x=telemetryRun(()=>[requestEvent(),dbSpan(otherTraceId)],{deadlineMs:7200});
  await assert.rejects(x.result,/missing exact Cloudflare D1 spans/);
  assert.equal(x.calls(),3);
});
test("privacy is enforced on early and late snapshots, not only the final request snapshot",async()=>{
  for(const unsafe of [{source:{header:"synthetic-secret"}},{$metadata:{type:"cf-worker-event"}}]){
    for(const sequence of [[[unsafe],[requestEvent(),dbSpan()]],[[requestEvent()],[dbSpan(),unsafe]]]){
      const x=telemetryRun(i=>sequence[i]);
      await assert.rejects(x.result,/Synthetic value leaked|Unsafe automatic invocation log/);
    }
  }
});

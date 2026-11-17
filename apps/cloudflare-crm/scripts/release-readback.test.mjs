import test from "node:test";
import assert from "node:assert/strict";
import {verifyAnonymousDenial} from "./release-readback.mjs";

const endpoint="https://example.test/api/version";
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
test("persistent edge failure never passes the bounded readback",async()=>{
  for(const status of [404,429,502,503,504,524]){
    const x=await run([status,status,status,403]);
    await assert.rejects(x.result,new RegExp("HTTP "+status));
    assert.equal(x.calls.length,3);
    assert.deepEqual(x.delays,[1000,2000]);
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

import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {descriptorMethods,validateBindings,validateRuntime,targetMethods,MethodOptions_IdempotencyLevel} from "./crm-rpc-descriptors.mjs";
const descriptor=process.env.CRM_PROTO_DESCRIPTOR_FILE;
const source=process.env.CRM_MODULE_COVERAGE_REPORT;
if(!descriptor||!source)throw Error("compiled Buf descriptor and module report required");
const available=descriptorMethods(readFileSync(descriptor));
const actual=JSON.parse(readFileSync(source,"utf8"));
test("canonical manifest signatures match compiled Buf descriptors",()=>{
 assert.ok(validateBindings(actual,available).size>0);
});
const path="/crm.parties.v3.PartyService/GetParty";
function fixture(){
 const report=structuredClone(actual);
 report.runtime_evidence="EXECUTED";
 report.active_rpc_paths=targetMethods(available);
 report.runtime={source_sha:report.source_sha,result:"PASS",features:["native-ci"],
  unauthenticated_denials:[...report.active_rpc_paths],
  viewer_denials:report.active_rpc_paths.filter(p=>available.get(p).idempotency===MethodOptions_IdempotencyLevel.IDEMPOTENT),
  successful_rpc_calls:[...report.active_rpc_paths],rpc:{compiled_policy_check:true,
   registered_methods:[...report.active_rpc_paths],
   policies:report.active_rpc_paths.map(p=>[p,{permission:available.get(p).idempotency===MethodOptions_IdempotencyLevel.IDEMPOTENT?"Write":"Read"}])}};
 return report;
}
function reject(name,mutate,pattern){
 test(name,()=>{
  const report=fixture();mutate(report);
  assert.throws(()=>validateRuntime(report,available,validateBindings(report,available)),pattern);
 });
}
test("synthetic evidence fixture exercises the validator, not the Worker",()=>{
 const report=fixture();
 assert.deepEqual(validateRuntime(report,available,validateBindings(report,available)).registered,targetMethods(available));
});
reject("missing executed evidence fails",r=>{r.runtime_evidence="NOT_EXECUTED";},/executed workerd evidence required/);
reject("stale evidence fails",r=>{r.runtime.source_sha="0".repeat(40);},/stale runtime evidence/);
reject("missing policy fails",r=>{r.runtime.rpc.policies=[];},/permission denial coverage|missing or invalid compiled policy/);
reject("unregistered policy fails",r=>{
 r.runtime.rpc.policies.push(["/crm.missing.v1.Service/Method",{permission:"Read"}]);
},/policy without registered RPC/);
reject("missing successful call fails",r=>{r.runtime.successful_rpc_calls=[];},/missing successful executed call/);
reject("wrong request type fails",r=>{
 r.modules.find(m=>m.module_id==="crm.parties").rpc_bindings.find(b=>b.path===path).request="wrong";
},/request\/response type mismatch/);
reject("missing exact owner binding fails",r=>{
 const m=r.modules.find(m=>m.module_id==="crm.parties");
 m.rpc_bindings=m.rpc_bindings.filter(b=>b.path!==path);
},/absent from canonical module.yaml binding/);
reject("unlinked owner fails",r=>{
 r.modules.find(m=>m.module_id==="crm.parties").linked=false;
},/owner is not linked/);
reject("duplicate registration fails",r=>{r.runtime.rpc.registered_methods.push(path);},/duplicate registered/);
reject("missing method fails even if policy and calls omit it",r=>{
 r.runtime.rpc.registered_methods=r.runtime.rpc.registered_methods.filter(p=>p!==path);
 r.runtime.rpc.policies=r.runtime.rpc.policies.filter(([p])=>p!==path);
 r.runtime.successful_rpc_calls=r.runtime.successful_rpc_calls.filter(p=>p!==path);
},/authentication denial coverage|full contract differs/);

reject("read method cannot acquire a mutation policy",r=>{r.runtime.rpc.policies.find(([p])=>p===path)[1].permission="Write";},/permission denial coverage|policy effect differs from Protobuf/);
reject("missing unauthenticated scenario fails",r=>{r.runtime.unauthenticated_denials.pop();},/authentication denial coverage/);
reject("missing denied mutation scenario fails",r=>{r.runtime.viewer_denials.pop();},/permission denial coverage/);
test("future v4 method must enter the required scope without a registry edit",()=>{
 const extended=new Map(available);
 const future="/crm.parties.v4.PartyService/GetParty";
 extended.set(future,{...available.get(path),file:"crm/parties/v4/party.proto",package:"crm.parties.v4"});
 assert.ok(targetMethods(extended).includes(future));
 const report=fixture();
 assert.throws(()=>validateRuntime(report,extended,validateBindings(report,extended)),/full contract differs/);
});

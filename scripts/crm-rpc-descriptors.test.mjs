import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {descriptorMethods,validateBindings,validateRuntime} from "./crm-rpc-descriptors.mjs";
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
 report.active_rpc_paths=[path];
 report.runtime={source_sha:report.source_sha,result:"PASS",features:["native-ci"],
  successful_rpc_calls:[path],rpc:{compiled_policy_check:true,
   registered_methods:[path],policies:[[path,{permission:"Read",activation:"Published"}]]}};
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
 assert.deepEqual(validateRuntime(report,available,validateBindings(report,available)).registered,[path]);
});
reject("missing executed evidence fails",r=>{r.runtime_evidence="NOT_EXECUTED";},/executed workerd evidence required/);
reject("stale evidence fails",r=>{r.runtime.source_sha="0".repeat(40);},/stale runtime evidence/);
reject("missing policy fails",r=>{r.runtime.rpc.policies=[];},/missing or invalid compiled policy/);
reject("unregistered policy fails",r=>{
 r.runtime.rpc.policies.push(["/crm.missing.v1.Service/Method",{permission:"Read",activation:"Published"}]);
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
reject("incomplete final cutover fails",r=>{r.phase="cutover";},/full cutover contract differs/);

reject("read method cannot acquire a mutation policy",r=>{r.runtime.rpc.policies[0][1].permission="Write";},/policy effect differs from Protobuf/);

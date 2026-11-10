// Real FileDescriptorSet conformance: mutate the CI-derived module evidence.
// A grep match in Rust/proto or an invented test marker cannot pass these cases.
import test from "node:test";
import assert from "node:assert/strict";
import {spawnSync} from "node:child_process";
import {readFileSync,writeFileSync,mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join,resolve,dirname} from "node:path";
import {fileURLToPath} from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const descriptor=process.argv[2];
const source=process.env.CRM_MODULE_COVERAGE_REPORT;
if(!descriptor||!source)throw Error("compiled Buf descriptor and CI module report required");
const accepted=JSON.parse(readFileSync(source,"utf8"));
const active="/crm.parties.v2.PartyService/GetParty";
const original=accepted.modules.find(x=>x.module_id==="crm.parties");
if(!original?.rpc_bindings?.some(x=>x.path===active))
 throw Error("active Party v2 canonical manifest binding missing");
const directory=mkdtempSync(join(tmpdir(),"crm-rpc-conformance-"));
const script=resolve(root,"scripts/crm-rpc-descriptors.mjs");
const run=report=>{
 const file=join(directory,"candidate.json");
 writeFileSync(file,JSON.stringify(report));
 return spawnSync(process.execPath,[script,descriptor],{
   cwd:root,encoding:"utf8",env:{...process.env,CRM_MODULE_COVERAGE_REPORT:file},
 });
};
const mutate=change=>{
 const candidate=structuredClone(accepted);
 const party=candidate.modules.find(x=>x.module_id==="crm.parties");
 const binding=party.rpc_bindings.find(x=>x.path===active);
 change({candidate,party,binding});
 return run(candidate);
};
const reject=(label,change,pattern)=>test(label,()=>{
 const result=mutate(change);
 assert.notEqual(result.status,0,result.stdout);
 assert.match(result.stderr+result.stdout,pattern);
});
test("actual canonical active Party v2 types match compiled descriptor",()=>{
 const result=run(accepted);
 assert.equal(result.status,0,result.stderr);
 assert.match(result.stdout,/crm\.protobuf-descriptor-coverage/);
});
reject("wrong request type fails",({binding})=>{
 binding.request="crm.parties.v2.NotARealRequest";
},/request\/response type mismatch/);
reject("wrong response type fails",({binding})=>{
 binding.response="crm.parties.v2.NotARealResponse";
},/request\/response type mismatch/);
reject("no Rust ownership activation fails",({party})=>{
 party.linked=false;
},/owner is not linked into Worker/);
reject("no canonical RPC capability binding fails",({party})=>{
 party.rpc_bindings=party.rpc_bindings.filter(x=>x.path!==active);
},/absent from canonical module.yaml binding/);
reject("invented extra active RPC fails",({candidate})=>{
 candidate.active_rpc_paths.push("/crm.parties.v2.PartyService/NotAnActiveMethod");
},/compiled active RPCs differ from generated module inventory/);
test.after(()=>rmSync(directory,{recursive:true,force:true}));

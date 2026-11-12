// Protobuf and ownership are parsed by their standard decoders.
// Runtime evidence is produced only by the completed signed workerd suite.
import {readFileSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {resolve,dirname} from "node:path";
import {createRequire} from "node:module";
import {fileURLToPath} from "node:url";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const requireClient=createRequire(resolve(root,"packages/client/package.json"));
const {fromBinary}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=requireClient("@bufbuild/protobuf/wkt");
const fail=message=>{throw Error("CRM_RPC_DESCRIPTOR_VIOLATION: "+message);};
export function descriptorMethods(bytes){
 const set=fromBinary(FileDescriptorSetSchema,bytes);
 const methods=new Map();
 for(const file of set.file)for(const service of file.service)for(const method of service.method){
  const path="/"+file.package+"."+service.name+"/"+method.name;
  if(methods.has(path))fail("duplicate method descriptor "+path);
  methods.set(path,{file:file.name,package:file.package,
   request:method.inputType.replace(/^\./,""),response:method.outputType.replace(/^\./,"")});
 }
 return methods;
}
export function validateBindings(report,available){
 const bound=new Map();
 for(const module of report.modules)for(const binding of module.rpc_bindings){
  if(bound.has(binding.path))fail("duplicate canonical RPC ownership "+binding.path);
  const descriptor=available.get(binding.path);
  if(!descriptor)fail("canonical RPC absent from Buf descriptor "+binding.path);
  if(descriptor.request!==binding.request||descriptor.response!==binding.response)
   fail("request/response type mismatch "+binding.path);
  bound.set(binding.path,{...binding,module_id:module.module_id,linked:module.linked});
 }
 return bound;
}
export function validateRuntime(report,available,bound){
 const evidence=report.runtime;
 if(report.runtime_evidence!=="EXECUTED"||evidence?.result!=="PASS")
  fail("executed workerd evidence required");
 if(evidence.source_sha!==report.source_sha)fail("stale runtime evidence");
 if(JSON.stringify(evidence.features)!==JSON.stringify(["native-ci"]))
  fail("native-ci feature evidence required");
 const rpc=evidence.rpc;
 if(rpc?.compiled_policy_check!==true)fail("compiled policy check failed");
 const registered=new Set(rpc.registered_methods);
 if(!registered.size||registered.size!==rpc.registered_methods.length)
  fail("empty or duplicate registered methods");
 const policies=new Map(rpc.policies);
 if(policies.size!==rpc.policies.length)fail("duplicate compiled policy");
 const success=new Set(evidence.successful_rpc_calls);
 for(const path of registered){
  if(!available.has(path))fail("registered RPC absent from Buf descriptor "+path);
  if(!bound.has(path))fail("registered RPC absent from canonical module.yaml binding "+path);
  if(!bound.get(path).linked)fail("RPC owner is not linked into Worker "+path);
  const rule=policies.get(path);
  if(!rule||!["Read","Write"].includes(rule.permission)||
     !["Published","Candidate"].includes(rule.activation))fail("missing or invalid compiled policy "+path);
  if(!success.has(path))fail("registered RPC missing successful executed call "+path);
 }
 for(const path of policies.keys())if(!registered.has(path))fail("policy without registered RPC "+path);
 for(const path of success)if(!registered.has(path))fail("executed RPC without registration "+path);
 const published=[...policies].filter(([,rule])=>rule.activation==="Published").map(([path])=>path).sort();
 if(JSON.stringify(published)!==JSON.stringify([...report.active_rpc_paths].sort()))
  fail("published methods differ from executed policy");
 if(report.phase==="contract-only"&&published.length!==1)
  fail("contract-only must expose exactly one verified published RPC");
 if(report.phase==="cutover"){
  const expected=[...available].filter(([,d])=>
   /^crm\/[^/]+\/v(?:2|3)\/[^/]+\.proto$/.test(d.file)&&d.package!=="crm.parties.v2")
   .map(([path])=>path);
  if(expected.length!==registered.size||expected.some(path=>!registered.has(path)))
   fail("full cutover contract differs from registered methods");
  if(published.length!==registered.size)fail("cutover retains unpublished candidates");
 }
 return {registered:[...registered].sort(),published};
}
export function run(){
 const descriptor=process.argv[2],reportFile=process.env.CRM_MODULE_COVERAGE_REPORT;
 if(!descriptor||!reportFile)fail("Buf descriptor and module report required");
 const available=descriptorMethods(readFileSync(descriptor));
 const report=JSON.parse(readFileSync(reportFile,"utf8"));
 const sha=execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim();
 if(report.source_sha!==sha)fail("stale module inventory");
 const bound=validateBindings(report,available);
 const required=process.argv.includes("--require-runtime");
 const runtime=required||report.runtime_evidence==="EXECUTED"
   ?validateRuntime(report,available,bound):null;
 console.log(JSON.stringify({event:"crm.protobuf-descriptor-coverage",result:"PASS",
  scope:runtime?"compiled router/policy and successful workerd calls":"manifest/descriptor signatures only",
  runtime_evidence:runtime?"EXECUTED":"NOT_EXECUTED",
  canonical_bindings:bound.size,total_schema_methods:available.size,...runtime}));
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))run();

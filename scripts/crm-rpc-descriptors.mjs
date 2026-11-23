// Protobuf and ownership are parsed by their standard decoders.
// Runtime evidence is produced only by the completed signed workerd suite.
import {readFileSync,writeFileSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {resolve,dirname} from "node:path";
import {createRequire} from "node:module";
import {fileURLToPath} from "node:url";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const requireClient=createRequire(resolve(root,"packages/client/package.json"));
const {fromBinary}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema,MethodOptions_IdempotencyLevel}=requireClient("@bufbuild/protobuf/wkt");
export {MethodOptions_IdempotencyLevel};
const fail=message=>{throw Error("CRM_RPC_DESCRIPTOR_VIOLATION: "+message);};
export function descriptorMethods(bytes){
 const set=fromBinary(FileDescriptorSetSchema,bytes);
 const methods=new Map();
 for(const file of set.file)for(const service of file.service)for(const method of service.method){
  const path="/"+file.package+"."+service.name+"/"+method.name;
  if(methods.has(path))fail("duplicate method descriptor "+path);
  methods.set(path,{file:file.name,package:file.package,
   request:method.inputType.replace(/^\./,""),response:method.outputType.replace(/^\./,""),
   idempotency:method.options?.idempotencyLevel});
 }
 return methods;
}
export function validateBindings(report,available){
 const bound=new Map();
 for(const module of report.modules)for(const binding of module.rpc_bindings){
  if(bound.has(binding.path))fail("duplicate canonical RPC ownership "+binding.path);
  const descriptor=available.get(binding.path);
  if(!descriptor)fail("canonical RPC absent from Buf descriptor "+binding.path);
  if("request" in binding||"response" in binding)
   fail("RPC input/output types must come from Buf descriptors "+binding.path);
  bound.set(binding.path,{...binding,request:descriptor.request,response:descriptor.response,
   module_id:module.module_id,linked:module.linked});
 }
 for(const path of targetMethods(available)){
  const binding=bound.get(path);
  if(!binding)fail("active RPC absent from canonical module.yaml binding "+path);
  if(!binding.linked)fail("RPC owner is not linked into Worker "+path);
 }
 return bound;
}
// Select the Cloudflare generation of contracts from compiled file descriptors.
// Historical v1 and superseded Party v2 remain available to other consumers.
// Later versions automatically enter the required scope; omission cannot pass.
const activeVersions=JSON.parse(readFileSync(resolve(root,"apps/cloudflare-crm/active-proto-versions.json"),"utf8"));
export function targetMethods(available){
 return [...available].filter(([,d])=>{
  const file=/^crm\/([^/]+)\/v([1-9][0-9]*)\/[^/]+\.proto$/.exec(d.file);
  return file&&Number(file[2])>=(activeVersions.domain_minimums[file[1]]??activeVersions.minimum_version);
 })
  .map(([path])=>path).sort();
}
export function validateRuntime(report,available,bound,evidence=report.runtime){
 if(evidence?.result!=="PASS")
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
 const equalPaths=(actual,expected,label)=>{
  if(!Array.isArray(actual)||actual.length!==new Set(actual).size||
     JSON.stringify([...actual].sort())!==JSON.stringify([...expected].sort()))
   fail(label+" differs from compiled methods/policy");
 };
 for(const path of registered){
  if(!available.has(path))fail("registered RPC absent from Buf descriptor "+path);
  if(!bound.has(path))fail("registered RPC absent from canonical module.yaml binding "+path);
  if(!bound.get(path).linked)fail("RPC owner is not linked into Worker "+path);
  const declaredOwner=bound.get(path).module_id;
  const compiledOwner=rpc.method_owners?.[path];
  if(compiledOwner!==declaredOwner)
   fail(path+" registered by "+compiledOwner+", declared by "+declaredOwner);
  const rule=policies.get(path);
  if(!rule||!["Read","AdminRead","Write","SelfWrite"].includes(rule.permission))fail("missing or invalid compiled policy "+path);
  const definition=available.get(path);
  {
   const expected=["Write","SelfWrite"].includes(rule.permission)
    ?MethodOptions_IdempotencyLevel.IDEMPOTENT:MethodOptions_IdempotencyLevel.NO_SIDE_EFFECTS;
   if(definition.idempotency!==expected)fail("compiled policy effect differs from Protobuf "+path);
  }
  if(!success.has(path))fail("registered RPC missing successful executed call "+path);
 }
 for(const path of policies.keys())if(!registered.has(path))fail("policy without registered RPC "+path);
 for(const path of success)if(!registered.has(path))fail("executed RPC without registration "+path);
 const expected=targetMethods(available);
 if(expected.length!==registered.size||expected.some(path=>!registered.has(path)))
  fail("full contract differs from registered methods");
 equalPaths(evidence.unauthenticated_denials,registered,"authentication denial coverage");
 equalPaths(evidence.viewer_denials,[...policies].filter(([,rule])=>
   rule.permission==="Write"||rule.permission==="AdminRead").map(([path])=>path),"permission denial coverage");
 equalPaths(Object.keys(rpc.method_owners),registered,"compiled ownership");
 return {registered:[...registered].sort()};
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
 let runtime=null;
 if(required){
  const evidence=JSON.parse(readFileSync(process.env.CRM_RPC_RUNTIME_PROOF,"utf8"));
  runtime=validateRuntime(report,available,bound,evidence);
  report.runtime=evidence;
  report.runtime_evidence="EXECUTED";
  report.rpc_policy=Object.fromEntries(evidence.rpc.policies);
  report.active_rpc_paths=runtime.registered;
  report.counts.active_rpc_methods=runtime.registered.length;
  for(const module of report.modules){
   module.active_rpc_paths=runtime.registered.filter(path=>bound.get(path).module_id===module.module_id);
  }
  writeFileSync(reportFile,JSON.stringify(report,null,2)+"\n");
 }
 console.log(JSON.stringify({event:"crm.protobuf-descriptor-coverage",result:"PASS",
  scope:runtime?"compiled router/policy and successful workerd calls":"manifest/descriptor signatures only",
  runtime_evidence:runtime?"EXECUTED":"NOT_EXECUTED",
  canonical_bindings:bound.size,total_schema_methods:available.size,...runtime}));
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))run();

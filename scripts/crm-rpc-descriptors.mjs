// Match actual Buf FileDescriptorSet services to Rust trait implementations and
// physical local/published test fixtures. Do not infer active RPCs from .proto text.
import {readFileSync,existsSync,readdirSync} from "node:fs";
import {resolve,dirname} from "node:path";
import {createRequire} from "node:module";
import {fileURLToPath} from "node:url";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const requireClient=createRequire(resolve(root,"packages/client/package.json"));
const {fromBinary}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=requireClient("@bufbuild/protobuf/wkt");
const rd=p=>readFileSync(resolve(root,p),"utf8");
const fail=s=>{throw Error("CRM_RPC_DESCRIPTOR_VIOLATION: "+s);};
const policy=JSON.parse(rd("apps/cloudflare-crm/architecture-policy.json"));
// The active inventory is CI-generated from the exact Rust RpcPolicy table.
const activeReportFile=process.env.CRM_MODULE_COVERAGE_REPORT;
if(!activeReportFile||!existsSync(activeReportFile))
  fail("generated module coverage report required");
const activeReport=JSON.parse(readFileSync(activeReportFile,"utf8"));
if(activeReport.schema_version!==1||activeReport.phase!==policy.phase)
  fail("module report phase/schema mismatch");
const proof={methods:(activeReport.active_rpc_paths||[]).map(path=>({path}))};
if(!Array.isArray(proof.methods)||!proof.methods.length)fail("active Connect RPC proof is missing");
const arg=process.argv[2];
if(!arg||!existsSync(arg))fail("expected compiled Buf FileDescriptorSet path");
const set=fromBinary(FileDescriptorSetSchema,readFileSync(arg));
const available=new Map();
for(const f of set.file){
  const pkg=f.package;
  for(const svc of f.service){
    for(const method of svc.method){
      const name="/"+pkg+"."+svc.name+"/"+method.name;
      if(available.has(name))fail("duplicate method descriptor "+name);
      available.set(name,{file:f.name,service:svc.name,method:method.name,
        inputType:method.inputType,outputType:method.outputType});
    }
  }
}
const snake=name=>name.replace(/([a-z0-9])([A-Z])/g,"$1_$2").toLowerCase();
// Publication evidence comes from executed fixtures, not a checked-in
// duplicate RPC-to-test catalog.
const publishedNative=rd(policy.phase==="contract-only"?
  "apps/cloudflare-crm/rust-party/test/contract-only-native.mjs":
  "apps/cloudflare-crm/rust-party/test/native.mjs");
const publishedBrowser=rd(policy.phase==="contract-only"?
  "apps/cloudflare-crm/e2e/contract-only.mjs":
  "apps/cloudflare-crm/e2e/live.mjs");
const publishedPaths=new Set();
for(const x of proof.methods){
 const route=x.path;
 if(!available.has(route))fail("published RPC absent from Buf descriptor "+route);
 if(publishedPaths.has(route))fail("duplicate published RPC "+route);
 publishedPaths.add(route);
 if(!publishedNative.includes(route)||!publishedBrowser.includes(route))
   fail("published RPC missing native/published browser evidence "+route);
}
const wrangler=JSON.parse(rd("apps/cloudflare-crm/wrangler.jsonc"));
const local=JSON.parse(rd("apps/cloudflare-crm/rust-party/wrangler.jsonc"));
const workerFirst=["/api/*","/crm.*/*"];
if(JSON.stringify(wrangler.assets?.run_worker_first)!==JSON.stringify(workerFirst)||
   JSON.stringify(local.assets?.run_worker_first)!==JSON.stringify(workerFirst))
 fail("RPC not routed through universal Worker-first CRM namespace");
// Discover EVERY implemented native candidate from the existing generated Rust
// trait implementations and compiled Buf service descriptors. No second catalog.
// This runs while unpublished RPCs are still denied by contract-only production.
const connectRoot="apps/cloudflare-crm/rust-party/src/connect_sales";
const candidateFiles=readdirSync(resolve(root,connectRoot),{withFileTypes:true})
  .filter(entry=>entry.isFile()&&entry.name.endsWith(".rs"))
  .map(entry=>connectRoot+"/"+entry.name).sort();
if(!candidateFiles.length)fail("native Connect domain handlers absent");
const candidateRust=[
  rd("apps/cloudflare-crm/rust-party/src/connect_sales.rs"),
  ...candidateFiles.map(rd),
].join("\n\n");
for(const file of candidateFiles){
  if(!/impl\s+\w+::\w+\s+for\s+\w+\s*\{/.test(rd(file)))
    fail("Connect domain handler has no generated trait implementation "+file);
}
const signedNative=rd("apps/cloudflare-crm/rust-party/test/contract-only-native.mjs");
const fixture=rd("scripts/crm-connect-binary-fixture.mjs");
const aliases=new Map([...candidateRust.matchAll(
  /use\s+proto::crm::([a-z0-9_]+)::(v[0-9]+)\s+as\s+(\w+)\s*;/g
)].map(x=>[x[3],"crm."+x[1]+"."+x[2]]));
const candidatePaths=new Set();
for(const impl of candidateRust.matchAll(
  /impl\s+(\w+)::(\w+)\s+for\s+(\w+)\s*\{/g
)){
  const [,alias,service,rustStruct]=impl;
  const pkg=aliases.get(alias);
  if(!pkg||pkg.endsWith(".v1"))continue; // published legacy v1 is not a cutover candidate
  const qualified=pkg+"."+service;
  const methods=[...available].filter(([path])=>path.startsWith("/"+qualified+"/"));
  if(!methods.length)fail("candidate Rust service absent from Buf descriptor "+qualified);
  if(!candidateRust.includes(".add_service(Arc::new("+rustStruct))
    fail("candidate service absent from the one Rust Connect router "+qualified);
  if(!fixture.includes('registry.getService("'+qualified+'")'))
    fail("candidate lacks a compiled-descriptor binary test fixture "+qualified);
  const end=candidateRust.indexOf("\n}\n",impl.index);
  if(end<0)fail("candidate Rust trait block is not closed "+qualified);
  const body=candidateRust.slice(impl.index,end+2);
  for(const [path,info] of methods){
    if(candidatePaths.has(path))fail("duplicate candidate implementation "+path);
    candidatePaths.add(path);
    if(!new RegExp("async\\s+fn\\s+"+snake(info.method)+"\\s*\\(").test(body))
      fail("candidate RPC missing in Rust trait block "+path);
    if(!proof.methods.some(m=>m.path===path)&&
       !signedNative.includes('"'+info.method+'"'))
      fail("candidate RPC missing signed binary native E2E "+path);
  }
}
// The Worker owns one explicit, exact-path RPC policy (permission + phase).
// Derive the test/registration comparison from that Rust table, not a second
// handwritten list of service prefixes or another capability manifest.
const runtimeRust=rd("apps/cloudflare-crm/rust-party/src/lib.rs");
if(!runtimeRust.includes('fn rpc_policy(path: &str)') ||
   !runtimeRust.includes('cfg!(feature = "native-ci")'))
  fail("single fail-closed Rust RPC policy is missing");
const policies=[...runtimeRust.matchAll(
  /^\s*"(\/crm\.[^"]+)"\s*=>\s*\((Read|Write),\s*(Published|Candidate)\),?\s*$/gm
)];
const registered=new Map();
for(const match of policies){
  const [,path,permission,activation]=match;
  if(registered.has(path))fail("duplicate Rust RPC policy "+path);
  if(!available.has(path))fail("Rust RPC policy absent from compiled Buf "+path);
  registered.set(path,{permission,activation});
}
if(!registered.size)fail("no exact-path Rust RPC policy found");
for(const path of candidatePaths){
  // The historical published Party v2 implementation is discovered along
  // with candidates, but must remain Published rather than Candidate.
  const activation=proof.methods.some(item=>item.path===path)?"Published":"Candidate";
  if(registered.get(path)?.activation!==activation)
    fail("implemented RPC missing exact Rust activation policy "+path);
}
for(const [path,rule] of registered){
  if(rule.activation==="Candidate" && !candidatePaths.has(path))
    fail("candidate Rust policy has no implemented RPC "+path);
  if(rule.activation==="Published" && !proof.methods.some(item=>item.path===path))
    fail("published Rust policy absent from active RPC evidence "+path);
}
for(const item of proof.methods){
  if(registered.get(item.path)?.activation!=="Published")
    fail("active RPC missing published Rust policy "+item.path);
}
const allowed=new Set(proof.methods.map(x=>x.path));
if(policy.phase==="contract-only"){
 if(allowed.size!==1)
   fail("contract-only must expose only one approved RPC");
}
// Compare each active method with its canonical module capability, independent
// of rollout phase. Historical declared methods are NOT automatically published.
const reportFile=process.env.CRM_MODULE_COVERAGE_REPORT;
if(!reportFile||!existsSync(reportFile))fail("generated module coverage report required");
const report=JSON.parse(readFileSync(reportFile,"utf8"));
if(report.schema_version!==1||report.phase!==policy.phase)
  fail("module report phase/schema mismatch");
const runtime=new Set(report.active_rpc_paths||[]);
if(runtime.size!==allowed.size||
  [...runtime].some(method=>!allowed.has(method))||
  [...allowed].some(route=>!runtime.has(route)))
  fail("compiled active RPCs differ from generated module inventory");
const bound=new Map();
for(const module of report.modules){
  for(const binding of module.rpc_bindings||[]){
    const route=binding.path;
    if(bound.has(route))fail("duplicate canonical RPC ownership "+route);
    bound.set(route,{...binding,module_id:module.module_id,linked:module.linked});
  }
}
for(const route of allowed){
 const spec=bound.get(route);
 if(!spec)fail("active RPC absent from canonical module.yaml binding "+route);
 if(!spec.linked)fail("active RPC owner is not linked into Worker "+route);
 const descriptor=available.get(route);
 if(!descriptor)fail("active RPC absent from compiled protobuf descriptor "+route);
 const input=(descriptor.inputType||"").replace(/^\./,"");
 const output=(descriptor.outputType||"").replace(/^\./,"");
 if(input!==spec.request||output!==spec.response)
   fail("active RPC request/response type mismatch "+route+
     " manifest="+spec.request+" -> "+spec.response+
     " descriptor="+input+" -> "+output);
}
// Original v1 module.yaml bindings and Cargo links prove the linked owner.
// Versioned v2/v3 RPCs do not need another manually maintained owner manifest.
const linkedOwnerPackages=new Set();
for(const module of report.modules){
  if(!module.linked)continue;
  for(const binding of module.rpc_bindings||[]){
    const match=/^\/(crm\.[a-z0-9_]+)\.v[0-9]+\.[^/]+\//.exec(binding.path);
    if(match)linkedOwnerPackages.add(match[1]);
  }
}
for(const path of candidatePaths){
  const match=/^\/(crm\.[a-z0-9_]+)\.v[0-9]+\.[^/]+\//.exec(path);
  if(!match||!linkedOwnerPackages.has(match[1]))
    fail("candidate has no linked original Rust owner "+path);
}
if(policy.phase==="cutover" && report.unclaimed_active_rpc_paths?.length)
 fail("unclaimed active RPCs in final cutover");
console.log(JSON.stringify({event:"crm.protobuf-descriptor-coverage",result:"PASS",
 active:proof.methods.map(x=>x.path),candidate_methods:[...candidatePaths].sort(),
  candidate_count:candidatePaths.size,total_schema_methods:available.size,phase:policy.phase}));
export {snake};

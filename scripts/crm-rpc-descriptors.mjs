// Match actual Buf FileDescriptorSet services to Rust trait implementations and
// physical local/published test fixtures. Do not infer active RPCs from .proto text.
import {readFileSync,existsSync} from "node:fs";
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
const proof=JSON.parse(rd("apps/cloudflare-crm/active-connect.json"));
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
      available.set(name,{file:f.name,service:svc.name,method:method.name});
    }
  }
}
const snake=name=>name.replace(/([a-z0-9])([A-Z])/g,"$1_$2").toLowerCase();
const prefixes=new Set();
for(const x of proof.methods){
 const route=x.path;
 if(typeof route!=="string"||!available.has(route))fail("active RPC absent from descriptor "+route);
 if(prefixes.has(route))fail("duplicate active RPC proof "+route);
 prefixes.add(route);
 const info=available.get(route);
 const rust=rd(x.handler);
 const impl=new RegExp("impl\\s+"+x.wire_alias+"::"+info.service+"\\s+for\\s+"+x.rust_struct+"\\b");
 if(!impl.test(rust))fail("Rust generated trait implementation not found for "+route);
 if(!new RegExp("async\\s+fn\\s+"+snake(info.method)+"\\s*\\(").test(rust))
   fail("Rust RPC handler method missing "+route);
 if(!rust.includes("ConnectRouter::new()")||!rust.includes(".add_service(Arc::new("+x.rust_struct))
   fail("RPC service not registered in one embedded router "+route);
 for(const key of ["native_test","published_test"]){
   const source=rd(x[key]);
   if(!source.includes(route))fail(key+" does not request "+route);
   if(!source.includes(x[key+"_marker"]))fail(key+" has no proof marker "+route);
 }
 const wrangler=JSON.parse(rd("apps/cloudflare-crm/wrangler.jsonc"));
 const local=JSON.parse(rd("apps/cloudflare-crm/rust-party/wrangler.jsonc"));
 const prefix=route.slice(0,route.lastIndexOf("/"))+"/*";
 if(!wrangler.assets.run_worker_first.includes(prefix)||
    !local.assets.run_worker_first.includes(prefix))fail("RPC not routed to Worker "+route);
}
const allowed=new Set(proof.methods.map(x=>x.path));
if(policy.phase==="contract-only"){
 const expected=new Set(policy.contract_only_rpc_methods||[]);
 if(expected.size!==allowed.size||[...expected].some(x=>!allowed.has(x)))
   fail("contract-only must expose exactly the approved RPC list");
}
if(policy.phase==="cutover"){
 const matrix=JSON.parse(rd(policy.cutover_coverage_file));
 const runtime=new Set(matrix.owners.flatMap(x=>x.rpc_methods||[]));
 for(const method of runtime)if(!available.has(method))fail("coverage RPC missing descriptor "+method);
 for(const route of allowed)if(!runtime.has(route))fail("active RPC lacks owner coverage "+route);
}
console.log(JSON.stringify({event:"crm.protobuf-descriptor-coverage",result:"PASS",
 active:proof.methods.map(x=>x.path),total_schema_methods:available.size,phase:policy.phase}));
export {snake};

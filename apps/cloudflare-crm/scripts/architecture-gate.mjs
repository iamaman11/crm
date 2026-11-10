// Architecture as code for the ONE Cloudflare CRM. Node built-ins only.
// migration-freeze: no newly released legacy/half-migrated runtime.
// cutover: fail closed until the whole TSX/Connect/one-D1 architecture is ready.
import {readFileSync, readdirSync, existsSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {dirname, resolve, relative, sep} from "node:path";
import {fileURLToPath} from "node:url";
import {implementationOwners} from "../shared/catalog.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const pathInRepo = path => resolve(repo,path);
const read = path => readFileSync(pathInRepo(path), "utf8");
const check = (ok, message) => {if(!ok)throw Error("CRM_ARCHITECTURE_VIOLATION: "+message);};
const git = (...args) => execFileSync("git", args, {cwd:repo,encoding:"utf8"});
const policy = JSON.parse(read("apps/cloudflare-crm/architecture-policy.json"));
check(policy.schema_version===1, "unknown policy schema");
check(["migration-freeze","contract-only","cutover"].includes(policy.phase), "unknown phase");
check(/^[a-f0-9]{40}$/.test(policy.accepted_runtime_sha), "baseline must be an immutable SHA");
check(policy.browser_wire_format==="Connect binary Protobuf", "binary Connect wire choice changed");
check(Array.isArray(policy.allowed_non_rpc_http), "HTTP exceptions must be explicit");
const baseline = policy.accepted_runtime_sha;

function validateCoverageMatrix(matrix,phase){
  check(matrix?.schema_version===1,"coverage matrix schema must be 1");
  check(Array.isArray(matrix.owners),"owner coverage must be an array");
  const declared=Object.keys(implementationOwners);
  const actual=matrix.owners.map(row=>row?.owner);
  check(new Set(actual).size===actual.length,"duplicate owner coverage");
  check(declared.length===actual.length && declared.every(name=>actual.includes(name)),
    "coverage must exactly match original ownership catalog");
  const allRpc=new Set();
  for(const row of matrix.owners){
    check(row && row.domain_module && typeof row.domain_module==="string",
      "coverage must identify original Rust domain "+row?.owner);
    const isInternal=row.owner==="sales-activities-link";
    check(row.transport===(isInternal?"internal":"connect"),
      "only the existing Sales–Activities link is internal: "+row.owner);
    check(["NOT_CONNECTED","PARTIAL","CONNECTED"].includes(row.status),
      "invalid explicit connection status "+row.owner);
    check(Array.isArray(row.required_operations)&&row.required_operations.length>0 &&
      row.required_operations.every(x=>typeof x==="string"&&x.length>0)&&
      new Set(row.required_operations).size===row.required_operations.length,
      "missing/duplicate required operations "+row.owner);
    check(Array.isArray(row.accepted_operations)&&
      row.accepted_operations.every(x=>row.required_operations.includes(x))&&
      new Set(row.accepted_operations).size===row.accepted_operations.length,
      "nonexistent/duplicate accepted operation "+row.owner);
    check(Array.isArray(row.rpc_methods),"missing method list "+row.owner);
    check(isInternal?row.rpc_methods.length===0:row.rpc_methods.every(x=>
      typeof x==="string"&&/^\/crm\.[A-Za-z0-9_.]+\/[A-Za-z0-9_]+$/.test(x)),
      "invalid RPC route "+row.owner);
    for(const rpc of row.rpc_methods){
      check(!allRpc.has(rpc),"duplicate RPC ownership "+rpc);
      allRpc.add(rpc);
    }
    if(row.status==="NOT_CONNECTED")check(row.accepted_operations.length===0 &&
      row.rpc_methods.length===0,"NOT_CONNECTED owner cannot claim RPC "+row.owner);
    if(phase==="cutover"){
      check(row.status==="CONNECTED","cutover cannot ship unconnected owner "+row.owner);
      check(row.required_operations.length===row.accepted_operations.length &&
        row.required_operations.every(x=>row.accepted_operations.includes(x)),
        "cutover lacks owner operation "+row.owner);
      check(isInternal||row.rpc_methods.length>0,
        "public owner must have generated Connect RPC "+row.owner);
    }
  }
  return allRpc;
}
const coveragePath=policy.cutover_coverage_file;
const coverageMatrix=existsSync(pathInRepo(coveragePath))?
  JSON.parse(read(coveragePath)):null;
if(coverageMatrix)validateCoverageMatrix(coverageMatrix,policy.phase);
else check(policy.phase!=="cutover","complete owner coverage matrix required");

git("cat-file","-e",baseline+"^{commit}");

function treeFiles(dir){
  if(!existsSync(pathInRepo(dir)))return [];
  const result=[];
  const visit=(folder)=>{
    for(const item of readdirSync(pathInRepo(folder),{withFileTypes:true})){
      const next=folder+"/"+item.name;
      if(item.isDirectory())visit(next);
      else if(item.isFile())result.push(next);
    }
  };
  visit(dir);
  return result;
}
function baselineFiles(dir){
  return git("ls-tree","-r","--name-only",baseline,"--",dir)
    .split("\n").map(x=>x.trim()).filter(Boolean);
}
function baselineContent(path){
  return git("show",baseline+":"+path);
}
function requireUnchanged(path){
  check(existsSync(pathInRepo(path)), "accepted runtime file removed during freeze: "+path);
  check(read(path)===baselineContent(path), "legacy runtime modified before single cutover: "+path);
}
function unique(paths){return [...new Set(paths)];}
function frozenRuntimeFiles(){
  const app="apps/cloudflare-crm/";
  const web=app+"web/src";
  const rust=app+"rust-party/src";
  const fixed=[
    app+"wrangler.jsonc",
    app+"rust-party/wrangler.jsonc",
    app+"rust-party/build.rs",
    "packages/client/src/cloudflare.ts"
  ];
  const files=unique([
    ...baselineFiles(web),...treeFiles(web),
    ...baselineFiles(rust),...treeFiles(rust),
    ...fixed
  ]).filter(path => path.startsWith(web+"/") || path.endsWith(".rs") ||
    fixed.includes(path));
  return files;
}
function failNewRest(current, old){
  const routes = text => new Set([...text.matchAll(/["'](\/api\/[A-Za-z0-9_.*?/-]+)["']/g)].map(m=>m[1]));
  const accepted=routes(old);
  const added=[...routes(current)].filter(x=>!accepted.has(x));
  check(added.length===0,"new manual REST business path: "+added.join(", "));
}
function validateGeneratedBrowserBoundary(source){
  check(source.includes("createConnectTransport"),"generated Cloudflare client must use Connect");
  check(/useBinaryFormat\s*:\s*true/.test(source),"binary Protobuf must be default");
  check(!/createGrpcWebTransport|ApplicationGatewayService|GovernedCrmClient|fetch\s*\(/.test(source),
    "Cloudflare client must not use legacy gateway or direct fetch");
}
function validateOneWorkerRouter(source,wrangler,local){
  check(wrangler.main==="./rust-party/build/index.js","Worker must be native workers-rs");
  check(wrangler.d1_databases?.length===1 && wrangler.d1_databases[0].binding==="DB",
    "exactly one native D1 binding");
  check(JSON.stringify(wrangler.assets?.run_worker_first)===JSON.stringify(local.assets?.run_worker_first),
    "local and published RPC routing differ");
  check(!/axum::serve|TcpListener/.test(source),"second listening server forbidden");
}
const client=read("packages/client/src/cloudflare.ts");
validateGeneratedBrowserBoundary(client);
const production=JSON.parse(read("apps/cloudflare-crm/wrangler.jsonc"));
const local=JSON.parse(read("apps/cloudflare-crm/rust-party/wrangler.jsonc"));
const runtime=read("apps/cloudflare-crm/rust-party/src/lib.rs");
validateOneWorkerRouter(runtime,production,local);
const rustSources=treeFiles("apps/cloudflare-crm/rust-party/src").filter(x=>x.endsWith(".rs"));
const routerCount=rustSources.reduce((n,p)=>
  n+([...read(p).matchAll(/ConnectRouter::new\s*\(/g)].length),0);
check(routerCount===1,"exactly one embedded Connect router required, found "+routerCount);

if(policy.phase!=="cutover"){
  const prev=new Set(baselineFiles("apps/cloudflare-crm"));
  prev.add("packages/client/src/cloudflare.ts");
  for(const path of frozenRuntimeFiles()){
    check(prev.has(path),"new runtime file before full cutover: "+path);
    if(policy.phase==="contract-only"&&path==="apps/cloudflare-crm/rust-party/src/lib.rs")continue;
    requireUnchanged(path);
  }
  // No new REST endpoints are permitted, even in generated/future features.
  failNewRest(runtime,baselineContent("apps/cloudflare-crm/rust-party/src/lib.rs"));
  if(policy.phase==="contract-only"){
    const html=read("apps/cloudflare-crm/web/index.html");
    check(html.includes('src="/contract-only.tsx"')&&!html.includes("main.jsx"),
      "legacy React entrypoint must not load");
    check(runtime.includes('fn legacy_transport_disabled('),
      "missing fail-closed legacy runtime router");
    check(runtime.includes('error("LEGACY_TRANSPORT_DISABLED", 410)'),
      "legacy business endpoints must return HTTP 410");
    check(JSON.stringify(policy.contract_only_rpc_methods)===
      JSON.stringify(["/crm.parties.v2.PartyService/GetParty"]),
      "only physically proven Party GetParty RPC may remain live");
    check(read("apps/cloudflare-crm/web/contract-only.tsx").includes("CRM: переход на Connect"),
      "read-only transition UI must be visible");
  }
}else{
  const webFiles=treeFiles("apps/cloudflare-crm/web/src");
  check(webFiles.some(x=>x.endsWith(".tsx")),"React must use TSX");
  check(!webFiles.some(x=>/\.(jsx|js|mjs)$/.test(x)),"legacy JS/JSX remains in React");
  for(const path of webFiles.filter(x=>/\.(ts|tsx)$/.test(x))){
    const content=read(path);
    check(!/\bfetch\s*\(|createGrpcWebTransport|ApplicationGatewayService|GovernedCrmClient|["']\/api\//.test(content),
      "manual REST/legacy gateway in typed React: "+path);
  }
  const appPackage=JSON.parse(read("apps/cloudflare-crm/package.json"));
  check(!!appPackage.scripts?.typecheck,"strict CRM typecheck script missing");
  check(!!appPackage.dependencies?.["@ultimate-crm/client"] &&
    !!appPackage.dependencies?.["@ultimate-crm/ui"],
    "must reuse existing typed client and UI packages");
  const tsconfig=JSON.parse(read("apps/cloudflare-crm/tsconfig.json"));
  for(const key of ["strict","noUncheckedIndexedAccess","exactOptionalPropertyTypes"])
    check(tsconfig.compilerOptions?.[key]===true,"missing TS option "+key);
  check(tsconfig.compilerOptions?.allowJs===false,"allowJs must be false");
  check(!read("pnpm-workspace.yaml").includes("!apps/cloudflare-crm"),
    "Cloudflare CRM still excluded from the single pnpm workspace");
  const workflow=read(".github/workflows/cloudflare-crm-spike.yml");
  check(!workflow.includes("npm install --no-audit --no-fund"),
    "old separate npm install still in CRM CI");
  check(workflow.includes("pnpm --filter cloudflare-crm typecheck"),
    "full CRM TS strict typecheck missing in CI");

  const allowed=new Set(policy.allowed_non_rpc_http);
  for(const [literal] of [...runtime.matchAll(/["'](\/api\/[A-Za-z0-9_.*?/-]+)["']/g)].map(x=>[x[1]]))
    check(allowed.has(literal),"legacy REST business endpoint in Worker: "+literal);
  for(const path of rustSources.filter(x=>/connect.*\.rs$/.test(x))){
    const code=read(path);
    check(!/ConnectError::unimplemented|\.encode_to_vec\s*\(|::decode_from_slice\s*\(/.test(code),
      "unimplemented RPC / Buffa-Prost binary bridge remains: "+path);
  }
  const coverage=coverageMatrix;
  check(coverage!==null,"typed RPC coverage matrix missing");
  const declared=Object.keys(implementationOwners);
  check(coverage.owners.length===declared.length,"some original owners not covered");
  check(declared.every(k=>coverage.owners.some(x=>x.owner===k)),
    "coverage must include every original owner, including internal composition");
  const protoFiles=treeFiles("proto/crm").filter(p=>p.endsWith(".proto"));
  const protoText=protoFiles.map(read).join("\n");
  for(const row of coverage.owners){
    check(row.owner && ["connect","internal","http-download"].includes(row.transport),
      "invalid owner coverage entry");
    if(row.transport==="connect"){
      check(Array.isArray(row.rpc_methods) && row.rpc_methods.length,
        "owner must have actual protobuf RPC methods: "+row.owner);
      for(const method of row.rpc_methods){
        const slash=method.lastIndexOf("/");
        check(slash>0,"expected canonical Connect route: "+method);
        const service=method.slice(0,slash).split(".").pop();
        const rpc=method.slice(slash+1);
        check(new RegExp("service\\s+"+service+"\\s*\\{").test(protoText) &&
          new RegExp("rpc\\s+"+rpc+"\\s*\\(").test(protoText),
          "RPC must exist in canonical proto: "+method);
      }
    }
  }
}
console.log(JSON.stringify({event:"crm.architecture-gate",result:"PASS",
  phase:policy.phase,baseline,router_count:routerCount,
  d1:production.d1_databases[0].binding,
  legacy_runtime_policy:policy.phase==="migration-freeze"?"FROZEN":policy.phase==="contract-only"?"DISABLED":"REMOVED"}));
export {failNewRest,validateGeneratedBrowserBoundary,validateOneWorkerRouter,validateCoverageMatrix};

// Static configuration checks only. Runtime acceptance is performed after
// compiled workerd execution; TypeScript owns source/module analysis.
import {existsSync,readFileSync,readdirSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {dirname,resolve} from "node:path";
import {fileURLToPath} from "node:url";
const repo=resolve(dirname(fileURLToPath(import.meta.url)),"../../..");
const read=path=>readFileSync(resolve(repo,path),"utf8");
const check=(ok,message)=>{if(!ok)throw Error("CRM_ARCHITECTURE_VIOLATION: "+message);};
const policy=JSON.parse(read("apps/cloudflare-crm/architecture-policy.json"));
check(policy.schema_version===1,"unknown policy schema");
check(policy.browser_wire_format==="Connect binary Protobuf","binary Connect wire choice changed");
function validateModuleReport(report){
  check(report?.schema_version===1,"generated module report schema must be 1");
  check(/^[a-f0-9]{40}$/.test(report?.source_sha||""),
    "module report must identify one immutable source SHA");
  check(Array.isArray(report.modules)&&report.modules.length>0,
    "canonical module manifests not discovered");
  check(Array.isArray(report.active_rpc_paths),
    "generated active RPC inventory missing");
  check(Array.isArray(report.unclaimed_active_rpc_paths),
    "generated unmatched active RPC inventory missing");
  const names=report.modules.map(m=>m.module_id);
  check(new Set(names).size===names.length,"duplicate canonical module id");
  const paths=report.modules.map(m=>m.manifest);
  check(new Set(paths).size===paths.length,"duplicate canonical module manifest");
  const rpcOwner=new Map();
  for(const m of report.modules){
    check(/^crm\.[a-z0-9.-]+$/.test(m.module_id),
      "invalid canonical module id "+m.module_id);
    check(m.manifest.startsWith("modules/")&&m.manifest.endsWith("/module.yaml"),
      "module must originate in modules/*/module.yaml: "+m.module_id);
    check(["domain","composition"].includes(m.kind),"invalid module kind "+m.module_id);
    check(typeof m.linked==="boolean","missing Worker activation status "+m.module_id);
    check(Array.isArray(m.declared_rpc_paths)&&Array.isArray(m.active_rpc_paths),
      "module RPCs must be compiler-derived "+m.module_id);
    for(const rpc of m.declared_rpc_paths){
      check(!rpcOwner.has(rpc),"duplicate declared RPC owner "+rpc);
      rpcOwner.set(rpc,m.module_id);
    }
    for(const rpc of m.active_rpc_paths){
      check(m.declared_rpc_paths.includes(rpc),
        "active RPC lacks canonical manifest binding "+rpc);
    }
  }
  check(new Set(report.active_rpc_paths).size===report.active_rpc_paths.length,
    "duplicate active RPC path in compiled report");
  const missing=report.active_rpc_paths.filter(rpc=>!rpcOwner.has(rpc)).sort();
  check(JSON.stringify(missing)===JSON.stringify([...report.unclaimed_active_rpc_paths].sort()),
    "generated report omitted unresolved active RPCs");
  if(report.runtime_evidence==="EXECUTED"){
    check(missing.length===0,"active RPC lacks canonical module.yaml binding: "+missing.join(","));
  }
}

// Active CRM integration fixtures use the shared D1 binding, never raw files.
export function validateNativeFixtureSource(source,file){
 check(!/(?:node:sqlite|better-sqlite3|DatabaseSync|\.sqlite["']|BEGIN\s+IMMEDIATE)/i.test(source),
   "direct SQLite access bypasses native D1 binding: "+file);
}
export function validateWorkerConfig(production,local){
 check(production.name==="crm-dev","unexpected Worker");
 check(production.main==="./rust-party/build/index.js","Worker must be native workers-rs");
 check(production.workers_dev===false&&production.preview_urls===false&&!production.routes?.length,
   "release must explicitly enable only the verified workers.dev entry");
 check(production.account_id==="5bd5e1e1e813f061afc259e91334262d","unexpected Cloudflare account");
 check(production.d1_databases?.length===1&&production.d1_databases[0].binding==="DB",
   "exactly one native D1 binding");
 check(production.d1_databases[0].database_id==="35fc7ee9-9710-485f-8fb3-f5a398b229b4","unexpected D1");
 check(local.d1_databases?.length===1&&local.d1_databases[0].binding==="DB","one local D1 required");
 check(local.d1_databases[0].database_id==="00000000-0000-4000-8000-000000000001"&&
   local.vars?.CRM_CI_LOCAL_ONLY==="yes","integration runtime must use isolated test D1");
 check(local.main==="./build/index.js"&&local.assets?.binding===production.assets?.binding,
   "integration runtime must use built Rust Worker and canonical ASSETS binding");
 const routes=["/api/*","/crm.*/*"];
 check(JSON.stringify(production.assets?.run_worker_first)===JSON.stringify(routes),"production Worker-first namespace differs");
 check(JSON.stringify(local.assets?.run_worker_first)===JSON.stringify(routes),"local Worker-first namespace differs");
 check(production.observability?.enabled===true,"Worker logs required");
 check(production.version_metadata?.binding==="CF_VERSION_METADATA","version metadata required");
}
check(!existsSync(resolve(repo,"apps/cloudflare-crm/rust-party/src/pilot.rs")),"retired pilot source in native Worker");
const production=JSON.parse(read("apps/cloudflare-crm/wrangler.jsonc"));
const local=JSON.parse(read("apps/cloudflare-crm/rust-party/wrangler.jsonc"));
validateWorkerConfig(production,local);
{
 const scope="apps/cloudflare-crm/rust-party/test";
 const sources=readdirSync(resolve(repo,scope),{recursive:true})
   .filter(file=>/\.(?:mjs|js|ts|mts)$/.test(file));
 for(const file of sources)validateNativeFixtureSource(read(scope+"/"+file),file);
 const integration=policy.integration_runtime;
 check(integration?.owner==="Wrangler/createTestHarness(workerd)"&&integration.fixture_binding==="DB"&&
   integration.direct_sqlite===false&&integration.production_worker_count===1&&integration.local_d1_count===1,
   "one canonical integration runtime policy required");
 // Real workerd/Chromium CI executes Wrangler createTestHarness and its D1
 // migrations; source spelling is not a substitute for executed proof.
}
const reportPath=process.env.CRM_MODULE_COVERAGE_REPORT;
check(!!reportPath,"canonical manifest/Cargo report required");
const report=JSON.parse(readFileSync(reportPath,"utf8"));
validateModuleReport(report);
check(report.source_sha===execFileSync("git",["rev-parse","HEAD"],{cwd:repo,encoding:"utf8"}).trim(),
 "stale module inventory");
// Fail fast on compiler policy drift before npm/Cargo or Cloudflare work.
// This is an existing machine-readable governance policy, never Markdown.
const rustPolicy=JSON.parse(read("rust-governance-policy.json")).supported_toolchain;
const tomlScalar=(path,section,key)=>{
  let current="",values=[];
  for(const line of read(path).split(/\r?\n/)){
    const heading=/^\[([^\]]+)\]\s*$/.exec(line);
    if(heading){current=heading[1];continue;}
    if(current!==section)continue;
    const pair=/^\s*([a-z-]+)\s*=\s*"([^"]+)"\s*$/.exec(line);
    if(pair?.[1]===key)values.push(pair[2]);
  }
  check(values.length===1,"exact TOML field required: "+path+" ["+section+"] "+key);
  return values[0];
};
const compiler=tomlScalar("rust-toolchain.toml","toolchain","channel");
check(/^\d+\.\d+\.\d+$/.test(rustPolicy?.channel)&&compiler===rustPolicy.channel,
  "Rust toolchain and machine policy disagree");
check(tomlScalar("rust-toolchain.toml","toolchain","profile")===rustPolicy.profile,
  "Rust toolchain profile and policy disagree");
check(tomlScalar("Cargo.toml","workspace.package","edition")===rustPolicy.edition&&
  tomlScalar("Cargo.toml","workspace.package","rust-version")===rustPolicy.minimum_rust_version,
  "Rust workspace and machine policy disagree");
check(tomlScalar("apps/cloudflare-crm/rust-party/Cargo.toml","package","edition")===rustPolicy.edition&&
  tomlScalar("apps/cloudflare-crm/rust-party/Cargo.toml","package","rust-version")===rustPolicy.minimum_rust_version,
  "native Worker compiler boundary differs from workspace policy");
// Normal CI executes Rust tests and both builds; exact-SHA workerd and live
// deployment evidence verify these instead of matching shell text fragments.
const appPackage=JSON.parse(read("apps/cloudflare-crm/package.json"));
check(!!appPackage.dependencies?.["@ultimate-crm/client"]&&!!appPackage.dependencies?.["@ultimate-crm/ui"],
 "existing typed client and UI packages required");
const tsconfig=JSON.parse(read("apps/cloudflare-crm/tsconfig.json"));
for(const key of ["strict","noUncheckedIndexedAccess","exactOptionalPropertyTypes"])
 check(tsconfig.compilerOptions?.[key]===true,"missing TS option "+key);
check(tsconfig.compilerOptions?.allowJs===false,"allowJs must be false");
{
 const sources=readdirSync(resolve(repo,"apps/cloudflare-crm/web/src"),{recursive:true});
 check(sources.some(path=>path.endsWith(".tsx")),"React TSX missing");
 check(!sources.some(path=>/\.(js|jsx|mjs)$/.test(path)),"legacy JS/JSX remains");
}
console.log(JSON.stringify({event:"crm.architecture-gate",result:"PASS",
 scope:"configuration and manifest/Cargo inventory",runtime_evidence:report.runtime_evidence,
 policy_schema:policy.schema_version}));
export {validateModuleReport};

// Static configuration checks only. Runtime acceptance is performed after
// compiled workerd execution; TypeScript owns source/module analysis.
import {existsSync,readFileSync,readdirSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {dirname,resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {record, records, string, strings} from "./json-boundary.mts";
const repo=resolve(dirname(fileURLToPath(import.meta.url)),"../../..");
const read=(path: string)=>readFileSync(resolve(repo,path),"utf8");
function readJson(path: string): unknown {
  return JSON.parse(read(path));
}
function check(ok: unknown,message: string): asserts ok {
  if(!ok)throw Error("CRM_ARCHITECTURE_VIOLATION: "+message);
}
const policy=record(readJson("apps/cloudflare-crm/architecture-policy.json"));
check(policy.schema_version===1,"unknown policy schema");
check(policy.browser_wire_format==="Connect binary Protobuf","binary Connect wire choice changed");
function validateModuleReport(value: unknown){
  const report=record(value);
  check(report.schema_version===1,"generated module report schema must be 1");
  const sourceSha=string(report.source_sha);
  check(/^[a-f0-9]{40}$/.test(sourceSha),
    "module report must identify one immutable source SHA");
  check(Array.isArray(report.modules)&&report.modules.length>0,
    "canonical module manifests not discovered");
  check(Array.isArray(report.active_rpc_paths),
    "generated active RPC inventory missing");
  check(Array.isArray(report.unclaimed_active_rpc_paths),
    "generated unmatched active RPC inventory missing");
  const modules=records(report.modules);
  const activePaths=strings(report.active_rpc_paths);
  const unclaimedPaths=strings(report.unclaimed_active_rpc_paths);
  const names=modules.map(m=>string(m.module_id));
  check(new Set(names).size===names.length,"duplicate canonical module id");
  const paths=modules.map(m=>string(m.manifest));
  check(new Set(paths).size===paths.length,"duplicate canonical module manifest");
  const rpcOwner=new Map<string,string>();
  for(const m of modules){
    const id=string(m.module_id),manifest=string(m.manifest);
    check(/^crm\.[a-z0-9.-]+$/.test(id),
      "invalid canonical module id "+id);
    check(manifest.startsWith("modules/")&&manifest.endsWith("/module.yaml"),
      "module must originate in modules/*/module.yaml: "+id);
    check(m.kind==="domain"||m.kind==="composition","invalid module kind "+id);
    check(typeof m.linked==="boolean","missing Worker activation status "+m.module_id);
    check(Array.isArray(m.declared_rpc_paths)&&Array.isArray(m.active_rpc_paths),
      "module RPCs must be compiler-derived "+m.module_id);
    const declared=strings(m.declared_rpc_paths);
    for(const rpc of declared){
      check(!rpcOwner.has(rpc),"duplicate declared RPC owner "+rpc);
      rpcOwner.set(rpc,id);
    }
    for(const rpc of strings(m.active_rpc_paths)){
      check(declared.includes(rpc),
        "active RPC lacks canonical manifest binding "+rpc);
    }
  }
  check(new Set(activePaths).size===activePaths.length,
    "duplicate active RPC path in compiled report");
  const missing=activePaths.filter(rpc=>!rpcOwner.has(rpc)).sort();
  check(JSON.stringify(missing)===JSON.stringify(unclaimedPaths.sort()),
    "generated report omitted unresolved active RPCs");
  if(report.runtime_evidence==="EXECUTED"){
    check(missing.length===0,"active RPC lacks canonical module.yaml binding: "+missing.join(","));
  }
  return {sourceSha,runtimeEvidence:string(report.runtime_evidence)};
}

// Active CRM integration fixtures use the shared D1 binding, never raw files.
export function validateNativeFixtureSource(source: string,file: string){
 check(!/(?:node:sqlite|better-sqlite3|DatabaseSync|\.sqlite["']|BEGIN\s+IMMEDIATE)/i.test(source),
   "direct SQLite access bypasses native D1 binding: "+file);
}
export function validateWorkerConfig(productionValue: unknown,localValue: unknown){
 const production=record(productionValue),local=record(localValue);
 check(production.name==="crm-dev","unexpected Worker");
 check(production.main==="./rust-party/build/index.js","Worker must be native workers-rs");
 check(production.workers_dev===false&&production.preview_urls===false&&
   (production.routes===undefined||(Array.isArray(production.routes)&&production.routes.length===0)),
   "release must explicitly enable only the verified workers.dev entry");
 check(production.account_id==="5bd5e1e1e813f061afc259e91334262d","unexpected Cloudflare account");
 const prodDatabases=records(production.d1_databases),localDatabases=records(local.d1_databases);
 check(prodDatabases.length===1&&prodDatabases[0]?.binding==="DB",
   "exactly one native D1 binding");
 check(prodDatabases[0].database_id==="35fc7ee9-9710-485f-8fb3-f5a398b229b4","unexpected D1");
 check(localDatabases.length===1&&localDatabases[0]?.binding==="DB","one local D1 required");
 check(localDatabases[0].database_id==="00000000-0000-4000-8000-000000000001"&&
   record(local.vars).CRM_CI_LOCAL_ONLY==="yes","integration runtime must use isolated test D1");
 const productionAssets=record(production.assets), localAssets=record(local.assets);
 check(local.main==="./build/index.js"&&typeof productionAssets.binding==="string"&&localAssets.binding===productionAssets.binding,
   "integration runtime must use built Rust Worker and canonical ASSETS binding");
 const routes=["/api/*","/crm.*/*"];
 check(JSON.stringify(productionAssets.run_worker_first)===JSON.stringify(routes),"production Worker-first namespace differs");
 check(JSON.stringify(localAssets.run_worker_first)===JSON.stringify(routes),"local Worker-first namespace differs");
 check(record(production.observability).enabled===true,"Worker logs required");
 check(record(production.version_metadata).binding==="CF_VERSION_METADATA","version metadata required");
}
check(!existsSync(resolve(repo,"apps/cloudflare-crm/rust-party/src/pilot.rs")),"retired pilot source in native Worker");
const production=readJson("apps/cloudflare-crm/wrangler.jsonc");
const local=readJson("apps/cloudflare-crm/rust-party/wrangler.jsonc");
validateWorkerConfig(production,local);
{
 const scope="apps/cloudflare-crm/rust-party/test";
 const sources=readdirSync(resolve(repo,scope),{recursive:true,encoding:"utf8"})
   .filter(file=>/\.(?:mjs|js|ts|mts)$/.test(file));
 for(const file of sources)validateNativeFixtureSource(read(scope+"/"+file),file);
 const integration=record(policy.integration_runtime);
 check(integration?.owner==="Wrangler/createTestHarness(workerd)"&&integration.fixture_binding==="DB"&&
   integration.direct_sqlite===false&&integration.production_worker_count===1&&integration.local_d1_count===1,
   "one canonical integration runtime policy required");
 // Real workerd/Chromium CI executes Wrangler createTestHarness and its D1
 // migrations; source spelling is not a substitute for executed proof.
}
const reportPath=process.env.CRM_MODULE_COVERAGE_REPORT;
check(!!reportPath,"canonical manifest/Cargo report required");
const report: unknown=JSON.parse(readFileSync(reportPath,"utf8"));
const {sourceSha,runtimeEvidence}=validateModuleReport(report);
check(sourceSha===execFileSync("git",["rev-parse","HEAD"],{cwd:repo,encoding:"utf8"}).trim(),
 "stale module inventory");
// Fail fast on compiler policy drift before npm/Cargo or Cloudflare work.
// This is an existing machine-readable governance policy, never Markdown.
const rustPolicy=record(record(readJson("rust-governance-policy.json")).supported_toolchain);
const tomlScalar=(path: string,section: string,key: string)=>{
  let current: string | undefined="";
  const values: string[]=[];
  for(const line of read(path).split(/\r?\n/)){
    const heading=/^\[([^\]]+)\]\s*$/.exec(line);
    if(heading){current=heading[1];continue;}
    if(current!==section)continue;
    const pair=/^\s*([a-z-]+)\s*=\s*"([^"]+)"\s*$/.exec(line);
    if(pair?.[1]===key&&pair[2]!==undefined)values.push(pair[2]);
  }
  check(values.length===1&&values[0]!==undefined,"exact TOML field required: "+path+" ["+section+"] "+key);
  return values[0];
};
const compiler=tomlScalar("rust-toolchain.toml","toolchain","channel");
check(/^\d+\.\d+\.\d+$/.test(string(rustPolicy.channel))&&compiler===rustPolicy.channel,
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
const dependencies=record(record(readJson("apps/cloudflare-crm/package.json")).dependencies);
check(typeof dependencies["@ultimate-crm/client"]==="string"&&typeof dependencies["@ultimate-crm/ui"]==="string",
 "existing typed client and UI packages required");
console.log(JSON.stringify({event:"crm.architecture-gate",result:"PASS",
 scope:"configuration and manifest/Cargo inventory",runtime_evidence:runtimeEvidence,
 policy_schema:policy.schema_version}));
export {validateModuleReport};

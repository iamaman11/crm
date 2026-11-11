// Architecture as code for the ONE Cloudflare CRM. Node built-ins only.
// migration-freeze: no newly released legacy/half-migrated runtime.
// cutover: fail closed until the whole TSX/Connect/one-D1 architecture is ready.
import {readFileSync, readdirSync, existsSync} from "node:fs";
import {execFileSync} from "node:child_process";
import {dirname, resolve, relative, sep} from "node:path";
import {fileURLToPath} from "node:url";

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

function validateModuleReport(report,phase){
  check(report?.schema_version===1,"generated module report schema must be 1");
  check(report?.phase===phase,"module report must match architecture phase");
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
  if(phase==="cutover"){
    check(missing.length===0,"active RPC lacks canonical module.yaml binding: "+missing.join(","));
    for(const m of report.modules){
      if(m.linked&&m.declared_rpc_paths.length){
        check(m.active_rpc_paths.length>0,
          "linked module has no accepted Connect service: "+m.module_id);
      }
    }
  }
}
const moduleReportPath=process.env.CRM_MODULE_COVERAGE_REPORT;
const moduleReport=moduleReportPath?JSON.parse(readFileSync(moduleReportPath,"utf8")):null;
if(moduleReport){
  validateModuleReport(moduleReport,policy.phase);
  check(moduleReport.source_sha===git("rev-parse","HEAD").trim(),
    "stale module inventory: source SHA mismatch");
}else check(policy.phase!=="cutover","cutover requires generated module coverage report");

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
  const canonical=["/api/*","/crm.*/*"];
  check(JSON.stringify(wrangler.assets?.run_worker_first)===JSON.stringify(canonical),
    "production must use universal Worker-first Connect namespace and API routes");
  check(JSON.stringify(local.assets?.run_worker_first)===JSON.stringify(canonical),
    "local and published RPC routing differ");
  // /crm.*/* matches versioned Connect /crm.<domain>.vN.Service/Method
  // without routing ordinary static asset filenames through the Worker.
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
  // Only the in-place Party owner, the existing single Connect router and
  // build-time Protobuf codegen may evolve as a private signed-workerd G3
  // candidate. All production v3 requests remain 410 until the full cutover.
  // These are source-file freeze exceptions, NOT an API capability registry.
  const candidateSources=new Set([
    "apps/cloudflare-crm/rust-party/src/parties.rs",
    "apps/cloudflare-crm/rust-party/src/accounts.rs",
    "apps/cloudflare-crm/rust-party/src/contact_points.rs",
    "apps/cloudflare-crm/rust-party/src/party_relationships.rs",
    "apps/cloudflare-crm/rust-party/src/activity_tasks.rs",
    "apps/cloudflare-crm/rust-party/src/data_quality.rs",
    "apps/cloudflare-crm/rust-party/src/identity_resolution.rs",
    "apps/cloudflare-crm/rust-party/src/connect_sales.rs",
    "apps/cloudflare-crm/rust-party/build.rs",
    // Candidate-only generated Connect-Web facade; never loaded by the
    // contract-only public SPA. The boundary above rejects manual fetch,
    // legacy gateways, nonbinary transport and JS tenant forwarding.
    "packages/client/src/cloudflare.ts"
  ]);
  for(const path of frozenRuntimeFiles()){
    check(prev.has(path),"new runtime file before full cutover: "+path);
    if(policy.phase==="contract-only"&&
       (path==="apps/cloudflare-crm/rust-party/src/lib.rs"||
        candidateSources.has(path)))continue;
    if(path==="apps/cloudflare-crm/wrangler.jsonc"||
       path==="apps/cloudflare-crm/rust-party/wrangler.jsonc"){
      // Only this security-strengthening routing rule may change during freeze.
      // Everything else in both frozen Wrangler configurations remains identical.
      const before=JSON.parse(baselineContent(path));
      const now=JSON.parse(read(path));
      delete before.assets.run_worker_first;
      delete now.assets.run_worker_first;
      check(JSON.stringify(before)===JSON.stringify(now),
        "unexpected frozen Wrangler config change: "+path);
      continue;
    }
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
    check(runtime.includes('cfg!(feature = "native-ci")') &&
      runtime.includes('path.starts_with("/crm.parties.v3.PartyService/")') &&
      runtime.includes('path.starts_with("/crm.accounts.v2.AccountService/")') &&
      runtime.includes('path.starts_with("/crm.contact_points.v2.ContactPointService/")') &&
      runtime.includes('path.starts_with("/crm.party_relationships.v2.PartyRelationshipService/")') &&
      runtime.includes('path.starts_with("/crm.activities.v2.TaskService/")') &&
      runtime.includes('path.starts_with("/crm.data_quality.v2.DataQualityService/")') &&
      runtime.includes('path.starts_with("/crm.identity_resolution.v2.IdentityResolutionService/")') &&
      runtime.includes('path != "/crm.parties.v2.PartyService/GetParty" && !ci_candidate'),
      "Party v3 candidate MUST remain inaccessible in production contract-only");
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
  check(moduleReport!==null,"generated canonical module report required");
  const activeProof=JSON.parse(read("apps/cloudflare-crm/active-connect.json"));
  const reported=[...moduleReport.active_rpc_paths].sort();
  const proven=activeProof.methods.map(x=>x.path).sort();
  check(JSON.stringify(reported)===JSON.stringify(proven),
    "generated module report differs from active compiled Connect RPC evidence");
  const protoFiles=treeFiles("proto/crm").filter(p=>p.endsWith(".proto"));
  const protoText=protoFiles.map(read).join("\n");
  for(const method of moduleReport.active_rpc_paths){
    const slash=method.lastIndexOf("/");
    check(slash>0,"expected canonical Connect route: "+method);
    const service=method.slice(0,slash).split(".").pop();
    const rpc=method.slice(slash+1);
    check(new RegExp("service\\s+"+service+"\\s*\\{").test(protoText) &&
      new RegExp("rpc\\s+"+rpc+"\\s*\\(").test(protoText),
      "active RPC must exist in canonical Protobuf: "+method);
  }
}
console.log(JSON.stringify({event:"crm.architecture-gate",result:"PASS",
  phase:policy.phase,baseline,router_count:routerCount,
  d1:production.d1_databases[0].binding,
  legacy_runtime_policy:policy.phase==="migration-freeze"?"FROZEN":policy.phase==="contract-only"?"DISABLED":"REMOVED"}));
export {failNewRest,validateGeneratedBrowserBoundary,validateOneWorkerRouter,validateModuleReport};

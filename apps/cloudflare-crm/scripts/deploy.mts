// One protected GitHub Actions release owner: deploy, verify-data, fail closed.
import {existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from "node:fs";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {basename,join,relative} from "node:path";
import {spawnSync} from "node:child_process";
import {DatabaseSync} from "node:sqlite";
import {isRecord, record as jsonRecord, records, string, strings, number} from "./json-boundary.mts";
import {
  verifyAnonymousDenial,
  verifyNfcMigrationNames,
  verifyTracePrivacySettings,
  waitForNativeTelemetry,
  classifySignedSessionReadback,
  parseRetryAfterMs,
  releaseVersionAffinityKey
} from "./release-readback.mts";
const required=(name: string)=>{const value=process.env[name];if(!value)throw Error("Missing "+name);return value;};
const sha=required("GITHUB_SHA");
const versionKey=releaseVersionAffinityKey(required("GITHUB_RUN_ID"),required("GITHUB_RUN_ATTEMPT"));
if(!/^[0-9a-f]{40}$/.test(sha)||process.env.GITHUB_REF!=="refs/heads/dev"||process.env.GITHUB_EVENT_NAME!=="push")throw Error("Untrusted deployment");
const account="5bd5e1e1e813f061afc259e91334262d", database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
if(required("CLOUDFLARE_ACCOUNT_ID")!==account||required("CRM_D1_DATABASE_ID")!==database)throw Error("Wrong Cloudflare CRM target");
const token=required("CLOUDFLARE_API_TOKEN"),api="https://api.cloudflare.com/client/v4/accounts/"+account;
const worker="/workers/scripts/crm-dev",url="https://crm-dev.crm-5bd5e1e1.workers.dev",out="apps/cloudflare-crm/.release";
const record=(name: string,value: object)=>{mkdirSync(out,{recursive:true});writeFileSync(join(out,name),JSON.stringify(value,null,2)+"\n");};
function loadJson(path: string): unknown {
  return JSON.parse(readFileSync(path,"utf8"));
}
const load=(name: string)=>jsonRecord(loadJson(join(out,name)));
async function cf(path: string,method="GET",body?: object,timeoutMs=0): Promise<unknown>{
  const r=await fetch(api+path,{method,
    headers:{authorization:"Bearer "+token,...(body?{"content-type":"application/json"}:{})},
    ...(body?{body:JSON.stringify(body)}:{}),
    ...(timeoutMs?{signal:AbortSignal.timeout(timeoutMs)}:{})});
  // The API can return an HTML edge error (502/503/524) instead of JSON.
  // Never echo its body, request headers, Access credentials or bearer token.
  const ray=r.headers.get("cf-ray");
  const retryAfterMs=parseRetryAfterMs(r.headers.get("retry-after"));
  const contentType=r.headers.get("content-type")||"";
  let payload: unknown;
  try{payload=await r.json();}
  catch{
    throw Object.assign(new Error("Cloudflare "+method+" "+path+
      " HTTP "+r.status+" NON_JSON content_type="+
      (contentType||"none").slice(0,80)+" cf_ray="+(ray||"none")),
      {cfHttpStatus:r.status,cfRay:ray,cfRetryAfterMs:retryAfterMs,cfInvalidJson:true,
        cfJsonContent:/^application\/(?:json|[^;]+\+json)(?:\s*;|\s*$)/i.test(contentType)});
  }
  const envelope=isRecord(payload)?payload:undefined;
  if(!r.ok||envelope?.success!==true)throw Object.assign(new Error(
    "Cloudflare "+method+" "+path+" HTTP "+r.status+" cf_ray="+(ray||"none")+
    " codes="+JSON.stringify(Array.isArray(envelope?.errors)?
      envelope.errors.flatMap((error: unknown)=>isRecord(error)&&typeof error.code==="number"?[error.code]:[]):[])),
    {cfHttpStatus:r.status,cfRay:ray,cfRetryAfterMs:retryAfterMs});
  return envelope.result;
}
// Only read-only D1 SELECT and Cloudflare telemetry may be retried.
// Telemetry readback uses at most two concurrent calls, never the old
// four-call burst. Shared 429 cooldown applies across both read lanes.
let telemetryNextAt=0;
const pause=(ms: number)=>new Promise<void>(resolve=>setTimeout(resolve,ms));
const d1Endpoint="/d1/database/"+database+"/query";
const telemetryEndpoint="/workers/observability/telemetry/query";
type D1Query = {sql: string; params?: string[]};
type TelemetryQuery = {
  queryId: "crm-471-native-batch"; view: "events"; timeframe: {from: number; to: number};
  limit: number; offset?: string; offsetDirection?: "next";
  parameters: {filters: {key: string; operation: string; type: string; value: string}[]};
};
type ReadbackRequest = {kind: "d1"; body: D1Query} | {kind: "telemetry"; body: TelemetryQuery};
async function cfReadback({kind,body}: ReadbackRequest,phaseDeadline=Infinity){
  const audit=kind==="d1"&&/^SELECT\b/i.test(body.sql.trim())&&!body.sql.includes(";");
  const traces=kind==="telemetry"&&body.view==="events"&&body.queryId==="crm-471-native-batch"&&
    body.parameters.filters.some(f=>f.key==="$workers.scriptName"&&f.operation==="eq"&&f.value==="crm-dev");
  if(!audit&&!traces)throw Error("CRM_READBACK_RETRY_NOT_READ_ONLY");
  const target=audit?"d1-audit":"telemetry";
  const deadline=Math.min(Date.now()+90000,phaseDeadline);
  for(let attempt=1;;attempt++){
    const remaining=deadline-Date.now();
    if(remaining<=0)throw Error("CRM_READBACK_DEADLINE_EXCEEDED "+target);
    if(traces){
      const delayMs=Math.max(0,telemetryNextAt-Date.now());
      if(delayMs>0){
        if(delayMs>=deadline-Date.now())
          throw Error("CRM_TELEMETRY_COOLDOWN_EXCEEDS_DEADLINE");
        await pause(delayMs);
      }
    }
    try{
      const result=await cf(audit?d1Endpoint:telemetryEndpoint,"POST",body,
        Math.max(1,Math.min(30000,deadline-Date.now())));
      // Pace consecutive read batches; preserve any longer 429 cooldown.
      if(traces)telemetryNextAt=Math.max(telemetryNextAt,Date.now()+1000);
      return result;
    }catch(error){
      const failure=isRecord(error)?error:{};
      const status=failure.cfHttpStatus;
      const retryable=status===429||(typeof status==="number"&&status>=500&&status<=599)||
        (status===undefined&&typeof failure.name==="string"&&["TimeoutError","AbortError","TypeError"].includes(failure.name))||
        (traces&&status===200&&failure.cfInvalidJson===true&&failure.cfJsonContent===true);
      // A valid Retry-After is a floor, not a hint to cap at our backoff.
      const delayMs=Math.max(Math.min(2500*2**(attempt-1),10000)+
        Math.floor(Math.random()*250),typeof failure.cfRetryAfterMs==="number"?failure.cfRetryAfterMs:0);
      if(traces&&status===429)
        telemetryNextAt=Math.max(telemetryNextAt,Date.now()+delayMs);
      if(!retryable||attempt>=3||Date.now()+delayMs>=deadline)throw error;
      console.log(JSON.stringify({event:"crm.cf-readback-retry",target,attempt,
        http_status:status??"transport",cf_ray:failure.cfRay??null,
        ...(failure.cfInvalidJson?{reason:"NON_JSON"}:{}),delay_ms:delayMs}));
      // Telemetry waits through the shared cooldown at the next attempt.
      if(!traces)await pause(delayMs);
    }
  }
}
function d1Rows(value: unknown){
  const [result,...extra]=records(value);
  assert.ok(result?.success===true&&extra.length===0,"D1 query result is invalid");
  return records(result.results);
}
function cmd(...args: string[]){const r=spawnSync("npx",args,{cwd:"apps/cloudflare-crm",stdio:"inherit"});if(r.status!==0)throw Error("Failed npx "+args.join(" "));}
function paths(dir: string): string[]{return readdirSync(dir).flatMap(n=>{const p=join(dir,n);return statSync(p).isDirectory()?paths(p):[p];});}
function builtRust(){
  const dir="apps/cloudflare-crm/rust-party/build";
  if(!existsSync(join(dir,"index.js")))throw Error("Native Rust Worker build missing");
  const list=paths(dir).filter(p=>p.endsWith(".wasm")||basename(p)==="index.js");
  if(!list.some(p=>p.endsWith(".wasm")))throw Error("Native Rust WASM missing");
  return list.map(p=>{const b=readFileSync(p);return{file:relative("apps/cloudflare-crm",p),bytes:b.byteLength,sha256:createHash("sha256").update(b).digest("hex")};});
}
async function version(){
  const [deployment]=records(jsonRecord(await cf(worker+"/deployments")).deployments);
  const [active,...others]=records(deployment?.versions);
  if(active?.percentage!==100||others.length!==0)throw Error("CRM must have one 100% Worker deployment");
  const id=string(active.version_id);
  assert.match(id,/^[a-zA-Z0-9-]+$/,"Malformed active Worker version");
  return id;
}
async function bindings(deployedVersion: string){
  // The script settings endpoint reports the latest uploaded version, not the active deployment.
  const b=records(jsonRecord(jsonRecord(await cf(worker+"/versions/"+deployedVersion)).resources).bindings);
  return{sha:string(b.find(x=>x.name==="GIT_SHA")?.text),db:string(b.find(x=>x.name==="DB")?.id)};
}
// Cloudflare deployments, version metadata and the public edge can converge
// separately after Wrangler deploy. Never treat the first old-version read
// as a final release result; read back the exact source and binding.
async function waitForDeployment(previousVersion: string,expectedSha: string,stage: string){
  const start=Date.now(),deadline=start+60000;
  let observed: object={version:null,sha:null,db:null},attempts=0;
  for(;;){
    attempts++;
    try{
      const current=await version();
      const b=await bindings(current);
      observed={version:current,sha:b.sha||null,db:b.db||null};
      if(current!==previousVersion&&
          b.sha===expectedSha&&b.db===database){
        console.log(JSON.stringify({event:"crm.deployment-readback",
          stage,result:"PASS",worker_version:current,
          source:expectedSha,attempts,elapsed_ms:Date.now()-start}));
        return current;
      }
    }catch(error){observed={error:error instanceof Error?error.name:"UnknownError"};}
    if(Date.now()>=deadline)throw Error("CRM deployment readback mismatch stage="+stage+
      " previous_version="+previousVersion+" expected_sha="+expectedSha+
      " observed="+JSON.stringify(observed));
    if(attempts===1)console.log(JSON.stringify({event:"crm.deployment-readback",
      stage,result:"RETRY",observed}));
    await new Promise(resolve=>setTimeout(resolve,Math.min(attempts*500,2500)));
  }
}
const auth=()=>({"CF-Access-Client-Id":required("CRM_E2E_A_CLIENT_ID"),"CF-Access-Client-Secret":required("CRM_E2E_A_CLIENT_SECRET")});
async function publicSha(){
  // One isolated request per attempt. Read the active public Worker, not just
  // Cloudflare's deployment metadata; preserve safe edge diagnostics on mismatch.
  const requestUrl=url+"/api/version?readback="+Date.now()+"-"+Math.random().toString(36).slice(2);
  const r=await fetch(requestUrl,{headers:{...auth(),"cache-control":"no-cache",
    "Cloudflare-Workers-Version-Key":versionKey},
    redirect:"manual",cache:"no-store"});
  const edge={http_status:r.status,ray:r.headers.get("cf-ray"),
    cache_status:r.headers.get("cf-cache-status"),
    runtime:r.headers.get("x-crm-runtime"),
    header_sha:r.headers.get("x-crm-source-sha")};
  if(r.status!==200)throw Error("CRM public version: "+JSON.stringify(edge));
  const body: unknown=await r.json();
  const source=string(jsonRecord(body).source);
  assert.match(source,/^[a-f0-9]{40}$/,"Malformed public Worker source");
  return {...edge,source};
}

async function waitForPublicSha(expected: string,stage: string){
  // Cloudflare control-plane convergence is separate from workers.dev edge
  // routing. Two independent releases exposed stale edge SHA for >45 seconds.
  // This remains a strict exact-SHA gate with a bounded, observable retry.
  const start=Date.now(),deadline=start+120000;
  let last: Awaited<ReturnType<typeof publicSha>> | {error: string}={error:"NOT_CHECKED"},attempts=0;
  for(;;){
    attempts++;
    try{last=await publicSha();}
    catch(error){last={error:error instanceof Error?error.name:"UnknownError"};}
    if("source" in last&&last.source===expected){
      console.log(JSON.stringify({event:"crm.public-readback",stage,sha:expected,
        result:"PASS",attempts,elapsed_ms:Date.now()-start,
        edge:{ray:last.ray,cache_status:last.cache_status,header_sha:last.header_sha}}));
      return;
    }
    if(Date.now()>=deadline)throw Error("Public Worker SHA readback mismatch stage="+stage+
      " expected="+expected+" last_observed="+JSON.stringify(last)+
      " attempts="+attempts);
    if(attempts===1||attempts%5===0)console.log(JSON.stringify({
      event:"crm.public-readback",stage,expected,observed:last,
      result:"RETRY",attempts,elapsed_ms:Date.now()-start}));
    await new Promise(resolve=>setTimeout(resolve,Math.min(attempts*600,3500)));
  }
}

async function deploy(){
  const startedAt=Date.now();
  const config=jsonRecord(loadJson("apps/cloudflare-crm/wrangler.jsonc"));
  if(config.main!=="./rust-party/build/index.js"||config.workers_dev!==false||config.preview_urls!==false||
      (config.routes!==undefined&&(!Array.isArray(config.routes)||config.routes.length!==0)))
    throw Error("Refusing release: public Worker is not native workers-rs");
  // Prevent a privacy regression BEFORE changing any live deployment or D1.
  verifyTracePrivacySettings(config.observability);
  const rust=builtRust(), oldVersion=await version(), before=await bindings(oldVersion);
  if(before.db!==database||!/^[a-f0-9]{40}$/.test(before.sha||""))throw Error("Previous CRM binding/SHA mismatch");
  const bookmark=string(jsonRecord(await cf("/d1/database/"+database+"/time_travel/bookmark")).bookmark);
  if(!bookmark)throw Error("D1 Time Travel bookmark missing");
  // Fail closed before migrations: the deploy job must use exactly the CI-built files.
  const source=readFileSync(join(out,"source-sha.txt"),"utf8").trim();
  if(source!==sha)throw Error("Compiled CRM bundle source SHA mismatch");
  const checked=spawnSync("sha256sum",["--check","--strict",".release/compiled-sha256.txt"],{
    cwd:"apps/cloudflare-crm",stdio:"inherit"
  });
  if(checked.status!==0||!existsSync("apps/cloudflare-crm/dist/index.html"))
    throw Error("Compiled CRM release artifact integrity failed");
  const query=async(sql: string,params: string[]=[])=>
    d1Rows(await cf(d1Endpoint,"POST",{sql,...(params.length?{params}:{})}));
  // Canonical files plus the existing D1 ledger: no third schema registry.
  const localMigrations=readdirSync("apps/cloudflare-crm/migrations")
    .filter(name=>name.endsWith(".sql")).sort();
  const appliedMigrations=(await query("SELECT name FROM d1_migrations ORDER BY name"))
    .map(row=>string(row.name));
  assert.deepEqual(appliedMigrations,localMigrations.slice(0,appliedMigrations.length),
    "D1 migration ledger must match the committed schema history");
  const pendingMigrations=localMigrations.length>appliedMigrations.length;
  const resetPending=!appliedMigrations.includes("0002_hard_cutover.sql");
  const nfcPending=!resetPending&&localMigrations.includes("0016_nfc_search_name.sql")&&
    !appliedMigrations.includes("0016_nfc_search_name.sql");
  if(nfcPending)await verifyNfcMigrationNames(query);
  if(resetPending)assert.equal(before.sha,"90a352be2a70b74a64e5a774c9a76b7f02c86a5f",
    "One-time reset requires the audited frozen predecessor");
  console.log(JSON.stringify({event:"crm.migration-readback",result:"PASS",
    pending:pendingMigrations,applied:appliedMigrations.length,canonical:localMigrations.length}));
  // Compare every pre-existing principal column across migrations. SELECT *
  // would reject legitimate ADD COLUMN changes even when all prior account
  // identities, roles and membership data were preserved byte-for-byte.
  // SQLite itself supplies this list; no parallel schema registry.
  const principalColumns=(await query("PRAGMA table_info(crm_users)")).map(row=>string(row.name));
  assert.ok(["email","tenant_id","role","active"].every(name=>principalColumns.includes(name)),
    "The existing signed Access admission columns must remain present");
  const principalColumnsSql=principalColumns.map(name=>{
    assert.match(name,/^[a-z_][a-z0-9_]*$/,"Invalid canonical D1 column name");
    return '"'+name+'"';
  }).join(",");
  const principalReadSql="SELECT "+principalColumnsSql+" FROM crm_users ORDER BY email";
  const principals=await query(principalReadSql);
  const tenants=await query("SELECT * FROM crm_tenants ORDER BY id");
  record("checkpoint.json",{git_sha:sha,previous_sha:before.sha,previous_version:oldVersion,
    d1_database:database,pre_migration_bookmark:bookmark,
    reset_pending:resetPending,migration_pending:pendingMigrations,rust});
  if(pendingMigrations){
    // Stop traffic only when the D1 schema will actually change.
    await cf(worker+"/subdomain","POST",{enabled:false,previews_enabled:false});
    await confirmClosed();
    if(nfcPending){
      await verifyNfcMigrationNames(query);
      console.log(JSON.stringify({event:"crm.nfc-migration-precondition",result:"PASS"}));
    }
    cmd("wrangler","d1","migrations","apply","crm-dev","--remote");
  }
  assert.deepEqual((await query("SELECT name FROM d1_migrations ORDER BY name"))
    .map(row=>string(row.name)),localMigrations,"D1 must have exactly the canonical migrations");
  assert.deepEqual(await query(principalReadSql),principals,
    "Migration changed previously existing signed CRM user membership data");
  assert.deepEqual(await query("SELECT * FROM crm_tenants ORDER BY id"),tenants);
  assert.deepEqual(await query("PRAGMA foreign_key_check"),[]);
  // SQLite compiles the committed schema; no SQL source parser or second table registry.
  const canonical=new DatabaseSync(":memory:");
  let expected;
  const inventory="SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND tbl_name LIKE 'crm_%' ORDER BY type,name";
  try{
    for(const name of readdirSync("apps/cloudflare-crm/migrations").filter(name=>name.endsWith(".sql")).sort())
      canonical.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
    expected=canonical.prepare(inventory).all().map(row=>({...row}));
  }finally{canonical.close();}
  assert.deepEqual(await query(inventory),expected,"Live D1 must have the exact canonical schema");
  if(resetPending)for(const table of expected.filter(x=>x.type==="table")){
    const name=string(table.name);
    if(["crm_users","crm_tenants"].includes(name))continue;
    assert.match(name,/^[a-z_][a-z0-9_]*$/);
    assert.equal((await query("SELECT COUNT(*) AS n FROM "+name))[0]?.n,0,"Reset left business data");
  }
  cmd("wrangler","deploy","--var","GIT_SHA:"+sha);
  const newVersion=await waitForDeployment(oldVersion,sha,"deploy");
  // Read the settings returned by Cloudflare, not an assumed Wrangler default.
  // Do not reopen public entry if trace privacy differs from committed config.
  const traceSettings=verifyTracePrivacySettings(jsonRecord(await cf(worker+"/settings")).observability);
  await cf(worker+"/subdomain","POST",{enabled:true,previews_enabled:false});
  await waitForPublicSha(sha,"deploy");
  // Signed exact-SHA /api/version above already establishes Access plus Worker
  // availability. Require the same endpoint to reject anonymous requests.
  await verifyAnonymousDenial(url+"/api/version");
  record("deployed.json",{git_sha:sha,worker_version:newVersion,d1_database:database,rust,
    trace_settings:traceSettings,started_at:startedAt,result:"E2E_PENDING"});
  console.log(JSON.stringify({event:"crm.deploy",sha,result:"E2E_PENDING",version:newVersion}));
}
async function confirmClosed(){
  // The Cloudflare control-plane GET can briefly return the previous state after POST.
  // Require convergence of the real API setting AND the public edge; never infer closure.
  let state: Record<string,unknown> | undefined;
  for(let attempt=0;attempt<30;attempt++){
    state=jsonRecord(await cf(worker+"/subdomain"));
    if(state.enabled===false&&state.previews_enabled===false)break;
    await new Promise(resolve=>setTimeout(resolve,2000));
  }
  assert.equal(state?.enabled,false,"Cloudflare Worker subdomain remained enabled");
  assert.equal(state?.previews_enabled,false,"Cloudflare Worker previews remained enabled");
  let stopped=0;
  for(let attempt=0;attempt<30&&stopped<3;attempt++){
    const response=await fetch(url+"/api/version?closed="+Date.now(),{headers:auth(),redirect:"manual"});
    stopped=response.status===404&&!response.headers.get("x-crm-runtime")?stopped+1:0;
    if(stopped<3)await new Promise(resolve=>setTimeout(resolve,2000));
  }
  assert.equal(stopped,3,"Public Worker entry did not stop; no schema change is allowed");
}
// Each release tests two freshly enrolled signed-Access service principals
// in isolated tenants of the SAME clean CRM D1. Existing CRM records are
// disposable test data; no pilot schema or historical data is migrated.
async function prepareE2e(){
  const run=required("GITHUB_RUN_ID"),attempt=required("GITHUB_RUN_ATTEMPT");
  if(!/^\d+$/.test(run)||!/^\d+$/.test(attempt))throw Error("Invalid E2E run identity");
  const principal=(n: "A" | "B")=>{
    const id=required("CRM_E2E_"+n+"_CLIENT_ID").toLowerCase();
    if(!/^[a-z0-9.-]{8,120}\.access$/.test(id))throw Error("Invalid E2E service identity");
    return "service:"+id;
  };
  const a=principal("A"),b=principal("B");
  if(a===b)throw Error("E2E identities must be distinct");
  // Service actors are explicitly enrolled by the trusted release process
  // BEFORE contacting the signed Access endpoint. The Rust admission gate
  // never creates a tenant or grants admin to an unknown service principal.
  // These two CI-only tokens are distinct from the human owner and Browser MCP.
  const query=async(sql: string,params: string[]=[])=>d1Rows(await cf(d1Endpoint,"POST",{sql,params}));
  const ta="crm-e2e-"+run+"-"+attempt+"-a",tb="crm-e2e-"+run+"-"+attempt+"-b";
  if(ta===tb)throw Error("E2E tenants must differ");
  const batch=[
    {sql:"INSERT OR IGNORE INTO crm_tenants(id,name) VALUES (?,?)",
      params:[ta,"CRM protected E2E A"]},
    {sql:"INSERT OR IGNORE INTO crm_tenants(id,name) VALUES (?,?)",
      params:[tb,"CRM protected E2E B"]},
    {sql:"INSERT INTO crm_users(email,tenant_id,role,active) VALUES (?,?,'admin',1) "+
      "ON CONFLICT(email) DO UPDATE SET tenant_id=excluded.tenant_id,role='admin',active=1",
      params:[a,ta]},
    {sql:"INSERT INTO crm_users(email,tenant_id,role,active) VALUES (?,?,'admin',1) "+
      "ON CONFLICT(email) DO UPDATE SET tenant_id=excluded.tenant_id,role='admin',active=1",
      params:[b,tb]}
  ];
  const changed=records(await cf(d1Endpoint,"POST",{batch}));
  // D1 meta.changes is sqlite3_total_changes(), which INCLUDES writes made
  // by triggers. Migration 0034 intentionally advances one forecast fence
  // for each member mutation, so exact equality with the direct row count
  // (1) is incorrect. Require at least the direct mutation, then verify the
  // exact actor rows and the independently observable tenant-local fences.
  const memberChanges=changed.slice(2).map(x=>number(jsonRecord(x.meta).changes));
  if(changed.length!==4||changed.some(x=>x.success!==true)||
      memberChanges.some(changes=>!Number.isInteger(changes)||changes<1))
    throw Error("E2E explicit service enrollment failed for isolated actors");
  const readback=await query(
    "SELECT email,tenant_id,role,active FROM crm_users WHERE email IN (?,?) ORDER BY email",[a,b]);
  if(readback.length!==2||readback.some(x=>
    x.role!=="admin"||x.active!==1||
    (x.email===a?x.tenant_id!==ta:x.email===b?x.tenant_id!==tb:true)))
    throw Error("E2E enrolled actors do not match the two isolated tenants");
  const fences=await query(
    "SELECT tenant,revision FROM crm_sales_forecast_revisions WHERE tenant IN (?,?) ORDER BY tenant",[ta,tb]);
  if(fences.length!==2||[ta,tb].some(tenant=>
    !fences.some(row=>row.tenant===tenant&&row.revision===1)))
    throw Error("E2E member enrollment did not advance each tenant forecast fence exactly once");
  console.log(JSON.stringify({event:"crm.e2e-enrollment",result:"PASS",
    isolated_tenants:2,direct_member_mutations:2,
    d1_total_changes:memberChanges,
    tenant_fence_revisions:fences.map(x=>x.revision)}));
  // Verify the real Cloudflare Access signature and Rust service lookup after
  // the exact trusted D1 enrollment, never by forging an actor from the UI.
  for(const n of ["A","B"]){
    const id=required("CRM_E2E_"+n+"_CLIENT_ID");
    const secret=required("CRM_E2E_"+n+"_CLIENT_SECRET");
    const started=Date.now(),deadline=started+120000;
    let attempts=0;
    for(;;){
      attempts++;
      const reply=await fetch(url+"/api/session?readback="+Date.now()+"-"+n,{
        headers:{"CF-Access-Client-Id":id,"CF-Access-Client-Secret":secret,
          "cache-control":"no-cache"},
        redirect:"manual",cache:"no-store"});
      const source=reply.headers.get("x-crm-source-sha");
      const session: unknown=reply.status===200?await reply.json():null;
      let classification;
      try{
        classification=classifySignedSessionReadback({
          status:reply.status,source,session,expectedSha:sha
        });
      }catch(error){
        throw Error("Signed E2E service "+n+" failed Access verification: "+(error instanceof Error?error.message:"UnknownError"));
      }
      if(classification.kind==="pass"){
        console.log(JSON.stringify({event:"crm.e2e-access-readback",service:n,
          result:"PASS",attempts,elapsed_ms:Date.now()-started,sha:source}));
        break;
      }
      // Workers.dev can briefly serve either its edge 404 page or a previous
      // signed Worker version. Retry only those two proven propagation states.
      // Authentication, admin role, and explicit SHA provenance remain mandatory.
      if(Date.now()>=deadline)
        throw Error("Signed E2E service "+n+" Workers.dev source not ready after "+
          attempts+" attempts and "+(Date.now()-started)+"ms");
      if(attempts===1||attempts%5===0)
        console.log(JSON.stringify({event:"crm.e2e-access-readback",service:n,
          result:"RETRY",reason:classification.reason,http_status:reply.status,
          observed_sha:source,attempts,elapsed_ms:Date.now()-started}));
      await new Promise(resolve=>setTimeout(resolve,Math.min(attempts*600,3500)));
    }
  }
  const clean=await query(
    "SELECT COUNT(*) AS existing FROM crm_party_records WHERE tenant IN (?,?)",[ta,tb]);
  if(clean[0]?.existing!==0)
    throw Error("E2E isolated test tenants contain old Party data");
  record("e2e-tenants.json",{source_sha:sha,database,run_id:run,attempt,actor_a:a,tenant_a:ta,tenant_b:tb});
  console.log(JSON.stringify({event:"crm.e2e-tenant-isolation",
    result:"PASS",database:"existing D1",actors:2,tenants:2,previous_data_deleted:false}));
}

// Complete native Cloudflare Observability cursor, with no per-request HTTP loop.
async function readTelemetrySnapshot(from: number,to: number,deadline: number){
  const limit=2000,cursors=new Set<string>(),events: Record<string,unknown>[]=[];
  let offset: string | null=null;
  for(let page=0;page<32;page++){
    const data=await cfReadback({kind:"telemetry",body:{
      queryId:"crm-471-native-batch",view:"events",timeframe:{from,to},limit,
      ...(offset===null?{}:{offset,offsetDirection:"next"}),
      parameters:{filters:[{key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
    }},deadline);
    const batch=records(jsonRecord(jsonRecord(data).events).events);
    assert.ok(batch.length<=limit,"CRM_TELEMETRY_PAGE_INVALID");
    let nextCursor: string | undefined;
    for(const event of batch){
      const id=isRecord(event.$metadata)?event.$metadata.id:undefined;
      assert.ok(typeof id==="string"&&id,"CRM_TELEMETRY_CURSOR_MISSING");
      // Cloudflare may reuse one metadata ID for multiple legitimate
      // events on the same page. Never drop those logs or native spans.
      events.push(event);
      nextCursor=id;
    }
    if(batch.length<limit)return events;
    assert.ok(nextCursor,"CRM_TELEMETRY_CURSOR_MISSING");
    assert.ok(!cursors.has(nextCursor),"CRM_TELEMETRY_CURSOR_REPEATED");
    cursors.add(nextCursor);
    offset=nextCursor;
  }
  throw Error("CRM_TELEMETRY_PAGE_LIMIT_EXCEEDED");
}

async function verifyData(){
  const deployed=load("deployed.json");
  assert.equal(deployed.git_sha,sha);
  const liveVersion=await version(),liveBindings=await bindings(liveVersion);
  assert.equal(liveVersion,deployed.worker_version);
  assert.deepEqual(liveBindings,{sha,db:database});
  const traceSettings=verifyTracePrivacySettings(jsonRecord(await cf(worker+"/settings")).observability);
  assert.deepEqual(deployed.trace_settings,traceSettings);
  // Release smoke is NOT Gate B. Detailed 68-checkpoint browser/owner regressions
  // run in workerd in the upstream job; independent live Gate B is post-merge.
  const evidence=jsonRecord(loadJson("apps/cloudflare-crm/e2e-evidence/published-smoke.json"));
  assert.equal(evidence.result,"PASS");
  assert.equal(evidence.source_sha,sha);
  assert.equal(evidence.version_affinity_key,versionKey);
  assert.deepEqual(evidence.features,["native","published-smoke"]);
  const startedAt=number(evidence.started_at);
  assert.ok(startedAt>=number(deployed.started_at));
  const setup=load("e2e-tenants.json");
  const tenant=string(evidence.tenant);
  const actor=string(setup.actor_a);
  const steps=strings(evidence.steps);
  assert.equal(tenant,setup.tenant_a);
  assert.ok(actor,"Release must identify the enrolled Cloudflare Access actor");
  assert.deepEqual(steps,[
    "version-affinity","signed-access","party-create","party-reload","account-create",
    "viewer-denial","tenant-isolation","anonymous-denial"
  ],"Published Chromium must execute all mandatory release smoke properties");
  const requests=new Map<string,{request_id: string; path: string; status: number; error_code: string | null}>();
  for(const row of records(evidence.requests)){
    assert.equal(row.source_sha,sha);
    assert.equal(row.runtime,"native-workers-rs");
    assert.equal(row.header_error,undefined,"Complete browser correlation evidence is required");
    const request_id=string(row.request_id);
    assert.match(request_id,/^[a-f0-9-]{36}$/i);
    const request={request_id,path:string(row.path),status:number(row.status),
      error_code:row.error_code==null?null:string(row.error_code)};
    const prior=requests.get(request_id);
    if(prior)assert.deepEqual(prior,request);
    requests.set(request_id,request);
  }
  assert.ok(requests.size>=4,"Published UI must use signed typed Connect RPCs");
  assert.ok([...requests.values()].some(item=>item.status===403&&
    item.path.endsWith("/CreateParty")),
    "Real viewer forbidden mutation must be traced");
  assert.ok([...requests.values()].some(item=>item.status===200&&
    item.path.endsWith("/ListAccounts")),
    "Cross-tenant list must run through the published Worker");
  const writes=records(evidence.writes).map(row=>({
    path:string(row.path),request_id:string(row.request_id),audit:records(row.audit)
  }));
  assert.equal(writes.length,2,"Smoke must commit precisely Party and Account");
  const writeContracts=[
    {method:"/CreateParty",action:"parties.create"},
    {method:"/CreateAccount",action:"accounts.create"}
  ];
  const mutationIds=new Set<string>();
  for(const [index,write] of writes.entries()){
    const expected=writeContracts[index];
    assert.ok(expected);
    assert.ok(write.path.endsWith(expected.method));
    assert.ok(!mutationIds.has(write.request_id));
    mutationIds.add(write.request_id);
    assert.equal(requests.get(write.request_id)?.path,write.path);
    assert.equal(requests.get(write.request_id)?.status,200);
    assert.equal(write.audit.filter(row=>row.action===expected.action).length,1);
    for(const row of write.audit)assert.equal(row.actor,actor);
  }
  // One actual Cloudflare D1 snapshot proves both published writes and that
  // read-only RPCs, forbidden viewer writes and tenant-B reads created no audit.
  const ids=[...requests.keys()];
  const result=await cfReadback({kind:"d1",body:{
    sql:"SELECT request_id,action,subject_id,actor FROM crm_audit WHERE tenant_id=? "+
      "AND request_id IN ("+ids.map(()=>"?").join(",")+") ORDER BY request_id,action,subject_id",
    params:[tenant,...ids]}});
  const auditByRequest=new Map<string,Record<string,unknown>[]>();
  for(const row of d1Rows(result)){
    const requestId=string(row.request_id);
    assert.ok(requests.has(requestId),"No unrelated audit may enter release evidence");
    const rows=auditByRequest.get(requestId)??[];
    rows.push(row);auditByRequest.set(requestId,rows);
  }
  const sort=(rows: Record<string,unknown>[])=>rows.map(row=>[string(row.action),string(row.subject_id),string(row.actor)]).sort(
    (a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
  for(const write of writes)
    assert.deepEqual(sort(auditByRequest.get(write.request_id)??[]),sort(write.audit),
      "Live D1 audit must equal independently observed browser commit");
  for(const id of ids)
    if(!mutationIds.has(id))
      assert.equal((auditByRequest.get(id)??[]).length,0,
        "Browser reads/denials must not append audit: "+requests.get(id)?.path);
  const allowedPrivate: Record<string,RegExp>={
    "party-name":/^Browser Party [0-9]{10,}$/,
    "cookie":/^crm-s3-cookie-[0-9]{10,}$/,
    "header":/^crm-s3-header-[0-9]{10,}$/,
    "authorization":/^Bearer crm-s3-authorization-[0-9]{10,}$/,
    "jwt":/^eyJhbGciOiJub25lIn0\.eyJzdWIiOiJjcm0tc3ludGhldGljIn0\.signature-[0-9]{10,}$/,
    "query-string":/^crm-s3-query-[0-9]{10,}$/
  };
  const probes=records(evidence.synthetic_private_probes??[]).map(row=>({
    kind:string(row.kind),request_id:string(row.request_id),value:string(row.value)
  }));
  assert.deepEqual(probes.map(item=>item.kind).sort(),Object.keys(allowedPrivate).sort(),
    "Production trace privacy must include six signed synthetic probes");
  for(const probe of probes){
    assert.ok(allowedPrivate[probe.kind]?.test(probe.value));
    assert.equal(probe.request_id,writes[0]?.request_id);
  }
  const from=startedAt-1000;
  const {verifiedD1}=await waitForNativeTelemetry({
    requests,mutationIds,probes,liveVersion,
    readSnapshot:deadline=>readTelemetrySnapshot(from,Date.now()+1000,deadline)
  });
  assert.equal(verifiedD1,2,"Both published mutations require native D1 spans");
  assert.equal(await version(),liveVersion,"Deployment changed during release smoke");
  assert.deepEqual(await bindings(liveVersion),{sha,db:database});
  await waitForPublicSha(sha,"final");
  record("accepted.json",{result:"PASS",git_sha:sha,worker_version:liveVersion,
    d1_database:database,features:["native","published-smoke"],traffic_percentage:100,
    mutations:mutationIds.size,logs_correlated:requests.size,
    d1_mutation_spans:verifiedD1,synthetic_private_probes:probes.length,
    trace_settings:traceSettings,steps,request_ids:ids,
    independent_gate_b:"REQUIRES_SEPARATE_POST_MERGE_BROWSER_EVIDENCE"});
}
async function failClosed(){
  // Never switch an old Worker onto a new D1 schema.
  if(!existsSync(join(out,"checkpoint.json")))return;
  const checkpoint=load("checkpoint.json");
  assert.equal(checkpoint.git_sha,sha);assert.equal(checkpoint.d1_database,database);
  const current=await bindings(await version());
  assert.equal(current.db,database);
  assert.ok(current.sha===sha||current.sha===checkpoint.previous_sha,
    "Refusing to stop an unrelated deployment");
  await cf(worker+"/subdomain","POST",{enabled:false,previews_enabled:false});
  await confirmClosed();
  record("failed-release.json",{result:"NO_GO_PUBLIC_ENTRY_DISABLED",git_sha:sha,
    d1_database:database,worker_restored:false,d1_restored:false});
  // Also recover after a cancelled browser process, whose finally cannot run.
  if(existsSync(join(out,"e2e-tenants.json"))){
    const setup=load("e2e-tenants.json");
    assert.equal(setup.source_sha,sha);assert.equal(setup.database,database);
    assert.equal(setup.run_id,required("GITHUB_RUN_ID"));
    assert.equal(setup.attempt,required("GITHUB_RUN_ATTEMPT"));
    assert.equal(setup.tenant_a,"crm-e2e-"+setup.run_id+"-"+setup.attempt+"-a");
    assert.equal(setup.actor_a,"service:"+required("CRM_E2E_A_CLIENT_ID").toLowerCase());
    const query=async(sql: string)=>d1Rows(await cf(d1Endpoint,"POST",{
      sql,params:[string(setup.actor_a),string(setup.tenant_a)]}));
    await query("UPDATE crm_users SET role='admin' WHERE email=? AND tenant_id=? AND active=1 AND role='viewer'");
    assert.deepEqual(await query("SELECT role FROM crm_users WHERE email=? AND tenant_id=? AND active=1"),[{role:"admin"}]);
  }
}
const mode=process.argv[2]||"deploy";
if(mode==="deploy")await deploy();
else if(mode==="prepare-e2e")await prepareE2e();
else if(mode==="verify-data")await verifyData();
else if(mode==="fail-closed")await failClosed();
else throw Error("Unknown CRM deploy operation "+mode);

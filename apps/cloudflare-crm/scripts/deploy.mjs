// One protected GitHub Actions release owner: deploy, verify-data, fail closed.
import {existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from "node:fs";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {join,relative} from "node:path";
import {spawnSync} from "node:child_process";
import {DatabaseSync} from "node:sqlite";
import {
  verifyAnonymousDenial,
  verifyNfcMigrationNames,
  verifyTracePrivacySettings,
  classifySignedSessionReadback
} from "./release-readback.mjs";
const required=n=>{if(!process.env[n])throw Error("Missing "+n);return process.env[n];};
const sha=required("GITHUB_SHA");
if(!/^[0-9a-f]{40}$/.test(sha)||process.env.GITHUB_REF!=="refs/heads/dev"||process.env.GITHUB_EVENT_NAME!=="push")throw Error("Untrusted deployment");
const account="5bd5e1e1e813f061afc259e91334262d", database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
if(required("CLOUDFLARE_ACCOUNT_ID")!==account||required("CRM_D1_DATABASE_ID")!==database)throw Error("Wrong Cloudflare CRM target");
const token=required("CLOUDFLARE_API_TOKEN"),api="https://api.cloudflare.com/client/v4/accounts/"+account;
const worker="/workers/scripts/crm-dev",url="https://crm-dev.crm-5bd5e1e1.workers.dev",out="apps/cloudflare-crm/.release";
const record=(name,v)=>{mkdirSync(out,{recursive:true});writeFileSync(join(out,name),JSON.stringify(v,null,2)+"\n");};
const load=name=>JSON.parse(readFileSync(join(out,name),"utf8"));
async function cf(path,method="GET",body,timeoutMs=0){
  const r=await fetch(api+path,{method,
    headers:{authorization:"Bearer "+token,...(body?{"content-type":"application/json"}:{})},
    ...(body?{body:JSON.stringify(body)}:{}),
    ...(timeoutMs?{signal:AbortSignal.timeout(timeoutMs)}:{})});
  // The API can return an HTML edge error (502/503/524) instead of JSON.
  // Never echo its body, request headers, Access credentials or bearer token.
  const ray=r.headers.get("cf-ray");
  const contentType=r.headers.get("content-type")||"";
  let j;
  try{j=await r.json();}
  catch{
    throw Object.assign(new Error("Cloudflare "+method+" "+path+
      " HTTP "+r.status+" NON_JSON content_type="+
      (contentType||"none").slice(0,80)+" cf_ray="+(ray||"none")),
      {cfHttpStatus:r.status,cfRay:ray,cfInvalidJson:true,
        cfJsonContent:/^application\/(?:json|[^;]+\+json)(?:\s*;|\s*$)/i.test(contentType)});
  }
  if(!r.ok||!j?.success)throw Object.assign(new Error(
    "Cloudflare "+method+" "+path+" HTTP "+r.status+" cf_ray="+(ray||"none")+
    " codes="+JSON.stringify((j?.errors||[]).map(e=>e.code))),
    {cfHttpStatus:r.status,cfRay:ray});
  return j.result;
}
// Only explicitly read-only D1 audit SELECT and telemetry search are replayable.
// Never retry a D1 mutation, Worker deploy, subdomain POST or an ambiguous
// business request. A failing 401/403/400 or HTML 200 remains fail-closed.
// A malformed/truncated declared JSON 200 from read-only telemetry can
// be replayed within the same existing bounded readback budget.
async function cfReadback(path,body){
  const audit=path==="/d1/database/"+database+"/query"&&
    typeof body?.sql==="string"&&/^SELECT\b/i.test(body.sql.trim())&&
    !body.sql.includes(";");
  const traces=path==="/workers/observability/telemetry/query"&&
    body?.view==="events"&&typeof body?.parameters?.needle?.value==="string";
  if(!audit&&!traces)throw Error("CRM_READBACK_RETRY_NOT_READ_ONLY");
  const target=audit?"d1-audit":"telemetry";
  const started=Date.now();
  for(let attempt=1;;attempt++){
    try{return await cf(path,"POST",body,30000);}
    catch(error){
      const status=error?.cfHttpStatus;
      const retryable=status===429||(typeof status==="number"&&status>=500&&status<=599)||
        (status===undefined&&["TimeoutError","AbortError","TypeError"].includes(error?.name))||
        (traces&&status===200&&error?.cfInvalidJson===true&&error.cfJsonContent===true);
      if(!retryable||attempt>=3||Date.now()-started>=90000)throw error;
      const delayMs=Math.min(attempt*2500,5000);
      console.log(JSON.stringify({event:"crm.cf-readback-retry",target,attempt,
        http_status:status??"transport",cf_ray:error?.cfRay??null,
        ...(error?.cfInvalidJson?{reason:"NON_JSON"}:{}),delay_ms:delayMs}));
      await new Promise(resolve=>setTimeout(resolve,delayMs));
    }
  }
}
function cmd(...args){const r=spawnSync("npx",args,{cwd:"apps/cloudflare-crm",stdio:"inherit"});if(r.status!==0)throw Error("Failed npx "+args.join(" "));}
function paths(dir){return readdirSync(dir).flatMap(n=>{const p=join(dir,n);return statSync(p).isDirectory()?paths(p):[p];});}
function builtRust(){
  const dir="apps/cloudflare-crm/rust-party/build";
  if(!existsSync(join(dir,"index.js")))throw Error("Native Rust Worker build missing");
  const list=paths(dir).filter(p=>p.endsWith(".wasm")||p.endsWith("/index.js"));
  if(!list.some(p=>p.endsWith(".wasm")))throw Error("Native Rust WASM missing");
  return list.map(p=>{const b=readFileSync(p);return{file:relative("apps/cloudflare-crm",p),bytes:b.byteLength,sha256:createHash("sha256").update(b).digest("hex")};});
}
async function version(){
  const d=(await cf(worker+"/deployments")).deployments?.[0]?.versions;
  if(!d||d.length!==1||d[0].percentage!==100)throw Error("CRM must have one 100% Worker deployment");
  return d[0].version_id;
}
async function bindings(deployedVersion){
  // The script settings endpoint reports the latest uploaded version, not the active deployment.
  const b=(await cf(worker+"/versions/"+deployedVersion)).resources?.bindings||[];
  return{sha:b.find(x=>x.name==="GIT_SHA")?.text,db:b.find(x=>x.name==="DB")?.id};
}
// Cloudflare deployments, version metadata and the public edge can converge
// separately after Wrangler deploy. Never treat the first old-version read
// as a final release result; read back the exact source and binding.
async function waitForDeployment(expectedVersion,expectedSha,stage){
  const start=Date.now(),deadline=start+60000;
  let observed={version:null,sha:null,db:null},attempts=0;
  for(;;){
    attempts++;
    try{
      const current=await version();
      const b=await bindings(current);
      observed={version:current,sha:b.sha||null,db:b.db||null};
      if((expectedVersion===null||current===expectedVersion)&&
          b.sha===expectedSha&&b.db===database){
        console.log(JSON.stringify({event:"crm.deployment-readback",
          stage,result:"PASS",worker_version:current,
          source:expectedSha,attempts,elapsed_ms:Date.now()-start}));
        return current;
      }
    }catch(error){observed={error:String(error.message).slice(0,160)};}
    if(Date.now()>=deadline)throw Error("CRM deployment readback mismatch stage="+stage+
      " expected_version="+(expectedVersion||"new")+" expected_sha="+expectedSha+
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
  const r=await fetch(requestUrl,{headers:{...auth(),"cache-control":"no-cache"},
    redirect:"manual",cache:"no-store"});
  const edge={http_status:r.status,ray:r.headers.get("cf-ray"),
    cache_status:r.headers.get("cf-cache-status"),
    runtime:r.headers.get("x-crm-runtime"),
    header_sha:r.headers.get("x-crm-source-sha")};
  if(r.status!==200)throw Error("CRM public version: "+JSON.stringify(edge));
  const body=await r.json();
  return {...edge,source:body.source||null};
}

async function waitForPublicSha(expected,stage){
  // Cloudflare control-plane convergence is separate from workers.dev edge
  // routing. Two independent releases exposed stale edge SHA for >45 seconds.
  // This remains a strict exact-SHA gate with a bounded, observable retry.
  const start=Date.now(),deadline=start+120000;
  let last={error:"NOT_CHECKED"},attempts=0;
  for(;;){
    attempts++;
    try{last=await publicSha();}
    catch(error){last={error:String(error.message).slice(0,220)};}
    if(last.source===expected){
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
  const config=JSON.parse(readFileSync("apps/cloudflare-crm/wrangler.jsonc","utf8"));
  if(config.main!=="./rust-party/build/index.js"||config.workers_dev!==false||config.preview_urls!==false||config.routes?.length)
    throw Error("Refusing release: public Worker is not native workers-rs");
  // Prevent a privacy regression BEFORE changing any live deployment or D1.
  verifyTracePrivacySettings(config.observability);
  const rust=builtRust(), oldVersion=await version(), before=await bindings(oldVersion);
  if(before.db!==database||!/^[a-f0-9]{40}$/.test(before.sha||""))throw Error("Previous CRM binding/SHA mismatch");
  const bookmark=(await cf("/d1/database/"+database+"/time_travel/bookmark")).bookmark;
  if(!bookmark)throw Error("D1 Time Travel bookmark missing");
  // Fail closed before migrations: the deploy job must use exactly the CI-built files.
  const source=readFileSync(join(out,"source-sha.txt"),"utf8").trim();
  if(source!==sha)throw Error("Compiled CRM bundle source SHA mismatch");
  const checked=spawnSync("sha256sum",["--check","--strict",".release/compiled-sha256.txt"],{
    cwd:"apps/cloudflare-crm",stdio:"inherit"
  });
  if(checked.status!==0||!existsSync("apps/cloudflare-crm/dist/index.html"))
    throw Error("Compiled CRM release artifact integrity failed");
  const query=async(sql,params=[])=>{
    const result=await cf("/d1/database/"+database+"/query","POST",{sql,...(params.length?{params}:{})});
    assert.equal(result?.[0]?.success,true);return result[0].results;
  };
  // Canonical files plus the existing D1 ledger: no third schema registry.
  const localMigrations=readdirSync("apps/cloudflare-crm/migrations")
    .filter(name=>name.endsWith(".sql")).sort();
  const appliedMigrations=(await query("SELECT name FROM d1_migrations ORDER BY name"))
    .map(row=>row.name);
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
  const principals=await query("SELECT * FROM crm_users ORDER BY email");
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
    .map(row=>row.name),localMigrations,"D1 must have exactly the canonical migrations");
  assert.deepEqual(await query("SELECT * FROM crm_users ORDER BY email"),principals);
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
  if(resetPending)for(const table of expected.filter(x=>x.type==="table"&&!['crm_users','crm_tenants'].includes(x.name))){
    assert.match(table.name,/^[a-z_][a-z0-9_]*$/);
    assert.equal((await query("SELECT COUNT(*) AS n FROM "+table.name))[0].n,0,"Reset left business data");
  }
  cmd("wrangler","deploy","--var","GIT_SHA:"+sha);
  const newVersion=await waitForDeployment(null,sha,"deploy");
  if(newVersion===oldVersion)throw Error("Worker version did not advance");
  // Read the settings returned by Cloudflare, not an assumed Wrangler default.
  // Do not reopen public entry if trace privacy differs from committed config.
  const traceSettings=verifyTracePrivacySettings((await cf(worker+"/settings")).observability);
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
  let state=null;
  for(let attempt=0;attempt<30;attempt++){
    state=await cf(worker+"/subdomain");
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
  const principal=n=>{
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
  const endpoint="/d1/database/"+database+"/query";
  const query=async(sql,params=[])=>{
    const r=await cf(endpoint,"POST",{sql,params});
    if(!Array.isArray(r)||r.length!==1||r[0].success!==true)
      throw Error("E2E D1 query failed");
    return r[0].results;
  };
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
  const changed=await cf(endpoint,"POST",{batch});
  if(!Array.isArray(changed)||changed.length!==4||
      changed.some(x=>x.success!==true)||changed[2].meta?.changes!==1||
      changed[3].meta?.changes!==1)
    throw Error("E2E explicit service enrollment failed for isolated actors");
  const readback=await query(
    "SELECT email,tenant_id,role,active FROM crm_users WHERE email IN (?,?) ORDER BY email",[a,b]);
  if(readback.length!==2||readback.some(x=>
    x.role!=="admin"||x.active!==1||
    (x.email===a?x.tenant_id!==ta:x.email===b?x.tenant_id!==tb:true)))
    throw Error("E2E enrolled actors do not match the two isolated tenants");
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
      const session=reply.status===200?await reply.json():null;
      let classification;
      try{
        classification=classifySignedSessionReadback({
          status:reply.status,source,session,expectedSha:sha
        });
      }catch(error){
        throw Error("Signed E2E service "+n+" failed Access verification: "+error.message);
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

async function verifyData(){
  const deployed=load("deployed.json");
  assert.equal(deployed.git_sha,sha);
  const liveVersion=await version(),liveBindings=await bindings(liveVersion);
  assert.equal(liveVersion,deployed.worker_version);
  assert.deepEqual(liveBindings,{sha,db:database});
  const traceSettings=verifyTracePrivacySettings((await cf(worker+"/settings")).observability);
  assert.deepEqual(deployed.trace_settings,traceSettings);
  const evidence=JSON.parse(readFileSync("apps/cloudflare-crm/e2e-evidence/typed-browser.json","utf8"));
  assert.equal(evidence.result,"PASS");assert.equal(evidence.source_sha,sha);
  assert.deepEqual(evidence.features,["native"]);
  assert.ok(evidence.started_at>=deployed.started_at);
  const setup=load("e2e-tenants.json");
  assert.equal(evidence.tenant,setup.tenant_a);
  assert.ok(setup.actor_a,"Release must identify its enrolled Access actor");
  assert.ok(evidence.writes.length>0&&evidence.requests.length>0);
  const requests=new Map();
  for(const request of evidence.requests){
    assert.equal(request.source_sha,sha);assert.equal(request.runtime,"native-workers-rs");
    assert.match(request.request_id,/^[a-f0-9-]{36}$/i);
    const prior=requests.get(request.request_id);
    if(prior)assert.deepEqual(prior,request);
    requests.set(request.request_id,request);
  }
  const mutationIds=new Set();
  for(const write of evidence.writes){
    assert.ok(!mutationIds.has(write.request_id),"Duplicate mutation evidence");
    mutationIds.add(write.request_id);
    assert.equal(requests.get(write.request_id)?.path,write.path);
    assert.equal(requests.get(write.request_id)?.status,200);
    const result=await cfReadback("/d1/database/"+database+"/query",{
      sql:"SELECT action,subject_id,actor FROM crm_audit WHERE tenant_id=? AND request_id=? ORDER BY action,subject_id",
      params:[evidence.tenant,write.request_id]});
    assert.equal(result?.[0]?.success,true);
    const sort=rows=>rows.map(r=>[r.action,r.subject_id,r.actor]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
    assert.ok(write.audit.length>0);
    assert.deepEqual(sort(result[0].results),sort(write.audit),"D1 changed since browser assertion");
    for(const row of result[0].results){
      if(row.action.startsWith("parties."))assert.equal(row.actor,setup.actor_a,"Party audit must attribute the enrolled Access actor");
    }
  }
  // The browser executed two distinct recovery semantics: same-key replay
  // without a second commit, and receipt lookup after response-loss/reload.
  // Check the independently persisted audit again; never equate HTTP 200
  // or a Cloudflare D1 span with a committed mutation.
  const recoveryKinds=new Map();
  for(const item of evidence.recovery??[]){
    assert.ok(["replay","lookup"].includes(item.kind),"Unknown recovery evidence");
    assert.ok(!recoveryKinds.has(item.kind),"Duplicate recovery proof");
    const original=requests.get(item.original_request_id);
    const subsequent=requests.get(item.subsequent_request_id);
    assert.ok(original&&subsequent,"Recovery request not observed in signed browser");
    assert.notEqual(item.original_request_id,item.subsequent_request_id,
      "Recovery must be a separate signed request");
    assert.equal(original.status,200);
    assert.equal(subsequent.status,200);
    assert.ok(mutationIds.has(item.original_request_id),"Recovery has no original commit");
    assert.ok(!mutationIds.has(item.subsequent_request_id),"Recovery must not create a new commit");
    if(item.kind==="replay")assert.equal(subsequent.path,original.path,
      "Same-key replay must target the original RPC");
    else assert.ok(subsequent.path.endsWith("/GetDeal"),
      "Receipt reconciliation must use read-only GetDeal");
    const result=await cfReadback("/d1/database/"+database+"/query",{
      sql:"SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND request_id=?",
      params:[evidence.tenant,item.subsequent_request_id]});
    assert.equal(result?.[0]?.success,true);
    assert.equal(Number(result[0].results?.[0]?.n),0,
      "Recovery request must not commit a second audit row");
    recoveryKinds.set(item.kind,item);
  }
  assert.deepEqual([...recoveryKinds.keys()].sort(),["lookup","replay"],
    "Both response-loss recovery paths are mandatory release evidence");
  // S5a adds one independent original Party owner, not a fabricated Deal
  // write. It must prove a committed request and a distinct read-only GetParty.
  const ownerProofs=evidence.owner_recovery??[];
  const ownerContracts={
    party:{write:"/CreateParty",read:"/GetParty",audit:"parties.create"},
    consent:{write:"/SuppressCommunication",read:"/GetConsentAuthorization",audit:"consents.operator_suppress"},
    privacy:{write:"/CreatePrivacyCase",read:"/GetPrivacyCase",audit:"customer_privacy.create"},
    "enrichment-profile":{write:"/PublishProviderProfile",read:"/GetProviderProfile",audit:"customer_enrichment.profile_publish"},
    "enrichment-mapping":{write:"/PublishMapping",read:"/GetMapping",audit:"customer_enrichment.mapping_publish"},
    "party-csv-import":{write:"/ApplyPartyImport",read:"/PreviewPartyImport",audit:"parties.create"}
  };
  assert.deepEqual(ownerProofs.map(proof=>proof.owner).sort(),Object.keys(ownerContracts).sort(),
    "Every active S5 owner must provide one lost-ack/reload receipt proof");
  for(const proof of ownerProofs){
    const contract=ownerContracts[proof.owner];
    assert.equal(proof.kind,"lookup");
    const original=requests.get(proof.original_request_id);
    const read=requests.get(proof.subsequent_request_id);
    assert.ok(original&&read,"Signed owner recovery requests missing: "+proof.owner);
    assert.ok(original.path.endsWith(contract.write)&&read.path.endsWith(contract.read),
      "Owner recovery must use its existing write and read RPC: "+proof.owner);
    assert.equal(original.status,200);assert.equal(read.status,200);
    assert.ok(mutationIds.has(proof.original_request_id),"Owner recovery has no original commit: "+proof.owner);
    assert.ok(!mutationIds.has(proof.subsequent_request_id),"Read-only recovery must not be a mutation: "+proof.owner);
    assert.notEqual(proof.original_request_id,proof.subsequent_request_id);
    const committed=await cfReadback("/d1/database/"+database+"/query",{
      sql:"SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND request_id=? AND action=?",
      params:[evidence.tenant,proof.original_request_id,contract.audit]});
    assert.equal(committed?.[0]?.success,true);
    assert.equal(Number(committed[0].results?.[0]?.n),1,
      "Original owner write must have exactly one matching audit row: "+proof.owner);
    const checked=await cfReadback("/d1/database/"+database+"/query",{
      sql:"SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND request_id=?",
      params:[evidence.tenant,proof.subsequent_request_id]});
    assert.equal(checked?.[0]?.success,true);
    assert.equal(Number(checked[0].results?.[0]?.n),0,
      "Read-only owner recovery must never append audit: "+proof.owner);
  }
  // Synthetic-only business values actually sent by the browser must not
  // appear in the native Cloudflare events of their committed requests.
  // Validate known fixture formats and never log the values in diagnostics.
  const syntheticFormats={
    "party-name":/^Browser Party [0-9]{10,}$/,
    "contact-email":/^browser-[0-9]{10,}@example\.org$/,
    "contact-phone":/^\+12025550102$/,
    "task-subject":/^Browser Task [0-9]{10,}$/,
    "cookie":/^crm-s3-cookie-[0-9]{10,}$/,
    "header":/^crm-s3-header-[0-9]{10,}$/,
    "authorization":/^Bearer crm-s3-authorization-[0-9]{10,}$/,
    "jwt":/^eyJhbGciOiJub25lIn0\.eyJzdWIiOiJjcm0tc3ludGhldGljIn0\.signature-[0-9]{10,}$/,
    "query-string":/^crm-s3-query-[0-9]{10,}$/,
    "enrichment-provider-key":/^browser_registry_[0-9]{10,}$/,
    "enrichment-license":/^Browser test license$/,
    "enrichment-credential-handle":/^browser-credential-handle-[0-9]{10,}$/,
    "enrichment-mapping-key":/^browser_mapping_[0-9]{10,}$/,
    "csv-party-name":/^Browser Import [0-9]{10,}$/,
    "csv-payload":/^party_id,kind,display_name,resource_version\r\nbrowser-import,person,Browser Import [0-9]{10,},1\r\n$/
  };
  const probeValuesByRequest=new Map(),probeKinds=[];
  for(const probe of evidence.synthetic_private_probes??[]){
    assert.ok(syntheticFormats[probe.kind]?.test(probe.value),
      "Only approved synthetic privacy probes may be released: "+probe.kind);
    assert.ok(mutationIds.has(probe.request_id),
      "Synthetic privacy probe must refer to a committed browser request");
    assert.ok(!probeKinds.includes(probe.kind),"Duplicate privacy probe kind");
    probeKinds.push(probe.kind);
    const values=probeValuesByRequest.get(probe.request_id)??[];
    values.push(probe.value);
    probeValuesByRequest.set(probe.request_id,values);
  }
  assert.deepEqual(probeKinds.sort(),Object.keys(syntheticFormats).sort(),
    "Party, email/phone, Task, cookies, Authorization/JWT, query, Enrichment and CSV probes are mandatory");
  // Observe every exact browser request, including rejected requests and read-only RPCs.
  // Audit rows are required only for committed mutations, never invented for reads.
  let pending=[...requests.values()];
  const requestTraces=new Map();
  const from=evidence.started_at-1000;
  // Cloudflare telemetry can become searchable after the browser/D1 readback.
  // A delayed event remains mandatory: extend the bounded indexing window,
  // never weaken the exact source-version/request-id/path/status match below.
  const traceAttempts=120;
  for(let attempt=0;attempt<traceAttempts&&pending.length;attempt++){
    const missing=[];
    for(let i=0;i<pending.length;i+=4){
      const batch=pending.slice(i,i+4);
      const found=await Promise.all(batch.map(async entry=>{
        const data=await cfReadback("/workers/observability/telemetry/query",{
          queryId:"crm-350-typed-browser",view:"events",
          timeframe:{from,to:Date.now()+1000},limit:20,
          parameters:{needle:{value:entry.request_id},filters:[
            {key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
        });
        const matched=(data.events?.events||[]).find(event=>
          event.source?.event==="crm.native.request"&&event.source?.request_id===entry.request_id&&
          event.source?.operation===entry.path&&event.source?.status===entry.status&&
          (event.source?.error_code??null)===(entry.error_code??null)&&
          event.$workers?.scriptVersion?.id===liveVersion);
        if(!matched)return false;
        // Cloudflare's native request log supplies the trace ID, not a
        // handwritten correlation key or a second tracing SDK.
        const traceId=matched.$metadata?.traceId;
        assert.match(traceId??"",/^[a-f0-9]{32}$/i,
          "CRM native request log must carry a valid Cloudflare trace ID");
        requestTraces.set(entry.request_id,traceId);
        return true;
      }));
      batch.forEach((entry,index)=>{if(!found[index])missing.push(entry);});
    }
    pending=missing;
    if(pending.length&&(attempt===0||(attempt+1)%20===0))
      console.log(JSON.stringify({event:"crm.request-trace-readback",result:"WAIT",
        attempt:attempt+1,pending:pending.length,elapsed_ms:Date.now()-evidence.started_at}));
    if(pending.length&&attempt<traceAttempts-1)
      await new Promise(resolve=>setTimeout(resolve,2500));
  }
  assert.equal(pending.length,0,"Missing Cloudflare request traces: "+pending.map(x=>x.path).join(","));
  // Validate the actual trace graph, not only the existence of our console
  // request logs. Representative accepted writes must have D1 spans (their
  // commit fact was already verified against the audit table above). Reads
  // and rejected requests may legitimately have no D1 call.
  const representative=[
    {entry:requests.get(evidence.writes[0].request_id),needsD1:true},
    ...[...recoveryKinds.values()].flatMap(item=>[
      {entry:requests.get(item.original_request_id),needsD1:true},
      {entry:requests.get(item.subsequent_request_id),needsD1:true}
    ]),
    ...[...probeValuesByRequest.keys()].map(id=>({entry:requests.get(id),needsD1:true})),
    ...ownerProofs.flatMap(item=>[
      {entry:requests.get(item.original_request_id),needsD1:true},
      {entry:requests.get(item.subsequent_request_id),needsD1:true}
    ]),
    ...[
      [...requests.values()].find(e=>!mutationIds.has(e.request_id)&&e.status===200),
      [...requests.values()].find(e=>e.status===400),
      [...requests.values()].find(e=>e.status===403),
      [...requests.values()].find(e=>e.status===409)
    ].map((entry,index)=>{
      assert.ok(entry,["read-only","validation rejection","authorization denial","CAS conflict"][index]+
        " request must be observed in signed browser evidence");
      return {entry,needsD1:false};
    })
  ];
  // A recovery scenario is proved only by distinct Cloudflare traces for
  // its original and follow-up requests. Do not pass on the same trace twice.
  for(const item of [...recoveryKinds.values(),...ownerProofs])
    assert.notEqual(requestTraces.get(item.original_request_id),
      requestTraces.get(item.subsequent_request_id),
      "Recovery reused the original Cloudflare trace ID");
  const required=new Map();
  for(const candidate of representative){
    const prior=required.get(candidate.entry.request_id);
    required.set(candidate.entry.request_id,{
      entry:candidate.entry,needsD1:candidate.needsD1||(prior?.needsD1??false)
    });
  }
  let spanVerified=0,d1SpanVerified=0;
  for(const {entry,needsD1} of required.values()){
    const traceId=requestTraces.get(entry.request_id);
    assert.match(traceId??"",/^[a-f0-9]{32}$/i,
      "Accepted browser request missing its Cloudflare trace ID");
    let verified=false;
    for(let attempt=0;attempt<30;attempt++){
      const data=await cfReadback("/workers/observability/telemetry/query",{
        queryId:"crm-471-native-trace-span-evidence",view:"events",
        timeframe:{from,to:Date.now()+1000},limit:100,
        parameters:{needle:{value:traceId},filters:[
          {key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
      });
      const linked=(data.events?.events||[]).filter(event=>
        (event.source?.traceId??event.$metadata?.traceId)===traceId);
      const eventsText=JSON.stringify(linked);
      for(const marker of probeValuesByRequest.get(entry.request_id)??[])
        assert.ok(!eventsText.includes(marker),
          "A synthetic private value appeared in a Cloudflare trace");
      assert.ok(!linked.some(event=>event.$metadata?.type==="cf-worker-event"),
        "Unsafe automatic invocation log in an accepted CRM trace");
      const spans=linked.filter(event=>event.$metadata?.type==="span");
      if(spans.length>0&&(!needsD1||spans.some(event=>event.source?.db))){
        spanVerified++;
        if(needsD1)d1SpanVerified++;
        verified=true;
        break;
      }
      if(attempt<29)await new Promise(resolve=>setTimeout(resolve,2500));
    }
    assert.ok(verified,needsD1?
      "Committed CRM mutation has no native Cloudflare D1 span":
      "CRM request has no native Cloudflare span");
  }
  assert.ok(d1SpanVerified>=1,"D1 trace proof must cover a committed mutation");
  console.log(JSON.stringify({event:"crm.native-s3-trace-to-d1",
    result:"PASS",traces:spanVerified,committed_d1_traces:d1SpanVerified,
    recovery_paths:[...recoveryKinds.keys()].sort(),owner_recovery_proofs:ownerProofs.length,
    synthetic_private_probes:probeKinds.length,
    automatic_invocation_logs:false}));
  assert.equal(await version(),liveVersion,"Deployment changed during acceptance");
  assert.deepEqual(await bindings(liveVersion),{sha,db:database});
  await waitForPublicSha(sha,"final");
  record("accepted.json",{result:"PASS",git_sha:sha,worker_version:liveVersion,
    d1_database:database,features:["native"],traffic_percentage:100,
    mutations:mutationIds.size,logs_correlated:requests.size,
    spans_correlated:spanVerified,d1_mutation_spans:d1SpanVerified,
    recovery_paths:[...recoveryKinds.keys()].sort(),owner_recovery_proofs:ownerProofs.length,
    synthetic_private_probes:probeKinds.length,
    trace_settings:traceSettings,steps:evidence.steps,
    request_ids:[...requests.keys()]});
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
    const query=async(sql)=>{
      const rows=await cf("/d1/database/"+database+"/query","POST",{
        sql,params:[setup.actor_a,setup.tenant_a]});
      assert.equal(rows?.[0]?.success,true);return rows[0].results;
    };
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

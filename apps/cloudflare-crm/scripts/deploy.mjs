// One protected GitHub Actions release owner: deploy, verify-data, fail closed.
import {existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from "node:fs";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {join,relative} from "node:path";
import {spawnSync} from "node:child_process";
import {DatabaseSync} from "node:sqlite";
const required=n=>{if(!process.env[n])throw Error("Missing "+n);return process.env[n];};
const sha=required("GITHUB_SHA");
if(!/^[0-9a-f]{40}$/.test(sha)||process.env.GITHUB_REF!=="refs/heads/dev"||process.env.GITHUB_EVENT_NAME!=="push")throw Error("Untrusted deployment");
const account="5bd5e1e1e813f061afc259e91334262d", database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
if(required("CLOUDFLARE_ACCOUNT_ID")!==account||required("CRM_D1_DATABASE_ID")!==database)throw Error("Wrong Cloudflare CRM target");
const token=required("CLOUDFLARE_API_TOKEN"),api="https://api.cloudflare.com/client/v4/accounts/"+account;
const worker="/workers/scripts/crm-dev",url="https://crm-dev.crm-5bd5e1e1.workers.dev",out="apps/cloudflare-crm/.release";
const record=(name,v)=>{mkdirSync(out,{recursive:true});writeFileSync(join(out,name),JSON.stringify(v,null,2)+"\n");};
const load=name=>JSON.parse(readFileSync(join(out,name),"utf8"));
async function cf(path,method="GET",body){
  const r=await fetch(api+path,{method,headers:{authorization:"Bearer "+token,...(body?{"content-type":"application/json"}:{})},...(body?{body:JSON.stringify(body)}:{})});
  const j=await r.json();
  if(!r.ok||!j.success)throw Error("Cloudflare "+method+" "+path+" HTTP "+r.status+" codes="+JSON.stringify((j.errors||[]).map(e=>e.code)));
  return j.result;
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
  const query=async(sql)=>{
    const result=await cf("/d1/database/"+database+"/query","POST",{sql});
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
  await cf(worker+"/subdomain","POST",{enabled:true,previews_enabled:false});
  await waitForPublicSha(sha,"deploy");
  // Signed exact-SHA /api/version above already establishes Access plus Worker
  // availability. Require the same endpoint to reject anonymous requests.
  const anonymous=await fetch(url+"/api/version",{redirect:"manual",cache:"no-store"});
  if(anonymous.status===200||anonymous.status>=500)
    throw Error("Cloudflare Access anonymous denial failed: HTTP "+anonymous.status+
      " ray="+(anonymous.headers.get("cf-ray")||"missing"));
  record("deployed.json",{git_sha:sha,worker_version:newVersion,d1_database:database,rust,started_at:startedAt,result:"E2E_PENDING"});
  console.log(JSON.stringify({event:"crm.deploy",sha,result:"E2E_PENDING",version:newVersion}));
}
async function confirmClosed(){
  const state=await cf(worker+"/subdomain");
  assert.equal(state.enabled,false);assert.equal(state.previews_enabled,false);
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
  // A clean D1 has no pre-enrolled service actors. Enroll through the same
  // signed Cloudflare Access entrypoint as production, not with forged DB roles.
  for(const n of ["A","B"]){
    const id=required("CRM_E2E_"+n+"_CLIENT_ID");
    const secret=required("CRM_E2E_"+n+"_CLIENT_SECRET");
    const reply=await fetch(url+"/api/session",{
      headers:{"CF-Access-Client-Id":id,"CF-Access-Client-Secret":secret},
      redirect:"manual",cache:"no-store"});
    if(reply.status!==200)
      throw Error("Signed E2E service principal "+n+" failed to enroll: HTTP "+reply.status);
  }
  const endpoint="/d1/database/"+database+"/query";
  const query=async(sql,params)=>{
    const r=await cf(endpoint,"POST",{sql,params});
    if(!Array.isArray(r)||r.length!==1||r[0].success!==true)
      throw Error("E2E D1 query failed");
    return r[0].results;
  };
  const current=await query(
    "SELECT email,role,active FROM crm_users WHERE email IN (?,?)",[a,b]);
  if(current.length!==2||current.some(r=>r.role!=="admin"||r.active!==1))
    throw Error("Protected E2E service actors are not enrolled active administrators");
  const ta="crm-e2e-"+run+"-"+attempt+"-a",tb="crm-e2e-"+run+"-"+attempt+"-b";
  if(ta===tb)throw Error("E2E tenants must differ");
  const batch=[
    {sql:"INSERT OR IGNORE INTO crm_tenants(id,name) VALUES (?,?)",
      params:[ta,"CRM protected E2E A"]},
    {sql:"INSERT OR IGNORE INTO crm_tenants(id,name) VALUES (?,?)",
      params:[tb,"CRM protected E2E B"]},
    {sql:"UPDATE crm_users SET tenant_id=? WHERE email=? AND role='admin' AND active=1",
      params:[ta,a]},
    {sql:"UPDATE crm_users SET tenant_id=? WHERE email=? AND role='admin' AND active=1",
      params:[tb,b]}
  ];
  const changed=await cf(endpoint,"POST",{batch});
  if(!Array.isArray(changed)||changed.length!==4||
      changed.some(x=>x.success!==true)||changed[2].meta?.changes!==1||
      changed[3].meta?.changes!==1)
    throw Error("E2E tenant assignment did not update exactly two service actors");
  const readback=await query(
    "SELECT COUNT(*) AS assigned FROM crm_users WHERE (email=? AND tenant_id=?) OR (email=? AND tenant_id=?)",
    [a,ta,b,tb]);
  const clean=await query(
    "SELECT COUNT(*) AS existing FROM crm_party_records WHERE tenant IN (?,?)",[ta,tb]);
  if(readback[0]?.assigned!==2||clean[0]?.existing!==0)
    throw Error("E2E tenant assignment/readback failed or contains old Party data");
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
  const evidence=JSON.parse(readFileSync("apps/cloudflare-crm/e2e-evidence/typed-browser.json","utf8"));
  assert.equal(evidence.result,"PASS");assert.equal(evidence.source_sha,sha);
  assert.deepEqual(evidence.features,["native"]);
  assert.ok(evidence.started_at>=deployed.started_at);
  const setup=load("e2e-tenants.json");
  assert.equal(evidence.tenant,setup.tenant_a);
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
    const result=await cf("/d1/database/"+database+"/query","POST",{
      sql:"SELECT action,subject_id FROM crm_audit WHERE tenant_id=? AND request_id=? ORDER BY action,subject_id",
      params:[evidence.tenant,write.request_id]});
    assert.equal(result?.[0]?.success,true);
    const sort=rows=>rows.map(r=>[r.action,r.subject_id]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
    assert.ok(write.audit.length>0);
    assert.deepEqual(sort(result[0].results),sort(write.audit),"D1 changed since browser assertion");
  }
  // Observe every exact browser request, including rejected requests and read-only RPCs.
  // Audit rows are required only for committed mutations, never invented for reads.
  let pending=[...requests.values()];
  const from=evidence.started_at-1000;
  for(let attempt=0;attempt<60&&pending.length;attempt++){
    const missing=[];
    for(let i=0;i<pending.length;i+=4){
      const batch=pending.slice(i,i+4);
      const found=await Promise.all(batch.map(async entry=>{
        const data=await cf("/workers/observability/telemetry/query","POST",{
          queryId:"crm-350-typed-browser",view:"events",
          timeframe:{from,to:Date.now()+1000},limit:20,
          parameters:{needle:{value:entry.request_id},filters:[
            {key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
        });
        return (data.events?.events||[]).some(event=>
          event.source?.event==="crm.native.request"&&event.source?.request_id===entry.request_id&&
          event.source?.operation===entry.path&&event.source?.status===entry.status&&
          (event.source?.error_code??null)===(entry.error_code??null)&&
          event.$workers?.scriptVersion?.id===liveVersion);
      }));
      batch.forEach((entry,index)=>{if(!found[index])missing.push(entry);});
    }
    pending=missing;
    if(pending.length&&attempt<59)await new Promise(resolve=>setTimeout(resolve,2500));
  }
  assert.equal(pending.length,0,"Missing Cloudflare request traces: "+pending.map(x=>x.path).join(","));
  assert.equal(await version(),liveVersion,"Deployment changed during acceptance");
  assert.deepEqual(await bindings(liveVersion),{sha,db:database});
  await waitForPublicSha(sha,"final");
  record("accepted.json",{result:"PASS",git_sha:sha,worker_version:liveVersion,
    d1_database:database,features:["native"],traffic_percentage:100,
    mutations:mutationIds.size,logs_correlated:requests.size,steps:evidence.steps,
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

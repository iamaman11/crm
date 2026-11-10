// One protected GitHub Actions release owner: deploy, verify-data, rollback.
import {existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from "node:fs";
import {createHash} from "node:crypto";
import {join,relative} from "node:path";
import {spawnSync} from "node:child_process";
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
// as a failed release or a reason to skip rollback.
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
  const config=JSON.parse(readFileSync("apps/cloudflare-crm/wrangler.jsonc","utf8"));
  if(config.main!=="./rust-party/build/index.js")
    throw Error("Refusing release: public Worker is not native workers-rs");
  const rust=builtRust(), oldVersion=await version(), before=await bindings(oldVersion);
  if(before.db!==database||!/^[a-f0-9]{40}$/.test(before.sha||""))throw Error("Previous CRM binding/SHA mismatch");
  const bookmark=(await cf("/d1/database/"+database+"/time_travel/bookmark")).bookmark;
  if(!bookmark)throw Error("D1 Time Travel bookmark missing");
  record("checkpoint.json",{git_sha:sha,previous_sha:before.sha,previous_version:oldVersion,d1_database:database,pre_migration_bookmark:bookmark,rust});
  console.log(JSON.stringify({event:"crm.release.checkpoint",git_sha:sha,previous_sha:before.sha,previous_version:oldVersion,bookmark_present:true,rust}));
  // Fail closed before migrations: the deploy job must use exactly the CI-built files.
  const source=readFileSync(join(out,"source-sha.txt"),"utf8").trim();
  if(source!==sha)throw Error("Compiled CRM bundle source SHA mismatch");
  const checked=spawnSync("sha256sum",["--check","--strict",".release/compiled-sha256.txt"],{
    cwd:"apps/cloudflare-crm",stdio:"inherit"
  });
  if(checked.status!==0||!existsSync("apps/cloudflare-crm/dist/index.html"))
    throw Error("Compiled CRM release artifact integrity failed");
  cmd("wrangler","d1","migrations","apply","crm-dev","--remote");
  cmd("wrangler","deploy","--var","GIT_SHA:"+sha);
  const newVersion=await waitForDeployment(null,sha,"deploy");
  if(newVersion===oldVersion)throw Error("Worker version did not advance");
  const anonymous=await fetch(url+"/api/health",{redirect:"manual"});
  if(anonymous.status===200||anonymous.status>=500)throw Error("Cloudflare Access denied check failed");
  const authorized=await fetch(url+"/api/health",{headers:auth(),redirect:"manual"});
  if(authorized.status!==200)throw Error("Cloudflare Access authorized health failed");
  await waitForPublicSha(sha,"deploy");
  record("deployed.json",{git_sha:sha,worker_version:newVersion,d1_database:database,rust,result:"E2E_PENDING"});
  console.log(JSON.stringify({event:"crm.deploy",sha,result:"E2E_PENDING",version:newVersion}));
}
// Each protected E2E deploy gets clean, isolated tenants IN THE SAME D1.
// The two dedicated Access service identities alone are reassigned; nothing
// belonging to another tenant is deleted or modified. Old test data is retained
// for audit instead of being silently destroyed to make an E2E gate green.
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
  console.log(JSON.stringify({event:"crm.e2e-tenant-isolation",
    result:"PASS",database:"existing D1",actors:2,tenants:2,previous_data_deleted:false}));
}

async function verifyContractOnly(){
  const liveVersion=await version();
  const b=await bindings(liveVersion);
  if(b.sha!==sha||b.db!==database)throw Error("Contract-only live SHA/D1 mismatch");
  const e=load("contract-only-evidence.json");
  if(e.git_sha!==sha||e.phase!=="contract-only"||
    e.result!=="BROWSER_FAIL_CLOSED_PASS_LOGS_PENDING"||
    !Array.isArray(e.requests)||e.requests.length<12||
    e.requests.some(r=>r.status!==410||!/^[0-9a-f-]{36}$/i.test(r.request_id||""))||
    e.rpc_read?.method!=="/crm.parties.v2.PartyService/GetParty"||
    e.rpc_read?.status!==404)
    throw Error("Contract-only physical browser/Access/Connect evidence invalid");
  const run=required("GITHUB_RUN_ID"),attempt=required("GITHUB_RUN_ATTEMPT");
  if(!/^\d+$/.test(run)||!/^\d+$/.test(attempt))throw Error("E2E tenant names unsafe");
  const ta="crm-e2e-"+run+"-"+attempt+"-a";
  const tb="crm-e2e-"+run+"-"+attempt+"-b";
  const audited=await cf("/d1/database/"+database+"/query","POST",{
    sql:"SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id IN (?,?)",
    params:[ta,tb]
  });
  const persisted=await cf("/d1/database/"+database+"/query","POST",{
    sql:"SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant IN (?,?)",
    params:[ta,tb]
  });
  if(audited?.[0]?.results?.[0]?.n!==0||persisted?.[0]?.results?.[0]?.n!==0)
    throw Error("DISABLED legacy API created real D1 mutation");
  const samples=e.requests.filter(r=>r.operation).slice(0,2);
  if(samples.length!==2)throw Error("no distinct verified operations for Logs");
  const pending=[...samples];
  const start=Date.now();
  for(let attemptNo=0;attemptNo<60&&pending.length;attemptNo++){
    const data=await cf("/workers/observability/telemetry/query","POST",{
      queryId:"crm-350-contract-only-no-legacy",
      view:"events",timeframe:{from:start-20*60*1000,to:Date.now()+1000},limit:100,
      parameters:{needle:{value:"crm.native.request"},
        filters:[{key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
    });
    const events=data.events?.events||[];
    for(let i=pending.length-1;i>=0;i--){
      const item=pending[i];
      if(events.some(x=>x.source?.event==="crm.native.request"&&
          x.source?.request_id===item.request_id&&
          x.source?.operation===item.operation&&
          x.source?.status===410&&x.$workers?.scriptVersion?.id===liveVersion))
        pending.splice(i,1);
    }
    if(pending.length&&attemptNo<59)
      await new Promise(resolve=>setTimeout(resolve,2500));
  }
  if(pending.length)throw Error("Contract-only Cloudflare Logs exact denied request IDs missing "+
    JSON.stringify(pending));
  await waitForPublicSha(sha,"final");
  record("accepted.json",{git_sha:sha,result:"PASS",phase:"contract-only",
    active_worker_version:liveVersion,legacy_writes:0,denied_calls:e.requests.length,
    traces:2,d1_database:database});
  console.log(JSON.stringify({event:"crm.contract-only-live-acceptance",result:"PASS",
    source:sha,worker_version:liveVersion,legacy_writes:0,
    blocked:e.requests.length,verified_d1:true,verified_cloudflare_logs:true}));
}

async function verifyData(){
  if(JSON.parse(readFileSync("apps/cloudflare-crm/architecture-policy.json","utf8")).phase==="contract-only")return verifyContractOnly();
  const main=await cf(worker+"/deployments");
  const versions=main.deployments?.[0]?.versions||[];
  if(versions.length!==1||versions[0].percentage!==100)
    throw Error("Native release requires exactly 100% normal traffic");
  const liveVersion=versions[0].version_id;
  const nativeBindings=await bindings(liveVersion);
  if(nativeBindings.sha!==sha||nativeBindings.db!==database)
    throw Error("Live native version SHA/D1 mismatch");
  const click=load("native-click.json");
  if(click.git_sha!==sha||click.action!=="parties.create"||click.status!==201||
    !/^[0-9a-f-]{36}$/i.test(click.request_id||""))
    throw Error("Native browser click evidence missing");
  const check=await cf("/d1/database/"+database+"/query","POST",{
    sql:"SELECT action, request_id FROM crm_audit WHERE subject_id = ? AND action = 'parties.create' ORDER BY created_at DESC LIMIT 1",
    params:[click.party_id]});
  const saved=check?.[0]?.results?.[0];
  if(saved?.request_id!==click.request_id)
    throw Error("D1 Party create audit does not match the real browser response request_id");
  const start=Date.now();
  let observed=false;
  for(let attempt=0;attempt<12&&!observed;attempt++){
    const data=await cf("/workers/observability/telemetry/query","POST",{
      queryId:"crm-338-native-main-cutover",
      view:"events",timeframe:{from:start-10*60*1000,to:Date.now()+1000},limit:20,
      parameters:{needle:{value:click.request_id},
        filters:[{key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
    });
    observed=(data.events?.events||[]).some(e=>e.source?.event==="crm.native.request"&&
      e.source?.request_id===click.request_id&&e.source?.operation==="parties.create"&&
      e.source?.status===201&&e.$workers?.scriptVersion?.id===liveVersion);
    if(!observed)await new Promise(resolve=>setTimeout(resolve,2500));
  }
  if(!observed)throw Error("Live native Chromium -> response -> D1 -> Cloudflare Logs correlation FAILED");
  console.log(JSON.stringify({event:"crm.native-live-trace",result:"PASS",
    source:sha,worker_version:liveVersion,request_id:click.request_id}));

  // Real user click evidence across ALL current React modules, not just Party.
  // A green infrastructure doctor alone must never approve this release.
  const cycle=load("native-functional-cycle.json");
  if(cycle.git_sha!==sha||cycle.runtime!=="native-workers-rs"||
    cycle.result!=="BROWSER_PASS_D1_LOGS_PENDING"||!Array.isArray(cycle.writes)||
    !Array.isArray(cycle.ui_coverage)||cycle.ui_coverage.length<18)
    throw Error("Exact-source native UI functional-cycle evidence missing");
  const writes=cycle.writes,expected=["customer-enrichment","customer-privacy","parties","party-relationships","consents",
    "accounts","contact-points","sales-deals","activity-tasks"];
  const required=["customer-enrichment.profile_publish","customer-enrichment.mapping_publish",
    "customer-privacy.create","customer-privacy.submit",
    "customer-privacy.cancel","parties.create","parties.update",
    "party-relationships.create","party-relationships.update","consents.suppress",
    "accounts.create","accounts.update","contact-points.create","contact-points.update",
    "sales-deals.create","sales-deals.update","sales-deals.advance",
    "activity-tasks.create","activity-tasks.complete"];
  if(writes.length<required.length)throw Error("Original native UI mutation proof incomplete");
  for(const action of required){
    if(!writes.some(w=>w.operation===action)||!cycle.ui_coverage.includes(action))
      throw Error("Missing original Rust owner browser operation "+action);
  }
  const seen=new Set();
  for(const entry of writes){
    if(entry.source_sha!==sha||!expected.includes(entry.module)||
      !["create","update","advance","complete","suppress","submit","cancel","profile_publish","mapping_publish"].includes(entry.operation.split(".")[1])||
      !entry.subject_id||
      !/^[0-9a-f-]{36}$/i.test(entry.request_id||"")||
      seen.has(entry.request_id))throw Error("Invalid or duplicated native UI mutation evidence");
    seen.add(entry.request_id);
  }
  // The read-only Identity Resolution click must be traced in Cloudflare
  // Logs, but must NOT be fabricated as a D1 mutation/audit record.
  const enrichProfileRead=cycle.requests?.find(r=>r.operation==="customer-enrichment.profiles_list"&&
    r.method==="GET"&&r.status===200&&r.module==="customer-enrichment");
  const enrichMappingRead=cycle.requests?.find(r=>r.operation==="customer-enrichment.mappings_list"&&
    r.method==="GET"&&r.status===200&&r.module==="customer-enrichment");
  if(!cycle.ui_coverage.includes("customer-enrichment.profile_publish")||
    !cycle.ui_coverage.includes("customer-enrichment.mapping_publish")||
    !enrichProfileRead||!enrichMappingRead||
    ![enrichProfileRead,enrichMappingRead].every(r=>r.source_sha===sha&&
      /^[0-9a-f-]{36}$/i.test(r.request_id||"")))
    throw Error("Customer Enrichment provider/mapping readback missing");
  const readOnly=cycle.requests?.filter(r=>
    r.operation==="identity-resolution.compare"&&r.method==="GET"&&
    r.status===200&&r.module==="identity-resolution")||[];
  const compareRead=readOnly[0];
  const importPreview=cycle.requests?.find(r=>
    r.operation==="customer-data-operations.import-preview"&&r.method==="POST"&&
    r.status===200&&r.module==="customer-data-operations");
  const importApply=cycle.requests?.find(r=>
    r.operation==="customer-data-operations.import-apply"&&r.method==="POST"&&
    r.status===201&&r.module==="customer-data-operations");
  const exportPreview=cycle.requests?.find(r=>
    r.operation==="customer-data-operations.preview"&&r.method==="GET"&&
    r.status===200&&r.module==="customer-data-operations");
  const exportDownload=cycle.requests?.find(r=>
    r.operation==="customer-data-operations.export"&&r.method==="GET"&&
    r.status===200&&r.module==="customer-data-operations");
  const qualityRead=cycle.requests?.find(r=>
    r.operation==="data-quality.evaluate"&&r.method==="GET"&&
    r.status===200&&r.module==="data-quality");
  const candidateRead=cycle.requests?.find(r=>
    r.operation==="identity-resolution.candidates"&&r.method==="GET"&&
    r.status===200&&r.module==="identity-resolution");
  if(!cycle.ui_coverage.includes("identity-resolution.compare")||
    !compareRead||compareRead.source_sha!==sha||
    !/^[0-9a-f-]{36}$/i.test(compareRead.request_id||""))
    throw Error("Identity Resolution read-only Chromium trace missing");
  if(!cycle.ui_coverage.includes("identity-resolution.candidates")||
    !candidateRead||candidateRead.source_sha!==sha||
    !/^[0-9a-f-]{36}$/i.test(candidateRead.request_id||""))
    throw Error("Identity Resolution candidates live Chromium trace missing");

  if(!cycle.ui_coverage.includes("data-quality.evaluate.pass")||
    !cycle.ui_coverage.includes("data-quality.evaluate.placeholder")||
    !qualityRead||qualityRead.source_sha!==sha||
    !/^[0-9a-f-]{36}$/i.test(qualityRead.request_id||""))
    throw Error("Data Quality original Rust Chromium review trace missing");

  if(!cycle.ui_coverage.includes("customer-data-operations.preview")||
    !cycle.ui_coverage.includes("customer-data-operations.export.download")||
    !exportPreview||!exportDownload||
    ![exportPreview,exportDownload].every(r=>r.source_sha===sha&&
      /^[0-9a-f-]{36}$/i.test(r.request_id||"")))
    throw Error("Original Rust Party CSV export physical Chromium trace missing");

  if(!cycle.ui_coverage.includes("customer-data-operations.import-preview")||
    !cycle.ui_coverage.includes("customer-data-operations.import-apply")||
    !importPreview||!importApply||
    ![importPreview,importApply].every(r=>r.source_sha===sha&&
      /^[0-9a-f-]{36}$/i.test(r.request_id||"")))
    throw Error("Native Party CSV import browser trace missing");

  // Import-apply is a genuine D1 mutation with potentially many audit rows,
  // never count it as a read-only operation. One-row Chromium fixture must
  // have exactly one committed original Party create for the same request_id.
  const importedRows=await cf("/d1/database/"+database+"/query","POST",{
    sql:"SELECT action,subject_id FROM crm_audit WHERE request_id = ?",
    params:[importApply.request_id]});
  const importedAudit=importedRows?.[0]?.results||[];
  if(!cycle.imported_party_id||importedAudit.filter(r=>
    r.action==="parties.create"&&r.subject_id===cycle.imported_party_id).length!==1)
    throw Error("Original Party import D1 audit missing for exact browser request");

  // D1's audit is transactionally written by the same native mutation and
  // uses the exact x-request-id the real Chromium click received.
  const ids=writes.map(w=>w.request_id);
  const query=await cf("/d1/database/"+database+"/query","POST",{
    sql:"SELECT request_id,action,subject_id FROM crm_audit WHERE request_id IN ("+
      ids.map(()=>"?").join(",")+")",params:ids});
  const audit=query?.[0]?.results||[];
  for(const entry of writes){
    // An original Sales advance may atomically create a linked Activity and
    // thus more than one audit row for a single request_id. Match exact
    // owner/action/subject without hiding the additional original event.
    // Rust Consent records audit "operator_suppress", not a fictitious Grant.
    const action=entry.operation==="consents.suppress"?
      "consents.operator_suppress":entry.operation.replaceAll("-","_");
    const matched=audit.filter(row=>row.request_id===entry.request_id&&
      row.action===action&&row.subject_id===entry.subject_id);
    if(matched.length!==1)
      throw Error("Native D1 audit mismatch for "+action+" request "+entry.request_id+
        " matches="+matched.length);
  }
  record("native-functional-d1.json",{git_sha:sha,worker_version:liveVersion,
    result:"PASS",writes:writes.length,audit_rows:writes.length,
    ui_buttons:cycle.ui_coverage.length});
  // Query existing native structured Cloudflare Logs; no new monitoring
  // infrastructure. Probe each exact original owner mutation,
  // and require the version serving 100% of ordinary traffic.
  let pending=[...writes,compareRead,candidateRead,qualityRead,exportPreview,exportDownload,
    importPreview,enrichProfileRead,enrichMappingRead,importApply];
  const from=Date.now()-15*60*1000;
  // Workers Observability delivery is eventually consistent. Preserve the
  // exact all-request_id gate; only allow bounded ingestion time after E2E.
  // Previous 12 * 2.5s polls rejected a valid 24/24 D1 run when one
  // accounts.create event arrived in Logs after the 30s window.
  const telemetryAttempts=60;
  for(let attempt=0;attempt<telemetryAttempts&&pending.length;attempt++){
    const missing=[];
    for(let i=0;i<pending.length;i+=4){
      const batch=pending.slice(i,i+4);
      const results=await Promise.all(batch.map(async entry=>{
        const data=await cf("/workers/observability/telemetry/query","POST",{
          queryId:"crm-338-full-native-browser-cycle",view:"events",
          timeframe:{from,to:Date.now()+1000},limit:20,
          parameters:{needle:{value:entry.request_id},
            filters:[{key:"$workers.scriptName",operation:"eq",type:"string",value:"crm-dev"}]}
        });
        return (data.events?.events||[]).some(event=>
          event.source?.event==="crm.native.request"&&
          event.source?.request_id===entry.request_id&&
          event.source?.operation===entry.operation&&
          event.source?.status===entry.status&&
          event.$workers?.scriptVersion?.id===liveVersion);
      }));
      for(let n=0;n<batch.length;n++)if(!results[n])missing.push(batch[n]);
    }
    pending=missing;
    if(pending.length && attempt+1<telemetryAttempts)
      await new Promise(resolve=>setTimeout(resolve,2500));
  }
  if(pending.length)throw Error("Native Cloudflare Logs missing "+pending.length+
    " exact browser request(s): "+pending.map(x=>x.operation).join(","));
  record("native-functional-accepted.json",{git_sha:sha,runtime:"native-workers-rs",
    worker_version:liveVersion,database,ui_buttons:cycle.ui_coverage,
    mutations:writes.length,d1_correlated:writes.length,logs_correlated:writes.length,
    request_ids:ids,read_only_log_correlated:8,
    read_only_request_ids:[compareRead.request_id,candidateRead.request_id,
      qualityRead.request_id,exportPreview.request_id,exportDownload.request_id,
      importPreview.request_id,enrichProfileRead.request_id,enrichMappingRead.request_id],
    import_request_id:importApply.request_id,import_d1_correlated:1,import_log_correlated:1,
    result:"PASS"});
  console.log(JSON.stringify({event:"crm.native-full-functional-cycle",result:"PASS",
    sha,worker_version:liveVersion,ui_buttons:cycle.ui_coverage.length,
    mutations:writes.length,d1_correlated:writes.length,
    logs_correlated:writes.length,read_only_log_correlated:8,import_write_log_correlated:1}));

  const p=load("parties-e2e.json");
  if(p.git_sha!==sha||p.parties?.length!==2||p.keys?.length!==3)throw Error("Missing native Party A/B browser proof");
  const [a,b]=p.parties,[ka,ku,kb]=p.keys;
  const sql="SELECT (SELECT COUNT(*) FROM crm_party_records WHERE id IN (?,?)) AS records, "+
    "(SELECT COUNT(*) FROM crm_party_receipts WHERE key IN (?,?,?)) AS receipts, "+
    "(SELECT COUNT(*) FROM crm_audit WHERE subject_id IN (?,?) AND action LIKE 'parties.%') AS audits, "+
    "(SELECT COUNT(*) FROM d1_migrations WHERE name='0002_rust_party.sql') AS migration";
  const result=await cf("/d1/database/"+database+"/query","POST",{sql,params:[a,b,ka,ku,kb,a,b]});
  const row=result?.[0]?.results?.[0];
  if(!row||row.records!==2||row.receipts!==3||row.audits!==3||row.migration!==1)throw Error("Rust Party D1 audit/receipt/schema mismatch "+JSON.stringify(row));
  const v=await cf("/d1/database/"+database+"/query","POST",{sql:"SELECT id,version FROM crm_party_records WHERE id IN (?,?)",params:[a,b]});
  const values=v?.[0]?.results||[];
  if(values.length!==2||values.find(x=>x.id===a)?.version!==2||values.find(x=>x.id===b)?.version!==1)throw Error("Rust Party D1 persisted versions mismatch");
  await waitForPublicSha(sha,"final");
  record("accepted.json",{git_sha:sha,d1_database:database,runtime:"native-workers-rs",
    active_worker_version:liveVersion,trace_request_id:click.request_id,
    records:2,receipts:3,audits:3,migration:1,versions:"A=2 B=1",result:"PASS"});
  console.log(JSON.stringify({event:"crm.native-party-live-d1",sha,result:"PASS",records:2,receipts:3,audits:3,migration:1}));
}
async function rollback(){
  if(!existsSync(join(out,"checkpoint.json"))){console.log(JSON.stringify({event:"crm.rollback",result:"SKIP_NO_CHECKPOINT"}));return;}
  const p=load("checkpoint.json");
  if(p.git_sha!==sha||p.d1_database!==database)throw Error("Rollback checkpoint mismatch");
  const live=await version();
  const now=await bindings(live);
  // After a failed readback the first deployment response may still be stale
  // and report the *old* Worker even though the new upload is propagating.
  // Explicitly redeploy the checkpointed old version in both cases.
  if(now.db!==database||
      !((live===p.previous_version&&now.sha===p.previous_sha)||now.sha===sha))
    throw Error("Refusing rollback over unrelated deployment");
  await cf(worker+"/deployments","POST",{strategy:"percentage",
    versions:[{version_id:p.previous_version,percentage:100}],
    annotations:{"workers/message":"CRM failed-release rollback "+sha.slice(0,12)}});
  await waitForDeployment(p.previous_version,p.previous_sha,"rollback");
  await waitForPublicSha(p.previous_sha,"rollback");
  record("rollback.json",{git_sha:sha,result:"WORKER_RESTORED",previous_sha:p.previous_sha,previous_version:p.previous_version,d1_restored:false});
  console.log(JSON.stringify({event:"crm.rollback",result:"WORKER_RESTORED",old_sha:p.previous_sha,d1_restored:false}));
}
const mode=process.argv[2]||"deploy";
if(mode==="deploy")await deploy();
else if(mode==="prepare-e2e")await prepareE2e();
else if(mode==="verify-data")await verifyData();
else if(mode==="rollback")await rollback();
else throw Error("Unknown CRM deploy operation "+mode);

// Proof only: native Rust crm-parties, SAME existing crm-dev Worker, 0% candidate.
// Never run against another account/Worker. Restore original 100% even on failure.
import assert from "node:assert/strict";
import {spawnSync,execFileSync} from "node:child_process";
import {randomUUID} from "node:crypto";
import {existsSync,mkdirSync,readFileSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {chromium} from "playwright";

const account="5bd5e1e1e813f061afc259e91334262d";
const db="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
const script="crm-dev",url="https://crm-dev.crm-5bd5e1e1.workers.dev";
const path="/workers/scripts/"+script;
const required=k=>{if(!process.env[k])throw Error("Missing "+k);return process.env[k]};
const sha=required("CRM_SOURCE_SHA"),oldSha=required("CRM_EXPECTED_ACTIVE_SHA");
const authToken=required("CLOUDFLARE_API_TOKEN");
const actor=n=>({
  "CF-Access-Client-Id":required("CRM_E2E_"+n+"_CLIENT_ID"),
  "CF-Access-Client-Secret":required("CRM_E2E_"+n+"_CLIENT_SECRET")
});
const a=actor("A"),b=actor("B");
const root=resolve("apps/cloudflare-crm/.release");
const checkpoint=resolve(root,"native-cloud-checkpoint.json");
const evidence=resolve(root,"native-proof");
const log=(event,fields)=>console.log(JSON.stringify({event,...fields}));
const fail=s=>{throw Error(s)};
function guard(){
  assert.equal(process.env.GITHUB_REPOSITORY,"iamaman11/crm");
  assert.equal(process.env.GITHUB_EVENT_NAME,"push");
  assert.equal(process.env.GITHUB_REF,"refs/heads/main");
  assert.equal(process.env.GITHUB_SHA,sha);
  assert.match(sha,/^[0-9a-f]{40}$/);assert.match(oldSha,/^[0-9a-f]{40}$/);
  assert.equal(required("CLOUDFLARE_ACCOUNT_ID"),account);
  assert.equal(required("CRM_D1_DATABASE_ID"),db);
  assert.equal(execFileSync("git",["rev-parse","HEAD"],{encoding:"utf8"}).trim(),sha);
  assert.ok(existsSync("apps/cloudflare-crm/dist/index.html"));
  assert.ok(existsSync("apps/cloudflare-crm/rust-party/build/index_bg.wasm"));
  // Explicit cloud config prevents Wrangler discovering the nested CI-only
  // rust-party/wrangler.jsonc, which binds the nonexistent 0000... test D1.
  const config=JSON.parse(readFileSync("apps/cloudflare-crm/wrangler.jsonc","utf8"));
  assert.equal(config.name,script);
  assert.equal(config.main,"./worker/unified.mjs");
  assert.equal(config.account_id,account);
  assert.equal(config.d1_databases?.find(x=>x.binding==="DB")?.database_id,db);
  assert.equal(config.assets?.binding,"ASSETS");
  assert.ok(!config.vars?.CRM_CI_LOCAL_ONLY);
  const cargo=readFileSync("apps/cloudflare-crm/rust-party/Cargo.toml","utf8");
  assert.ok(cargo.includes('crm-parties = { path = "../../../modules/crm-parties" }'));
  assert.ok(!cargo.includes('crm-sales = {'),"only original Party crate permitted");
}
async function cf(p,method="GET",body){
  const response=await fetch("https://api.cloudflare.com/client/v4/accounts/"+account+p,{
    method,headers:{Authorization:"Bearer "+authToken,
      ...(body?{"Content-Type":"application/json"}:{})},
    ...(body?{body:JSON.stringify(body)}:{})
  });
  const json=await response.json();
  if(!response.ok||json.success!==true)
    fail("Cloudflare "+method+" "+p+" HTTP "+response.status+" error codes="+
      JSON.stringify((json.errors||[]).map(x=>x.code)));
  return json.result;
}
async function active(){
  const r=await cf(path+"/deployments");
  const latest=r.deployments?.[0];
  if(!latest?.id||!latest.versions)fail("Active deployment unavailable");
  return latest;
}
async function bindings(version){
  const v=await cf(path+"/versions/"+version);
  const xs=v.resources?.bindings||[];
  return {sha:xs.find(x=>x.name==="GIT_SHA")?.text,
    db:xs.find(x=>x.name==="DB")?.id,
    assets:xs.some(x=>x.type==="assets"&&x.name==="ASSETS")};
}
async function versions(){
  const r=await cf(path+"/versions");
  if(!Array.isArray(r.items))fail("Cloudflare versions list changed");
  return r.items.map(v=>v.id);
}
function save(data){mkdirSync(root,{recursive:true});writeFileSync(checkpoint,JSON.stringify(data,null,2)+"\n");}
async function http(endpoint,headers={},method="GET",body){
  const r=await fetch(url+endpoint,{
    method,headers,redirect:"manual",cache:"no-store",
    ...(body?{body:JSON.stringify(body)}:{})
  });
  let json;try{json=await r.json()}catch{json=null}
  return {status:r.status,headers:Object.fromEntries(r.headers),json};
}
const override=(headers,version)=>({
  ...headers,"Cloudflare-Workers-Version-Overrides":script+'="'+version+'"'
});
function native(r,status,module="crm.parties"){
  assert.equal(r.status,status,JSON.stringify({expected:status,actual:r.status,body:r.json}));
  assert.equal(r.headers["x-crm-runtime"],"native-workers-rs","override didn't select native");
  assert.equal(r.headers["x-crm-source-sha"],sha,"wrong native version");
  assert.equal(r.headers["x-crm-module"],module);
  assert.match(r.headers["x-request-id"]||"",/^[0-9a-f-]{36}$/i);
}
async function ordinary(oldVersion){
  const d=await active();
  assert.equal(d.versions.length,1);
  assert.equal(d.versions[0].version_id,oldVersion);
  assert.equal(d.versions[0].percentage,100);
  const health=await http("/api/version?proof="+Date.now(),a);
  assert.equal(health.status,200,"existing CRM unavailable");
  assert.equal(health.json?.source,oldSha,"main version SHA changed");
  const anon=await http("/api/parties");
  assert.ok(anon.status!==200&&anon.status<500,"anonymous Access denial broken");
}
async function restore(){
  if(!existsSync(checkpoint))return;
  const p=JSON.parse(readFileSync(checkpoint,"utf8"));
  if(p.sha!==sha||p.old_sha!==oldSha||p.db!==db)fail("Native rollback checkpoint mismatch");
  if(!p.may_have_changed)return;
  const d=await active();
  if(d.versions.length===1&&d.versions[0].version_id===p.old_version&&
    d.versions[0].percentage===100){save({...p,may_have_changed:false,restored:true});return}
  if(d.versions.length!==2||
    !d.versions.some(x=>x.version_id===p.old_version&&x.percentage===100)||
    !d.versions.some(x=>x.version_id===p.native_version&&x.percentage===0))
    fail("Unexpected concurrent deployment; refusing unsafe rollback");
  await cf(path+"/deployments","POST",{strategy:"percentage",
    versions:[{version_id:p.old_version,percentage:100}],
    annotations:{"workers/message":"CRM #338 native Party smoke: restore previous 100%"}});
  const after=await active();
  assert.equal(after.versions.length,1);
  assert.equal(after.versions[0].version_id,p.old_version);
  assert.equal(after.versions[0].percentage,100);
  save({...p,may_have_changed:false,restored:true});
  log("crm.native-cloud.restore",{result:"PASS",old_version:p.old_version,d1_rollback:false});
}
async function upload(){
  // Explicit PATH is the documented Wrangler entrypoint override.
  // Use the EXISTING crm-dev Wrangler config (D1 + Static Assets), not a
  // second config/Worker; dry-run uses the identical CLI contract.
  const r=spawnSync("npx",["wrangler","versions","upload",
    "rust-party/build/index.js","--config","wrangler.jsonc","--name",script,
    "--tag","party-native-"+sha.slice(0,12),
    "--message","CRM #338 native Party version proof "+sha,
    "--var","GIT_SHA:"+sha],{
    cwd:"apps/cloudflare-crm",stdio:"inherit",env:{...process.env}
  });
  if(r.status!==0)fail("wrangler versions upload failed "+r.status);
}
async function test(){
  guard();mkdirSync(evidence,{recursive:true});
  const report={sha,expected_old_sha:oldSha,result:"IN_PROGRESS",real_access:"NOT_TESTED",
    real_cloudflare_native_d1:"NOT_TESTED",screenshots:[],normal_traffic:"NOT_TESTED",
    restored:"NOT_TESTED"};
  const write=()=>writeFileSync(resolve(evidence,"manifest.json"),JSON.stringify(report,null,2)+"\n");
  let oldVersion=null;write();
  try{
    const d=await active();
    assert.equal(d.versions.length,1,"expected single active deployment");
    assert.equal(d.versions[0].percentage,100);
    oldVersion=d.versions[0].version_id;report.old_version=oldVersion;
    assert.deepEqual(await bindings(oldVersion),{sha:oldSha,db,assets:true});
    for(const h of [a,b])assert.equal((await http("/api/parties",h)).status,200,
      "existing real Access service identity must be enrolled");
    await ordinary(oldVersion);
    const before=await versions();
    await upload(); // does NOT activate a new version
    const candidates=(await versions()).filter(x=>!before.includes(x));
    const matches=[];
    for(const id of candidates)if(JSON.stringify(await bindings(id))===
      JSON.stringify({sha,db,assets:true}))matches.push(id);
    assert.equal(matches.length,1,"did not identify unique exact-SHA candidate");
    const version=matches[0];report.native_version=version;write();
    const base={sha,old_sha:oldSha,db,old_version:oldVersion,native_version:version};
    save({...base,may_have_changed:false});
    // Set rollback marker before mutating the deployment (also handles API timeout).
    save({...base,may_have_changed:true});
    // Use the documented, non-interactive Wrangler 0% version deployment.
    // Refuse any nonzero candidate percentage rather than expose Party-only API.
    const deployed=spawnSync("npx",["wrangler","versions","deploy",
      oldVersion+"@100%",version+"@0%","--yes",
      "--message","CRM #338 native Rust 0% verification ONLY"],{
      cwd:"apps/cloudflare-crm",stdio:"inherit",env:{...process.env}
    });
    if(deployed.status!==0)fail("Wrangler refused 0%-traffic native version deployment");
    const now=await active();
    assert.equal(now.versions.length,2);
    assert.ok(now.versions.some(x=>x.version_id===oldVersion&&x.percentage===100));
    assert.ok(now.versions.some(x=>x.version_id===version&&x.percentage===0));
    await ordinaryCheckZero(oldVersion,version);
    const hA=override(a,version),hB=override(b,version);
    const anon=await http("/api/parties",override({},version));
    assert.notEqual(anon.status,200,"real Access must reject anonymous");
    native(await http("/api/parties",hA),200);
    native(await http("/api/parties",hB),200);
    const diagnostic=await http("/api/doctor",hA);
    native(diagnostic,200,"crm.diagnostics");
    assert.equal(diagnostic.json?.runtime,"workers-rs");
    assert.equal(diagnostic.json?.source,sha);
    assert.equal(diagnostic.json?.worker_version,version);
    assert.equal(diagnostic.json?.access?.status,"PASS");
    assert.equal(diagnostic.json?.database?.status,"PASS");
    report.native_doctor="PASS";
    report.real_access="PASS";
    const browser=await chromium.launch({headless:true});
    let id=null;const marker="CloudParty-"+randomUUID().slice(0,12);
    try{
      const ctx=await browser.newContext({viewport:{width:1366,height:900},extraHTTPHeaders:hA});
      const page=await ctx.newPage(),errors=[];
      page.on("pageerror",e=>errors.push(e.message));
      const start=await page.goto(url,{waitUntil:"domcontentloaded"});
      assert.equal(start.status(),200);
      const nav=page.getByRole("navigation",{name:"Разделы"});
      await nav.getByRole("button",{name:/Лица и организации/}).click();
      const parties=page.locator('section[aria-label="Лица и организации (Rust)"]');
      await parties.waitFor();
      await parties.getByRole("button",{name:/Добавить лицо/}).click();
      await page.getByLabel("Имя / название").fill(marker);
      const creation=page.waitForResponse(r=>new URL(r.url()).pathname==="/api/parties"&&r.request().method()==="POST");
      await page.getByRole("button",{name:"Сохранить",exact:true}).click();
      report.create_request_id=(await creation).headers()["x-request-id"];
      assert.match(report.create_request_id||"",/^[0-9a-f-]{36}$/i);
      await parties.getByRole("row").filter({hasText:marker}).waitFor();
      const list=await http("/api/parties",hA);native(list,200);
      id=list.json?.items?.find(x=>x.name===marker)?.id;
      assert.ok(id,"React create not persisted in D1");
      native(await http("/api/parties/"+id,hB),404);
      await parties.getByRole("row").filter({hasText:marker})
        .getByRole("button",{name:"Изменить"}).click();
      await page.getByLabel("Имя / название").fill(marker+"-v2");
      await page.getByRole("button",{name:"Сохранить",exact:true}).click();
      await parties.getByRole("row").filter({hasText:marker+"-v2"}).waitFor();
      await page.reload({waitUntil:"domcontentloaded"});
      await nav.getByRole("button",{name:/Лица и организации/}).click();
      await parties.getByRole("row").filter({hasText:marker+"-v2"}).waitFor();
      const final=await http("/api/parties/"+id,hA);native(final,200);
      assert.equal(final.json.item.version,2);
      assert.equal(final.json.item.name,marker+"-v2");
      assert.deepEqual(errors,[]);
      await page.screenshot({path:resolve(evidence,"real-cloudflare-native-party.png"),
        fullPage:true,animations:"disabled"});
      report.screenshots.push("real-cloudflare-native-party.png");
      await ctx.close();
    } finally{await browser.close()}
    const result=await cf("/d1/database/"+db+"/query","POST",{
      sql:"SELECT (SELECT version FROM crm_party_records WHERE id=?) AS version,"+
        "(SELECT COUNT(*) FROM crm_audit WHERE subject_id=? AND action LIKE 'parties.%') AS audits",
      params:[id,id]});
    const row=result?.[0]?.results?.[0];
    assert.equal(row?.version,2);
    assert.equal(row?.audits,2);
    report.real_cloudflare_native_d1="PASS";
    report.d1_audits=row.audits;
    report.result="PASS";
    log("crm.native-cloud.e2e",{result:"PASS",sha,old_version:oldVersion,
      native_version:version,access_a_b:"PASS",chromium:"PASS",d1:"PASS",normal_traffic:"100% old"});
  }catch(error){
    report.result="FAIL";report.error=String(error.message||error).slice(0,750);
    throw error;
  }finally{
    try{
      await restore();
      report.restored="PASS";
      if(oldVersion)await ordinary(oldVersion);
      report.normal_traffic="100% original";
    }catch(e){
      report.restored="FAIL";report.result="FAIL";
      report.rollback_error=String(e.message||e).slice(0,750);
      log("crm.native-cloud.restore",{result:"FAIL",error:report.rollback_error});
    }
    write();
    if(report.rollback_error)fail("CRITICAL: restore failed "+report.rollback_error);
  }
}
async function ordinaryCheckZero(old,version){
  const d=await active();
  assert.ok(d.versions.some(x=>x.version_id===old&&x.percentage===100));
  assert.ok(d.versions.some(x=>x.version_id===version&&x.percentage===0));
  const r=await http("/api/version?proof="+Date.now(),a);
  assert.equal(r.status,200);
  assert.equal(r.json?.source,oldSha);
}
guard();
if(process.argv[2]==="test")await test();
else if(process.argv[2]==="restore")await restore();
else fail("Use: node native-cloud-proof.mjs test|restore");

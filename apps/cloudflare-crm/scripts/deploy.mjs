// One protected GitHub Actions release owner: deploy, verify-data, rollback.
import {existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from "node:fs";
import {createHash} from "node:crypto";
import {join,relative} from "node:path";
import {spawnSync} from "node:child_process";
const required=n=>{if(!process.env[n])throw Error("Missing "+n);return process.env[n];};
const sha=required("GITHUB_SHA");
if(!/^[0-9a-f]{40}$/.test(sha)||process.env.GITHUB_REF!=="refs/heads/main"||process.env.GITHUB_EVENT_NAME!=="push")throw Error("Untrusted deployment");
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
const auth=()=>({"CF-Access-Client-Id":required("CRM_E2E_A_CLIENT_ID"),"CF-Access-Client-Secret":required("CRM_E2E_A_CLIENT_SECRET")});
async function publicSha(){
  const r=await fetch(url+"/api/version?readback="+Date.now(),{headers:auth(),redirect:"manual",cache:"no-store"});
  if(r.status!==200)throw Error("CRM public version HTTP "+r.status);
  return (await r.json()).source;
}

async function waitForPublicSha(expected,stage){
  const start=Date.now(),deadline=start+45000;
  let last="NOT_CHECKED",attempts=0;
  for(;;){
    attempts++;
    try{last=await publicSha();}catch(error){last="READBACK_ERROR:"+String(error.message).slice(0,140);}
    if(last===expected){
      console.log(JSON.stringify({event:"crm.public-readback",stage,sha:expected,
        result:"PASS",attempts,elapsed_ms:Date.now()-start}));
      return;
    }
    if(Date.now()>=deadline)throw Error("Public Worker SHA readback mismatch stage="+stage+
      " expected="+expected+" last_observed="+last+" attempts="+attempts);
    if(attempts===1)console.log(JSON.stringify({event:"crm.public-readback",stage,
      expected,observed:last,result:"RETRY"}));
    await new Promise(resolve=>setTimeout(resolve,Math.min(attempts*600,3500)));
  }
}

async function deploy(){
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
  const newVersion=await version();
  const current=await bindings(newVersion);
  if(current.sha!==sha||current.db!==database)throw Error("Deployed CRM SHA/D1 binding mismatch");
  if(newVersion===oldVersion)throw Error("Worker version did not advance");
  const anonymous=await fetch(url+"/api/health",{redirect:"manual"});
  if(anonymous.status===200||anonymous.status>=500)throw Error("Cloudflare Access denied check failed");
  const authorized=await fetch(url+"/api/health",{headers:auth(),redirect:"manual"});
  if(authorized.status!==200)throw Error("Cloudflare Access authorized health failed");
  await waitForPublicSha(sha,"deploy");
  record("deployed.json",{git_sha:sha,worker_version:newVersion,d1_database:database,rust,result:"E2E_PENDING"});
  console.log(JSON.stringify({event:"crm.deploy",sha,result:"E2E_PENDING",version:newVersion}));
}
async function verifyData(){
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
  record("accepted.json",{git_sha:sha,d1_database:database,records:2,receipts:3,audits:3,migration:1,versions:"A=2 B=1",result:"PASS"});
  console.log(JSON.stringify({event:"crm.native-party-live-d1",sha,result:"PASS",records:2,receipts:3,audits:3,migration:1}));
}
async function rollback(){
  if(!existsSync(join(out,"checkpoint.json"))){console.log(JSON.stringify({event:"crm.rollback",result:"SKIP_NO_CHECKPOINT"}));return;}
  const p=load("checkpoint.json");
  if(p.git_sha!==sha||p.d1_database!==database)throw Error("Rollback checkpoint mismatch");
  if(await version()===p.previous_version){record("rollback.json",{result:"NO_WORKER_CHANGE",d1_restored:false});return;}
  const now=await bindings(await version());
  if(now.sha!==sha||now.db!==database)throw Error("Refusing rollback over unrelated deployment");
  await cf(worker+"/deployments","POST",{strategy:"percentage",versions:[{version_id:p.previous_version,percentage:100}],annotations:{"workers/message":"CRM failed-release rollback "+sha.slice(0,12)}});
  if(await version()!==p.previous_version)throw Error("Rollback version readback failed");
  await waitForPublicSha(p.previous_sha,"rollback");
  record("rollback.json",{git_sha:sha,result:"WORKER_RESTORED",previous_sha:p.previous_sha,previous_version:p.previous_version,d1_restored:false});
  console.log(JSON.stringify({event:"crm.rollback",result:"WORKER_RESTORED",old_sha:p.previous_sha,d1_restored:false}));
}
const mode=process.argv[2]||"deploy";
if(mode==="deploy")await deploy();
else if(mode==="verify-data")await verifyData();
else if(mode==="rollback")await rollback();
else throw Error("Unknown CRM deploy operation "+mode);

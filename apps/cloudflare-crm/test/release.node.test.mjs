// Tests of the real release script with a local Cloudflare API substitute.
// Synthetic fixtures do not constitute live acceptance evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {mkdtempSync,mkdirSync,copyFileSync,writeFileSync,readFileSync,rmSync,existsSync,cpSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {spawnSync} from "node:child_process";
const sha="a".repeat(40),previous="b".repeat(40),database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
const requestId="11111111-1111-4111-8111-111111111111";
async function subprocessEntry(){
 const {default:assert}=await import("node:assert/strict");
 const {writeFileSync,readFileSync,readdirSync}=await import("node:fs");
 const {mode,variant,sha,database,audit,requestId,rpcPath,fixtureRequests,fixtureWrites}=JSON.parse(process.env.CRM_RELEASE_TEST_FIXTURE);
 const posts=[];
 const nfcLegacy=variant==="nfc-migration"||variant==="nfc-noncanonical";
 let enabled=mode==="deploy",migrated=variant!=="migration-fails"&&!nfcLegacy,published=false,edgeAttempts=0,role="viewer";
 let auditAttempts=0,telemetryAttempts=0,anonymousAttempts=0;
 let telemetryInFlight=0,maxTelemetryInFlight=0,postDeployReads=0;
 const commands=[];
 let db;
 if(mode==="deploy"){
   const {DatabaseSync}=await import("node:sqlite");
   db=new DatabaseSync(":memory:");
   // "migration-fails" declares 0001 as applied in the fake D1 ledger.
   // Build that real SQLite schema: a pending migration must preserve the
   // original Access columns before traffic is closed. Empty query fixtures
   // no longer exercise the actual pre-migration integrity gate.
   if(variant==="migration-fails")
     db.exec(readFileSync("apps/cloudflare-crm/migrations/0001_initial.sql","utf8"));
   // The ordinary release starts with the full current SQL schema.
   if(migrated||nfcLegacy)for(const name of readdirSync("apps/cloudflare-crm/migrations").filter(n=>n.endsWith(".sql")).sort()){
     if(nfcLegacy&&name>="0016_nfc_search_name.sql")continue;
     db.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
   }
   if(nfcLegacy){
     db.exec("INSERT INTO crm_tenants VALUES('tenant-a','A')");
     db.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?)")
       .run("tenant-a","p",JSON.stringify({display_name:variant==="nfc-noncanonical"?"Cafe\u0301":"Café"}),1,"key","digest","request","actor");
   }
   const child=(await import("node:child_process")).default;
   child.spawnSync=(command,args)=>{
     commands.push([command,...args]);
     if(command==="sha256sum")return{status:0}; // Synthetic sealed files, not a real build.
     assert.equal(command,"npx");assert.equal(args[0],"wrangler");
     if(args[1]==="d1"){
       assert.equal(enabled,false,"Schema changes require closed public entry");
       assert.deepEqual(args,["wrangler","d1","migrations","apply","crm-dev","--remote"]);
       if(variant==="migration-fails")return{status:1};
       for(const name of readdirSync("apps/cloudflare-crm/migrations").sort()){
         if(nfcLegacy&&name<"0016_nfc_search_name.sql")continue;
         db.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
       }
       migrated=true;
     }else{
       assert.deepEqual(args,["wrangler","deploy","--var","GIT_SHA:"+sha]);
       assert.equal(migrated,true);published=true;
     }
     return{status:0};
   };
   (await import("node:module")).syncBuiltinESMExports();
 }
 // The existing accelerated waits must advance the same clock used by
 // release deadlines. Use Node's native virtual Date, not wall-clock sleeps.
 const {mock}=await import("node:test");
 mock.timers.enable({apis:["Date"],now:Date.now()});
 mock.method(globalThis,"setTimeout",(callback,ms=0)=>{
   queueMicrotask(()=>{if(ms>0)mock.timers.tick(ms);callback();});
   return 0;
 });
 globalThis.fetch=async(url,options={})=>{
   const path=new URL(url).pathname;
   const json=value=>new Response(JSON.stringify(value),{headers:{"content-type":"application/json"}});
   const html=status=>new Response("<!DOCTYPE html><title>Cloudflare edge error</title>",{
     status,headers:{"content-type":"text/html","cf-ray":"fixture-ray-123"}});
   if(new URL(url).hostname==="crm-dev.crm-5bd5e1e1.workers.dev"){
     if(mode==="fail-closed")return new Response("disabled",{status:404});
     if(mode==="deploy"){
       if(!enabled)return new Response("disabled",{status:404});
       if(path==="/api/version"){
         if(!new Headers(options.headers).has("CF-Access-Client-Id")){
           anonymousAttempts++;
           if((variant==="anonymous-reset"&&anonymousAttempts===1)||
              variant==="anonymous-persistent-reset"){
             throw Object.assign(new TypeError("fetch failed"),{cause:{code:"ECONNRESET"}});
           }
           if((variant==="anonymous-edge-404"&&anonymousAttempts===1)||
              (variant==="anonymous-delayed-edge-404"&&anonymousAttempts<=8)||
              variant==="anonymous-persistent-404")
             return new Response("propagating",{status:404,headers:{"cf-ray":"fixture-ray-123"}});
           return new Response(variant==="anonymous-success"?"exposed":"denied",
             {status:variant==="anonymous-success"?200:403});
         }
         if(++edgeAttempts<=2)return new Response("propagating",{status:404});
       }
     }
     return json({source:sha});
   }
   assert.equal(new URL(url).hostname,"api.cloudflare.com","Unexpected network target");
   if(options.method==="POST")posts.push(path);
   const wrap=result=>json({success:true,result});
   if(path.endsWith("/deployments")){
     const stale=published&&variant==="repeat-sha"&&++postDeployReads===1;
     const version=mode==="deploy"&&(!published||stale)?"version-old":"version-new";
     return wrap({deployments:[{versions:[{version_id:version,percentage:100}]}]});
   }
   if(path.endsWith("/versions/version-old"))return wrap({resources:{bindings:[
     {name:"GIT_SHA",text:variant==="repeat-sha"?sha:"90a352be2a70b74a64e5a774c9a76b7f02c86a5f"},{name:"DB",id:database}]}});
   if(path.endsWith("/time_travel/bookmark"))return wrap({bookmark:"synthetic-bookmark"});
   if(path.endsWith("/settings"))return wrap({observability:{
     enabled:true,redact_query_string:variant!=="trace-redaction-disabled",
     logs:{enabled:variant!=="trace-logs-disabled",head_sampling_rate:1,
       invocation_logs:variant==="trace-invocation-logs-enabled"},
     traces:{enabled:variant!=="trace-disabled",head_sampling_rate:1}
   }});
   if(path.endsWith("/versions/version-new"))return wrap({resources:{bindings:[
     {name:"GIT_SHA",text:variant==="unrelated"?"c".repeat(40):sha},
     {name:"DB",id:variant==="wrong-database"?"other":database}]}});
   if(path.endsWith("/subdomain")){
     if(mode==="deploy"){
       if(options.method==="POST"){
         const body=JSON.parse(options.body);assert.equal(body.previews_enabled,false);
         if(body.enabled)assert.equal(published,true);
         enabled=body.enabled;
       }
       return wrap({enabled,previews_enabled:false});
     }
     if(options.method==="POST")assert.deepEqual(JSON.parse(options.body),{enabled:false,previews_enabled:false});
     return wrap({enabled:false,previews_enabled:false});
   }
   if(path.endsWith("/telemetry/query")){
     telemetryAttempts++;
     if(mode==="verify-data"&&variant.startsWith("telemetry-429-")&&
        (variant.includes("-persistent-")||telemetryAttempts===1)){
       const headers={"content-type":"application/json","cf-ray":"fixture-ray-123"};
       if(variant.endsWith("-seconds"))headers["retry-after"]="3";
       if(variant.endsWith("-date"))headers["retry-after"]=new Date(Date.now()+3000).toUTCString();
       if(variant.endsWith("-invalid"))headers["retry-after"]="not-a-date";
       return new Response(JSON.stringify({success:false,errors:[{code:10102}]}),{status:429,headers});
     }
     if(mode==="verify-data"&&variant==="telemetry-403")
       return new Response(JSON.stringify({success:false,errors:[{code:10000}]}),
         {status:403,headers:{"content-type":"application/json","cf-ray":"fixture-ray-123"}});
     if(mode==="verify-data"&&variant==="transient-html-telemetry"&&telemetryAttempts===1)return html(503);
     if(mode==="verify-data"&&variant==="persistent-html-telemetry")return html(503);
     if(mode==="verify-data"&&variant==="html-success-telemetry")return html(200);
     if(mode==="verify-data"&&((variant==="transient-invalid-json-telemetry"&&telemetryAttempts===1)||
        variant==="persistent-invalid-json-telemetry"))
       return new Response('{"success":true,"result":',{status:200,
         headers:{"content-type":"application/json","cf-ray":"fixture-ray-123"}});
     const query=JSON.parse(options.body);
     assert.equal(query.queryId,"crm-471-native-batch");
     assert.equal(query.view,"events");
     assert.ok(query.limit>=1&&query.limit<=2000);
     assert.equal(query.parameters.filters[0].value,"crm-dev");
     // At 70 virtual seconds, still before the actual 120-second deadline.
     if(variant==="late-telemetry"&&telemetryAttempts<=35)
       return wrap({events:{events:[],count:0}});
     const traceFor=index=>String(index+1).padStart(32,"0"),events=[];
     const add=(id,metadata,source,workers)=>events.push({
       // Match real Cloudflare pages: distinct span and request events may
       // share $metadata.id; only a repeated *pagination cursor* is invalid.
       $metadata:{id:variant==="large-telemetry-scope"&&id==="span-0"?"request-0":id,...metadata},
       ...(source?{source}:{}),...(workers?{$workers:workers}:{})});
     for(let index=0;index<fixtureRequests.length;index++){
       const entry=fixtureRequests[index],traceId=traceFor(index);
       if(variant!=="missing-logs")add("request-"+index,
         {type:"cf-worker",...(variant==="missing-native-trace-id"&&index===0?{}:{traceId})},
         {event:"crm.native.request",request_id:entry.request_id,operation:entry.path,
          status:entry.status,error_code:variant==="wrong-error-code"?"aborted":entry.error_code??null},
         {scriptVersion:{id:"version-new"}});
       if(variant!=="missing-native-span"&&
          !(variant==="missing-recovery-span"&&index===2)&&
          !(variant==="missing-owner-span"&&index===7))
         add("span-"+index,{type:"span",traceId},{
           name:"D1 prepare",traceId:variant==="wrong-native-trace-id"?"f".repeat(32):traceId,
           ...(variant==="missing-d1-span"?{}:{db:{system:"cloudflare-d1"}})});
       if(variant==="unsafe-invocation-event")add("invocation-"+index,
         {type:"cf-worker-event",traceId},null,
         {event:{request:{headers:{authorization:"synthetic-secret"}}}});
       const leaks={
         "synthetic-private-leak":[0,"Browser Party 1234567890123"],
         "synthetic-cookie-leak":[0,"crm-s3-cookie-1234567890123"],
         "synthetic-header-leak":[0,"crm-s3-header-1234567890123"],
         "synthetic-authorization-leak":[0,"Bearer crm-s3-authorization-1234567890123"],
         "synthetic-jwt-leak":[0,"eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-1234567890123"],
         "synthetic-query-leak":[0,"crm-s3-query-1234567890123"]
       };
       if(leaks[variant]?.[0]===index)add("leak-"+index,{type:"log",traceId},
         {traceId,message:leaks[variant][1]});
     }
     // Generate >2000 events for native pagination and duplicate-cursor proofs.
     if(variant==="large-telemetry-scope"||variant==="telemetry-cursor-repeat")
       for(let i=0;i<2200;i++)add("unrelated-"+i,{type:"log",traceId:"unrelated"},
         {event:"unrelated"});
     let start=0;
     if(query.offset!==undefined){
       const i=events.findIndex(e=>e.$metadata.id===query.offset);
       assert.ok(i>=0,"Cursor must come from prior page");
       start=variant==="telemetry-cursor-repeat"?0:i+1;
     }
     const page=events.slice(start,start+query.limit);
     return wrap({events:{events:page,count:page.length}});
   }
   if(path.endsWith("/query")){
     if(mode==="verify-data"){
       auditAttempts++;
       if(variant==="transient-html-audit"&&auditAttempts===1)return html(502);
     }
     if(mode==="fail-closed"&&variant==="restore-role"){
       const {sql,params}=JSON.parse(options.body);
       assert.deepEqual(params,["service:synthetic.access","crm-e2e-123-1-a"]);
       if(sql.startsWith("UPDATE")){
         assert.ok(sql.includes("AND active=1 AND role='viewer'"));role="admin";
         return wrap([{success:true,results:[]}]);
       }
       assert.equal(sql,"SELECT role FROM crm_users WHERE email=? AND tenant_id=? AND active=1");
       return wrap([{success:true,results:[{role}]}]);
     }
     if(mode==="deploy"){
       const {sql,params=[]}=JSON.parse(options.body);
       const names=readdirSync("apps/cloudflare-crm/migrations").filter(n=>n.endsWith(".sql")).sort();
       const ledger=variant==="migration-fails"?names.slice(0,1):
         variant==="migration-history-drift"?["unrecognized.sql"]:nfcLegacy&&!migrated?
            names.filter(name=>name<"0016_nfc_search_name.sql"):names;
       const results=sql.includes("FROM d1_migrations")?ledger.map(name=>({name})):
         !migrated&&!nfcLegacy&&variant!=="migration-fails"?[]:db.prepare(sql).all(...params);
       return wrap([{success:true,results}]);
     }
     const {sql,params=[]}=JSON.parse(options.body);
     // One tenant-scoped D1 readback for exact mutations and recovery receipts.
     // Fail on legacy per-request queries.
     assert.ok(sql.startsWith(
       "SELECT request_id,action,subject_id,actor FROM crm_audit WHERE tenant_id=? AND request_id IN ("),
       "Unexpected serial or unscoped audit readback");
     assert.equal(params[0],"fixture-a");
     assert.equal((sql.match(/\?/g)??[]).length,params.length,
       "Every request ID must use a SQL placeholder");
     const ids=params.slice(1);
     assert.equal(new Set(ids).size,ids.length,"Duplicate audit ID");
     const auditRows=variant==="missing-audit"?[]:fixtureWrites
       .filter(write=>ids.includes(write.request_id))
       .flatMap(write=>write.audit.map(row=>({request_id:write.request_id,...row})));
     if(variant==="read-audited"){
       const readId=ids.find(id=>!fixtureWrites.some(write=>write.request_id===id));
       assert.ok(readId,"Read-only request must be in the snapshot");
       auditRows.push({request_id:readId,action:"unexpected.recovery",
         subject_id:"duplicate",actor:"service:synthetic.access"});
     }
     return wrap([{success:true,results:auditRows}]);
   }
   throw Error("Unexpected Cloudflare endpoint "+path);
 };
 // A microtask boundary makes any accidental Promise.all burst observable.
 const originalFetch=globalThis.fetch;
 globalThis.fetch=async (...args)=>{
   const telemetry=new URL(args[0]).pathname.endsWith("/telemetry/query");
   if(!telemetry)return originalFetch(...args);
   telemetryInFlight++;
   maxTelemetryInFlight=Math.max(maxTelemetryInFlight,telemetryInFlight);
   try{await Promise.resolve();return await originalFetch(...args);}
   finally{telemetryInFlight--;}
 };
 process.argv[2]=mode;
 try{await import("./deploy.mjs");}finally{
   writeFileSync("posts.json",JSON.stringify(posts));
   writeFileSync("state.json",JSON.stringify({enabled,migrated,published,edgeAttempts,anonymousAttempts,auditAttempts,telemetryAttempts,maxTelemetryInFlight,postDeployReads,commands,role}));
   db?.close();
   mock.restoreAll();
   mock.timers.reset();
 }
}
function run(mode,variant="valid"){
 const dir=mkdtempSync(join(tmpdir(),"crm-release-test-"));
 const app=join(dir,"apps/cloudflare-crm"),out=join(app,".release");
 mkdirSync(out,{recursive:true});mkdirSync(join(app,"e2e-evidence"));
 for(const name of ["deploy.mjs","release-readback.mjs"])
   copyFileSync(new URL("../scripts/"+name,import.meta.url),join(dir,name));
 const save=(path,value)=>writeFileSync(path,JSON.stringify(value));
 if(variant!=="no-checkpoint"&&mode!=="deploy")save(join(out,"checkpoint.json"),{git_sha:sha,previous_sha:previous,d1_database:database});
 save(join(out,"deployed.json"),{git_sha:sha,worker_version:"version-new",started_at:1,
   trace_settings:{traces_enabled:true,query_string_redacted:true,logs_enabled:true,
     invocation_logs_enabled:false,trace_sampling_rate:1}});
 if(mode==="verify-data")save(join(out,"e2e-tenants.json"),{tenant_a:"fixture-a",actor_a:"service:synthetic.access"});
 if(variant==="restore-role"||variant==="wrong-role-tenant")save(join(out,"e2e-tenants.json"),{
   source_sha:sha,database,run_id:"123",attempt:"1",actor_a:"service:synthetic.access",
   tenant_a:variant==="wrong-role-tenant"?"unrelated-tenant":"crm-e2e-123-1-a"});
 if(mode==="deploy"){
   rmSync(join(out,"deployed.json"));
   copyFileSync(new URL("../wrangler.jsonc",import.meta.url),join(app,"wrangler.jsonc"));
   cpSync(new URL("../migrations/",import.meta.url),join(app,"migrations"),{recursive:true});
   mkdirSync(join(app,"rust-party/build"),{recursive:true});mkdirSync(join(app,"dist"));
   writeFileSync(join(app,"rust-party/build/index.js"),"synthetic wrapper");
   writeFileSync(join(app,"rust-party/build/index_bg.wasm"),"synthetic wasm");
   writeFileSync(join(app,"dist/index.html"),"synthetic UI");
   writeFileSync(join(out,"source-sha.txt"),sha);
 }
 const rpcPath="/crm.parties.v3.PartyService/CreateParty";
 const accountPath="/crm.accounts.v2.AccountService/CreateAccount";
 const actor="service:synthetic.access";
 const audit=[{action:"parties.create",subject_id:"party",
   actor:variant==="wrong-actor"?"forged@example.test":actor}];
 const accountAudit=[{action:"accounts.create",subject_id:"account",actor}];
 const fixtureRequests=[
   {path:rpcPath,status:200,request_id:requestId,source_sha:sha,runtime:"native-workers-rs"},
   {path:accountPath,status:200,
     request_id:"22222222-2222-4222-8222-222222222222",source_sha:sha,runtime:"native-workers-rs"},
   {path:rpcPath,status:403,
     request_id:"55555555-5555-4555-8555-555555555555",error_code:"permission_denied",
     source_sha:sha,runtime:"native-workers-rs"}
 ];
 const fixtureWrites=[
   {path:rpcPath,request_id:requestId,audit},
   {path:accountPath,request_id:fixtureRequests[1].request_id,audit:accountAudit}
 ];
 // Full 68-step browser and owner recovery tests run in workerd, not here.
 const smokeRequests=[
   fixtureRequests[0],
   {path:"/crm.parties.v3.PartyService/ListParties",status:200,
     request_id:"56565656-5656-4565-8565-565656565656",source_sha:sha,runtime:"native-workers-rs"},
   fixtureRequests.find(x=>x.path===accountPath),
   {path:"/crm.accounts.v2.AccountService/ListAccounts",status:200,
     request_id:"67676767-6767-4676-8676-676767676767",source_sha:sha,runtime:"native-workers-rs"},
   fixtureRequests.find(x=>x.status===403)
 ];
 if(smokeRequests.some(x=>!x))throw Error("Incomplete smoke RPC fixture");
 if(variant==="wrong-source")smokeRequests[0]={...smokeRequests[0],source_sha:previous};
 if(variant==="missing-viewer-denial")smokeRequests.pop();
 if(variant==="missing-tenant-list")smokeRequests.splice(3,1);
 if(variant==="large-telemetry-scope")for(let i=smokeRequests.length;i<338;i++)
   smokeRequests.push({path:"/crm.parties.v3.PartyService/ListParties",status:200,
     request_id:"abababab-abab-4bab-8bab-"+i.toString(16).padStart(12,"0"),
     source_sha:sha,runtime:"native-workers-rs"});
 const smokeWrites=[fixtureWrites[0],fixtureWrites.find(x=>x.path===accountPath)];
 if(smokeWrites.some(x=>!x))throw Error("Incomplete smoke writes");
 if(variant==="missing-account-write")smokeWrites.pop();
 const privacy=[
   {kind:"party-name",request_id:requestId,value:"Browser Party 1234567890123"},
   {kind:"cookie",request_id:requestId,value:"crm-s3-cookie-1234567890123"},
   {kind:"header",request_id:requestId,value:"crm-s3-header-1234567890123"},
   {kind:"authorization",request_id:requestId,value:"Bearer crm-s3-authorization-1234567890123"},
   {kind:"jwt",request_id:requestId,value:"eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-1234567890123"},
   {kind:"query-string",request_id:requestId,value:"crm-s3-query-1234567890123"}
 ];
 save(join(app,"e2e-evidence/published-smoke.json"),{
   result:"PASS",source_sha:sha,
   features:variant==="wrong-features"?["native-ci"]:["native","published-smoke"],
   tenant:"fixture-a",started_at:2,
   steps:variant==="missing-step"?["signed-access"]:[
     "signed-access","party-create","party-reload","account-create",
     "viewer-denial","tenant-isolation","anonymous-denial"],
   requests:smokeRequests,writes:smokeWrites,
   synthetic_private_probes:variant==="missing-private-probes"?[]:privacy
 });
 try{
   const result=spawnSync(process.execPath,["--input-type=module","-e","("+subprocessEntry.toString()+")()"],{cwd:dir,encoding:"utf8",
     env:{...process.env,GITHUB_SHA:sha,GITHUB_REF:"refs/heads/dev",GITHUB_EVENT_NAME:"push",
       GITHUB_RUN_ID:"123",GITHUB_RUN_ATTEMPT:"1",
       CLOUDFLARE_ACCOUNT_ID:"5bd5e1e1e813f061afc259e91334262d",CRM_D1_DATABASE_ID:database,
       CLOUDFLARE_API_TOKEN:"synthetic-token",CRM_E2E_A_CLIENT_ID:"synthetic.access",CRM_E2E_A_CLIENT_SECRET:"synthetic-secret",
       CRM_RELEASE_TEST_FIXTURE:JSON.stringify({mode,variant,sha,database,audit,requestId,rpcPath,fixtureRequests:smokeRequests,fixtureWrites:smokeWrites})}});
   // A killed/overflowed child is a harness error, never a release refusal.
   if(result.error)throw result.error;
   assert.equal(result.signal,null,"Release fixture subprocess terminated by signal");
   return {status:result.status,error:result.stderr,stdout:result.stdout,posts:JSON.parse(readFileSync(join(dir,"posts.json"),"utf8")),
     state:JSON.parse(readFileSync(join(dir,"state.json"),"utf8")),deployed:existsSync(join(out,"deployed.json")),
     stopped:existsSync(join(out,"failed-release.json")),accepted:existsSync(join(out,"accepted.json")),checkpoint:existsSync(join(out,"checkpoint.json"))};
 }finally{rmSync(dir,{recursive:true,force:true});}
}
test("failed candidate stops public entry and never redeploys its predecessor",()=>{
 const result=run("fail-closed");
 assert.equal(result.status,0,result.error);assert.equal(result.stopped,true);
 assert.equal(result.posts.length,1);assert.ok(result.posts[0].endsWith("/subdomain"));
});
test("failure before a release checkpoint does not change the deployed app",()=>{
 const result=run("fail-closed","no-checkpoint");
 assert.equal(result.status,0,result.error);assert.deepEqual(result.posts,[]);
});
for(const variant of ["unrelated","wrong-database"])test("failure cannot stop "+variant+" deployment",()=>{
 const result=run("fail-closed",variant);
 assert.notEqual(result.status,0);assert.deepEqual(result.posts,[]);assert.equal(result.stopped,false);
});
test("matching browser, D1 and version-specific Logs evidence passes the release validator",()=>{
 const result=run("verify-data");
 assert.equal(result.status,0,result.error);assert.equal(result.accepted,true);
 assert.equal(result.state.auditAttempts,1,
   "All signed browser mutations and reads share one D1 snapshot");
 assert.equal(result.state.maxTelemetryInFlight,1,
   "Cloudflare Telemetry must use one native cursor reader");
});
test("the real release script accepts late telemetry after the old 30-round cutoff",()=>{
 const result=run("verify-data","late-telemetry");
 assert.equal(result.status,0,result.error);
 assert.equal(result.accepted,true);
 assert.equal(result.state.telemetryAttempts,36);
 assert.equal(result.state.maxTelemetryInFlight,1);
});
test("338 exact RPC readbacks retain request and span evidence without a burst",()=>{
 const result=run("verify-data","large-telemetry-scope");
 assert.equal(result.status,0,result.error);
 assert.equal(result.accepted,true);
 assert.equal(result.state.maxTelemetryInFlight,1);
 assert.ok(result.stdout.includes('"requests":338'));
 assert.ok(result.state.telemetryAttempts>=2&&result.state.telemetryAttempts<10,
   "338 RPCs and native spans must use only a few cursor pages");
});
test("repeating a Cloudflare telemetry cursor cannot pass release acceptance",()=>{
 const result=run("verify-data","telemetry-cursor-repeat");
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
 assert.match(result.error,/CRM_TELEMETRY_CURSOR_REPEATED/);
 assert.ok(result.state.telemetryAttempts<=3);
});
test("Retry-After delta-seconds, HTTP-date and invalid headers are parsed safely",async()=>{
 const {parseRetryAfterMs}=await import("../scripts/release-readback.mjs");
 const now=Date.parse("Fri, 20 Nov 2026 18:00:00 GMT");
 assert.equal(parseRetryAfterMs("3",now),3000);
 assert.equal(parseRetryAfterMs("Fri, 20 Nov 2026 18:00:04 GMT",now),4000);
 assert.equal(parseRetryAfterMs("Fri, 20 Nov 2026 17:59:59 GMT",now),0);
 for(const invalid of [null,"","not-a-date","-1","3.5","999999999999999999999999"])
   assert.equal(parseRetryAfterMs(invalid,now),null);
});
for(const variant of [
 "telemetry-429-transient-seconds","telemetry-429-transient-date",
 "telemetry-429-transient-invalid","telemetry-429-transient-missing"
])test("transient Telemetry 429 recovers without retrying writes: "+variant,()=>{
 const result=run("verify-data",variant);
 assert.equal(result.status,0,result.error);
 assert.equal(result.accepted,true);
 assert.equal(result.state.maxTelemetryInFlight,1);
 assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0);
 const retry=result.stdout.split("\n").filter(x=>x.includes("crm.cf-readback-retry"))
   .map(x=>JSON.parse(x)).find(x=>x.http_status===429);
 assert.ok(retry,"A 429 must be observable");
 if(variant.endsWith("-seconds"))assert.ok(retry.delay_ms>=3000,
   "A valid Retry-After must not be shortened");
});
for(const variant of ["telemetry-429-persistent-seconds","telemetry-429-persistent-date"])
 test("persistent Telemetry 429 stays fail-closed after three reads: "+variant,()=>{
   const result=run("verify-data",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.accepted,false);
   assert.match(result.error,/HTTP 429/);
   assert.ok(result.state.telemetryAttempts>=3&&result.state.telemetryAttempts<=6,
     "The cursor reader must stop after at most three attempts");
   assert.equal(result.state.maxTelemetryInFlight,1);
   assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0);
 });
test("Telemetry nonretryable 403 fails closed without another POST",()=>{
 const result=run("verify-data","telemetry-403");
 assert.notEqual(result.status,0);
 assert.equal(result.accepted,false);
 assert.match(result.error,/HTTP 403/);
 assert.ok(result.state.telemetryAttempts>=1&&result.state.telemetryAttempts<=2);
});
for(const variant of ["transient-html-audit","transient-html-telemetry","transient-invalid-json-telemetry"])
 test("transient HTML Cloudflare API readback is retried without replaying a mutation: "+variant,()=>{
   const result=run("verify-data",variant);
   assert.equal(result.status,0,result.error);
   assert.equal(result.accepted,true);
   assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0,
     "readback retries must not change public entry or redeploy");
 });
for(const variant of ["persistent-html-telemetry","html-success-telemetry","persistent-invalid-json-telemetry"])
 test("non-recoverable HTML Cloudflare response remains fail-closed: "+variant,()=>{
   const result=run("verify-data",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.accepted,false);
   assert.match(result.error,/NON_JSON/);
   assert.match(result.error,/cf_ray=fixture-ray-123/);
   // Only the read-only cursor query may retry.
   if(variant==="persistent-invalid-json-telemetry")
     assert.ok(result.state.telemetryAttempts>=3&&result.state.telemetryAttempts<=6);
   if(variant==="html-success-telemetry")
     assert.ok(result.state.telemetryAttempts>=1&&result.state.telemetryAttempts<=2);
   assert.equal(result.state.maxTelemetryInFlight,1);
   assert.ok(!result.error.includes("<!DOCTYPE"),
     "never print an HTML error body or upstream credentials");
 });
for(const variant of [
 "wrong-features","wrong-source","missing-step","missing-viewer-denial",
 "missing-tenant-list","missing-account-write","missing-audit","read-audited",
 "missing-logs","wrong-error-code","wrong-actor",
 "trace-redaction-disabled","trace-logs-disabled","trace-disabled",
 "trace-invocation-logs-enabled","missing-native-trace-id",
 "missing-native-span","missing-d1-span","wrong-native-trace-id",
 "unsafe-invocation-event","missing-private-probes",
 "synthetic-private-leak","synthetic-cookie-leak","synthetic-header-leak",
 "synthetic-authorization-leak","synthetic-jwt-leak","synthetic-query-leak"])test("release rejects "+variant,()=>{
 const result=run("verify-data",variant);
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
 // Deadline exhaustion must be an intentional release failure with its
 // original diagnostic, not a killed child or a missing fixture output.
 if(["missing-logs","wrong-error-code","missing-native-span","missing-d1-span","wrong-native-trace-id"].includes(variant)){
   assert.match(result.error,["missing-logs","wrong-error-code"].includes(variant)
     ?/Missing native Cloudflare traces/:/Published mutations missing exact Cloudflare D1 spans/);
   assert.equal(result.state.telemetryAttempts,60);
   assert.equal(result.state.maxTelemetryInFlight,1);
 }
});
test("deploy never reopens public entry with unsafe Cloudflare tracing settings",()=>{
  for(const variant of ["trace-redaction-disabled","trace-logs-disabled",
    "trace-invocation-logs-enabled","trace-disabled"]){
    const result=run("deploy",variant);
    assert.notEqual(result.status,0);
    assert.equal(result.deployed,false);
    assert.equal(result.state.published,true);
    assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,0,
      "No public-entry changes after unsafe trace settings readback");
    assert.match(result.error,/CRM_TRACE_PRIVACY_SETTINGS_UNSAFE/);
  }
});
test("unchanged D1 ledger keeps public entry online and signed version checks Access",()=>{
 const result=run("deploy");
 assert.equal(result.status,0,result.error);assert.equal(result.deployed,true);
 assert.equal(result.state.edgeAttempts,3);assert.equal(result.state.enabled,true);
 assert.equal(result.state.migrated,true);assert.equal(result.state.published,true);
 assert.equal(result.state.commands.filter(c=>c[2]==="d1"&&c[3]==="migrations").length,0);
 assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,1,
   "Code-only release must not stop public entry before deployment");
});
test("rebuilding the same SHA waits for the new active Worker version",()=>{
 const result=run("deploy","repeat-sha");
 assert.equal(result.status,0,result.error);
 assert.equal(result.state.postDeployReads,2);
 assert.equal(result.deployed,true);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
test("anonymous transport reset recovers through real deploy script without replaying writes",()=>{
 const result=run("deploy","anonymous-reset");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.anonymousAttempts,2);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
test("temporary anonymous edge 404 requires a later positive denial without redeploying",()=>{
 const result=run("deploy","anonymous-edge-404");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.anonymousAttempts,2);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
test("delayed Access edge propagation requires a positive denial, not a 404",()=>{
 const result=run("deploy","anonymous-delayed-edge-404");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.anonymousAttempts,9);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
});
for(const variant of ["anonymous-persistent-reset","anonymous-persistent-404","anonymous-success"])
 test("release rejects "+variant+" without replaying deployment",()=>{
   const result=run("deploy",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.deployed,false);
   assert.equal(result.state.anonymousAttempts,
   variant==="anonymous-success"?1:variant==="anonymous-persistent-404"?27:3);
   if(variant==="anonymous-persistent-404")
     assert.match(result.error,/CRM_ACCESS_ANONYMOUS_DENIAL_READBACK_DEADLINE_EXCEEDED/);
   assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,1);
 });
test("unknown D1 migration history rejects the release before changing public entry",()=>{
 const result=run("deploy","migration-history-drift");
 assert.notEqual(result.status,0);assert.equal(result.state.published,false);
 assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,0);
});
test("failed migration never publishes or reopens the Worker",()=>{
 const result=run("deploy","migration-fails");
 assert.notEqual(result.status,0);assert.equal(result.deployed,false);
 assert.equal(result.state.enabled,false);assert.equal(result.state.published,false);
 assert.equal(result.state.commands.filter(c=>c[2]==="deploy").length,0);
});
test("failure cleanup restores only the current run's signed E2E actor",()=>{
 const result=run("fail-closed","restore-role");
 assert.equal(result.status,0,result.error);assert.equal(result.stopped,true);
 assert.equal(result.state.role,"admin");
 assert.equal(result.posts.filter(path=>path.endsWith("/query")).length,2);
});
test("failure cleanup refuses to change a role in an unrelated tenant",()=>{
 const result=run("fail-closed","wrong-role-tenant");
 assert.notEqual(result.status,0);assert.equal(result.stopped,true);
 assert.equal(result.posts.filter(path=>path.endsWith("/query")).length,0);
});

test("NFC schema migration copies canonical names without changing receipts or audit",async()=>{
 const {DatabaseSync}=await import("node:sqlite");
 const {readdirSync}=await import("node:fs");
 const {verifyNfcMigrationNames}=await import("../scripts/release-readback.mjs");
 const db=new DatabaseSync(":memory:");
 try{
   const dir=new URL("../migrations/",import.meta.url);
   const migration="0016_nfc_search_name.sql";
   for(const name of readdirSync(dir).filter(n=>n.endsWith(".sql")&&n<migration).sort())
     db.exec(readFileSync(new URL(name,dir),"utf8"));
   db.exec("INSERT INTO crm_tenants VALUES('tenant-a','A')");
   const party=JSON.stringify({display_name:"Café Йошкар-Ола"});
   const account=JSON.stringify({name:"Café Йошкар-Ола"});
   db.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?)")
     .run("tenant-a","p",party,1,"p-create","digest","request","actor");
   db.prepare("INSERT INTO crm_account_records(tenant,id,primary_party,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?)")
     .run("tenant-a","a","p",account,1,"a-create","digest","request","actor");
   const read=async(sql,params)=>db.prepare(sql).all(...params);
   await verifyNfcMigrationNames(read);
   const counts=()=>["crm_party_receipts","crm_account_receipts","crm_audit"].map(t=>db.prepare("SELECT COUNT(*) AS n FROM "+t).get().n);
   const before=counts();
   db.exec(readFileSync(new URL(migration,dir),"utf8"));
   assert.deepEqual(counts(),before);
   assert.equal(db.prepare("SELECT search_name FROM crm_party_records WHERE id='p'").get().search_name,"Café Йошкар-Ола");
   assert.equal(db.prepare("SELECT search_name FROM crm_account_records WHERE id='a'").get().search_name,"Café Йошкар-Ола");
   assert.throws(()=>db.prepare("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?)")
     .run("tenant-a","missing-key",party,1,"bad-create","digest","request","actor"),/NFC_SEARCH_NAME_REQUIRED/);
   assert.deepEqual(counts(),before);
   assert.equal(db.prepare("SELECT state FROM crm_party_records WHERE id='p'").get().state,party);
   assert.equal(db.prepare("SELECT state FROM crm_account_records WHERE id='a'").get().state,account);
 }finally{db.close();}
});
test("NFC migration preflight fails closed and does not expose names",async()=>{
 const {verifyNfcMigrationNames}=await import("../scripts/release-readback.mjs");
 await assert.rejects(verifyNfcMigrationNames(async()=>[{tenant:"tenant-a",id:"p",name:"private-Cafe\u0301"}]),
  error=>error.message==="CRM_NFC_MIGRATION_NONCANONICAL_EXISTING_NAME crm_party_records");
 let calls=0;
 await verifyNfcMigrationNames(async(sql,params)=>{
   calls++;
   if(calls===1){assert.deepEqual(params,[]);return Array.from({length:200},(_,i)=>({tenant:"t",id:String(i).padStart(3,"0"),name:"Café"}));}
   if(calls===2)assert.deepEqual(params,["t","199"]);
   return [];
 });
 assert.equal(calls,3);
});

test("pending NFC migration runs under the same protected release path",()=>{
 const result=run("deploy","nfc-migration");
 assert.equal(result.status,0,result.error);
 assert.equal(result.deployed,true);
 assert.equal(result.state.migrated,true);
 assert.equal(result.state.enabled,true);
 assert.equal(result.state.commands.filter(c=>c[2]==="d1"&&c[3]==="migrations").length,1);
});
test("noncanonical existing names block release before checkpoint and traffic closure",()=>{
 const result=run("deploy","nfc-noncanonical");
 assert.notEqual(result.status,0);
 assert.match(result.error,/CRM_NFC_MIGRATION_NONCANONICAL_EXISTING_NAME/);
 assert.equal(result.checkpoint,false);
 assert.equal(result.state.enabled,true);
 assert.equal(result.state.published,false);
 assert.equal(result.state.commands.filter(c=>c[2]==="d1"&&c[3]==="migrations").length,0);
 assert.equal(result.posts.filter(p=>p.endsWith("/subdomain")).length,0);
});

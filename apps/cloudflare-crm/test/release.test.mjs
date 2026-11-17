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
 const {mode,variant,sha,database,audit,requestId,rpcPath}=JSON.parse(process.env.CRM_RELEASE_TEST_FIXTURE);
 const posts=[];
 let enabled=mode==="deploy",migrated=variant!=="migration-fails",published=false,edgeAttempts=0,role="viewer";
 let auditAttempts=0,telemetryAttempts=0,anonymousAttempts=0;
 const commands=[];
 let db;
 if(mode==="deploy"){
   const {DatabaseSync}=await import("node:sqlite");
   db=new DatabaseSync(":memory:");
   // The ordinary release starts with the full current SQL schema.
   if(migrated)for(const name of readdirSync("apps/cloudflare-crm/migrations").filter(n=>n.endsWith(".sql")).sort())
     db.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
   const child=(await import("node:child_process")).default;
   child.spawnSync=(command,args)=>{
     commands.push([command,...args]);
     if(command==="sha256sum")return{status:0}; // Synthetic sealed files, not a real build.
     assert.equal(command,"npx");assert.equal(args[0],"wrangler");
     if(args[1]==="d1"){
       assert.equal(enabled,false,"Schema changes require closed public entry");
       assert.deepEqual(args,["wrangler","d1","migrations","apply","crm-dev","--remote"]);
       if(variant==="migration-fails")return{status:1};
       for(const name of readdirSync("apps/cloudflare-crm/migrations").sort())
         db.exec(readFileSync("apps/cloudflare-crm/migrations/"+name,"utf8"));
       migrated=true;
     }else{
       assert.deepEqual(args,["wrangler","deploy","--var","GIT_SHA:"+sha]);
       assert.equal(migrated,true);published=true;
     }
     return{status:0};
   };
   (await import("node:module")).syncBuiltinESMExports();
 }
 globalThis.setTimeout=callback=>{queueMicrotask(callback);return 0;};
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
   if(path.endsWith("/deployments"))return wrap({deployments:[{versions:[{version_id:mode==="deploy"&&!published?"version-old":"version-new",percentage:100}]}]});
   if(path.endsWith("/versions/version-old"))return wrap({resources:{bindings:[
     {name:"GIT_SHA",text:"90a352be2a70b74a64e5a774c9a76b7f02c86a5f"},{name:"DB",id:database}]}});
   if(path.endsWith("/time_travel/bookmark"))return wrap({bookmark:"synthetic-bookmark"});
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
     if(mode==="verify-data"&&variant==="transient-html-telemetry"&&telemetryAttempts===1)
       return html(503);
     if(mode==="verify-data"&&variant==="persistent-html-telemetry")
       return html(503);
     if(mode==="verify-data"&&variant==="html-success-telemetry")
       return html(200);
     if(mode==="verify-data"&&((variant==="transient-invalid-json-telemetry"&&telemetryAttempts===1)||
        variant==="persistent-invalid-json-telemetry"))
       return new Response('{"success":true,"result":',{status:200,
         headers:{"content-type":"application/json","cf-ray":"fixture-ray-123"}});
     return wrap({events:{events:variant==="missing-logs"?[]:[{
       source:{event:"crm.native.request",request_id:requestId,operation:rpcPath,status:200,
         error_code:variant==="wrong-error-code"?"aborted":null},$workers:{scriptVersion:{id:"version-new"}}
     }]}});
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
       const {sql}=JSON.parse(options.body);
       const names=readdirSync("apps/cloudflare-crm/migrations").filter(n=>n.endsWith(".sql")).sort();
       const ledger=variant==="migration-fails"?names.slice(0,1):
         variant==="migration-history-drift"?["unrecognized.sql"]:names;
       const results=sql.includes("FROM d1_migrations")?ledger.map(name=>({name})):
         !migrated?[]:db.prepare(sql).all();
       return wrap([{success:true,results}]);
     }
     return wrap([{success:true,results:variant==="missing-audit"?[]:audit}]);
   }
   throw Error("Unexpected Cloudflare endpoint "+path);
 };
 process.argv[2]=mode;
 try{await import("./deploy.mjs");}finally{
   writeFileSync("posts.json",JSON.stringify(posts));
   writeFileSync("state.json",JSON.stringify({enabled,migrated,published,edgeAttempts,anonymousAttempts,telemetryAttempts,commands,role}));
   db?.close();
 }
}
function run(mode,variant="valid"){
 const dir=mkdtempSync(join(tmpdir(),"crm-release-test-"));
 const app=join(dir,"apps/cloudflare-crm"),out=join(app,".release");
 mkdirSync(out,{recursive:true});mkdirSync(join(app,"e2e-evidence"));
 for(const name of ["deploy.mjs","release-readback.mjs"])
   copyFileSync(new URL("../scripts/"+name,import.meta.url),join(dir,name));
 const save=(path,value)=>writeFileSync(path,JSON.stringify(value));
 if(variant!=="no-checkpoint")save(join(out,"checkpoint.json"),{git_sha:sha,previous_sha:previous,d1_database:database});
 save(join(out,"deployed.json"),{git_sha:sha,worker_version:"version-new",started_at:1});
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
 const rpcPath="/crm.parties.v3.PartyService/CreateParty",audit=[{action:"parties.create",subject_id:"party",
   actor:variant==="wrong-actor"?"forged@example.test":"service:synthetic.access"}];
 save(join(app,"e2e-evidence/typed-browser.json"),{result:"PASS",source_sha:sha,features:variant==="wrong-features"?["native-ci"]:["native"],
   tenant:"fixture-a",started_at:2,steps:["synthetic fixture"],requests:[
     {path:rpcPath,status:200,request_id:requestId,source_sha:sha,runtime:"native-workers-rs"}],
   writes:[{path:rpcPath,request_id:requestId,audit}]});
 try{
   const result=spawnSync(process.execPath,["--input-type=module","-e","("+subprocessEntry.toString()+")()"],{cwd:dir,encoding:"utf8",
     env:{...process.env,GITHUB_SHA:sha,GITHUB_REF:"refs/heads/dev",GITHUB_EVENT_NAME:"push",
       GITHUB_RUN_ID:"123",GITHUB_RUN_ATTEMPT:"1",
       CLOUDFLARE_ACCOUNT_ID:"5bd5e1e1e813f061afc259e91334262d",CRM_D1_DATABASE_ID:database,
       CLOUDFLARE_API_TOKEN:"synthetic-token",CRM_E2E_A_CLIENT_ID:"synthetic.access",CRM_E2E_A_CLIENT_SECRET:"synthetic-secret",
       CRM_RELEASE_TEST_FIXTURE:JSON.stringify({mode,variant,sha,database,audit,requestId,rpcPath})}});
   return {status:result.status,error:result.stderr,posts:JSON.parse(readFileSync(join(dir,"posts.json"),"utf8")),
     state:JSON.parse(readFileSync(join(dir,"state.json"),"utf8")),deployed:existsSync(join(out,"deployed.json")),
     stopped:existsSync(join(out,"failed-release.json")),accepted:existsSync(join(out,"accepted.json"))};
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
    if(variant==="persistent-invalid-json-telemetry")assert.equal(result.state.telemetryAttempts,3);
    if(variant==="html-success-telemetry")assert.equal(result.state.telemetryAttempts,1);
   assert.ok(!result.error.includes("<!DOCTYPE"),
     "never print an HTML error body or upstream credentials");
 });
for(const variant of ["wrong-features","missing-audit","missing-logs","wrong-error-code","wrong-actor"])test("release rejects "+variant,()=>{
 const result=run("verify-data",variant);
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
});
test("unchanged D1 ledger keeps public entry online and signed version checks Access",()=>{
 const result=run("deploy");
 assert.equal(result.status,0,result.error);assert.equal(result.deployed,true);
 assert.equal(result.state.edgeAttempts,3);assert.equal(result.state.enabled,true);
 assert.equal(result.state.migrated,true);assert.equal(result.state.published,true);
 assert.equal(result.state.commands.filter(c=>c[2]==="migrations").length,0);
 assert.equal(result.posts.filter(path=>path.endsWith("/subdomain")).length,1,
   "Code-only release must not stop public entry before deployment");
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
for(const variant of ["anonymous-persistent-reset","anonymous-persistent-404","anonymous-success"])
 test("release rejects "+variant+" without replaying deployment",()=>{
   const result=run("deploy",variant);
   assert.notEqual(result.status,0);
   assert.equal(result.deployed,false);
   assert.equal(result.state.anonymousAttempts,variant==="anonymous-success"?1:3);
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

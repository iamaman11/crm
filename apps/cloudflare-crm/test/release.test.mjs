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
 let enabled=false,migrated=false,published=false,edgeAttempts=0,role="viewer";
 const commands=[];
 let db;
 if(mode==="deploy"){
   const {DatabaseSync}=await import("node:sqlite");
   db=new DatabaseSync(":memory:");
   const child=(await import("node:child_process")).default;
   child.spawnSync=(command,args)=>{
     commands.push([command,...args]);
     if(command==="sha256sum")return{status:0}; // Synthetic sealed files, not a real build.
     assert.equal(command,"npx");assert.equal(args[0],"wrangler");
     assert.equal(enabled,false,"Migrations and upload require closed public entry");
     if(args[1]==="d1"){
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
   if(new URL(url).hostname==="crm-dev.crm-5bd5e1e1.workers.dev"){
     if(mode==="fail-closed")return new Response("disabled",{status:404});
     if(mode==="deploy"){
       if(!enabled)return new Response("disabled",{status:404});
       if(path==="/api/version"&&++edgeAttempts<=2)return new Response("propagating",{status:404});
       if(path==="/api/health"){
         assert.ok(edgeAttempts>=3,"Health checked before expected source reached the edge");
         return new Response("health",{status:options.headers?200:302});
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
   if(path.endsWith("/telemetry/query"))return wrap({events:{events:variant==="missing-logs"?[]:[{
     source:{event:"crm.native.request",request_id:requestId,operation:rpcPath,status:200,
       error_code:variant==="wrong-error-code"?"aborted":null},$workers:{scriptVersion:{id:"version-new"}}
   }]}});
   if(path.endsWith("/query")){
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
       const results=sql.includes("FROM d1_migrations")?(migrated?[{name:"0002_hard_cutover.sql"}]:[]):
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
   writeFileSync("state.json",JSON.stringify({enabled,migrated,published,edgeAttempts,commands,role}));
   db?.close();
 }
}
function run(mode,variant="valid"){
 const dir=mkdtempSync(join(tmpdir(),"crm-release-test-"));
 const app=join(dir,"apps/cloudflare-crm"),out=join(app,".release");
 mkdirSync(out,{recursive:true});mkdirSync(join(app,"e2e-evidence"));
 copyFileSync(new URL("../scripts/deploy.mjs",import.meta.url),join(dir,"deploy.mjs"));
 const save=(path,value)=>writeFileSync(path,JSON.stringify(value));
 if(variant!=="no-checkpoint")save(join(out,"checkpoint.json"),{git_sha:sha,previous_sha:previous,d1_database:database});
 save(join(out,"deployed.json"),{git_sha:sha,worker_version:"version-new",started_at:1});
 if(mode==="verify-data")save(join(out,"e2e-tenants.json"),{tenant_a:"fixture-a"});
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
 const rpcPath="/crm.parties.v3.PartyService/CreateParty",audit=[{action:"parties.create",subject_id:"party"}];
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
for(const variant of ["wrong-features","missing-audit","missing-logs","wrong-error-code"])test("release rejects "+variant,()=>{
 const result=run("verify-data",variant);
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
});
test("deploy waits for temporary edge 404s before checking authenticated health",()=>{
 const result=run("deploy");
 assert.equal(result.status,0,result.error);assert.equal(result.deployed,true);
 assert.equal(result.state.edgeAttempts,3);assert.equal(result.state.enabled,true);
 assert.equal(result.state.migrated,true);assert.equal(result.state.published,true);
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

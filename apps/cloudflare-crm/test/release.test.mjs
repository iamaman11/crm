// Tests of the real release script with a local Cloudflare API substitute.
// Synthetic fixtures do not constitute live acceptance evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {mkdtempSync,mkdirSync,copyFileSync,writeFileSync,readFileSync,rmSync,existsSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {spawnSync} from "node:child_process";
const sha="a".repeat(40),previous="b".repeat(40),database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
const requestId="11111111-1111-4111-8111-111111111111";
async function subprocessEntry(){
 const {default:assert}=await import("node:assert/strict");
 const {writeFileSync}=await import("node:fs");
 const {mode,variant,sha,database,audit,requestId,rpcPath}=JSON.parse(process.env.CRM_RELEASE_TEST_FIXTURE);
 const posts=[];
 globalThis.setTimeout=callback=>{queueMicrotask(callback);return 0;};
 globalThis.fetch=async(url,options={})=>{
   const path=new URL(url).pathname;
   const json=value=>new Response(JSON.stringify(value),{headers:{"content-type":"application/json"}});
   if(new URL(url).hostname==="crm-dev.crm-5bd5e1e1.workers.dev"){
     if(mode==="fail-closed")return new Response("disabled",{status:404});
     return json({source:sha});
   }
   assert.equal(new URL(url).hostname,"api.cloudflare.com","Unexpected network target");
   if(options.method==="POST")posts.push(path);
   const wrap=result=>json({success:true,result});
   if(path.endsWith("/deployments"))return wrap({deployments:[{versions:[{version_id:"version-new",percentage:100}]}]});
   if(path.endsWith("/versions/version-new"))return wrap({resources:{bindings:[
     {name:"GIT_SHA",text:variant==="unrelated"?"c".repeat(40):sha},
     {name:"DB",id:variant==="wrong-database"?"other":database}]}});
   if(path.endsWith("/subdomain")){
     if(options.method==="POST")assert.deepEqual(JSON.parse(options.body),{enabled:false,previews_enabled:false});
     return wrap({enabled:false,previews_enabled:false});
   }
   if(path.endsWith("/telemetry/query"))return wrap({events:{events:variant==="missing-logs"?[]:[{
     source:{event:"crm.native.request",request_id:requestId,operation:rpcPath,status:200},$workers:{scriptVersion:{id:"version-new"}}
   }]}});
   if(path.endsWith("/query"))return wrap([{success:true,results:variant==="missing-audit"?[]:audit}]);
   throw Error("Unexpected Cloudflare endpoint "+path);
 };
 process.argv[2]=mode;
 try{await import("./deploy.mjs");}finally{writeFileSync("posts.json",JSON.stringify(posts));}
}
function run(mode,variant="valid"){
 const dir=mkdtempSync(join(tmpdir(),"crm-release-test-"));
 const app=join(dir,"apps/cloudflare-crm"),out=join(app,".release");
 mkdirSync(out,{recursive:true});mkdirSync(join(app,"e2e-evidence"));
 copyFileSync(new URL("../scripts/deploy.mjs",import.meta.url),join(dir,"deploy.mjs"));
 const save=(path,value)=>writeFileSync(path,JSON.stringify(value));
 if(variant!=="no-checkpoint")save(join(out,"checkpoint.json"),{git_sha:sha,previous_sha:previous,d1_database:database});
 save(join(out,"deployed.json"),{git_sha:sha,worker_version:"version-new",started_at:1});
 save(join(out,"e2e-tenants.json"),{tenant_a:"fixture-a"});
 const rpcPath="/crm.parties.v3.PartyService/CreateParty",audit=[{action:"parties.create",subject_id:"party"}];
 save(join(app,"e2e-evidence/typed-browser.json"),{result:"PASS",source_sha:sha,features:variant==="wrong-features"?["native-ci"]:["native"],
   tenant:"fixture-a",started_at:2,steps:["synthetic fixture"],requests:[
     {path:rpcPath,status:200,request_id:requestId,source_sha:sha,runtime:"native-workers-rs"}],
   writes:[{path:rpcPath,request_id:requestId,audit}]});
 try{
   const result=spawnSync(process.execPath,["--input-type=module","-e","("+subprocessEntry.toString()+")()"],{cwd:dir,encoding:"utf8",
     env:{...process.env,GITHUB_SHA:sha,GITHUB_REF:"refs/heads/dev",GITHUB_EVENT_NAME:"push",
       CLOUDFLARE_ACCOUNT_ID:"5bd5e1e1e813f061afc259e91334262d",CRM_D1_DATABASE_ID:database,
       CLOUDFLARE_API_TOKEN:"synthetic-token",CRM_E2E_A_CLIENT_ID:"synthetic.access",CRM_E2E_A_CLIENT_SECRET:"synthetic-secret",
       CRM_RELEASE_TEST_FIXTURE:JSON.stringify({mode,variant,sha,database,audit,requestId,rpcPath})}});
   return {status:result.status,error:result.stderr,posts:JSON.parse(readFileSync(join(dir,"posts.json"),"utf8")),
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
for(const variant of ["wrong-features","missing-audit","missing-logs"])test("release rejects "+variant,()=>{
 const result=run("verify-data",variant);
 assert.notEqual(result.status,0);assert.equal(result.accepted,false);
});

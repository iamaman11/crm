// Isolated native-ci RSA-signed Access proof of intentional contract-only shutdown.
import {spawn,execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign} from "node:crypto";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const data=mkdtempSync(join(tmpdir(),"crm-native-contract-only-"));
const base="http://127.0.0.1:18789";
const run=(args)=>execFileSync("npx",["wrangler",...args],{cwd,encoding:"utf8"});
const {privateKey,publicKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const kid="local-signed-rsa-fixture";
const jwks=JSON.stringify({keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]});
const iss="https://royal-limit-ea43.cloudflareaccess.com";
const aud="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const now=Math.floor(Date.now()/1000);
function jwt(email){
 const parts=[{alg:"RS256",kid,typ:"JWT"},{
   iss,aud,exp:now+3600,iat:now-10,sub:email,email
 }].map(x=>Buffer.from(JSON.stringify(x)).toString("base64url"));
 const raw=parts.join(".");
 return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(privateKey).toString("base64url");
}
let child;
try{
 run(["d1","execute","crm-rust-party-local","--local",
   "--file","./migrations/0001_local.sql","--persist-to",data]);
 run(["d1","execute","crm-rust-party-local","--local","--persist-to",data,
   "--command","CREATE TABLE crm_tenants(id TEXT PRIMARY KEY,name TEXT NOT NULL);"+
   "INSERT INTO crm_tenants VALUES('tenant-a','A');"+
   "CREATE TABLE crm_users(email TEXT PRIMARY KEY,tenant_id TEXT NOT NULL,role TEXT NOT NULL,active INTEGER NOT NULL);"+
   "INSERT INTO crm_users VALUES('native-a@example.test','tenant-a','admin',1);"+
   "INSERT INTO crm_users VALUES('native-viewer@example.test','tenant-a','viewer',1);"]);
 child=spawn("npx",["wrangler","dev","--local","--port","18789","--persist-to",data,
  "--var","CRM_CI_LOCAL_ONLY:yes","--var","CRM_NATIVE_CI_JWKS:"+jwks],
  {cwd,detached:true,stdio:["ignore","ignore","pipe"]});
 let stderr="";
 child.stderr.on("data",b=>stderr=(stderr+b.toString()).slice(-2500));
 let ready=false;
 for(let i=0;i<120;i++){
   if(child.exitCode!==null)throw Error("native-ci exited "+stderr);
   try{if((await fetch(base+"/api/parties")).status===401){ready=true;break;}}catch{}
   await new Promise(resolve=>setTimeout(resolve,250));
 }
 assert(ready,"native-ci not ready: "+stderr);
 async function call(path,token,method="GET"){
   const response=await fetch(base+path,{method,
     headers:{...(token?{"cf-access-jwt-assertion":token}:{}),"content-type":"application/json"},
     ...(method==="POST"?{body:"{}"}:{})});
   return {status:response.status,body:await response.json()};
 }
 assert.equal((await call("/api/parties",null)).status,401);
 const admin=jwt("native-a@example.test"),viewer=jwt("native-viewer@example.test");
 for(const [path,method] of [["/api/parties","GET"],["/api/parties","POST"],
     ["/api/accounts","GET"],["/crm.sales.v1.DealService/GetDeal","POST"],
     ["/crm.future.v9.FutureService/NotRegistered","POST"]]){
   const r=await call(path,admin,method);
   assert.equal(r.status,410,path);
   assert.equal(r.body.code,"LEGACY_TRANSPORT_DISABLED");
 }
 assert.equal((await call("/api/parties",viewer)).status,410);
 assert.equal((await call("/crm.future.v9.FutureService/NotRegistered",viewer,"POST")).status,410);
 console.log(JSON.stringify({event:"crm.contract-only-signed-access",
   result:"PASS",admin:true,viewer:true,unauthenticated_denied:true,
   legacy_mutations:0,verified_auth:true}));
}finally{
 if(child){
   try{process.kill(-child.pid,"SIGTERM");}catch(e){if(e.code!=="ESRCH")throw e;}
   if(child.exitCode===null)await Promise.race([new Promise(resolve=>child.once("exit",resolve)),
     new Promise(resolve=>setTimeout(resolve,1400))]);
   try{process.kill(-child.pid,"SIGKILL");}catch(e){if(e.code!=="ESRCH")throw e;}
   child.stderr.destroy();child.unref();
 }
 rmSync(data,{recursive:true,force:true});
}

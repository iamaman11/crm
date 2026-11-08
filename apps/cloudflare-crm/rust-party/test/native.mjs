// Exact native workerd Access RS256 proof, isolated D1 and signed CI-only JWTs.
import {spawn,execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,randomUUID} from "node:crypto";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const data=mkdtempSync(join(tmpdir(),"crm-native-jwt-"));
const origin="http://127.0.0.1:18789";
const run=args=>execFileSync("npx",["wrangler",...args],{cwd,encoding:"utf8"});
const {privateKey,publicKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const kid="local-signed-rsa-fixture";
const jwks=JSON.stringify({keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]});
const iss="https://royal-limit-ea43.cloudflareaccess.com";
const aud="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const now=Math.floor(Date.now()/1000);
const jwt=(claims={},header={})=>{
  const parts=[{alg:"RS256",kid,typ:"JWT",...header},
    {iss,aud,exp:now+3600,iat:now-10,sub:"native-test-a",email:"native-a@example.test",...claims}]
    .map(x=>Buffer.from(JSON.stringify(x)).toString("base64url"));
  const input=parts.join(".");
  return input+"."+createSign("RSA-SHA256").update(input).end().sign(privateKey).toString("base64url");
};
const call=async(path,token,init={})=>{
  const response=await fetch(origin+path,{...init,headers:{
    ...(token?{"cf-access-jwt-assertion":token}:{}),...(init.headers||{})}});
  return {status:response.status,body:await response.json()};
};
let child;
try{
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","./migrations/0001_local.sql","--persist-to",data]);
  const sql="CREATE TABLE crm_users (email TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role TEXT NOT NULL, active INTEGER NOT NULL);"+
    "INSERT INTO crm_users VALUES('native-a@example.test','tenant-a','admin',1);"+
    "INSERT INTO crm_users VALUES('native-b@example.test','tenant-b','admin',1);";
  run(["d1","execute","crm-rust-party-local","--local",
    "--command",sql,"--persist-to",data]);
  child=spawn("npx",["wrangler","dev","--local","--port","18789","--persist-to",data,
    "--var","CRM_CI_LOCAL_ONLY:yes","--var","CRM_NATIVE_CI_JWKS:"+jwks],
    {cwd,detached:true,stdio:["ignore","ignore","pipe"]});
  let stderr="";
  child.stderr.on("data",b=>{stderr=(stderr+b.toString()).slice(-4000);});
  let ready=false;
  for(let i=0;i<100;i++){
    if(child.exitCode!==null)throw Error("native workerd exited "+stderr);
    try{if((await fetch(origin+"/api/parties")).status===401){ready=true;break;}}catch{}
    await new Promise(resolve=>setTimeout(resolve,250));
  }
  assert.ok(ready,"native workerd not ready: "+stderr);
  assert.equal((await call("/api/parties",null)).status,401);
  assert.equal((await call("/api/parties","broken")).status,401);
  assert.equal((await call("/api/parties",jwt({exp:now-1}))).status,401);
  assert.equal((await call("/api/parties",jwt({iss:"https://fake.example"}))).status,401);
  assert.equal((await call("/api/parties",jwt({aud:"wrong"}))).status,401);
  assert.equal((await call("/api/parties",jwt({}, {alg:"HS256"}))).status,401);
  assert.equal((await call("/api/parties",jwt({}, {kid:"missing"}))).status,401);
  assert.equal((await call("/api/parties",jwt({email:"not-enrolled@example.test"}))).status,401);
  const a=jwt(),b=jwt({email:"native-b@example.test",sub:"native-test-b"});
  const parts=a.split(".");
  parts[1]=Buffer.from(JSON.stringify({iss,aud,exp:now+3600,sub:"forged",email:"native-b@example.test"})).toString("base64url");
  assert.equal((await call("/api/parties",parts.join("."))).status,401,
    "modified claims must fail signature");
  assert.equal((await call("/api/parties",a)).status,200,"signed A accepted");
  assert.equal((await call("/api/native-modules/proof",null)).status,401);
  const probe=await call("/api/native-modules/proof",a);
  assert.equal(probe.status,200,JSON.stringify(probe.body));
  assert.equal(probe.body.result,"PASS");
  assert.equal(probe.body.original_crates_called,13);
  assert.equal(probe.body.modules.length,13);
  assert.equal(new Set(probe.body.modules.map(x=>x.module)).size,13);
  assert.ok(probe.body.modules.every(x=>x.passed===true && x.d1_persistence==="NOT_TESTED"));
  assert.equal(probe.body.full_application_integrations_accepted,0,
    "smoke proof must not claim fully integrated modules");
  assert.equal((await call("/api/parties",b)).status,200,"signed B accepted");
  const id="native-jwt-"+randomUUID();
  const created=await call("/api/parties",a,{method:"POST",headers:{
    "content-type":"application/json","idempotency-key":"native-signed-create",
    "x-crm-native-tenant":"tenant-b","x-crm-ci-tenant":"tenant-b"},
    body:JSON.stringify({id,kind:"organization",name:"  Original  Native  Rust  "})});
  assert.equal(created.status,201,JSON.stringify(created));
  assert.equal(created.body.item.name,"Original Native Rust");
  assert.equal((await call("/api/parties/"+id,a)).status,200);
  assert.equal((await call("/api/parties/"+id,b)).status,404,"B cannot read A");
  assert.equal((await call("/api/parties/"+id,b,{headers:{
    "x-crm-native-tenant":"tenant-a","x-crm-ci-tenant":"tenant-a"}})).status,404);
  const init={method:"PATCH",headers:{"content-type":"application/json",
    "idempotency-key":"native-signed-update"},
    body:JSON.stringify({expected_version:1,name:"Native Updated"})};
  const changed=await call("/api/parties/"+id,a,init);
  assert.equal(changed.status,200,JSON.stringify(changed));
  assert.equal(changed.body.item.version,2);
  const replay=await call("/api/parties/"+id,a,init);
  assert.equal(replay.status,200);
  assert.equal(replay.body.replayed,true);
  console.log(JSON.stringify({event:"crm.native-jwt-workerd",result:"PASS",
    proven:["host WebCrypto RS256 verified","issuer/audience/expiry/alg/kid/forged denied",
      "A/B D1 tenant isolation","untrusted tenant headers denied",
      "original Rust Party create/update/replay"]}));
}finally{
  if(child){
    try{process.kill(-child.pid,"SIGTERM");}catch(e){if(e.code!=="ESRCH")throw e;}
    if(child.exitCode===null)await Promise.race([
      new Promise(r=>child.once("exit",r)),new Promise(r=>setTimeout(r,1500))]);
    try{process.kill(-child.pid,"SIGKILL");}catch(e){if(e.code!=="ESRCH")throw e;}
    child.stderr.destroy();child.unref();
  }
  rmSync(data,{recursive:true,force:true});
}

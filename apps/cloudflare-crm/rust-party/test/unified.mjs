import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const persist=mkdtempSync(join(tmpdir(),"crm-native-unified-"));
const origin="http://127.0.0.1:18788", env={...process.env,NO_COLOR:"1"};
const cfg="./wrangler.unified.jsonc";
const wrangler=(args)=>execFileSync("npx",["wrangler",...args],{
  cwd,env,encoding:"utf8"
});
let child;
let stdout="";
const json=async(path,init={})=>{
  const r=await fetch(origin+path,init);
  return {status:r.status,body:await r.json()};
};
const headers=(tenant,key)=>({
  ...(tenant?{"x-crm-ci-tenant":tenant}:{}),
  ...(key?{"idempotency-key":key}:{}),
  "content-type":"application/json",
  "x-crm-native-tenant":"forged-tenant-from-the-browser",
});
try {
  wrangler(["d1","execute","crm-rust-party-local","--config",cfg,"--local",
    "--file","./migrations/0001_local.sql","--persist-to",persist]);
  child=spawn("npx",["wrangler","dev","--config",cfg,"--local","--port","18788",
    "--persist-to",persist],{cwd,env,detached:true,stdio:["ignore","pipe","pipe"]});
  child.stdout.on("data",b=>{stdout=(stdout+b.toString()).slice(-8000);});
  let stderr="";
  child.stderr.on("data",b=>{stderr=(stderr+b.toString()).slice(-5000);});
  let available=false;
  for(let i=0;i<100;i++){
    if(child.exitCode!==null)throw Error("unified workerd stopped: "+stderr);
    try{const r=await fetch(origin+"/api/health");if(r.status===200){available=true;break;}}catch{}
    await new Promise(r=>setTimeout(r,250));
  }
  if(!available)throw Error("unified worker did not start: "+stderr);
  assert.equal((await json("/api/health")).status,200,
    "existing CRM JS non-PARTY routes still operate");
  assert.equal((await json("/api/parties",{headers:headers(null)})).status,401,
    "forged internal tenant cannot bypass the public entrypoint");
  const id="unified-original-rust-a";
  const created=await json("/api/parties",{
    method:"POST",headers:headers("tenant-a","unified-create-a"),
    body:JSON.stringify({id,kind:"organization",name:"  Native  CRM "})});
  if(created.status!==201) console.error("UNIFIED_WORKER_DIAGNOSTICS",JSON.stringify({stdout,stderr}));
  assert.equal(created.status,201,JSON.stringify(created));
  assert.equal(created.body.item.name,"Native CRM");
  const own=(await json("/api/parties/"+id,{headers:headers("tenant-a")}));
  assert.equal(own.status,200,JSON.stringify(own));
  assert.equal(own.body.item.version,1);
  assert.equal((await json("/api/parties/"+id,{headers:headers("tenant-b")})).status,404);
  const modified=await json("/api/parties/"+id,{
    method:"PATCH",headers:headers("tenant-a","unified-update-a"),
    body:JSON.stringify({name:"Native CRM Updated",expected_version:1})});
  assert.equal(modified.status,200,JSON.stringify(modified));
  assert.equal(modified.body.item.version,2);
  const replay=await json("/api/parties/"+id,{
    method:"PATCH",headers:headers("tenant-a","unified-update-a"),
    body:JSON.stringify({name:"Native CRM Updated",expected_version:1})});
  assert.equal(replay.status,200);
  assert.equal(replay.body.replayed,true);
  const b=await json("/api/parties",{
    method:"POST",headers:headers("tenant-b","unified-create-b"),
    body:JSON.stringify({id,kind:"person",name:"Separate Tenant"})});
  assert.equal(b.status,201,JSON.stringify(b));
  const ownB=await json("/api/parties/"+id,{headers:headers("tenant-b")});
  assert.equal(ownB.body.item.name,"Separate Tenant");
  const anonymous=await json("/api/parties/"+id,{headers:headers(null)});
  assert.equal(anonymous.status,401);
  const pending=(await json("/api/parties",{headers:headers("tenant-a")}));
  assert.equal(pending.body.items.length,1);
  console.log(JSON.stringify({event:"crm.unified-rust-party-workerd",result:"PASS",
    tested:["single Worker module / JS CRM health", "original Rust domain via WASM",
      "D1 persistence and version check", "A/B tenant isolation",
      "idempotency replay", "untrusted internal header stripped"]}));
}finally{
 if(child){
  try{process.kill(-child.pid,"SIGTERM");}catch(e){if(e.code!=="ESRCH")throw e;}
  if(child.exitCode===null)await Promise.race([
    new Promise(r=>child.once("exit",r)),new Promise(r=>setTimeout(r,1500))]);
  try{process.kill(-child.pid,"SIGKILL");}catch(e){if(e.code!=="ESRCH")throw e;}
  child.stdout.destroy();child.stderr.destroy();child.unref();
 }
 rmSync(persist,{recursive:true,force:true});
}

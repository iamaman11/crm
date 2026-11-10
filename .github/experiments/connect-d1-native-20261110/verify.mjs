// Real workerd runtime proof with isolated native D1. No production deployment.
import {spawn, execFileSync} from "node:child_process";
import {mkdtempSync, rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname, join, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";

const root=dirname(fileURLToPath(import.meta.url));
const wrangler=resolve(root,"../../../apps/cloudflare-crm/node_modules/.bin/wrangler");
const config=join(root,"wrangler.jsonc");
const persist=mkdtempSync(join(tmpdir(),"crm-connect-native-d1-"));
const port=18792, url="http://127.0.0.1:"+port;
let child,stderr="";
const call=async(marker)=>{
  const r=await fetch(url+"/crm.compatproof.v1.ProofService/Ping",{
    method:"POST",headers:{"content-type":"application/json",
      "connect-protocol-version":"1"},
    body:JSON.stringify({marker})
  });
  const body=await r.text();
  return {status:r.status,headers:Object.fromEntries(r.headers),body,
    json:(()=>{try{return JSON.parse(body)}catch{return null}})()};
};
try{
  child=spawn(wrangler,["dev","--local","--config",config,"--port",String(port),
    "--persist-to",persist],{cwd:root,detached:true,stdio:["ignore","ignore","pipe"]});
  child.stderr.on("data",chunk=>{stderr=(stderr+chunk.toString()).slice(-9000)});
  let ready=false;
  for(let i=0;i<140;i++){
    if(child.exitCode!==null)throw Error("wrangler quit: "+stderr);
    try{const r=await fetch(url+"/__readiness");if(r.status){ready=true;break}}catch{}
    await new Promise(done=>setTimeout(done,250));
  }
  assert.ok(ready,"workerd did not become ready: "+stderr);
  const marker="native-d1-"+Date.now().toString(36);
  const first=await call(marker);
  assert.equal(first.status,200,"Connect call failed: "+JSON.stringify(first)+" STDERR:"+stderr);
  assert.equal(first.json?.marker,marker);
  assert.equal(Number(first.json?.d1Readback ?? first.json?.d1_readback),1);
  const second=await call(marker);
  assert.equal(second.status,200,"second Connect call failed "+JSON.stringify(second));
  assert.equal(Number(second.json?.d1Readback ?? second.json?.d1_readback),1);
  const out=execFileSync(wrangler,["d1","execute","crm-connect-proof-local",
    "--config",config,"--local",
    "--command","SELECT marker FROM crm_connect_native_proof WHERE marker = '"+marker+"'",
    "--persist-to",persist,"--json"],{cwd:root,encoding:"utf8"});
  const rows=JSON.parse(out);
  assert.equal(rows.length,1);
  assert.equal(rows[0].success,true);
  assert.deepEqual(rows[0].results,[{marker}],"independent real D1 readback");
  console.log(JSON.stringify({result:"PASS",worker:"workers-rs 0.8.7",connectrpc:"0.9.1",
    runtime:"workerd",endpoint:"ProofService/Ping",writes:2,
    d1_readback:true,db:"isolated native D1",marker}));
}finally{
  if(child?.pid){try{process.kill(-child.pid,"SIGTERM")}catch{}}
  rmSync(persist,{recursive:true,force:true});
}

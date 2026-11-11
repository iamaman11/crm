import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const cwd = dirname(dirname(fileURLToPath(import.meta.url)));
const persist = mkdtempSync(join(tmpdir(), "crm-native-party-d1-"));
const url = "http://127.0.0.1:18787";
const env = { ...process.env, NO_COLOR: "1" };
const run = (args) => execFileSync("npx", ["wrangler", ...args], {cwd, env, encoding:"utf8"});
let child;
let scenarioCompleted=false;
function header(tenant,key) {
  return { "x-crm-ci-tenant": tenant, ...(key ? {"Idempotency-Key":key}:{}),
    "content-type":"application/json" };
}
const json = async (path, options={}) => {
 const r=await fetch(url+path,options);
 return {status:r.status, body:await r.json()};
};
try {
  console.log(run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0001_initial.sql","--persist-to",persist]));
  run(["d1","execute","crm-rust-party-local","--local",
    "--command","INSERT INTO crm_tenants VALUES('tenant-a','A'),('tenant-b','B')",
    "--persist-to",persist]);
  child = spawn("npx", ["wrangler","dev","--local","--port","18787",
    "--persist-to",persist], {cwd,env,detached:true,stdio:["ignore","ignore","pipe"]});
  let stderr = "";
  child.stderr.on("data", bytes=>{stderr+=(bytes.toString()).slice(-4000);});
  let started=false;
  for(let i=0;i<100;i++){
    if(child.exitCode!==null) throw Error("workerd exited: "+stderr);
    try {
      const r=await fetch(url+"/api/parties");
      if(r.status===403) {started=true;break;}
    }catch{}
    await new Promise(resolve=>setTimeout(resolve,250));
  }
  if(!started) throw Error("workerd did not start: "+stderr);
  assert.equal((await json("/api/parties")).status,403,"denies request without CI tenant");
  const id="native-party-e2e-1";
  const createInput={id,kind:"organization",name:"  Original    Rust   Owner "};
  const create=await json("/api/parties",{
    method:"POST",headers:header("tenant-a","create-native-key"),
    body:JSON.stringify(createInput)});
  assert.equal(create.status,201,JSON.stringify(create));
  assert.equal(create.body.item.name,"Original Rust Owner");
  assert.equal(create.body.item.version,1);
  assert.equal(create.body.replayed,false);
  const replay=await json("/api/parties",{
    method:"POST",headers:header("tenant-a","create-native-key"),
    body:JSON.stringify(createInput)});
  assert.equal(replay.status,200);
  assert.equal(replay.body.replayed,true);
  assert.equal((await json("/api/parties",{
    method:"POST",headers:header("tenant-a","create-native-key"),
    body:JSON.stringify({...createInput,name:"different"})})).status,409);
  assert.equal((await json("/api/parties/"+id,{headers:header("tenant-a")})).status,200);
  assert.equal((await json("/api/parties/"+id,{headers:header("tenant-b")})).status,404);
  const stale=await json("/api/parties/"+id,{
    method:"PATCH",headers:header("tenant-a","stale-native-key"),
    body:JSON.stringify({expected_version:2,name:"Rejected"})});
  assert.equal(stale.status,409,JSON.stringify(stale));
  assert.equal(stale.body.code,"PARTIES_PARTY_VERSION_CONFLICT");
  const changeInput={expected_version:1,name:"Native Rust Updated"};
  const changed=await json("/api/parties/"+id,{
    method:"PATCH",headers:header("tenant-a","update-native-key"),
    body:JSON.stringify(changeInput)});
  assert.equal(changed.status,200,JSON.stringify(changed));
  assert.equal(changed.body.item.version,2);
  assert.equal(changed.body.item.name,"Native Rust Updated");
  const updateReplay=await json("/api/parties/"+id,{
    method:"PATCH",headers:header("tenant-a","update-native-key"),
    body:JSON.stringify(changeInput)});
  assert.equal(updateReplay.status,200);
  assert.equal(updateReplay.body.replayed,true);
  assert.equal((await json("/api/parties/"+id,{
    method:"PATCH",headers:header("tenant-a","older-native-key"),
    body:JSON.stringify({expected_version:1,name:"Should Not Change"})})).status,409);
  assert.equal((await json("/api/parties/"+id,{headers:header("tenant-a")})).body.item.version,2);
  const ownB=await json("/api/parties",{
    method:"POST",headers:header("tenant-b","create-native-bkey"),
    body:JSON.stringify({id,kind:"person",name:"Separate Tenant B"})});
  assert.equal(ownB.status,201,JSON.stringify(ownB));
  assert.equal((await json("/api/parties/"+id,{headers:header("tenant-b")})).body.item.name,"Separate Tenant B");
  assert.equal((await json("/api/parties",{
    method:"POST",headers:header("tenant-a","invalid-native-key"),
    body:JSON.stringify({id:"bad",kind:"person",name:" "})})).status,422);
  const a=(await json("/api/parties",{headers:header("tenant-a")})).body.items;
  const b=(await json("/api/parties",{headers:header("tenant-b")})).body.items;
  assert.equal(a.length,1);
  assert.equal(b.length,1);
  assert.equal(a[0].name,"Native Rust Updated");
  assert.equal(b[0].name,"Separate Tenant B");
  scenarioCompleted=true;
  console.log(JSON.stringify({event:"crm.native-rust-party-workerd",
    result:"PASS",runtime:"workerd",database:"isolated local D1",
    proven:["real original Rust Party create/update/serialize", "D1 persistent readback",
      "replay and idempotency mismatch", "stale version conflict", "tenant A/B isolation",
      "invalid input denied", "unauthenticated denied"]}));
} finally {
 if(child){
  // npx launches Wrangler and workerd as descendants. Stop the entire isolated
  // process group, otherwise a surviving descendant can keep Node's pipe open
  // and prevent the GitHub Actions step from terminating after a successful E2E.
  try { process.kill(-child.pid, "SIGTERM"); } catch (e) {
    if(e.code!=="ESRCH") throw e;
  }
  if(child.exitCode===null) await Promise.race([
    new Promise(resolve=>child.once("exit",resolve)),
    new Promise(resolve=>setTimeout(resolve,1500)),
  ]);
  try { process.kill(-child.pid, "SIGKILL"); } catch (e) {
    if(e.code!=="ESRCH") throw e;
  }
  child.stderr.destroy();
  child.unref();
 }
 try {
  const a=run(["d1","execute","crm-rust-party-local","--local",
    "--command","SELECT tenant_id AS tenant, COUNT(*) AS audited FROM crm_audit WHERE action LIKE 'parties.%' GROUP BY tenant_id ORDER BY tenant_id",
    "--persist-to",persist]);
  console.log("D1 audit readback:",a.slice(-2200));
  if(scenarioCompleted){
    assert.match(a,/tenant-a/);assert.match(a,/tenant-b/);
    assert.match(a,/2/);console.log("native Party D1 audit readback PASS");
  }
 } catch (e) {console.error("D1 audit readback failed",e.message);throw e;}
 rmSync(persist,{recursive:true,force:true});
}

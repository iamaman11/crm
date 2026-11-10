// Contract-only physical workerd fixture: legacy HTTP is forbidden and cannot write D1.
import {spawn,execFileSync} from "node:child_process";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join,dirname} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const persist=mkdtempSync(join(tmpdir(),"crm-contract-only-"));
const base="http://127.0.0.1:18787";
const cmd=(...args)=>execFileSync("npx",["wrangler",...args],{cwd,encoding:"utf8"});
let child;
try {
  cmd("d1","execute","crm-rust-party-local","--local",
    "--file","./migrations/0001_local.sql","--persist-to",persist);
  child=spawn("npx",["wrangler","dev","--local","--port","18787","--persist-to",persist],
    {cwd,detached:true,stdio:["ignore","ignore","pipe"]});
  let errors="";
  child.stderr.on("data",bytes=>errors=(errors+bytes.toString()).slice(-3500));
  const head={"x-crm-ci-tenant":"tenant-a","content-type":"application/json"};
  async function request(path,opts={}){
    const r=await fetch(base+path,{...opts,headers:{...head,...opts.headers}});
    return {status:r.status,body:await r.json()};
  }
  let ready=false;
  for(let i=0;i<120;i++){
    if(child.exitCode!==null)throw Error("workerd exited "+errors);
    try{if((await fetch(base+"/api/parties")).status===403){ready=true;break;}}catch{}
    await new Promise(resolve=>setTimeout(resolve,250));
  }
  assert(ready,"contract-only workerd did not start: "+errors);
  const disabled=[
    ["/api/parties","GET"],["/api/parties","POST"],["/api/accounts","GET"],
    ["/api/contact-points","POST"],["/api/sales-deals","POST"],
    ["/api/activity-tasks","GET"],["/api/consents","GET"],
    ["/api/customer-data-operations/parties.csv","GET"],
    ["/crm.sales.v1.DealService/GetDeal","POST"]
  ];
  for(const [path,method] of disabled){
    const r=await request(path,{method,body:method==="POST"?"{}":undefined});
    assert.equal(r.status,410,path+" must be disabled: "+JSON.stringify(r));
    assert.equal(r.body.code,"LEGACY_TRANSPORT_DISABLED");
  }
  assert.equal((await request("/crm.parties.v2.PartyService/GetParty",{
    method:"POST",body:'{"partyId":"not-present"}'})).status,404);
  const unauth=await fetch(base+"/api/parties");
  assert.equal(unauth.status,403,"unsigned CI tenant is still rejected");
  const result=cmd("d1","execute","crm-rust-party-local","--local",
    "--command","SELECT COUNT(*) AS n FROM crm_party_records","--persist-to",persist,"--json");
  const rows=JSON.parse(result);
  assert.equal(rows[0]?.results?.[0]?.n,0,"no legacy Party mutation may persist");
  console.log(JSON.stringify({event:"crm.contract-only-native-d1",result:"PASS",
    disabled_paths:disabled.length,authorized_rpc:"Party GetParty",legacy_writes:0}));
}finally {
  if(child){
    try{process.kill(-child.pid,"SIGTERM");}catch(e){if(e.code!=="ESRCH")throw e;}
    if(child.exitCode===null)await Promise.race([new Promise(resolve=>child.once("exit",resolve)),
      new Promise(resolve=>setTimeout(resolve,1300))]);
    try{process.kill(-child.pid,"SIGKILL");}catch(e){if(e.code!=="ESRCH")throw e;}
    child.stderr.destroy();child.unref();
  }
  rmSync(persist,{recursive:true,force:true});
}

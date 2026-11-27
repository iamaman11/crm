// Independent post-merge Gate B is separate; this runs only a compact CI release smoke.
import assert from "node:assert/strict";
import {readFileSync,mkdirSync} from "node:fs";
import {publishedSmoke} from "./published-smoke.mts";
import {record, records, string} from "../scripts/json-boundary.mts";
import {waitForExactSourceSha,releaseVersionAffinityKey} from "../scripts/release-readback.mts";
import {reportBrowserFailure,withBrowserCleanup} from "../rust-party/test/browser-boundaries.mts";
const required=(name: string)=>{const value=process.env[name];if(!value)throw Error("Missing "+name);return value;};
const sha=required("GITHUB_SHA");
const versionKey=releaseVersionAffinityKey(required("GITHUB_RUN_ID"),required("GITHUB_RUN_ATTEMPT"));
assert.match(sha,/^[a-f0-9]{40}$/);
assert.equal(process.env.GITHUB_REF,"refs/heads/dev");
assert.equal(process.env.GITHUB_EVENT_NAME,"push");
const account="5bd5e1e1e813f061afc259e91334262d",database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
assert.equal(required("CLOUDFLARE_ACCOUNT_ID"),account);
assert.equal(required("CRM_D1_DATABASE_ID"),database);
const token=required("CLOUDFLARE_API_TOKEN"),base="https://crm-dev.crm-5bd5e1e1.workers.dev";
const access=(name: "A" | "B")=>({"CF-Access-Client-Id":required("CRM_E2E_"+name+"_CLIENT_ID"),
 "CF-Access-Client-Secret":required("CRM_E2E_"+name+"_CLIENT_SECRET")});
const setup=record(JSON.parse(readFileSync("apps/cloudflare-crm/.release/e2e-tenants.json","utf8")));
const tenantA=string(setup.tenant_a);
assert.equal(setup.source_sha,sha);assert.equal(setup.database,database);
assert.equal(setup.run_id,required("GITHUB_RUN_ID"));
assert.equal(setup.attempt,required("GITHUB_RUN_ATTEMPT"));
const principal="service:"+access("A")["CF-Access-Client-Id"].toLowerCase();
assert.equal(setup.actor_a,principal);
async function query(sql: string,params: string[]=[]){
 const response=await fetch("https://api.cloudflare.com/client/v4/accounts/"+account+"/d1/database/"+database+"/query",{
   method:"POST",headers:{authorization:"Bearer "+token,"content-type":"application/json"},body:JSON.stringify({sql,params})});
 const body=record(await response.json());
 assert.ok(response.ok&&body.success,"D1 readback HTTP "+response.status);
 const [result, ...extra]=records(body.result);
 assert.ok(result && extra.length===0 && result.success===true);
 return result;
}
const directory="apps/cloudflare-crm/e2e-evidence";
mkdirSync(directory,{recursive:true});
let roleMayHaveChanged=false;
async function restoreRole(){
 if(!roleMayHaveChanged)return;
 await query("UPDATE crm_users SET role='admin' WHERE email=? AND tenant_id=? AND active=1 AND role='viewer'",[principal,tenantA]);
 const rows=(await query("SELECT role FROM crm_users WHERE email=? AND tenant_id=? AND active=1",[principal,tenantA])).results;
 assert.deepEqual(rows,[{role:"admin"}],"E2E administrator role must be restored");
 roleMayHaveChanged=false;
}
let smokeStarted=false;
let smokePassed=false;
try{
 await withBrowserCleanup(async()=>{
 await waitForExactSourceSha(base+"/api/version?browser-preflight="+Date.now(),sha,{
   headers:{...access("A"),"Cloudflare-Workers-Version-Key":versionKey,
     "cache-control":"no-cache",connection:"close"}
 });
 smokeStarted=true;
 await publishedSmoke({base,sourceSha:sha,versionKey,adminHeaders:access("A"),
   tenantBHeaders:access("B"),tenantA:tenantA,adminIdentity:principal,directory,
   sql:async (statement,params=[])=>records((await query(statement,params)).results),
   beforeViewer:async()=>{
     // Signed/enrolled test actor, real persisted role; no JWT fabrication or role header.
     // Register cleanup before UPDATE: its response can be lost after commit.
     roleMayHaveChanged=true;
     const beforeFence=records((await query(
       "SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?",[tenantA])).results);
     const changed=await query("UPDATE crm_users SET role='viewer' WHERE email=? AND tenant_id=? AND active=1 AND role='admin'",[principal,tenantA]);
     // D1 meta.changes includes trigger writes (the forecast fence). Prove
     // the direct role transition and exactly one tenant-local fence instead
     // of assuming sqlite3_total_changes() counts only the target row.
     const changes=record(changed.meta).changes;
     assert.ok(typeof changes==="number"&&Number.isInteger(changes)&&changes>=1,
       "The isolated E2E actor role mutation must commit");
     const [roleRows,fenceRows]=await Promise.all([
       query("SELECT role FROM crm_users WHERE email=? AND tenant_id=? AND active=1",[principal,tenantA]),
       query("SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?",[tenantA])
     ]);
     assert.deepEqual(roleRows.results,[{role:"viewer"}],"The isolated actor must become viewer");
     const revision=beforeFence[0]?.revision;
     assert.ok(typeof revision==="number");
     assert.equal(records(fenceRows.results)[0]?.revision,revision+1,
       "The member update must advance exactly one tenant-local forecast fence");
     console.log(JSON.stringify({event:"crm.e2e-viewer-transition",result:"PASS",
       direct_role:"viewer",d1_total_changes:changes,fence_advanced:1}));
     return restoreRole;
   }
 });
 smokePassed=true;
 },restoreRole);
}catch(error){
 if(smokeStarted && !smokePassed)throw error;
 throw reportBrowserFailure(error,{scenario:"live",phase:smokePassed?"cleanup":"signed-readback",sourceSha:sha,directory});
}

// Same Chromium scenarios against workerd and the existing live Worker.
import assert from "node:assert/strict";
import {readFileSync,writeFileSync,mkdirSync} from "node:fs";
import {typedBrowser} from "../rust-party/test/typed-browser.mjs";
const required=name=>{const value=process.env[name];if(!value)throw Error("Missing "+name);return value;};
const sha=required("GITHUB_SHA");
assert.match(sha,/^[a-f0-9]{40}$/);
assert.equal(process.env.GITHUB_REF,"refs/heads/dev");
assert.equal(process.env.GITHUB_EVENT_NAME,"push");
const account="5bd5e1e1e813f061afc259e91334262d",database="35fc7ee9-9710-485f-8fb3-f5a398b229b4";
assert.equal(required("CLOUDFLARE_ACCOUNT_ID"),account);
assert.equal(required("CRM_D1_DATABASE_ID"),database);
const token=required("CLOUDFLARE_API_TOKEN"),base="https://crm-dev.crm-5bd5e1e1.workers.dev";
const access=name=>({"CF-Access-Client-Id":required("CRM_E2E_"+name+"_CLIENT_ID"),
 "CF-Access-Client-Secret":required("CRM_E2E_"+name+"_CLIENT_SECRET")});
const setup=JSON.parse(readFileSync("apps/cloudflare-crm/.release/e2e-tenants.json","utf8"));
assert.equal(setup.source_sha,sha);assert.equal(setup.database,database);
assert.equal(setup.run_id,required("GITHUB_RUN_ID"));
assert.equal(setup.attempt,required("GITHUB_RUN_ATTEMPT"));
const principal="service:"+access("A")["CF-Access-Client-Id"].toLowerCase();
assert.equal(setup.actor_a,principal);
async function query(sql,params=[]){
 const response=await fetch("https://api.cloudflare.com/client/v4/accounts/"+account+"/d1/database/"+database+"/query",{
   method:"POST",headers:{authorization:"Bearer "+token,"content-type":"application/json"},body:JSON.stringify({sql,params})});
 const body=await response.json();
 assert.ok(response.ok&&body.success,"D1 readback HTTP "+response.status);
 assert.equal(body.result.length,1);assert.equal(body.result[0].success,true);
 return body.result[0];
}
const directory="apps/cloudflare-crm/e2e-evidence";
mkdirSync(directory,{recursive:true});
try{
 let stable=0;
 for(let attempt=0;attempt<40&&stable<3;attempt++){
   const response=await fetch(base+"/api/version?browser-preflight="+Date.now(),{
     headers:{...access("A"),"cache-control":"no-cache"},redirect:"manual"});
   stable=response.status===200&&(await response.json()).source===sha?stable+1:0;
   if(stable<3)await new Promise(resolve=>setTimeout(resolve,1500));
 }
 assert.equal(stable,3,"Browser suite must start on the expected deployed source");
 await typedBrowser({base,sourceSha:sha,adminHeaders:access("A"),viewerHeaders:access("A"),
   tenantBHeaders:access("B"),tenantA:setup.tenant_a,features:["native"],directory,
   sql:async sql=>(await query(sql)).results,
   beforeViewer:async()=>{
     // Signed/enrolled test actor, real persisted role; no JWT fabrication or role header.
     const changed=await query("UPDATE crm_users SET role='viewer' WHERE email=? AND tenant_id=? AND active=1 AND role='admin'",[principal,setup.tenant_a]);
     assert.equal(changed.meta.changes,1,"Only the isolated E2E actor may change role");
     return async()=>{
       await query("UPDATE crm_users SET role='admin' WHERE email=? AND tenant_id=? AND active=1 AND role='viewer'",[principal,setup.tenant_a]);
       const rows=(await query("SELECT role FROM crm_users WHERE email=? AND tenant_id=? AND active=1",[principal,setup.tenant_a])).results;
       assert.deepEqual(rows,[{role:"admin"}],"E2E administrator role must be restored");
     };
   }
 });
}catch(error){
 writeFileSync(directory+"/failure.json",JSON.stringify({result:"FAIL",source_sha:sha,error:String(error.message)},null,2)+"\n");
 throw error;
}

// The protected contract-only release must prove FAIL-CLOSED live behavior.
// It intentionally forbids all legacy UI business operations and D1 writes.
import {chromium} from "playwright";
import assert from "node:assert/strict";
import {writeFile,mkdir} from "node:fs/promises";
const sha=process.env.GITHUB_SHA;
assert.match(sha||"",/^[a-f0-9]{40}$/);
const url="https://crm-dev.crm-5bd5e1e1.workers.dev";
const credentials=side=>({
  "CF-Access-Client-Id":process.env["CRM_E2E_"+side+"_CLIENT_ID"],
  "CF-Access-Client-Secret":process.env["CRM_E2E_"+side+"_CLIENT_SECRET"]
});
const browser=await chromium.launch({headless:true});
const evidence={git_sha:sha,phase:"contract-only",requests:[],browser:"chromium",
  result:"FAIL_CLOSED_PENDING"};
try{
  const a=await browser.newContext({extraHTTPHeaders:credentials("A")});
  const b=await browser.newContext({extraHTTPHeaders:credentials("B")});
  const page=await a.newPage();
  const response=await page.goto(url,{waitUntil:"domcontentloaded"});
  assert.equal(response?.status(),200);
  await page.getByRole("heading",{name:"CRM: переход на Connect"}).waitFor();
  const disabled=[
    ["GET","/api/parties"],["POST","/api/parties"],
    ["GET","/api/accounts"],["POST","/api/accounts"],
    ["GET","/api/contact-points"],["POST","/api/sales-deals"],
    ["GET","/api/activity-tasks"],["GET","/api/consents"],
    ["GET","/api/party-relationships"],["GET","/api/customer-privacy/cases"],
    ["GET","/api/customer-data-operations/parties.csv"],
    ["GET","/api/customer-enrichment/profiles"],
    ["POST","/crm.sales.v1.DealService/GetDeal"]
  ];
  for(const [method,path] of disabled){
    const r=await a.request.fetch(url+path,{method,headers:{"content-type":"application/json"},
      ...(method==="POST"?{data:{}}:{})});
    const result=await r.json();
    assert.equal(r.status(),410,"obsolete endpoint still works: "+method+" "+path);
    assert.equal(result.code,"LEGACY_TRANSPORT_DISABLED");
    const headers=r.headers();
    assert.equal(headers["x-crm-source-sha"],sha,"wrong native Worker version");
    assert.equal(headers["x-crm-runtime"],"native-workers-rs");
    assert.match(headers["x-request-id"]||"",/^[0-9a-f-]{36}$/i);
    evidence.requests.push({method,path,status:r.status(),request_id:headers["x-request-id"],
      operation:method==="GET"&&path==="/api/parties"?"parties.list":
        method==="POST"&&path==="/api/accounts"?"accounts.create":null});
  }
  const bDenied=await b.request.get(url+"/api/parties");
  assert.equal(bDenied.status(),410,"second Access tenant must also be blocked");
  const rpc=await a.request.post(url+"/crm.parties.v2.PartyService/GetParty",{
    data:{partyId:"contract-only-nonexistent"},
    headers:{"content-type":"application/json"}
  });
  assert.equal(rpc.status(),404,"the approved Connect RPC stays available");
  assert.equal(rpc.headers()["x-crm-source-sha"],sha);
  evidence.rpc_read={method:"/crm.parties.v2.PartyService/GetParty",status:rpc.status(),
    request_id:rpc.headers()["x-request-id"]};
  assert.match(evidence.rpc_read.request_id||"",/^[0-9a-f-]{36}$/i);
  await mkdir("apps/cloudflare-crm/.release",{recursive:true});
  await page.screenshot({path:"apps/cloudflare-crm/e2e-failure.png",fullPage:true});
  evidence.result="BROWSER_FAIL_CLOSED_PASS_LOGS_PENDING";
  await writeFile("apps/cloudflare-crm/.release/contract-only-evidence.json",
    JSON.stringify(evidence,null,2)+"\n");
  console.log(JSON.stringify({event:"crm.contract-only-browser",result:"PASS",
    sha,old_endpoints_blocked:disabled.length,access_tenants:2,
    connect_rpc:"GetParty",legacy_mutations:0}));
}finally{await browser.close();}

// Exact native workerd Access RS256 proof, isolated D1 and signed CI-only JWTs.
import {spawn,execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,createHash,randomUUID} from "node:crypto";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { mkdirSync, writeFileSync } from "node:fs";
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
  return {status:response.status,headers:Object.fromEntries(response.headers),body:await response.json()};
};
let child;
try{
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","./migrations/0001_local.sql","--persist-to",data]);
  const sql="CREATE TABLE crm_tenants (id TEXT PRIMARY KEY, name TEXT NOT NULL);"+
    "INSERT INTO crm_tenants VALUES('tenant-a','A'),('tenant-b','B');"+
    "CREATE TABLE crm_users (email TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role TEXT NOT NULL DEFAULT 'admin', active INTEGER NOT NULL DEFAULT 1);"+
    "CREATE TABLE crm_customers (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, updated_at TEXT NOT NULL);"+
    "CREATE TABLE crm_contacts (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, customer_id TEXT NOT NULL, name TEXT NOT NULL, email TEXT NOT NULL, phone TEXT NOT NULL, updated_at TEXT NOT NULL);"+
    "CREATE TABLE crm_deals (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, customer_id TEXT NOT NULL, title TEXT NOT NULL, stage TEXT NOT NULL, amount REAL NOT NULL, updated_at TEXT NOT NULL);"+
    "CREATE TABLE crm_tasks (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, customer_id TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, due_date TEXT NOT NULL, updated_at TEXT NOT NULL);"+
    "CREATE TABLE crm_audit (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, action TEXT NOT NULL, subject_id TEXT NOT NULL, created_at TEXT NOT NULL, request_id TEXT);"+
    "INSERT INTO crm_users VALUES('native-a@example.test','tenant-a','admin',1);"+
    "INSERT INTO crm_users VALUES('native-b@example.test','tenant-b','admin',1);"+
    "INSERT INTO crm_users VALUES('service:ci-native-b-0001.access','tenant-b','admin',1);";
  run(["d1","execute","crm-rust-party-local","--local",
    "--command",sql,"--persist-to",data]);
  // Exactly the production original Account migration against isolated D1;
  // no duplicate CI SQL rules, no second datastore.
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0004_original_customer_accounts.sql","--persist-to",data]);
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0005_original_contact_points.sql","--persist-to",data]);
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0006_original_party_relationships.sql","--persist-to",data]);
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0007_original_activity_tasks.sql","--persist-to",data]);
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0008_original_sales_deals.sql","--persist-to",data]);
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0009_original_consents.sql","--persist-to",data]);
  run(["d1","execute","crm-rust-party-local","--local",
    "--file","../migrations/0010_original_sales_activities_link.sql","--persist-to",data]);
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
  assert.equal((await call("/api/parties",jwt({email:"not-enrolled@example.test"}))).status,200,
    "verified first entry creates tenant through native Rust");
  const a=jwt(),b=jwt({email:"native-b@example.test",sub:"native-test-b"});
  // Real Cloudflare Access service-token JWT has sub:"" and a signed
  // common_name. It must map to service:<client-id>, NOT to an email.
  const serviceB=jwt({sub:"",email:"",common_name:"ci-native-b-0001.access"});
  assert.equal((await call("/api/parties",serviceB)).status,200,
    "signed Access service JWT with empty sub accepted");
  assert.equal((await call("/api/parties",jwt({sub:"",email:"native-a@example.test"}))).status,401,
    "empty sub with only human email denied");
  assert.equal((await call("/api/parties",jwt({sub:"",email:"",common_name:"invalid"}))).status,401,
    "empty sub requires valid signed service common_name");
  assert.equal((await call("/api/parties",jwt({sub:"",email:"",common_name:"unknown-service.access"}))).status,200,
    "signed new service token receives isolated native tenant");
  assert.equal((await call("/api/parties",jwt({sub:"",email:"",common_name:"ci-native-b-0001.access",aud:"wrong"}))).status,401,
    "service token still requires application audience");
  assert.equal((await call("/api/parties",jwt({sub:"",email:"",common_name:"ci-native-b-0001.access"},{kid:"wrong"}))).status,401,
    "service token still requires trusted signing key");
  assert.equal((await call("/api/parties",jwt({sub:"",email:"native-a@example.test",common_name:"ci-native-b-0001.access"}))).status,200,
    "service identity is selected from signed common_name even with an email field");

  const parts=a.split(".");
  parts[1]=Buffer.from(JSON.stringify({iss,aud,exp:now+3600,sub:"forged",email:"native-b@example.test"})).toString("base64url");
  assert.equal((await call("/api/parties",parts.join("."))).status,401,
    "modified claims must fail signature");
  assert.equal((await call("/api/parties",a)).status,200,"signed A accepted");
  const correlation=await call("/api/parties",a);
  assert.match(correlation.headers["x-request-id"]||"",/^[0-9a-f-]{36}$/i);
  assert.equal(correlation.headers["x-crm-module"],"crm.parties");
  const forbidden=await call("/api/parties",null);
  assert.match(forbidden.headers["x-request-id"]||"",/^[0-9a-f-]{36}$/i);
  assert.equal(forbidden.headers["x-crm-error-code"],"AUTH_REQUIRED");
  assert.equal((await call("/api/parties",b)).status,200,"signed B accepted");
  // Execute original Account, ContactPoint, Sales and Activities domain code
  // INSIDE this same native workerd, after the signed Access identity boundary.
  assert.equal((await call("/api/_ci/original-domain-check",null)).status,401,
    "CI-only owner probe is not accessible without verified Access");
  const ownerProof=await call("/api/_ci/original-domain-check",a);
  assert.equal(ownerProof.status,200,JSON.stringify(ownerProof));
  assert.equal(ownerProof.body.result,"PASS");
  assert.equal(ownerProof.body.code,0);
  assert.equal(ownerProof.body.owners_executed.length,4);
  assert.equal(ownerProof.body.d1_owner_persistence,"NOT_TESTED");
  assert.equal((await call("/api/_ci/original-domain-check",b)).status,200,
    "CI-only in-memory proof is tenant-independent but still Access-gated");
  console.log(JSON.stringify({event:"crm.native-original-owners-workerd",result:"PASS",
    owners_executed:ownerProof.body.owners_executed,
    cloudflare_live_d1:"NOT_TESTED",remaining_owners:"NOT_TESTED"}));
  const id="native-jwt-"+randomUUID();
  const created=await call("/api/parties",a,{method:"POST",headers:{
    "content-type":"application/json","idempotency-key":"native-signed-create",
    "x-crm-native-tenant":"tenant-b","x-crm-ci-tenant":"tenant-b"},
    body:JSON.stringify({id,kind:"organization",name:"  Original  Native  Rust  "})});
  assert.equal(created.status,201,JSON.stringify(created));
  assert.equal(created.body.item.name,"Original Native Rust");
  assert.equal((await call("/api/parties/"+id,a)).status,200);
  assert.equal((await call("/api/parties/"+id,b)).status,404,"B cannot read A");
  assert.equal((await call("/api/parties/"+id,serviceB)).status,404,
    "Cloudflare service token B cannot read Party belonging to human A");
  assert.equal((await call("/api/parties/"+id,serviceB,{headers:{
    "x-crm-native-tenant":"tenant-a","x-crm-ci-tenant":"tenant-a"}})).status,404,
    "service-token identity cannot be overridden by forged tenant headers");

  assert.equal((await call("/api/parties/"+id,b,{headers:{
    "x-crm-native-tenant":"tenant-a","x-crm-ci-tenant":"tenant-a"}})).status,404);

  // Original Account owner real D1 lifecycle. Separate from legacy pilot customer.
  assert.equal((await call("/api/accounts",null)).status,401);
  assert.deepEqual((await call("/api/accounts",b)).body.items,[]);
  const accountId="native-account-"+randomUUID();
  const accountData={id:accountId,name:"  Original    Account  ",party_id:id};
  const accountHeaders={"content-type":"application/json","idempotency-key":"signed-account-create"};
  const missing=await call("/api/accounts",b,{method:"POST",headers:accountHeaders,
    body:JSON.stringify(accountData)});
  assert.equal(missing.status,422,"cross-tenant Party reference must fail");
  assert.equal(missing.body.code,"CUSTOMER_ACCOUNTS_PARTY_NOT_FOUND");
  const accountExtra=await call("/api/accounts",a,{method:"POST",headers:{
    ...accountHeaders,"idempotency-key":"account-unknown-field"},
    body:JSON.stringify({...accountData,legacy_customer_id:"spoof"})});
  assert.equal(accountExtra.status,422,"Account rejects old pilot fields, not silently ignore");
  const accountCreate=await call("/api/accounts",a,{method:"POST",headers:{
    ...accountHeaders,"x-crm-native-tenant":"tenant-b"},
    body:JSON.stringify(accountData)});
  assert.equal(accountCreate.status,201,JSON.stringify(accountCreate));
  assert.equal(accountCreate.body.item.name,"Original Account");
  assert.equal(accountCreate.body.item.version,1);
  assert.equal(accountCreate.body.item.parties[0].id,id);
  assert.equal(accountCreate.headers["x-crm-module"],"crm.customer-accounts");
  assert.match(accountCreate.headers["x-request-id"],/^[0-9a-f-]{36}$/i);
  assert.equal((await call("/api/accounts/"+accountId,b)).status,404);
  assert.equal((await call("/api/accounts/"+accountId,serviceB)).status,404);
  const accountRead=await call("/api/accounts/"+accountId,a);
  assert.equal(accountRead.status,200);
  assert.deepEqual(accountRead.body.item,accountCreate.body.item,
    "original Account encode -> D1 -> decode preserves state");
  const accountReplay=await call("/api/accounts",a,{method:"POST",headers:accountHeaders,
    body:JSON.stringify(accountData)});
  assert.equal(accountReplay.status,200);
  assert.equal(accountReplay.body.replayed,true,"Account create replay cannot duplicate");
  const reused=await call("/api/accounts",a,{method:"POST",headers:accountHeaders,
    body:JSON.stringify({...accountData,name:"Other"})});
  assert.equal(reused.status,409);
  assert.equal(reused.body.code,"IDEMPOTENCY_CONFLICT");
  const accountUpdate={name:"  Original Updated Account ",status:"inactive",expected_version:1};
  const accountUpdateInit={method:"PATCH",headers:{"content-type":"application/json",
    "idempotency-key":"signed-account-update"},body:JSON.stringify(accountUpdate)};
  const savedAccount=await call("/api/accounts/"+accountId,a,accountUpdateInit);
  assert.equal(savedAccount.status,200,JSON.stringify(savedAccount));
  assert.equal(savedAccount.body.item.version,2);
  assert.equal(savedAccount.body.item.status,"inactive");
  assert.equal(savedAccount.body.item.name,"Original Updated Account");
  const savedReplay=await call("/api/accounts/"+accountId,a,accountUpdateInit);
  assert.equal(savedReplay.status,200);
  assert.equal(savedReplay.body.replayed,true);
  const staleAccount=await call("/api/accounts/"+accountId,a,{...accountUpdateInit,
    headers:{...accountUpdateInit.headers,"idempotency-key":"stale-account-version"}});
  assert.equal(staleAccount.status,409);
  assert.equal(staleAccount.body.code,"CUSTOMER_ACCOUNTS_VERSION_CONFLICT");
  const accountList=await call("/api/accounts",a);
  assert.equal(accountList.status,200);
  assert.ok(accountList.body.items.some(x=>x.id===accountId&&x.version===2));
  assert.deepEqual((await call("/api/accounts",b)).body.items,[]);
  console.log(JSON.stringify({event:"crm.native-original-account-d1",result:"PASS",
    owner:"crm.customer-accounts",
    proven:["signed Access A/B","Party foreign key","original domain create/update",
      "strict codec roundtrip","D1 durable state","idempotent receipts",
      "optimistic version","tenant isolation","request correlation"],
    published_live:"NOT_TESTED"}));
  // Original Activities Task: belongs to ORIGINAL Account; actor is verified
  // Access identity, NEVER owner submitted by the client.
  assert.equal((await call("/api/activity-tasks",null)).status,401);
  assert.deepEqual((await call("/api/activity-tasks",b)).body.items,[]);
  const taskId="native-activity-task-"+randomUUID();
  const taskData={id:taskId,subject:"Call account owner",
    description:"Follow up",account_id:accountId,priority:"normal",
    due_at_unix_nanos:null,reminder_at_unix_nanos:null};
  const taskHeaders={"content-type":"application/json",
    "idempotency-key":"signed-original-activity-task-create"};
  const taskWrongTenant=await call("/api/activity-tasks",b,{
    method:"POST",headers:taskHeaders,body:JSON.stringify(taskData)});
  assert.equal(taskWrongTenant.status,422,"Task cannot link to another tenant's Account");
  assert.equal(taskWrongTenant.body.code,"ACTIVITIES_ACCOUNT_NOT_FOUND");
  const taskInvalid=await call("/api/activity-tasks",a,{
    method:"POST",headers:{...taskHeaders,"idempotency-key":"activity-invalid-subject"},
    body:JSON.stringify({...taskData,subject:""})});
  assert.equal(taskInvalid.status,422,"original Task domain rejects blank subject");
  const taskOwnerForgery=await call("/api/activity-tasks",a,{
    method:"POST",headers:{...taskHeaders,"idempotency-key":"activity-owner-forgery"},
    body:JSON.stringify({...taskData,owner:"admin"})});
  assert.equal(taskOwnerForgery.status,422,"caller must not supply trusted Task owner");
  const taskCreated=await call("/api/activity-tasks",a,{
    method:"POST",headers:taskHeaders,body:JSON.stringify(taskData)});
  assert.equal(taskCreated.status,201,JSON.stringify(taskCreated));
  assert.equal(taskCreated.body.item.id,taskId);
  assert.equal(taskCreated.body.item.status,"open");
  assert.equal(taskCreated.body.item.version,1);
  assert.equal(taskCreated.body.item.owner.actor,"native-a@example.test");
  assert.equal(taskCreated.body.item.related_resources[0].resource_id,accountId);
  assert.equal(taskCreated.body.item.related_resources[0].resource_type,"crm.customer-accounts");
  assert.equal(taskCreated.headers["x-crm-module"],"crm.activities");
  assert.match(taskCreated.headers["x-request-id"],/^[0-9a-f-]{36}$/i);
  assert.equal((await call("/api/activity-tasks/"+taskId,b)).status,404);
  assert.equal((await call("/api/activity-tasks/"+taskId,serviceB)).status,404);
  const taskRead=await call("/api/activity-tasks/"+taskId,a);
  assert.equal(taskRead.status,200);
  assert.deepEqual(taskRead.body.item,taskCreated.body.item,
    "original Task codec must survive D1 roundtrip");
  const taskReplay=await call("/api/activity-tasks",a,{
    method:"POST",headers:taskHeaders,body:JSON.stringify(taskData)});
  assert.equal(taskReplay.status,200);
  assert.equal(taskReplay.body.replayed,true);
  const taskIdemConflict=await call("/api/activity-tasks",a,{
    method:"POST",headers:taskHeaders,
    body:JSON.stringify({...taskData,subject:"Change subject"})});
  assert.equal(taskIdemConflict.status,409);
  assert.equal(taskIdemConflict.body.code,"IDEMPOTENCY_CONFLICT");
  const completeInit={method:"POST",headers:{"content-type":"application/json",
    "idempotency-key":"signed-original-activity-task-complete"},
    body:JSON.stringify({expected_version:1})};
  const taskComplete=await call("/api/activity-tasks/"+taskId+"/complete",a,completeInit);
  assert.equal(taskComplete.status,200,JSON.stringify(taskComplete));
  assert.equal(taskComplete.body.item.status,"completed");
  assert.equal(taskComplete.body.item.version,2);
  assert.equal(taskComplete.body.item.reminder_at_unix_nanos,null);
  assert.ok(taskComplete.body.item.completed_at_unix_nanos>0);
  const completeReplay=await call("/api/activity-tasks/"+taskId+"/complete",a,completeInit);
  assert.equal(completeReplay.status,200);
  assert.equal(completeReplay.body.replayed,true);
  const taskStale=await call("/api/activity-tasks/"+taskId+"/complete",a,{
    ...completeInit,headers:{...completeInit.headers,
      "idempotency-key":"original-activity-task-stale"}});
  assert.equal(taskStale.status,409);
  assert.equal(taskStale.body.code,"ACTIVITIES_TASK_VERSION_CONFLICT");
  assert.deepEqual((await call("/api/activity-tasks/"+taskId,a)).body.item,
    taskComplete.body.item,"completed original aggregate persisted in D1");
  assert.ok((await call("/api/activity-tasks",a)).body.items
    .some(x=>x.id===taskId&&x.status==="completed"));
  assert.deepEqual((await call("/api/activity-tasks",b)).body.items,[]);
  console.log(JSON.stringify({event:"crm.native-original-activities-d1",result:"PASS",
    owner:"crm.activities",proven:["signed actor provenance","Account FK",
      "original Task create/complete","strict codec D1 roundtrip",
      "Access A/B isolation","idempotency receipts","version CAS",
      "forged owner rejected","request correlation"],
    ui:"NOT_CONNECTED",published_live:"NOT_TESTED"}));

  // Original Sales Deal: native exact money, Access actor, domain stage policy,
  // Account FK and original state codec, never legacy pilot crm_deals.
  assert.equal((await call("/api/sales-deals",null)).status,401);
  assert.deepEqual((await call("/api/sales-deals",b)).body.items,[]);
  const dealId="native-sales-deal-"+randomUUID();
  const dealData={id:dealId,name:"Enterprise renewal",account_id:accountId,
    pipeline_id:"pipeline.enterprise",stage_id:"lead",ordinal:1,
    probability_bps:2500,amount_minor_units:12500,currency:"USD"};
  const dealHeaders={"content-type":"application/json",
    "idempotency-key":"signed-original-sales-deal-create"};
  const dealWrongTenant=await call("/api/sales-deals",b,{
    method:"POST",headers:dealHeaders,body:JSON.stringify(dealData)});
  assert.equal(dealWrongTenant.status,422,"no foreign tenant Account reference");
  assert.equal(dealWrongTenant.body.code,"SALES_ACCOUNT_NOT_FOUND");
  const dealSpoof=await call("/api/sales-deals",a,{
    method:"POST",headers:{...dealHeaders,"idempotency-key":"sales-owner-spoof"},
    body:JSON.stringify({...dealData,owner:"attacker"})});
  assert.equal(dealSpoof.status,422,"signed actor cannot be overwritten by request");
  const dealNegative=await call("/api/sales-deals",a,{
    method:"POST",headers:{...dealHeaders,"idempotency-key":"sales-negative-money"},
    body:JSON.stringify({...dealData,amount_minor_units:-1})});
  assert.equal(dealNegative.status,422);
  assert.equal(dealNegative.body.code,"SALES_DEAL_AMOUNT_NEGATIVE");
  const dealCurrency=await call("/api/sales-deals",a,{
    method:"POST",headers:{...dealHeaders,"idempotency-key":"sales-invalid-currency"},
    body:JSON.stringify({...dealData,currency:"usd"})});
  assert.equal(dealCurrency.status,422);
  assert.equal(dealCurrency.body.code,"CONTRACT_CURRENCY_INVALID");
  const dealCreated=await call("/api/sales-deals",a,{
    method:"POST",headers:dealHeaders,body:JSON.stringify(dealData)});
  assert.equal(dealCreated.status,201,JSON.stringify(dealCreated));
  assert.equal(dealCreated.body.item.id,dealId);
  assert.equal(dealCreated.body.item.account_id,accountId);
  assert.equal(dealCreated.body.item.owner.actor,"native-a@example.test");
  assert.equal(dealCreated.body.item.stage.stage_id,"lead");
  assert.equal(dealCreated.body.item.stage.ordinal,1);
  assert.equal(dealCreated.body.item.amount.minor_units,"12500");
  assert.equal(dealCreated.body.item.amount.currency,"USD");
  assert.equal(dealCreated.body.item.status,"open");
  assert.equal(dealCreated.body.item.probability_bps,2500);
  assert.equal(dealCreated.body.item.version,1);
  assert.equal(dealCreated.headers["x-crm-module"],"crm.sales");
  assert.match(dealCreated.headers["x-request-id"],/^[0-9a-f-]{36}$/i);
  assert.equal((await call("/api/sales-deals/"+dealId,b)).status,404);
  assert.equal((await call("/api/sales-deals/"+dealId,serviceB)).status,404);
  const dealRead=await call("/api/sales-deals/"+dealId,a);
  assert.equal(dealRead.status,200);
  assert.deepEqual(dealRead.body.item,dealCreated.body.item,
    "original strict Deal codec -> D1 -> decode preserves exact Money");
  const dealReplay=await call("/api/sales-deals",a,{
    method:"POST",headers:dealHeaders,body:JSON.stringify(dealData)});
  assert.equal(dealReplay.status,200);
  assert.equal(dealReplay.body.replayed,true);
  const dealConflict=await call("/api/sales-deals",a,{
    method:"POST",headers:dealHeaders,
    body:JSON.stringify({...dealData,name:"Fake"})});
  assert.equal(dealConflict.status,409);
  assert.equal(dealConflict.body.code,"IDEMPOTENCY_CONFLICT");
  const advanceData={expected_version:1,pipeline_id:"pipeline.enterprise",
    stage_id:"qualified",ordinal:2,status:"open",close_reason_code:null};
  const advanceHeaders={"content-type":"application/json",
    "idempotency-key":"signed-original-sales-advance"};
  const advanceInit={method:"POST",headers:advanceHeaders,
    body:JSON.stringify(advanceData)};
  const dealAdvanced=await call("/api/sales-deals/"+dealId+"/advance",a,advanceInit);
  assert.equal(dealAdvanced.status,200,JSON.stringify(dealAdvanced));
  assert.equal(dealAdvanced.body.item.version,2);
  assert.equal(dealAdvanced.body.item.stage.stage_id,"qualified");
  assert.equal(dealAdvanced.body.item.status,"open");
  // Original SalesActivitiesLink::handle, original Protobuf contract adapter,
  // and original crm-activities Task::create in ONE atomic D1 transition.
  assert.equal(dealAdvanced.body.link.disposition,"applied");
  assert.match(dealAdvanced.body.link.delivery_id,/^delivery-[0-9a-f]{40}$/);
  assert.equal(dealAdvanced.body.link.task_id,dealAdvanced.body.link.delivery_id);
  const linkedId=dealAdvanced.body.link.task_id;
  const linked=await call("/api/activity-tasks/"+linkedId,a);
  assert.equal(linked.status,200,JSON.stringify(linked));
  assert.equal(linked.body.item.subject,"Follow up deal after stage change");
  assert.equal(linked.body.item.owner.actor,"native-a@example.test");
  assert.deepEqual(linked.body.item.related_resources,[{
    resource_type:"sales.deal",resource_id:dealId,version:2}]);
  assert.equal(linked.body.item.status,"open");
  assert.equal((await call("/api/activity-tasks/"+linkedId,b)).status,404);
  assert.equal((await call("/api/activity-tasks/"+linkedId,serviceB)).status,404);

  const advanceReplay=await call("/api/sales-deals/"+dealId+"/advance",a,advanceInit);
  assert.equal(advanceReplay.status,200);
  assert.equal(advanceReplay.body.replayed,true);
  const linkedAgain=await call("/api/activity-tasks/"+linkedId,a);
  assert.deepEqual(linkedAgain.body.item,linked.body.item,
    "Sales replay must not create a second task or mutate the original Task");
  assert.equal((await call("/api/activity-tasks",a)).body.items
    .filter(x=>x.id===linkedId).length,1,
    "one linked task per original stage change");

  const stageRegression=await call("/api/sales-deals/"+dealId+"/advance",a,{
    method:"POST",headers:{...advanceHeaders,
      "idempotency-key":"signed-original-sales-regression"},
    body:JSON.stringify({...advanceData,expected_version:2,
      stage_id:"lead",ordinal:1})});
  assert.equal(stageRegression.status,409,"original domain rejects stage regression");
  assert.equal(stageRegression.body.code,"SALES_STAGE_REGRESSION_FORBIDDEN");
  const missingReason=await call("/api/sales-deals/"+dealId+"/advance",a,{
    method:"POST",headers:{...advanceHeaders,
      "idempotency-key":"signed-original-sales-missing-reason"},
    body:JSON.stringify({...advanceData,expected_version:2,
      stage_id:"won",ordinal:3,status:"won"})});
  assert.equal(missingReason.status,422,"original domain requires close reason");
  assert.equal(missingReason.body.code,"SALES_CLOSE_REASON_REQUIRED");
  const closeInput={...advanceData,expected_version:2,
    stage_id:"won",ordinal:3,status:"won",close_reason_code:"contract-signed"};
  const closeHeaders={...advanceHeaders,
    "idempotency-key":"signed-original-sales-close"};
  const closed=await call("/api/sales-deals/"+dealId+"/advance",a,{
    method:"POST",headers:closeHeaders,body:JSON.stringify(closeInput)});
  assert.equal(closed.status,200,JSON.stringify(closed));
  assert.equal(closed.body.item.version,3);
  assert.equal(closed.body.item.status,"won");
  assert.equal(closed.body.item.close_outcome.reason_code,"contract-signed");
  assert.equal(closed.body.item.amount.minor_units,"12500");
  assert.equal(closed.body.link.disposition,"ignored",
    "original link ignores closed Sales events");
  assert.equal(closed.body.link.task_id,null);
  assert.notEqual(closed.body.link.delivery_id,dealAdvanced.body.link.delivery_id);
  assert.deepEqual((await call("/api/activity-tasks/"+linkedId,a)).body.item,
    linked.body.item,"close must not modify earlier original linked Task");

  const closeReplay=await call("/api/sales-deals/"+dealId+"/advance",a,{
    method:"POST",headers:closeHeaders,body:JSON.stringify(closeInput)});
  assert.equal(closeReplay.status,200);
  assert.equal(closeReplay.body.replayed,true);
  const closedAgain=await call("/api/sales-deals/"+dealId+"/advance",a,{
    method:"POST",headers:{...advanceHeaders,
      "idempotency-key":"signed-original-sales-already-closed"},
    body:JSON.stringify({...closeInput,expected_version:3,
      stage_id:"after-won",ordinal:4})});
  assert.equal(closedAgain.status,409);
  assert.equal(closedAgain.body.code,"SALES_DEAL_ALREADY_CLOSED");
  const staleDeal=await call("/api/sales-deals/"+dealId+"/advance",a,{
    method:"POST",headers:{...advanceHeaders,
      "idempotency-key":"signed-original-sales-stale"},
    body:JSON.stringify({...advanceData,expected_version:1,ordinal:4,
      stage_id:"unknown"})});
  assert.equal(staleDeal.status,409);
  assert.equal(staleDeal.body.code,"SALES_DEAL_VERSION_CONFLICT");
  assert.deepEqual((await call("/api/sales-deals/"+dealId,a)).body.item,
    closed.body.item,"closed original deal persisted after all negative calls");
  assert.ok((await call("/api/sales-deals",a)).body.items
    .some(x=>x.id===dealId&&x.status==="won"));
  assert.deepEqual((await call("/api/sales-deals",b)).body.items,[]);
  // Physical D1 transaction rollback: a caller-created Task deliberately
  // occupies the immutable ORIGINAL delivery ID before a Sales stage update.
  // The linked Task INSERT must fail, rolling back Deal UPDATE, receipts
  // and link delivery together; no half-updated Deal may escape.
  const rollbackDealId="native-link-rollback-"+randomUUID();
  const rollbackCreate=await call("/api/sales-deals",a,{method:"POST",
    headers:{"content-type":"application/json",
      "idempotency-key":"signed-link-rollback-create"},
    body:JSON.stringify({...dealData,id:rollbackDealId})});
  assert.equal(rollbackCreate.status,201,JSON.stringify(rollbackCreate));
  const v2=Buffer.alloc(8);v2.writeBigInt64BE(2n);
  const hash=createHash("sha256").update("tenant-a").update(Buffer.from([0]))
    .update(rollbackDealId).update(Buffer.from([0])).update(v2)
    .digest("hex").slice(0,40);
  const blockedId="delivery-"+hash;
  const occupied=await call("/api/activity-tasks",a,{method:"POST",
    headers:{"content-type":"application/json",
      "idempotency-key":"signed-link-rollback-occupied"},
    body:JSON.stringify({...taskData,id:blockedId,subject:"Existing unrelated Task"})});
  assert.equal(occupied.status,201,JSON.stringify(occupied));
  const rollbackInit={method:"POST",headers:{"content-type":"application/json",
    "idempotency-key":"signed-link-rollback-advance"},
    body:JSON.stringify(advanceData)};
  const rollback=await call("/api/sales-deals/"+rollbackDealId+"/advance",a,rollbackInit);
  assert.equal(rollback.status,409,JSON.stringify(rollback));
  assert.equal(rollback.body.code,"SALES_LINK_ATOMIC_COMMIT_CONFLICT");
  const unchanged=await call("/api/sales-deals/"+rollbackDealId,a);
  assert.equal(unchanged.status,200);
  assert.deepEqual(unchanged.body.item,rollbackCreate.body.item,
    "failed Task creation must roll back the original Deal stage/version");
  assert.deepEqual((await call("/api/activity-tasks/"+blockedId,a)).body.item,
    occupied.body.item,"rollback must not rewrite the colliding existing Task");
  const secondRollback=await call("/api/sales-deals/"+rollbackDealId+"/advance",a,rollbackInit);
  assert.equal(secondRollback.status,409,
    "failed stage change must not have written an idempotency receipt");
  assert.equal(secondRollback.body.code,"SALES_LINK_ATOMIC_COMMIT_CONFLICT");
  assert.deepEqual((await call("/api/sales-deals/"+rollbackDealId,a)).body.item,
    rollbackCreate.body.item,"retry after failure must still preserve original Deal");
  console.log(JSON.stringify({event:"crm.native-original-sales-activities-link-d1",
    result:"PASS",owner:"crm.sales-activities-link",proven:[
      "original SalesActivitiesLink::handle",
      "original ProtobufSalesActivitiesLinkContractAdapter",
      "original crm-activities Task::create",
      "same D1 transaction Deal+Task+immutable delivery",
      "exact account/tenant binding","closed stage ignored",
      "replay does not duplicate Task","signed Access owner",
      "A/B tenant Task denial","durable original Task codec",
      "D1 injected conflict rolls back Deal+Task+receipt atomically"],
    ui:"NOT_CONNECTED",published_live:"NOT_TESTED"}));
  console.log(JSON.stringify({event:"crm.native-original-sales-d1",result:"PASS",
    owner:"crm.sales",proven:["verified Access actor","original Account FK",
      "domain stage transition and close reason","exact integer currency",
      "strict codec D1 roundtrip","tenant isolation A/B",
      "idempotency receipts","optimistic version","forged owner rejected",
      "request correlation"],
    primary_contact:"NOT_CONNECTED",ui:"NOT_CONNECTED",published_live:"NOT_TESTED"}));
  // Original Contact Point lifecycle under real JWT and production migration.
  assert.equal((await call("/api/contact-points",null)).status,401);
  assert.deepEqual((await call("/api/contact-points",b)).body.items,[]);
  const contactPointId="native-contact-point-"+randomUUID();
  const pointData={id:contactPointId,party_id:id,kind:"email",
    value:"contact@example.org",preferred:true,
    valid_from_unix_nanos:null,valid_until_unix_nanos:null};
  const cpHeaders={"content-type":"application/json",
    "idempotency-key":"signed-contact-point-create"};
  const cpNotOwned=await call("/api/contact-points",b,
    {method:"POST",headers:cpHeaders,body:JSON.stringify(pointData)});
  assert.equal(cpNotOwned.status,422,"cross-tenant Party reference denied");
  assert.equal(cpNotOwned.body.code,"CONTACT_POINTS_PARTY_NOT_FOUND");
  const invalidPoint=await call("/api/contact-points",a,
    {method:"POST",headers:{...cpHeaders,"idempotency-key":"contact-point-invalid"},
      body:JSON.stringify({...pointData,value:"not-an-email"})});
  assert.equal(invalidPoint.status,422,"original domain rejects malformed email");
  const pointConsentSpoof=await call("/api/contact-points",a,{method:"POST",
    headers:{...cpHeaders,"idempotency-key":"contact-point-consent-spoof"},
    body:JSON.stringify({...pointData,consent:true})});
  assert.equal(pointConsentSpoof.status,422,"Contact Point cannot accept forged consent");
  const pointCreate=await call("/api/contact-points",a,
    {method:"POST",headers:cpHeaders,body:JSON.stringify(pointData)});
  assert.equal(pointCreate.status,201,JSON.stringify(pointCreate));
  assert.equal(pointCreate.body.item.party_id,id);
  assert.equal(pointCreate.body.item.kind,"email");
  assert.equal(pointCreate.body.item.normalized_value,"contact@example.org");
  assert.equal(pointCreate.body.item.verified,false,
    "record creation is not verification or communication consent");
  assert.equal(pointCreate.body.item.version,1);
  assert.equal(pointCreate.headers["x-crm-module"],"crm.contact-points");
  assert.match(pointCreate.headers["x-request-id"],/^[0-9a-f-]{36}$/i);
  assert.equal((await call("/api/contact-points/"+contactPointId,b)).status,404);
  assert.equal((await call("/api/contact-points/"+contactPointId,serviceB)).status,404);
  const pointRead=await call("/api/contact-points/"+contactPointId,a);
  assert.equal(pointRead.status,200);
  assert.deepEqual(pointRead.body.item,pointCreate.body.item,
    "original strict ContactPoint codec must survive D1 roundtrip");
  const pointReplay=await call("/api/contact-points",a,
    {method:"POST",headers:cpHeaders,body:JSON.stringify(pointData)});
  assert.equal(pointReplay.status,200);
  assert.equal(pointReplay.body.replayed,true);
  const keyConflict=await call("/api/contact-points",a,
    {method:"POST",headers:cpHeaders,
      body:JSON.stringify({...pointData,value:"different@example.org"})});
  assert.equal(keyConflict.status,409);
  assert.equal(keyConflict.body.code,"IDEMPOTENCY_CONFLICT");
  const pointUpdate={expected_version:1,value:"new@example.org",status:"active",
    preferred:false,valid_from_unix_nanos:null,valid_until_unix_nanos:null};
  const pointUpdateInit={method:"PATCH",headers:{"content-type":"application/json",
    "idempotency-key":"signed-contact-point-update"},
    body:JSON.stringify(pointUpdate)};
  const changedPoint=await call("/api/contact-points/"+contactPointId,a,pointUpdateInit);
  assert.equal(changedPoint.status,200,JSON.stringify(changedPoint));
  assert.equal(changedPoint.body.item.version,2);
  assert.equal(changedPoint.body.item.normalized_value,"new@example.org");
  assert.equal(changedPoint.body.item.verified,false);
  const changedReplay=await call("/api/contact-points/"+contactPointId,a,pointUpdateInit);
  assert.equal(changedReplay.status,200);
  assert.equal(changedReplay.body.replayed,true);
  const pointStale=await call("/api/contact-points/"+contactPointId,a,
    {...pointUpdateInit,headers:{...pointUpdateInit.headers,
      "idempotency-key":"signed-contact-point-stale"}});
  assert.equal(pointStale.status,409);
  assert.equal(pointStale.body.code,"CONTACT_POINTS_VERSION_CONFLICT");
  assert.equal((await call("/api/contact-points/"+contactPointId+"/verify",a,
    {method:"POST",headers:{"content-type":"application/json"},
      body:JSON.stringify({evidence_ref:"user-claims-verified"})})).status,404,
    "untrusted HTTP evidence must never create verified state");
  assert.equal((await call("/api/contact-points/"+contactPointId,a)).body.item.verified,false);
  const pointList=await call("/api/contact-points",a);
  assert.equal(pointList.status,200);
  assert.ok(pointList.body.items.some(x=>x.id===contactPointId&&x.version===2));
  assert.deepEqual((await call("/api/contact-points",b)).body.items,[]);
  console.log(JSON.stringify({event:"crm.native-original-contact-point-d1",result:"PASS",
    owner:"crm.contact-points",
    proven:["signed Access A/B","Party FK","domain email normalization and rejection",
      "strict codec D1 roundtrip","tenant isolation","idempotency","version",
      "no self-attested verification","request correlation"],
    consent:"NOT_CONNECTED",published_live:"NOT_TESTED"}));
  // Two ORIGINAL Party records are the ONLY valid relationship endpoints.
  assert.equal((await call("/api/party-relationships",null)).status,401);
  const otherPartyId="native-rel-party-"+randomUUID();
  const secondParty=await call("/api/parties",a,{method:"POST",headers:{
    "content-type":"application/json","idempotency-key":"signed-second-party"},
    body:JSON.stringify({id:otherPartyId,kind:"person",name:"Native Related Person"})});
  assert.equal(secondParty.status,201,JSON.stringify(secondParty));
  const relationshipId="native-relationship-"+randomUUID();
  const relationshipData={id:relationshipId,from_party_id:id,to_party_id:otherPartyId,
    code:"employment",directionality:"directional",from_role:"employer",to_role:"employee",
    valid_from_unix_nanos:null,valid_until_unix_nanos:null};
  const relationshipHeader={"content-type":"application/json",
    "idempotency-key":"signed-relationship-create"};
  const otherTenantRelationship=await call("/api/party-relationships",b,{
    method:"POST",headers:relationshipHeader,body:JSON.stringify(relationshipData)});
  assert.equal(otherTenantRelationship.status,422);
  assert.equal(otherTenantRelationship.body.code,"PARTY_RELATIONSHIPS_PARTY_NOT_FOUND");
  const invalidRelation=await call("/api/party-relationships",a,{
    method:"POST",headers:{...relationshipHeader,
      "idempotency-key":"signed-relationship-invalid"},
    body:JSON.stringify({...relationshipData,to_party_id:id})});
  assert.equal(invalidRelation.status,422,"original domain forbids self relationship");
  const relSpoof=await call("/api/party-relationships",a,{
    method:"POST",headers:{...relationshipHeader,
      "idempotency-key":"relationship-unknown-field"},
    body:JSON.stringify({...relationshipData,owner:"attacker"})});
  assert.equal(relSpoof.status,422,"Relationship rejects unknown owner field");
  const relCreated=await call("/api/party-relationships",a,{
    method:"POST",headers:relationshipHeader,body:JSON.stringify(relationshipData)});
  assert.equal(relCreated.status,201,JSON.stringify(relCreated));
  assert.equal(relCreated.body.item.id,relationshipId);
  assert.equal(relCreated.body.item.from_party_id,id);
  assert.equal(relCreated.body.item.to_party_id,otherPartyId);
  assert.equal(relCreated.body.item.type.code,"employment");
  assert.equal(relCreated.body.item.version,1);
  assert.equal(relCreated.headers["x-crm-module"],"crm.party-relationships");
  assert.match(relCreated.headers["x-request-id"],/^[0-9a-f-]{36}$/i);
  const relRead=await call("/api/party-relationships/"+relationshipId,a);
  assert.equal(relRead.status,200);
  assert.deepEqual(relRead.body.item,relCreated.body.item,
    "original relationship encode -> D1 -> decode must preserve type and roles");
  assert.equal((await call("/api/party-relationships/"+relationshipId,b)).status,404);
  assert.equal((await call("/api/party-relationships/"+relationshipId,serviceB)).status,404);
  const relReplay=await call("/api/party-relationships",a,{
    method:"POST",headers:relationshipHeader,body:JSON.stringify(relationshipData)});
  assert.equal(relReplay.status,200);
  assert.equal(relReplay.body.replayed,true);
  const relKeyConflict=await call("/api/party-relationships",a,{
    method:"POST",headers:relationshipHeader,
    body:JSON.stringify({...relationshipData,code:"other-code"})});
  assert.equal(relKeyConflict.status,409);
  assert.equal(relKeyConflict.body.code,"IDEMPOTENCY_CONFLICT");
  const relUpdate={expected_version:1,status:"inactive",
    valid_from_unix_nanos:null,valid_until_unix_nanos:null};
  const relInit={method:"PATCH",headers:{"content-type":"application/json",
    "idempotency-key":"signed-relationship-update"},body:JSON.stringify(relUpdate)};
  const relChanged=await call("/api/party-relationships/"+relationshipId,a,relInit);
  assert.equal(relChanged.status,200,JSON.stringify(relChanged));
  assert.equal(relChanged.body.item.version,2);
  assert.equal(relChanged.body.item.status,"inactive");
  const relUpdateReplay=await call("/api/party-relationships/"+relationshipId,a,relInit);
  assert.equal(relUpdateReplay.status,200);
  assert.equal(relUpdateReplay.body.replayed,true);
  const relStale=await call("/api/party-relationships/"+relationshipId,a,{
    ...relInit,headers:{...relInit.headers,
      "idempotency-key":"signed-relationship-stale"}});
  assert.equal(relStale.status,409);
  assert.equal(relStale.body.code,"PARTY_RELATIONSHIPS_VERSION_CONFLICT");
  assert.ok((await call("/api/party-relationships",a)).body.items
    .some(x=>x.id===relationshipId&&x.version===2));
  assert.deepEqual((await call("/api/party-relationships",b)).body.items,[]);
  console.log(JSON.stringify({event:"crm.native-original-party-relationships-d1",
    result:"PASS",owner:"crm.party-relationships",
    proven:["two Party FK in one tenant","original domain lifecycle",
      "strict codec/D1 roundtrip","Access A/B","idempotency receipts",
      "stale version","no reflexive relationship","request correlation"],
    published_live:"NOT_TESTED",ui:"NOT_CONNECTED"}));

  // Original Consent authority: fail closed until trusted subject grant evidence
  // exists. A signed operator can record a suppression, never a self-grant.
  assert.equal((await call("/api/consents",null)).status,401);
  assert.deepEqual((await call("/api/consents",b)).body.items,[]);
  const evalScope={party_id:id,contact_point_id:contactPointId,
    purpose:"marketing.newsletter",channel:"email"};
  const evalInit={method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify(evalScope)};
  const ungranted=await call("/api/consents/evaluate",a,evalInit);
  assert.equal(ungranted.status,200,JSON.stringify(ungranted));
  assert.equal(ungranted.body.allowed,false);
  assert.equal(ungranted.body.reason,"no_applicable_grant");
  assert.equal(ungranted.body.grant_ingress,"NOT_CONNECTED");
  const noGrant=await call("/api/consents",a,{
    method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({effect:"grant",evidence_ref:"i-consent"})});
  assert.equal(noGrant.status,404,"untrusted consent grant ingress absent");
  const forgedGrant=await call("/api/consents/suppress",a,{
    method:"POST",headers:{"content-type":"application/json",
      "idempotency-key":"forged-consent-grant"},
    body:JSON.stringify({id:"spoof",...evalScope,jurisdiction:"eu",
      effect:"grant",evidence_ref:"unverified"})});
  assert.equal(forgedGrant.status,422,"strict API denies forged Grant and evidence");
  const suppressionId="native-consent-suppression-"+randomUUID();
  const suppression={id:suppressionId,...evalScope,jurisdiction:"eu"};
  const supHeaders={"content-type":"application/json",
    "idempotency-key":"signed-original-consent-suppression"};
  const crossTenantSuppression=await call("/api/consents/suppress",b,{
    method:"POST",headers:supHeaders,body:JSON.stringify(suppression)});
  assert.equal(crossTenantSuppression.status,422);
  assert.equal(crossTenantSuppression.body.code,"CONSENTS_PARTY_NOT_FOUND");
  const wrongPartySuppression=await call("/api/consents/suppress",a,{
    method:"POST",headers:{...supHeaders,
      "idempotency-key":"consent-wrong-contact-party"},
    body:JSON.stringify({...suppression,party_id:otherPartyId})});
  assert.equal(wrongPartySuppression.status,422);
  assert.equal(wrongPartySuppression.body.code,"CONSENTS_CONTACT_POINT_PARTY_MISMATCH");
  const suppressed=await call("/api/consents/suppress",a,{
    method:"POST",headers:supHeaders,body:JSON.stringify(suppression)});
  assert.equal(suppressed.status,201,JSON.stringify(suppressed));
  assert.equal(suppressed.body.item.id,suppressionId);
  assert.equal(suppressed.body.item.effect,"deny");
  assert.equal(suppressed.body.item.status,"active");
  assert.equal(suppressed.body.item.source,"crm.access.operator");
  assert.match(suppressed.body.item.evidence_ref,/^operator-action:[0-9a-f-]{36}$/i);
  assert.equal(suppressed.body.operator_action,true);
  assert.equal(suppressed.body.subject_consent_evidence,false);
  assert.equal(suppressed.body.item.version,1);
  assert.equal(suppressed.headers["x-crm-module"],"crm.consents");
  const suppressionReplay=await call("/api/consents/suppress",a,{
    method:"POST",headers:supHeaders,body:JSON.stringify(suppression)});
  assert.equal(suppressionReplay.status,200);
  assert.equal(suppressionReplay.body.replayed,true);
  const suppressionIdemConflict=await call("/api/consents/suppress",a,{
    method:"POST",headers:supHeaders,
    body:JSON.stringify({...suppression,purpose:"other"})});
  assert.equal(suppressionIdemConflict.status,409);
  assert.equal(suppressionIdemConflict.body.code,"IDEMPOTENCY_CONFLICT");
  const denied=await call("/api/consents/evaluate",a,evalInit);
  assert.equal(denied.status,200,JSON.stringify(denied));
  assert.equal(denied.body.allowed,false);
  assert.equal(denied.body.reason,"active_deny");
  assert.deepEqual(denied.body.determining_authorization_ids,[suppressionId]);
  const unscoped=await call("/api/consents/evaluate",a,{
    ...evalInit,body:JSON.stringify({...evalScope,purpose:"nonmarketing"})});
  assert.equal(unscoped.status,200);
  assert.equal(unscoped.body.reason,"no_applicable_grant");
  const afterDenied=await call("/api/consents",a);
  assert.equal(afterDenied.status,200);
  assert.ok(afterDenied.body.items.some(x=>x.id===suppressionId&&x.effect==="deny"));
  assert.deepEqual((await call("/api/consents",b)).body.items,[]);
  assert.equal((await call("/api/consents/"+suppressionId+"/withdraw",a,
    {method:"POST",headers:{"content-type":"application/json"},
      body:JSON.stringify({expected_version:1})})).status,404,
    "operator suppression cannot be reinterpreted as subject consent withdrawal");
  console.log(JSON.stringify({event:"crm.native-original-consents-d1",result:"PASS",
    owner:"crm.consents",proven:["original Consent Authorization deny aggregate",
      "original evaluate_communication_authorization fail-closed",
      "signed Access operator provenance","Party + ContactPoint tenant integrity",
      "strict codec D1 roundtrip","no client-issued grant or forged evidence",
      "A/B tenant isolation","idempotency","request correlation"],
    grant_ingress:"NOT_CONNECTED",subject_consent_evidence:"NOT_CONNECTED",
    ui:"NOT_CONNECTED",published_live:"NOT_TESTED"}));

  // One native read-only Account card: reuse the ORIGINAL owner codecs and
  // existing D1, never legacy crm_customers/customer360 pilot tables.
  const overviewPath="/api/accounts/"+accountId+"/overview";
  assert.equal((await call(overviewPath,null)).status,401);
  assert.equal((await call(overviewPath,b)).status,404,
    "foreign Access tenant cannot see an original Account card");
  assert.equal((await call(overviewPath,serviceB)).status,404);
  assert.equal((await call("/api/accounts/does-not-exist/overview",a)).status,404);
  assert.equal((await call(overviewPath,a,{method:"POST"})).status,404,
    "native original Account card must remain read-only");
  const overview=await call(overviewPath,a);
  assert.equal(overview.status,200,JSON.stringify(overview));
  assert.equal(overview.headers["x-crm-module"],"crm.customer-accounts");
  assert.match(overview.headers["x-request-id"],/^[0-9a-f-]{36}$/i);
  assert.deepEqual(overview.body.account,savedAccount.body.item,
    "projection must reuse original Account state and view");
  assert.equal(overview.body.primary_party.id,id);
  assert.ok(overview.body.contact_points.some(x=>x.id===contactPointId &&
    x.normalized_value==="new@example.org" && x.verified===false));
  assert.ok(overview.body.party_relationships.some(x=>x.id===relationshipId &&
    x.status==="inactive"));
  assert.ok(overview.body.deals.some(x=>x.id===dealId &&
    x.status==="won" && x.amount.minor_units==="12500"));
  assert.ok(overview.body.tasks.some(x=>x.id===taskId &&
    x.status==="completed"));
  assert.ok(overview.body.tasks.some(x=>x.id===linkedId &&
    x.related_resources[0].resource_id===dealId));
  assert.equal(overview.body.scope.contact_points,"primary_party_only");
  assert.equal(overview.body.scope.limit_per_collection,100);
  assert.deepEqual(overview.body.truncated,{
    contact_points:false,party_relationships:false,deals:false,tasks:false});
  const overviewAgain=await call(overviewPath,a);
  assert.deepEqual(overviewAgain.body,overview.body,"read-only D1 projection is stable");
  console.log(JSON.stringify({event:"crm.native-original-account-overview-d1",
    result:"PASS",original_owners:["crm.customer-accounts","crm.parties",
      "crm.contact-points","crm.party-relationships","crm.sales","crm.activities"],
    persistence:"one native D1",writes:"none",ui:"NOT_CONNECTED",
    cloudflare_live:"NOT_TESTED"}));

  const init={method:"PATCH",headers:{"content-type":"application/json",
    "idempotency-key":"native-signed-update"},
    body:JSON.stringify({expected_version:1,name:"Native Updated"})};
  const changed=await call("/api/parties/"+id,a,init);
  assert.equal(changed.status,200,JSON.stringify(changed));
  assert.equal(changed.body.item.version,2);
  const replay=await call("/api/parties/"+id,a,init);
  assert.equal(replay.status,200);
  assert.equal(replay.body.replayed,true);
  // Browser proof: the EXISTING React Parties screen talks directly to this
  // single native workers-rs Worker, no JS API handler or internal Worker RPC.
  // The other pilot React tabs are intentionally not in scope for this gate.
  const evidenceDir="e2e-evidence-native";
  mkdirSync(join(cwd,"..",evidenceDir),{recursive:true});
  const browser=await chromium.launch({headless:true});
  const browserErrors=[];
  const marker="NativeRust-"+randomUUID().slice(0,8);
  const screenshot=join(cwd,"..",evidenceDir,"native-party-desktop.png");
  const manifest=join(cwd,"..",evidenceDir,"manifest.json");
  let uiSteps=[];
  try{
    const context=await browser.newContext({viewport:{width:1280,height:850},
      extraHTTPHeaders:{"cf-access-jwt-assertion":a}});
    const page=await context.newPage();
    page.on("pageerror",error=>browserErrors.push(error.message));
    const index=await page.goto(origin,{waitUntil:"domcontentloaded"});
    assert.equal(index.status(),200,"the SAME native Rust Worker serves React Static Assets");
    await page.getByRole("navigation",{name:"Разделы"}).getByRole("button",
      {name:/Лица и организации/}).click();
    const area=page.locator('section[aria-label="Лица и организации (Rust)"]');
    await area.waitFor();
    uiSteps.push("React existing Parties screen rendered from native Worker assets");
    await area.getByRole("button",{name:/Добавить лицо/}).click();
    await page.getByLabel("Имя / название").fill(marker);
    await page.getByRole("button",{name:"Сохранить",exact:true}).click();
    await area.getByRole("status").getByText("Сохранено через Rust / D1").waitFor();
    await area.getByRole("row").filter({hasText:marker}).waitFor();
    uiSteps.push("React click -> original Rust Party create -> D1 persistence -> UI");
    const persisted=await call("/api/parties",a);
    assert.equal(persisted.status,200);
    assert.ok(persisted.body.items.some(x=>x.name===marker),
      "Native API must read the exact browser-created Party from D1");
    const row=area.getByRole("row").filter({hasText:marker});
    await row.getByRole("button",{name:"Изменить"}).click();
    const updatedName=marker+"-updated";
    await page.getByLabel("Имя / название").fill(updatedName);
    await page.getByRole("button",{name:"Сохранить",exact:true}).click();
    await area.getByRole("row").filter({hasText:updatedName}).waitFor();
    uiSteps.push("Rust versioned update reflected in React UI");
    await page.reload({waitUntil:"domcontentloaded"});
    await page.getByRole("navigation",{name:"Разделы"}).getByRole("button",
      {name:/Лица и организации/}).click();
    await page.locator('section[aria-label="Лица и организации (Rust)"]')
      .getByRole("row").filter({hasText:updatedName}).waitFor();
    uiSteps.push("Browser reload -> persisted D1 state available");
    // Native-only parity: the same React frontend must create and read every
    // current screen using this one Rust fetch entrypoint. No JS fallback.
    const customer="Native customer "+marker;
    await page.getByRole("navigation",{name:"Разделы"}).getByRole("button",{name:/Клиенты/}).click();
    await page.getByRole("button",{name:"+ Добавить клиента"}).click();
    await page.getByLabel("Имя / название").fill(customer);
    await page.getByRole("button",{name:"Сохранить",exact:true}).click();
    await page.getByText(customer,{exact:true}).first().waitFor();
    const customers=await call("/api/customers",a);
    assert.equal(customers.status,200);
    const saved=customers.body.items.find(x=>x.name===customer);
    assert.ok(saved?.id);
    assert.equal((await call("/api/customers",b)).body.items.some(x=>x.id===saved.id),false);
    await page.getByRole("navigation",{name:"Разделы"}).getByRole("button",{name:/Контакты/}).click();
    await page.getByRole("button",{name:"+ Добавить контакт"}).click();
    await page.getByLabel("Клиент").selectOption({label:customer});
    await page.getByLabel("Имя",{exact:true}).fill(marker+" Contact");
    await page.getByRole("button",{name:"Сохранить",exact:true}).click();
    await page.getByText(marker+" Contact",{exact:true}).first().waitFor();
    await page.getByRole("navigation",{name:"Разделы"}).getByRole("button",{name:/Сделки/}).click();
    await page.getByRole("button",{name:"+ Добавить сделку"}).click();
    await page.getByLabel("Клиент").selectOption({label:customer});
    await page.getByLabel("Название сделки").fill(marker+" Deal");
    await page.getByRole("button",{name:"Сохранить",exact:true}).click();
    await page.getByText(marker+" Deal",{exact:true}).first().waitFor();
    await page.getByRole("navigation",{name:"Разделы"}).getByRole("button",{name:/Задачи/}).click();
    await page.getByRole("button",{name:"+ Добавить задачу"}).click();
    await page.getByLabel("Клиент").selectOption({label:customer});
    await page.getByLabel("Что сделать").fill(marker+" Task");
    await page.getByRole("button",{name:"Сохранить",exact:true}).click();
    await page.getByText(marker+" Task",{exact:true}).first().waitFor();
    const card=await call("/api/customer360/"+saved.id,a);
    assert.equal(card.status,200);
    assert.equal(card.body.summary.contacts,1);
    assert.equal(card.body.summary.deals,1);
    assert.equal(card.body.summary.open_tasks,1);
    assert.equal((await call("/api/customer360/"+saved.id,b)).status,404);
    const session=await call("/api/session",a);
    assert.equal(session.status,200);
    assert.equal(session.body.role,"admin");
    uiSteps.push("Native Rust-only customers/contacts/deals/tasks/Customer360 and Access A/B");
    assert.deepEqual(browserErrors,[],"React browser has no uncaught JS errors");
    await page.screenshot({path:screenshot,fullPage:true,animations:"disabled"});
    const result={git_sha:process.env.CRM_SOURCE_SHA||"unknown",result:"PASS",
      runtime:"one native workers-rs workerd + Cloudflare Static Assets + isolated D1",
      auth:"real self-signed RS256 verified in Rust WebCrypto (CI-only JWKS)",
      frontend:"existing React Parties.jsx",module:"crm.parties",
      checks:uiSteps, screenshot:"native-party-desktop.png",
      cloudflare_access_live:"NOT_TESTED",published_cloudflare_url:"NOT_TESTED",
      other_crm_modules:"customers/contacts/deals/tasks/Customer360 tested on native"};
    writeFileSync(manifest,JSON.stringify(result,null,2)+"\n");
    console.log(JSON.stringify({event:"crm.native-react-chromium",result:"PASS",checks:uiSteps}));
    await context.close();
  }finally{
    await browser.close();
  }
  console.log(JSON.stringify({event:"crm.native-jwt-workerd",result:"PASS",
    proven:["host WebCrypto RS256 verified","Access service-token empty-sub identity and forged-header negatives",
      "issuer/audience/expiry/alg/kid/forged denied",
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

// Exact native workerd Access RS256 proof, isolated D1 and signed CI-only JWTs.
import {spawn,execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,randomUUID} from "node:crypto";
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

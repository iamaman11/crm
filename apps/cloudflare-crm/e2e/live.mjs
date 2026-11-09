// One physical protected release: Chromium -> original React -> native Rust
// -> the real D1 -> independent verify-data/Cloudflare Logs, same source SHA.
import {chromium} from "playwright";
import {mkdir,writeFile} from "node:fs/promises";
import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";

const sha=process.env.GITHUB_SHA,url="https://crm-dev.crm-5bd5e1e1.workers.dev";
assert.match(sha||"",/^[a-f0-9]{40}$/);
const access=name=>({
  "CF-Access-Client-Id":process.env["CRM_E2E_"+name+"_CLIENT_ID"],
  "CF-Access-Client-Secret":process.env["CRM_E2E_"+name+"_CLIENT_SECRET"]
});
for(const name of ["A","B"])if(!access(name)["CF-Access-Client-Id"]||
  !access(name)["CF-Access-Client-Secret"])throw Error("Missing protected Access "+name);
const marker="Native-"+sha.slice(0,8)+"-"+Date.now().toString(36),dir="apps/cloudflare-crm/e2e-evidence";
const browser=await chromium.launch({headless:true});
const context=await browser.newContext({viewport:{width:1366,height:900},
  extraHTTPHeaders:access("A")});
const page=await context.newPage(),nativeResponses=[],uiCoverage=new Set(),errors=[];
const pending=[];
const evidence={git_sha:sha,deployed_url:url,browser:"Chromium",
  viewport:"desktop + mobile",status:"IN_PROGRESS",screenshots:[],api:[],error:null};
const cover=name=>uiCoverage.add(name);
const kindOf=path=>path.split("/")[2];
function operation(path,method){
  const owner=kindOf(path);
  let action=method==="PATCH"?"update":method==="POST"?"create":"read";
  if(path.endsWith("/advance"))action="advance";
  if(path.endsWith("/complete"))action="complete";
  return owner+"."+action;
}
page.on("pageerror",e=>errors.push(e.message));
page.on("response",r=>{
  const path=new URL(r.url()).pathname;
  if(!path.startsWith("/api/"))return;
  pending.push((async()=>{
    const method=r.request().method(),h=r.headers();
    const record={path,method,module:kindOf(path),operation:operation(path,method),
      status:r.status(),request_id:h["x-request-id"],
      source_sha:h["x-crm-source-sha"],error_code:h["x-crm-error-code"]||null,
      idempotency_key:r.request().headers()["idempotency-key"]||null};
    if(h["x-crm-runtime"]!=="native-workers-rs"||record.source_sha!==sha||
      !/^[0-9a-f-]{36}$/i.test(record.request_id||""))
      errors.push("Missing native trace "+method+" "+path);
    if(["POST","PATCH"].includes(method)&&[200,201].includes(r.status())){
      const body=await r.json().catch(()=>({}));
      record.subject_id=body.item?.id;
      if(!record.subject_id)errors.push("Mutation missing owner record id "+path);
    }
    nativeResponses.push(record);
  })());
});
const tab=title=>page.getByRole("navigation",{name:"Разделы"})
  .getByRole("button",{name:new RegExp(title)});
const button=name=>page.getByRole("button",{name,exact:true});
const row=name=>page.getByRole("row").filter({hasText:name});
async function save(){
  await button("Сохранить").click();
  await page.getByRole("status").getByText("Сохранено").waitFor({timeout:15000});
}
async function shot(file,operationName){
  await mkdir(dir,{recursive:true});
  await page.screenshot({path:dir+"/"+file+".png",fullPage:true,animations:"disabled"});
  evidence.screenshots.push({file:file+".png",operation:operationName,result:"PASS"});
  console.log(JSON.stringify({event:"crm.ui-evidence",operation:operationName,result:"PASS"}));
}
async function readOwner(owner,match){
  const r=await context.request.get(url+"/api/"+owner);
  assert.equal(r.status(),200,"Read owner "+owner);
  const items=(await r.json()).items;
  const item=items.find(match);
  assert.ok(item?.id,"Browser change must be durable in "+owner);
  return item;
}
async function accessB(){
  const other=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    const key=randomUUID();
    const r=await other.request.post(url+"/api/parties",{headers:{
      "idempotency-key":key},
      data:{id:randomUUID(),kind:"person",name:marker+" Tenant B"}});
    assert.equal(r.status(),201,"Real Access B Party creation");
    const created=(await r.json()).item;
    assert.equal((await context.request.get(url+"/api/parties/"+created.id)).status(),404);
    cover("access.tenant-A-B");
    return {id:created.id,key};
  }finally{await other.close();}
}
let outcome="FAIL";
try{
  const version=await context.request.get(url+"/api/version");
  assert.equal(version.status(),200,"Protected native version");
  assert.equal((await version.json()).source,sha,"Browser must use exact deployed source");
  const diagnosis=await context.request.get(url+"/api/doctor");
  assert.equal(diagnosis.status(),200,"Live D1 and Access doctor");
  const doctor=await diagnosis.json();
  assert.equal(doctor.runtime,"workers-rs");
  assert.equal(doctor.database.tables.expected,18);
  await page.goto(url,{waitUntil:"domcontentloaded"});
  await page.getByRole("heading",{name:"Моя CRM"}).waitFor();
  await shot("01-home","Home original Rust owners");
  await tab("Лица и организации").click();
  await button("+ Добавить лицо / организацию").click();
  await button("Отмена").click();cover("parties.create.cancel");
  await button("+ Добавить лицо / организацию").click();
  await page.getByLabel("Имя / название").fill(marker);
  await save();cover("parties.create");
  const party=await readOwner("parties",x=>x.name===marker);
  await row(marker).getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Имя / название").fill(marker+" Updated");
  await save();cover("parties.update");
  await shot("02-party","Original Party create and update");
  const partyA=await readOwner("parties",x=>x.id===party.id);
  assert.equal(partyA.version,2);
  const tenantB=await accessB();
  assert.equal((await browser.newContext({extraHTTPHeaders:access("B")})
    .then(async ctx=>{try{return (await ctx.request.get(url+"/api/parties/"+party.id)).status();}
      finally{await ctx.close();}})),404);
  await tab("Клиенты").click();
  await button("+ Добавить клиента").click();
  await page.getByLabel("Название",{exact:true}).fill(marker+" Account");
  await page.getByLabel("Лицо / организация",{exact:true}).selectOption({label:marker+" Updated"});
  await save();cover("accounts.create");
  const account=await readOwner("accounts",x=>x.name===marker+" Account");
  assert.equal(account.parties[0].id,party.id);
  await row(marker+" Account").getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Название",{exact:true}).fill(marker+" Account Updated");
  await save();cover("accounts.update");
  await shot("03-accounts","Original Account linked to Party");
  const contact=marker.toLowerCase()+"@example.test";
  await tab("Контакты").click();
  await button("+ Добавить контакт").click();
  await page.getByLabel("Лицо / организация",{exact:true}).selectOption({label:marker+" Updated"});
  await page.getByLabel("Значение").fill(contact);
  await save();cover("contact-points.create");
  await row(contact).getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Значение").fill("updated-"+contact);
  await save();cover("contact-points.update");
  const point=await readOwner("contact-points",x=>x.value==="updated-"+contact);
  assert.equal(point.verified,false,"Self-attested verification is not trusted evidence");
  await shot("04-contacts","Original ContactPoint normalization and update");
  const dealName=marker+" Deal";
  await tab("Сделки").click();
  await button("+ Добавить сделку").click();
  await page.getByLabel("Название сделки").fill(dealName);
  await page.getByLabel("Клиент",{exact:true}).selectOption({label:marker+" Account Updated"});
  await page.getByLabel("Сумма (минимальные единицы)").fill("12750");
  await save();cover("sales-deals.create");
  const deal=await readOwner("sales-deals",x=>x.name===dealName);
  assert.equal(deal.amount.minor_units,"12750");
  await row(dealName).getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Название сделки").fill(dealName+" Updated");
  await save();cover("sales-deals.update");
  await row(dealName+" Updated").getByRole("button",{name:"Этап сделки"}).click();
  await page.getByLabel("Новый этап").selectOption("qualified");
  await save();cover("sales-deals.advance");
  assert.equal((await readOwner("sales-deals",x=>x.id===deal.id)).stage.stage_id,"qualified");
  await shot("05-deals","Original Sales create/update/advance");
  await button("Воронка").click();cover("deals.board");
  await shot("06-pipeline","Original Sales pipeline board");
  await button("Таблица").click();cover("deals.table");
  const subject=marker+" Task";
  await tab("Задачи").click();
  await button("+ Добавить задачу").click();
  await page.getByLabel("Что сделать").fill(subject);
  await page.getByLabel("Клиент",{exact:true}).selectOption({label:marker+" Account Updated"});
  await save();cover("activity-tasks.create");
  const task=await readOwner("activity-tasks",x=>x.subject===subject);
  await row(subject).getByRole("button",{name:"Выполнить"}).click();
  await save();cover("activity-tasks.complete");
  assert.equal((await readOwner("activity-tasks",x=>x.id===task.id)).status,"completed");
  await shot("07-tasks","Original Activity task completion");
  await tab("Клиенты").click();
  await row(marker+" Account Updated").getByRole("button",{name:"Карточка"}).click();
  await page.getByRole("heading",{name:marker+" Account Updated"}).waitFor();
  await page.getByRole("region",{name:"Контакты клиента"}).getByText("updated-"+contact).waitFor();
  await page.getByRole("region",{name:"Сделки клиента"}).getByText(dealName+" Updated").waitFor();
  await page.getByRole("region",{name:"Задачи клиента"}).getByText(subject).waitFor();
  await shot("08-account-overview","Original multi-owner Account read projection");
  await page.route("**/api/accounts/**/overview",route=>route.abort());
  await button("Обновить карточку").click();
  await button("Повторить").waitFor();
  cover("customer360.retry");
  await shot("09-card-retry","Failed network request and explicit retry");
  await page.unroute("**/api/accounts/**/overview");
  await button("Повторить").click();
  await page.getByRole("heading",{name:marker+" Account Updated"}).waitFor();
  cover("customer360.refresh");
  await button("← К списку клиентов").click();cover("customer360.back");
  await button("Обновить").click();
  await page.getByRole("status").getByText("Обновлено").waitFor();
  cover("global.refresh");
  await page.reload({waitUntil:"domcontentloaded"});
  await row(marker+" Account Updated").waitFor();cover("browser.reload");
  await page.getByRole("textbox",{name:"Поиск"}).fill(marker+" Account Updated");
  await row(marker+" Account Updated").waitFor();cover("accounts.search");
  await page.getByRole("textbox",{name:"Поиск"}).fill("");
  await page.setViewportSize({width:390,height:844});
  await page.locator(".mobile article").filter({hasText:marker+" Account Updated"})
    .getByRole("button",{name:"Карточка"}).click();cover("mobile.customer.card");
  await page.getByRole("heading",{name:marker+" Account Updated"}).waitFor();
  await shot("10-mobile","Mobile native Account overview");
  await Promise.all(pending);
  const operations=["parties.create","parties.update","accounts.create","accounts.update",
    "contact-points.create","contact-points.update","sales-deals.create","sales-deals.update",
    "sales-deals.advance","activity-tasks.create","activity-tasks.complete"];
  const writes=nativeResponses.filter(r=>["POST","PATCH"].includes(r.method)&&
    [200,201].includes(r.status)&&operations.includes(r.operation));
  const missing=operations.filter(name=>!writes.some(w=>w.operation===name));
  assert.deepEqual(missing,[],"Native browser must complete each original owner operation");
  assert.equal(new Set(writes.map(x=>x.request_id)).size,writes.length);
  assert.deepEqual(errors,[],"No missing worker traces or browser errors");
  await mkdir("apps/cloudflare-crm/.release",{recursive:true});
  const click=writes.find(x=>x.operation==="parties.create");
  await writeFile("apps/cloudflare-crm/.release/native-click.json",JSON.stringify({
    git_sha:sha,action:"parties.create",party_id:party.id,
    status:click.status,request_id:click.request_id})+"\n");
  await writeFile("apps/cloudflare-crm/.release/parties-e2e.json",JSON.stringify({
    git_sha:sha,parties:[party.id,tenantB.id],
    keys:[writes.find(x=>x.operation==="parties.create").idempotency_key,
      writes.find(x=>x.operation==="parties.update").idempotency_key,tenantB.key]})+"\n");
  const coverage=Array.from(uiCoverage);
  await writeFile("apps/cloudflare-crm/.release/native-functional-cycle.json",
    JSON.stringify({git_sha:sha,runtime:"native-workers-rs",
      ui_coverage:coverage,requests:nativeResponses,writes,
      result:"BROWSER_PASS_D1_LOGS_PENDING"},null,2)+"\n");
  evidence.ui_coverage=coverage;evidence.native_http_responses=nativeResponses.length;
  evidence.native_mutations=writes.length;
  evidence.status=outcome="PASS";
  console.log(JSON.stringify({event:"crm.browser-e2e",result:"PASS",sha,
    original_actions:operations,ui_coverage:coverage.length,writes:writes.length}));
}catch(e){
  evidence.status="FAIL";
  evidence.error={message:String(e.message).slice(0,320)};
  await page.screenshot({path:"apps/cloudflare-crm/e2e-failure.png"}).catch(()=>{});
  console.error("CRM protected native browser E2E FAIL: "+e.message);
  throw e;
}finally{
  await mkdir(dir,{recursive:true});
  await writeFile(dir+"/manifest.json",JSON.stringify(evidence,null,2)+"\n");
  await browser.close();
}

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
  if(path.endsWith("/suppress"))action="suppress";
  if(path.startsWith("/api/identity-resolution/compare/"))action="compare";
  if(path==="/api/identity-resolution/candidates")action="candidates";
  if(path.startsWith("/api/data-quality/parties/")&&path.endsWith("/evaluate"))action="evaluate";
  if(path==="/api/customer-data-operations/preview")action="preview";
  if(path==="/api/customer-data-operations/parties.csv")action="export";
  if(path==="/api/customer-data-operations/import-preview")action="import-preview";
  if(path==="/api/customer-data-operations/import-apply")action="import-apply";
  if(path.endsWith("/evaluate"))action="evaluate";
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
    // Consents/evaluate is a read-only query carried over POST. Never claim a D1 mutation.
    if(["POST","PATCH"].includes(method)&&!path.endsWith("/evaluate")&&
      !path.startsWith("/api/customer-data-operations/import-")&&
      [200,201].includes(r.status())){
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
  // Physical browser proof of the original PartyRelationships owner.
  // No JS duplicate domain, no fake persistence or manual SQL shortcut.
  await tab("Лица и организации").click();
  await button("+ Добавить лицо / организацию").click();
  await page.getByLabel("Имя / название").fill(marker+" Related");
  await save();cover("parties.create.related");
  const relatedParty=await readOwner("parties",x=>x.name===marker+" Related");
  // Physical UI -> original Rust canonical pair -> scoped D1 Party read.
  // Read-only: no fictional match score and no D1 write/audit expected.
  await tab("Сравнение записей").click();
  await page.getByLabel("Первая запись").selectOption(party.id);
  await page.getByLabel("Вторая запись").selectOption(relatedParty.id);
  await button("Сравнить").click();
  await page.getByRole("status").getByText(
    "Сравнение выполнено без изменения записей.").waitFor();
  cover("identity-resolution.compare");
  const compared=await context.request.get(url+
    "/api/identity-resolution/compare/"+party.id+"/"+relatedParty.id);
  assert.equal(compared.status(),200);
  const comparison=await compared.json();
  assert.equal(comparison.read_only,true);
  assert.equal(comparison.candidate_cases,"NOT_CONNECTED");
  assert.equal(comparison.first.id,party.id,"first UI-selected Party");
  assert.equal(comparison.second.id,relatedParty.id,"second UI-selected Party");
  assert.deepEqual([comparison.canonical_pair.left_id,
    comparison.canonical_pair.right_id].sort(),[party.id,relatedParty.id].sort());
  await page.getByLabel("Вторая запись").selectOption(party.id);
  await button("Сравнить").click();
  await page.getByRole("alert").getByText(/IDENTITY_RESOLUTION_SELF_PAIR_INVALID/).waitFor();
  cover("identity-resolution.self-pair-denied");
  await button("Сбросить").click();
  assert.equal(await page.getByRole("alert").count(),0);
  cover("identity-resolution.reset");
  const identityContextB=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    assert.equal((await identityContextB.request.get(url+
      "/api/identity-resolution/compare/"+party.id+"/"+relatedParty.id)).status(),404);
    cover("identity-resolution.tenant-isolation");
  }finally{await identityContextB.close();}
  await shot("02-identity","Read-only original Identity Resolution Party comparison");
  // A true live Party quality user cycle: current D1 Party, original Rust rules,
  // error, tenant isolation and versioned policy. No invented completeness score.
  await tab("Качество данных").click();
  await page.getByLabel("Лицо или организация для проверки").selectOption(party.id);
  await button("Проверить качество").click();
  await page.getByRole("status").getByText(/Замечаний нет/).waitFor();
  cover("data-quality.evaluate.pass");
  const qualityGood=await context.request.get(url+"/api/data-quality/parties/"+party.id+"/evaluate");
  assert.equal(qualityGood.status(),200);
  const qualityGoodBody=await qualityGood.json();
  assert.equal(qualityGoodBody.party.id,party.id);
  assert.equal(qualityGoodBody.checked,2);
  assert.equal(qualityGoodBody.failed,0);
  assert.equal(qualityGoodBody.completeness_score,"NOT_COMPUTED");
  const tenantBQuality=await browser.newContext({extraHTTPHeaders:access("B")});
  try {
    assert.equal((await tenantBQuality.request.get(url+
      "/api/data-quality/parties/"+party.id+"/evaluate")).status(),404);
    cover("data-quality.tenant-isolation");
  }finally{await tenantBQuality.close();}
  assert.equal((await context.request.post(url+
    "/api/data-quality/parties/"+party.id+"/evaluate")).status(),404);
  cover("data-quality.mutation-unavailable");
  await button("Сбросить").click();
  assert.equal(await page.getByLabel("Лицо или организация для проверки").inputValue(),"");
  cover("data-quality.reset");
  await shot("02-data-quality","Original Rust read-only Party name evaluation");
  await tab("Связи").click();
  await button("+ Добавить связь").click();
  await button("Отмена").click();cover("party-relationships.create.cancel");
  await button("+ Добавить связь").click();
  await page.getByLabel("От кого").selectOption({label:marker+" Updated"});
  await page.getByLabel("К кому").selectOption({label:marker+" Updated"});
  await button("Сохранить").click();
  await page.getByRole("alert").waitFor();
  cover("party-relationships.create.self-denied");
  await button("Отмена").click();
  await button("+ Добавить связь").click();
  await page.getByLabel("От кого").selectOption({label:marker+" Updated"});
  await page.getByLabel("К кому").selectOption({label:marker+" Related"});
  await save();cover("party-relationships.create");
  const rel=await readOwner("party-relationships",x=>
    x.from_party_id===party.id&&x.to_party_id===relatedParty.id);
  assert.equal(rel.type.code,"employment");
  assert.equal(rel.type.from_role,"employer");
  assert.equal(rel.type.to_role,"employee");
  assert.equal(rel.version,1);
  const otherContext=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    assert.equal((await otherContext.request.get(
      url+"/api/party-relationships/"+rel.id)).status(),404);
    cover("party-relationships.tenant-isolation");
  }finally{await otherContext.close();}
  await row(marker+" Related").getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Статус связи").selectOption("inactive");
  await save();cover("party-relationships.update");
  assert.equal((await readOwner("party-relationships",x=>x.id===rel.id)).version,2);
  assert.equal((await readOwner("party-relationships",x=>x.id===rel.id)).status,"inactive");
  await shot("02-relationships","Original PartyRelationship create/update and tenant isolation");
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
  // A physical operator download, not a mocked file or JS-generated CSV.
  await tab("Экспорт данных").click();
  await page.getByLabel("Тип записей").selectOption("organization");
  await button("Проверить экспорт").click();
  await page.getByRole("region",{name:"Предпросмотр экспорта"})
    .getByText(/Подготовлено записей:/).waitFor();
  cover("customer-data-operations.preview");
  const previewResp=await context.request.get(url+
    "/api/customer-data-operations/preview?kind=organization");
  assert.equal(previewResp.status(),200);
  const preview=(await previewResp.json());
  assert.ok(preview.count>=1);
  assert.equal(preview.fields.join(","),"party_id,kind,display_name,resource_version");
  const downloadEvent=page.waitForEvent("download");
  await button("Скачать CSV").click();
  const download=await downloadEvent;
  assert.match(download.suggestedFilename(),/^crm-parties-organization\.csv$/);
  await page.getByRole("status").getByText(/CSV подготовлен/).waitFor();
  cover("customer-data-operations.export.download");
  const file=await context.request.get(url+
    "/api/customer-data-operations/parties.csv?kind=organization");
  assert.equal(file.status(),200);
  const exportText=await file.text();
  assert.ok(exportText.includes(party.id));
  assert.ok(exportText.includes(marker+" Updated"));
  assert.ok(!exportText.includes("Tenant B"),
    "CSV cannot export another Access tenant");
  assert.equal(file.headers()["x-crm-export-sha256"],preview.csv_sha256);
  const contextB=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    const other=await contextB.request.get(url+
      "/api/customer-data-operations/parties.csv?kind=all");
    assert.equal(other.status(),200);
    assert.ok(!(await other.text()).includes(party.id));
    cover("customer-data-operations.tenant-isolation");
  }finally{await contextB.close();}
  assert.equal((await context.request.post(url+
    "/api/customer-data-operations/parties.csv")).status(),404);
  cover("customer-data-operations.no-mutation");
  await shot("03-export","Original Rust Party CSV preview and download");
  // Real browser CSV upload: original parser -> preflight -> atomic Party
  // create via its authoritative D1 owner, then idempotent replay.
  const importedId="browser-import-"+randomUUID();
  const csv="party_id,kind,display_name,resource_version\n"+
    importedId+",person,"+marker+" Imported,1\n";
  await page.getByLabel("CSV-файл").setInputFiles({
    name:"test-parties.csv",mimeType:"text/csv",buffer:Buffer.from(csv)});
  await button("Проверить CSV").click();
  await page.getByRole("status").getByText(/Файл проверен. Строк: 1/).waitFor();
  cover("customer-data-operations.import-preview");
  await button("Импортировать новые записи").click();
  await page.getByRole("status").getByText(/Импорт выполнен. Создано: 1/).waitFor();
  cover("customer-data-operations.import-apply");
  const imported=await context.request.get(url+"/api/parties/"+importedId);
  assert.equal(imported.status(),200);
  assert.equal((await imported.json()).item.name,marker+" Imported");
  const otherImportContext=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    assert.equal((await otherImportContext.request.get(
      url+"/api/parties/"+importedId)).status(),404);
    cover("customer-data-operations.import-tenant-isolation");
  }finally{await otherImportContext.close();}
  await shot("03-import","Original Rust CSV preflight and atomic Party creation");
  await tab("Клиенты").click();
  // Live UI negative rule: new Party with a recorded placeholder name.
  await tab("Лица и организации").click();
  await button("+ Добавить лицо / организацию").click();
  await page.getByLabel("Имя / название").fill("unknown");
  const qualityCreateResponse=page.waitForResponse(r=>
    new URL(r.url()).pathname==="/api/parties"&&
    r.request().method()==="POST"&&r.status()===201);
  await save();cover("data-quality.fixture.party-create");
  const qualityParty=(await (await qualityCreateResponse).json()).item;
  await tab("Качество данных").click();
  await page.getByLabel("Лицо или организация для проверки").selectOption(qualityParty.id);
  await button("Проверить качество").click();
  await page.getByRole("status").getByText(/Есть замечания: 1/).waitFor();
  const qualityBad=await context.request.get(url+
    "/api/data-quality/parties/"+qualityParty.id+"/evaluate");
  assert.equal(qualityBad.status(),200);
  assert.equal((await qualityBad.json()).checks.some(x=>
    x.reason_code==="DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER"&&!x.passed),true);
  cover("data-quality.evaluate.placeholder");
  await shot("03-data-quality-finding","Original Rust placeholder rule, no persisted finding");
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
  // Create a second current email via the actual ContactPoint UI; the only
  // matching rule belongs to the original Rust Identity Resolution domain.
  await button("+ Добавить контакт").click();
  await page.getByLabel("Лицо / организация",{exact:true})
    .selectOption({label:marker+" Related"});
  await page.getByLabel("Значение").fill("updated-"+contact);
  await save();cover("contact-points.create.related");
  await tab("Сравнение записей").click();
  await button("Найти совпадения email").click();
  await page.getByRole("heading",{name:/Возможные совпадения:/}).waitFor();
  const observed=page.getByRole("region",{name:"Наблюдаемые совпадения"});
  await observed.getByRole("row").filter({hasText:marker+" Related"}).waitFor();
  cover("identity-resolution.candidates");
  const candidates=await context.request.get(url+"/api/identity-resolution/candidates");
  assert.equal(candidates.status(),200);
  const candidateBody=await candidates.json();
  assert.ok(candidateBody.items.some(x=>
    [x.first.id,x.second.id].includes(party.id)&&
    [x.first.id,x.second.id].includes(relatedParty.id)&&
    !x.identity_confirmed&&!x.both_contacts_verified));
  assert.equal(candidateBody.match_score,"NOT_COMPUTED");
  const candidateTenantB=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    const other=await candidateTenantB.request.get(url+"/api/identity-resolution/candidates");
    assert.equal(other.status(),200);
    const bItems=(await other.json()).items;
    assert.ok(bItems.every(x=>![x.first.id,x.second.id].includes(party.id)
      &&![x.first.id,x.second.id].includes(relatedParty.id)),
      "Tenant B must not see tenant A Party candidates");
    cover("identity-resolution.candidates.tenant-isolation");
  }finally{await candidateTenantB.close();}
  await shot("04-identity-email","Recorded same-email observations, no identity verdict");
  await tab("Контакты").click();
  await shot("04-contacts","Original ContactPoint normalization and update");
  // Browser -> Rust Consent evaluation (read-only), operator DENY -> real D1.
  await tab("Разрешения").click();
  await page.getByLabel("Лицо / организация").selectOption({label:marker+" Updated"});
  await page.getByLabel("Контакт (необязательно)")
    .selectOption({label:"updated-"+contact});
  await button("Проверить разрешение").click();
  await page.getByRole("status").getByText(/Нет подтверждённого согласия/).waitFor();
  cover("consents.evaluate.fail-closed");
  await button("Запретить коммуникацию").click();
  await button("Отмена").click();cover("consents.suppress.cancel");
  const noGrant=await context.request.post(url+"/api/consents",{
    data:{effect:"grant",evidence_ref:"unverified"}});
  assert.equal(noGrant.status(),404,"No self-attested Grant endpoint is published");
  cover("consents.grant-unavailable");
  await button("Запретить коммуникацию").click();
  await button("Подтвердить запрет").click();
  await page.getByRole("status").getByText("Запрет сохранён").waitFor();
  await page.getByRole("status").getByText(/Есть действующий запрет/).waitFor();
  cover("consents.suppress");
  cover("consents.evaluate.active-deny");
  const deniedRecord=await readOwner("consents",x=>
    x.party_id===party.id&&x.contact_point_id===point.id&&
    x.purpose==="marketing.newsletter"&&x.effect==="deny");
  assert.equal(deniedRecord.source,"crm.access.operator");
  assert.equal(deniedRecord.status,"active");
  const denied=await context.request.post(url+"/api/consents/evaluate",{
    data:{party_id:party.id,contact_point_id:point.id,
      purpose:"marketing.newsletter",channel:"email"}});
  assert.equal(denied.status(),200);
  const decision=await denied.json();
  assert.equal(decision.allowed,false);
  assert.equal(decision.reason,"active_deny");
  const tenantBConsents=await browser.newContext({extraHTTPHeaders:access("B")});
  try{
    const other=await tenantBConsents.request.get(url+"/api/consents");
    assert.equal(other.status(),200);
    assert.equal((await other.json()).items.some(x=>x.id===deniedRecord.id),false);
    cover("consents.tenant-isolation");
  }finally{await tenantBConsents.close();}
  await shot("04-consents","Original Consents fail-closed read and operator Deny");
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
  await tab("Разрешения").click();
  await page.locator(".mobile article").filter({hasText:"updated-"+contact}).waitFor();
  await shot("11-mobile-consents","Mobile original Consent Deny record");
  cover("mobile.consents");
  await Promise.all(pending);
  const operations=["parties.create","parties.update",
    "party-relationships.create","party-relationships.update",
    "consents.suppress",
    "accounts.create","accounts.update",
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
      imported_party_id:importedId,
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

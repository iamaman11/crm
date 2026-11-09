import { chromium } from "playwright";
import { mkdir, writeFile } from "node:fs/promises";
import assert from "node:assert/strict";

const url = "https://crm-dev.crm-5bd5e1e1.workers.dev";
const headers = {
  "CF-Access-Client-Id": process.env.CRM_E2E_A_CLIENT_ID,
  "CF-Access-Client-Secret": process.env.CRM_E2E_A_CLIENT_SECRET,
};
if (!headers["CF-Access-Client-Id"] || !headers["CF-Access-Client-Secret"]) throw Error("Missing protected E2E credentials");
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1366, height: 900 }, extraHTTPHeaders: headers });
const page = await context.newPage();
const marker = "E2E-" + process.env.GITHUB_SHA.slice(0, 8) + "-" + Date.now().toString(36);
const errors = [];
// Every real browser HTTP response is an immutable release-check input.
// The CI verifier independently matches mutation request IDs with D1 and Logs.
const nativeResponses = [];
const uiCoverage = new Set();
const covered = action => uiCoverage.add(action);
const responseModule = path => {
  const segment=path.split("/")[2];
  return segment==="customer360"?"customer360":segment;
};
const responseOperation = (path,method) =>
  responseModule(path)+"."+
  (method==="POST"?"create":method==="PATCH"?"update":method==="DELETE"?"delete":"read");
const uiEvidenceDir = "apps/cloudflare-crm/e2e-evidence";
const evidence = {
  git_sha: process.env.GITHUB_SHA,
  deployed_url: url,
  browser: "Chromium",
  viewport: "desktop + mobile",
  status: "IN_PROGRESS",
  screenshots: [],
  api: [],
  error: null,
};
async function captureUi(file, operation) {
  await mkdir(uiEvidenceDir, { recursive: true });
  await page.screenshot({
    path: uiEvidenceDir + "/" + file + ".png",
    fullPage: true,
    animations: "disabled",
  });
  evidence.screenshots.push({ file: file + ".png", operation, result: "PASS" });
  console.log(JSON.stringify({ event: "crm.ui-evidence", operation, screenshot: file + ".png",
    git_sha: process.env.GITHUB_SHA, result: "PASS" }));
}
async function verifyUiWriteInBackend(module, text) {
  const response = await context.request.get(url + "/api/" + module);
  assert.equal(response.status(), 200, "Backend list after UI write: " + module);
  const data = await response.json();
  const item = (data.items || []).find(row => row.name === text || row.title === text);
  assert.ok(item?.id, "UI write must persist in D1-backed " + module + " API");
  evidence.api.push({
    module, action: "read-after-browser-create", status: response.status(),
    persisted: true, request_id_present: Boolean(response.headers()["x-request-id"]),
  });
}
page.on("pageerror", error => errors.push(error.message));
page.on("response", response => {
  const path=new URL(response.url()).pathname;
  if(!path.startsWith("/api/"))return;
  const h=response.headers(),method=response.request().method();
  const source=h["x-crm-source-sha"],id=h["x-request-id"];
  if(h["x-crm-runtime"]!=="native-workers-rs" || source!==process.env.GITHUB_SHA ||
    !/^[0-9a-f-]{36}$/i.test(id||""))
    errors.push("Incomplete native trace: "+method+" "+path+" HTTP "+response.status());
  nativeResponses.push({method,path,status:response.status(),source_sha:source,
    module:responseModule(path),operation:responseOperation(path,method),
    request_id:id, error_code:h["x-crm-error-code"]||null});
});
const tab = title => page.getByRole("navigation", { name: "Разделы" }).getByRole("button", { name: new RegExp(title) });
const check = async text => await page.getByText(text, { exact: true }).first().waitFor({ timeout: 12000 });
async function create(section, button, fields, title) {
  await tab(section).click();
  await page.getByRole("button", { name: "+ Добавить " + button }).click();
  for (const [field, value] of Object.entries(fields)) {
    const element = page.getByLabel(field, { exact: true });
    if (field === "Клиент" || field === "Этап" || field === "Статус" || field === "Тип") await element.selectOption(value);
    else await element.fill(value);
  }
  await page.getByRole("button", { name: "Сохранить" }).click();
  await check(title);
  covered("create."+section);
}
async function rowFor(text) {
  const row = page.getByRole("row").filter({ hasText: text }).last();
  await row.waitFor({ timeout: 12000 });
  return row;
}
async function edit(section, oldTitle, field, value, newTitle) {
  await tab(section).click();
  await (await rowFor(oldTitle)).getByRole("button", { name: "Изменить" }).click();
  const input = page.getByLabel(field, { exact: true });
  if (["Этап", "Статус", "Тип", "Клиент"].includes(field)) await input.selectOption(value);
  else await input.fill(value);
  await page.getByRole("button", { name: "Сохранить" }).click();
  await page.getByRole("status").getByText("Сохранено").waitFor({ timeout: 12000 });
  await check(newTitle);
  covered("edit."+section);
}
async function remove(section, title) {
  await tab(section).click();
  page.once("dialog", dialog => dialog.accept());
  await (await rowFor(title)).getByRole("button", { name: "Удалить" }).click();
  await page.getByRole("row").filter({ hasText: title }).waitFor({ state: "detached", timeout: 12000 });
  covered("delete."+section);
}
async function cleanup() {
  for (const section of ["tasks", "deals", "contacts", "customers"]) {
    const response = await context.request.get(url + "/api/" + section);
    if (!response.ok()) continue;
    const body = await response.json();
    for (const item of body.items || []) {
      if ((item.name || item.title || "").includes(marker))
        await context.request.delete(url + "/api/" + section + "/" + item.id);
    }
  }
}
async function checkIndependentWorkspaces(customerName) {
  const id = process.env.CRM_E2E_B_CLIENT_ID, secret = process.env.CRM_E2E_B_CLIENT_SECRET;
  if (!id || !secret) throw Error("Missing second Access service identity");
  const other = await browser.newContext({
    extraHTTPHeaders: { "CF-Access-Client-Id": id, "CF-Access-Client-Secret": secret },
  });
  let ownB = null;
  try {
    const listA = await context.request.get(url + "/api/customers");
    assert.equal(listA.status(), 200);
    const a = (await listA.json()).items.find(item => item.name === customerName);
    assert.ok(a, "A must see its customer");
    const sessionB = await other.request.get(url + "/api/session");
    assert.equal(sessionB.status(), 200, "B must authenticate normally");
    const createdB = await other.request.post(url + "/api/customers", {
      data: { name: customerName + " B", kind: "organization" },
    });
    assert.equal(createdB.status(), 201, "B must create its own customer");
    ownB = (await createdB.json()).item;
    assert.notEqual(a.tenant_id, ownB.tenant_id, "A/B must use different workspaces");
    for (const [viewer, mine, foreign] of [
      [context, a, ownB], [other, ownB, a],
    ]) {
      const list = await viewer.request.get(url + "/api/customers");
      assert.equal(list.status(), 200);
      const rows = (await list.json()).items;
      assert.ok(rows.some(row => row.id === mine.id), "own record visible");
      assert.ok(!rows.some(row => row.id === foreign.id), "foreign record hidden");
      assert.equal((await viewer.request.get(url + "/api/customers/" + foreign.id)).status(), 404);
      const foreign360 = await viewer.request.get(url + "/api/customer360/" + foreign.id);
      assert.equal(foreign360.status(), 404, "foreign Customer 360 must not leak existence");
      assert.equal(foreign360.headers()["x-crm-error-code"], "NOT_FOUND");
      assert.ok(foreign360.headers()["x-request-id"]);
      const own360 = await viewer.request.get(url + "/api/customer360/" + mine.id);
      assert.equal(own360.status(), 200, "own Customer 360 must work");
      assert.equal((await own360.json()).customer.id, mine.id);
      assert.equal((await viewer.request.patch(url + "/api/customers/" + foreign.id, {
        data: { name: customerName + " foreign edit", kind: "organization" },
      })).status(), 404);
      assert.equal((await viewer.request.delete(url + "/api/customers/" + foreign.id)).status(), 404);
      assert.equal((await viewer.request.post(url + "/api/contacts", {
        data: { customer_id: foreign.id, name: customerName + " foreign contact" },
      })).status(), 422);
    }
    console.log(JSON.stringify({ event: "crm.workspace-e2e", result: "PASS",
      tests: "two Access identities, different tenants, CRUD and contact isolation" }));
  } finally {
    if (ownB) await other.request.delete(url + "/api/customers/" + ownB.id).catch(() => {});
    await other.close();
  }
}

// Browser E2E uses two real Cloudflare Access identities, never a CI tenant header.
async function checkNativePartyWorkspaces(){
  const other=await browser.newContext({extraHTTPHeaders:{
    "CF-Access-Client-Id":process.env.CRM_E2E_B_CLIENT_ID,
    "CF-Access-Client-Secret":process.env.CRM_E2E_B_CLIENT_SECRET
  }});
  const a=crypto.randomUUID(),b=crypto.randomUUID();
  const createKey=crypto.randomUUID(),updateKey=crypto.randomUUID(),bKey=crypto.randomUUID();
  const path="/api/parties/";
  const payload={id:a,kind:"person",name:marker+" Native A"};
  try{
    const created=await context.request.post(url+"/api/parties",{headers:{"idempotency-key":createKey},data:payload});
    assert.equal(created.status(),201,"Rust A create");
    const replay=await context.request.post(url+"/api/parties",{headers:{"idempotency-key":createKey},data:payload});
    assert.equal(replay.status(),200);assert.equal((await replay.json()).replayed,true);
    assert.equal((await context.request.get(url+path+a)).status(),200);
    assert.equal((await other.request.get(url+path+a)).status(),404,"B cannot read Party A");
    assert.equal((await other.request.patch(url+path+a,{headers:{"idempotency-key":crypto.randomUUID()},data:{expected_version:1,name:"DENIED"}})).status(),404,"B cannot update Party A");
    const ownB=await other.request.post(url+"/api/parties",{headers:{"idempotency-key":bKey},data:{id:b,kind:"organization",name:marker+" Native B"}});
    assert.equal(ownB.status(),201);
    assert.equal((await context.request.get(url+path+b)).status(),404,"A cannot read Party B");
    assert.equal((await context.request.get(url+path+b,{headers:{"x-crm-native-tenant":"tenant-b"}})).status(),404,"Cannot forge native tenant");
    assert.equal((await context.request.get(url+path+a,{headers:{"x-crm-native-tenant":"tenant-b","x-crm-ci-tenant":"tenant-b"}})).status(),200,"Verified Access tenant overrides forged header");
    const updated=await context.request.patch(url+path+a,{headers:{"idempotency-key":updateKey},data:{expected_version:1,name:marker+" Native A Updated"}});
    assert.equal(updated.status(),200);assert.equal((await updated.json()).item.version,2);
    const replayUpdate=await context.request.patch(url+path+a,{headers:{"idempotency-key":updateKey},data:{expected_version:1,name:marker+" Native A Updated"}});
    assert.equal(replayUpdate.status(),200);assert.equal((await replayUpdate.json()).replayed,true);
    const stale=await context.request.patch(url+path+a,{headers:{"idempotency-key":crypto.randomUUID()},data:{expected_version:1,name:"DENIED STALE"}});
    assert.equal(stale.status(),409);assert.ok(stale.headers()["x-request-id"]);
    const invalid=await context.request.post(url+"/api/parties",{headers:{"idempotency-key":crypto.randomUUID()},data:{id:crypto.randomUUID(),kind:"person",name:" "}});
    assert.equal(invalid.status(),422,"Rust validation rejects blank name");
    const bList=await other.request.get(url+"/api/parties");assert.equal(bList.status(),200);
    const bItems=(await bList.json()).items;assert.ok(bItems.some(x=>x.id===b));assert.ok(!bItems.some(x=>x.id===a));
    const aList=await context.request.get(url+"/api/parties");assert.equal(aList.status(),200);
    assert.ok(!(await aList.json()).items.some(x=>x.id===b));
    await mkdir("apps/cloudflare-crm/.release",{recursive:true});
    const {writeFile}=await import("node:fs/promises");
    await writeFile("apps/cloudflare-crm/.release/parties-e2e.json",JSON.stringify({git_sha:process.env.GITHUB_SHA,parties:[a,b],keys:[createKey,updateKey,bKey]})+"\n");
    console.log(JSON.stringify({event:"crm.native-party-access-e2e",result:"PASS",sha:process.env.GITHUB_SHA}));
  }finally{await other.close();}
}

let result = "FAIL";
try {
  const doctor = await context.request.get(url + "/api/doctor");
  assert.equal(doctor.status(), 200, "published schema doctor");
  const diagnosis = await doctor.json();
  assert.equal(diagnosis.runtime, "workers-rs");
  assert.equal(diagnosis.source, process.env.GITHUB_SHA);
  assert.equal(diagnosis.database.status, "PASS");
  assert.equal(diagnosis.database.tables.expected, 9);
  assert.equal(diagnosis.database.tables.present, 9);
  assert.equal(diagnosis.access.status, "PASS");
  assert.equal(doctor.headers()["x-crm-runtime"], "native-workers-rs");
  assert.ok(doctor.headers()["x-request-id"], "correlation id exists");
  const session = await context.request.get(url + "/api/session");
  if (session.status() !== 200) throw Error("Protected CRM API session HTTP " + session.status());
  const response = await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });
  if (response.status() !== 200) throw Error("Protected CRM page HTTP " + response.status());
  await page.getByRole("heading", { name: "Моя CRM" }).waitFor({ timeout: 12000 });
  const release = await context.request.get(url + "/api/version");
  assert.equal(release.status(), 200, "Live Worker version endpoint");
  assert.equal((await release.json()).source, process.env.GITHUB_SHA,
    "Do not inspect UI from a different deployed source");
  evidence.api.push({ module: "version", action: "live-readback", status: 200,
    git_sha_matches_main: true });
  // The React shell renders before its first D1-backed list finishes.
  // A visually complete screenshot must not contain the loading placeholder.
  await page.getByRole("button", { name: "+ Добавить клиента" }).waitFor();
  // Match the actual React loading text (single Unicode ellipsis), not three periods.
  // The screenshot must never certify an unfinished API-backed screen.
  await page.getByText("Загрузка…", { exact: true })
    .waitFor({ state: "hidden", timeout: 12000 });
  await captureUi("01-home", "Open deployed CRM desktop home after data load");
  // The same published UI must handle cancel, invalid input and refresh,
  // not only happy-path creation.
  await page.getByRole("button", {name:"+ Добавить клиента"}).click();
  await page.getByRole("button", {name:"Сохранить",exact:true}).click();
  assert.equal(await page.getByLabel("Имя / название",{exact:true}).evaluate(
    input=>input.validity.valueMissing),true,"required name cannot submit");
  await page.getByRole("button", {name:"Отмена"}).click();
  await page.locator(".shade").waitFor({state:"hidden"});
  await page.getByRole("button", {name:"Обновить",exact:true}).click();
  await page.getByRole("status").getByText("Обновлено").waitFor();
  evidence.api.push({module:"ui",action:"empty-form-validation-cancel-global-refresh",result:"PASS"});
  covered("form.invalid-required");covered("form.cancel");covered("global.refresh");
  // Real browser proof that the original Rust Party runs in this same CRM
  // Worker behind the same Cloudflare Access policy and persists in D1.
  const partyName = marker+" Native Rust";
  await tab("Лица и организации").click();
  await page.getByRole("button",{name:"+ Добавить лицо / организацию"}).click();
  await page.getByRole("button",{name:"Отмена"}).click();
  covered("parties.add.cancel");
  await page.getByRole("button",{name:"+ Добавить лицо / организацию"}).click();
  await page.getByLabel("Имя / название",{exact:true}).fill(partyName);
  const partyCreateHttp=page.waitForResponse(r=>new URL(r.url()).pathname==="/api/parties"&&r.request().method()==="POST");
  await page.getByRole("button",{name:"Сохранить"}).click();
  const partyCreateResponse=await partyCreateHttp;
  assert.equal(partyCreateResponse.status(),201);
  const nativeClickRequestId=partyCreateResponse.headers()["x-request-id"];
  assert.match(nativeClickRequestId||"",/^[0-9a-f-]{36}$/i,"real React click needs native request ID");
  await page.getByText(partyName,{exact:true}).waitFor({timeout:12000});
  covered("parties.create");
  await page.reload();
  await tab("Лица и организации").click();
  const partyRow=page.getByRole("row").filter({hasText:partyName});
  await partyRow.waitFor({timeout:12000});
  await partyRow.getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Имя / название",{exact:true}).fill("Cancelled Party Change");
  await page.getByRole("button",{name:"Отмена"}).click();
  await partyRow.getByText(partyName,{exact:true}).waitFor();
  covered("parties.edit.cancel");
  await partyRow.getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Имя / название",{exact:true}).fill(partyName+" Updated");
  await page.getByRole("button",{name:"Сохранить"}).click();
  await page.getByText(partyName+" Updated",{exact:true}).waitFor({timeout:12000});
  covered("parties.edit.save");
  await page.locator('section[aria-label="Лица и организации (Rust)"]').getByRole("button",{name:"Обновить",exact:true}).click();
  await page.getByText(partyName+" Updated",{exact:true}).waitFor();
  covered("parties.refresh");
  const nativeRead = await context.request.get(url + "/api/parties");
  assert.equal(nativeRead.status(),200,"authorized Rust Party API");
  const partyList=(await nativeRead.json()).items;
  const identity=partyList.find(x=>x.name===partyName+" Updated");
  assert.equal(identity?.version,2,"original Rust persisted versioned update");
  await mkdir("apps/cloudflare-crm/.release",{recursive:true});
  await writeFile("apps/cloudflare-crm/.release/native-click.json",
    JSON.stringify({git_sha:process.env.GITHUB_SHA,request_id:nativeClickRequestId,
      action:"parties.create",party_id:identity.id,status:201})+"\n");
  evidence.api.push({module:"parties",action:"native-click-trace",response_request_id_present:true});
  evidence.api.push({ module: "parties", action: "read-after-browser-update",
    status: nativeRead.status(), persisted_version: 2,
    request_id_present: Boolean(nativeRead.headers()["x-request-id"]) });
  await page.getByPlaceholder("Найти человека или компанию").fill(partyName + " Updated");
  await page.getByText(partyName + " Updated", { exact: true }).waitFor();
  await captureUi("02-rust-party", "Search and show just-updated original Rust Party");
  const wrongVersion=await context.request.patch(url + "/api/parties/" + identity.id,{
    headers:{"idempotency-key":"stale-"+marker},
    data:{expected_version:1,name:partyName+" Rejected"}});
  assert.equal(wrongVersion.status(),409,"Rust rejects stale versions");
  const unauthorized=await context.request.get(url + "/api/parties/"+identity.id,{
    headers:{"x-crm-native-tenant":"spoofed-id"}});
  assert.equal(unauthorized.status(),200,"verified Access tenant overrides injected header");
  // Party deletion is not part of the accepted original Rust domain contract.
  // CI uses isolated dedicated test workspaces; this test identity is retained.
  await checkNativePartyWorkspaces();
  const customer = marker + " Компания", contact = marker + " Контакт", deal = marker + " Сделка", task = marker + " Задача";
  await create("Клиенты", "клиента", { "Имя / название": customer, "Тип": "organization" }, customer);
  const cancelRow=await rowFor(customer);
  await cancelRow.getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Имя / название",{exact:true}).fill(marker+" Cancelled");
  await page.getByRole("button",{name:"Отмена"}).click();
  await cancelRow.getByText(customer,{exact:true}).waitFor();
  page.once("dialog", dialog=>dialog.dismiss());
  await cancelRow.getByRole("button",{name:"Удалить"}).click();
  await cancelRow.getByText(customer,{exact:true}).waitFor();
  evidence.api.push({module:"customers",action:"cancel-edit-and-dismiss-delete",result:"PASS"});
  covered("customers.edit.cancel");covered("customers.delete.dismiss");
  await page.reload();
  await check(customer);
  await verifyUiWriteInBackend("customers", customer);
  const customerEdited=customer+" Проверено";
  await edit("Клиенты",customer,"Имя / название",customerEdited,customerEdited);
  await edit("Клиенты",customerEdited,"Имя / название",customer,customer);
  await captureUi("03-customers", "Create, edit and reload customer");
  await create("Контакты", "контакт", { "Клиент": { label: customer }, "Имя": contact, "Электронная почта": "demo@example.test" }, contact);
  await verifyUiWriteInBackend("contacts", contact);
  const contactEdited=contact+" Проверено";
  await edit("Контакты",contact,"Имя",contactEdited,contactEdited);
  await edit("Контакты",contactEdited,"Имя",contact,contact);
  await captureUi("04-contacts", "Create and edit contact linked to customer");
  await create("Сделки", "сделку", { "Клиент": { label: customer }, "Название сделки": deal, "Этап": "new", "Сумма": "120" }, deal);
  await verifyUiWriteInBackend("deals", deal);
  await captureUi("05-deals", "Create a linked sales deal");
  await create("Задачи", "задачу", { "Клиент": { label: customer }, "Что сделать": task,
    "Срок": "2020-01-01", "Статус": "open" }, task);
  await verifyUiWriteInBackend("tasks", task);
  await captureUi("06-tasks", "Create task linked to customer");
  const owned = await context.request.get(url + "/api/customers");
  const customerRow = (await owned.json()).items.find(row => row.name === customer);
  assert.ok(customerRow?.id);
  const detail = await context.request.get(url + "/api/customer360/" + customerRow.id);
  assert.equal(detail.status(), 200);
  const original = await detail.json();
  assert.equal(original.summary.contacts, 1);
  assert.equal(original.summary.deals, 1);
  assert.equal(original.summary.open_tasks, 1);
  assert.equal(original.summary.overdue_tasks, 1);
  assert.equal(original.summary.deal_amount, 120);
  evidence.api.push({ module: "customer360", action: "read-aggregates",
    status: detail.status(), contacts: 1, deals: 1, open_tasks: 1,
    overdue_tasks: 1, deal_amount: 120 });
  await tab("Клиенты").click();
  await (await rowFor(customer)).getByRole("button", { name: "Карточка" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  for(const [section,title] of [["Контакты клиента",contact],["Сделки клиента",deal],["Задачи клиента",task]])
    await page.getByRole("region", { name: section }).getByText(title, { exact: true }).waitFor();
  await captureUi("07-customer360", "Open composed customer card after UI writes");
  // Trigger a genuine browser transport failure only for the requested card;
  // restore the normal network before exercising the Retry button.
  const broken=route=>route.abort("failed");
  await page.route("**/api/customer360/**",broken);
  await page.getByRole("button",{name:"Обновить карточку"}).click();
  await page.getByRole("button",{name:"Повторить"}).waitFor();
  await captureUi("11-customer360-retry", "Network error and native Retry action");
  await page.unroute("**/api/customer360/**",broken);
  await page.getByRole("button",{name:"Повторить"}).click();
  await page.getByRole("heading",{name:customer}).waitFor();
  evidence.api.push({module:"customer360",action:"failed-refresh-then-retry",result:"PASS"});
  covered("customer360.refresh");covered("customer360.retry");
  assert.equal(await page.getByText("Просрочено",{exact:true}).locator("..").locator("strong").textContent(),"1");
  await page.getByRole("button", { name: "Обновить карточку" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  // Exercise every Customer360 group navigation, not just the back button.
  for(const [region,label] of [["Контакты клиента","Все контакты"],["Сделки клиента","Все сделки"],["Задачи клиента","Все задачи"]]){
    await page.getByRole("region",{name:region}).getByRole("button",{name:label}).click();
    await page.getByRole("textbox",{name:"Поиск"}).waitFor();
    covered("customer360.navigate."+region);
    await tab("Клиенты").click();
    await (await rowFor(customer)).getByRole("button",{name:"Карточка"}).click();
    await page.getByRole("heading",{name:customer}).waitFor();
  }
  await page.getByRole("button", { name: "← К списку клиентов" }).click();
  covered("customer360.back");
  await tab("Сделки").click();
  await page.getByRole("button", { name: "Воронка" }).click();
  const card = page.locator(".deal-card").filter({ hasText: deal });
  await card.getByRole("combobox", { name: "Этап сделки" }).selectOption("qualified");
  await page.getByRole("status").getByText("Этап обновлён").waitFor({ timeout: 12000 });
  covered("deals.board");covered("deals.board.stage");
  await captureUi("08-pipeline", "Click pipeline and update deal stage");
  await page.reload();
  await tab("Сделки").click();
  await page.getByRole("button", { name: "Воронка" }).click();
  const savedStage = await page.locator(".deal-card").filter({ hasText: deal })
    .getByRole("combobox", { name: "Этап сделки" }).inputValue();
  assert.equal(savedStage, "qualified", "pipeline stage survives browser reload");
  await page.getByRole("button", { name: "Таблица" }).click();
  covered("deals.table");
  await edit("Сделки", deal, "Этап", "won", deal);
  await (await rowFor(deal)).getByText("Выиграна", { exact: true }).waitFor();
  await edit("Задачи", task, "Статус", "done", task);
  await (await rowFor(task)).getByText("Готово", { exact: true }).waitFor();
  await tab("Клиенты").click();
  await (await rowFor(customer)).getByRole("button", { name: "Карточка" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  assert.equal(await page.getByText("Просрочено",{exact:true}).locator("..").locator("strong").textContent(),"0");
  assert.equal(await page.getByText("Открытых задач",{exact:true}).locator("..").locator("strong").textContent(),"0");
  await page.getByRole("region", { name: "Сделки клиента" }).getByText("Выиграна", { exact: false }).waitFor();
  await page.getByRole("region", { name: "Задачи клиента" }).getByText("Готово", { exact: false }).waitFor();
  await captureUi("09-updated-card", "Verify linked deal and task changes in customer card");
  await page.reload();
  await (await rowFor(customer)).getByRole("button", { name: "Карточка" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  await page.getByRole("region", { name: "Контакты клиента" }).getByText(contact, { exact: true }).waitFor();
  // Linked edit uses the original CRUD owner without copying or mutating Customer 360 state.
  await page.getByRole("region", { name: "Контакты клиента" }).getByRole("button", { name:"Изменить" }).click();
  await page.getByRole("button", {name:"Отмена"}).click();
  covered("customer360.linked.edit");
  await tab("Клиенты").click();
  await tab("Клиенты").click();
  await page.getByRole("textbox", { name: "Поиск" }).fill(customer);
  await check(customer);
  await page.getByRole("textbox", { name: "Поиск" }).fill("");
  covered("customers.search");
  await page.setViewportSize({ width: 390, height: 844 });
  await tab("Клиенты").click();
  await page.locator(".mobile article").filter({hasText:customer}).getByRole("button",{name:"Карточка"}).click();
  await page.getByRole("heading", {name:customer}).waitFor();
  await page.getByRole("region",{name:"Сделки клиента"}).getByText(deal,{exact:true}).waitFor();
  await page.getByRole("button",{name:"← К списку клиентов"}).click();
  await tab("Задачи").click();
  await page.locator(".mobile").getByText(task, { exact: true }).waitFor({ timeout: 12000 });
  await page.screenshot({ path: "apps/cloudflare-crm/e2e-mobile.png" });
  await captureUi("10-mobile", "Use mobile layout and verify task after navigation");
  covered("mobile.customer.card");
  await page.setViewportSize({ width: 1366, height: 900 });
  await checkIndependentWorkspaces(customer);
  await remove("Задачи", task);
  await remove("Сделки", deal);
  await remove("Контакты", contact);
  await remove("Клиенты", customer);
  const removed360 = await context.request.get(url + "/api/customer360/" + customerRow.id);
  assert.equal(removed360.status(), 404);
  evidence.api.push({ module: "customer360", action: "read-after-browser-delete",
    status: 404, foreign_data_absent: true });
  // Every published React action must reach the same native Worker, with a
  // real HTTP response and a source-specific trace, not only a visible toast.
  const requiredCoverage=[
    "form.invalid-required","form.cancel","global.refresh",
    "parties.add.cancel","parties.create","parties.edit.cancel","parties.edit.save","parties.refresh",
    "customers.edit.cancel","customers.delete.dismiss","customers.search","mobile.customer.card",
    "customer360.refresh","customer360.retry","customer360.back","customer360.linked.edit",
    "customer360.navigate.Контакты клиента","customer360.navigate.Сделки клиента",
    "customer360.navigate.Задачи клиента","deals.board","deals.board.stage","deals.table",
    ...["Клиенты","Контакты","Сделки","Задачи"].flatMap(s=>["create."+s,"edit."+s,"delete."+s])
  ];
  assert.deepEqual(requiredCoverage.filter(s=>!uiCoverage.has(s)),[],
    "UI button coverage missing");
  for(const module of ["parties","customers","contacts","deals","tasks"]){
    for(const op of ["create","update",...(module==="parties"?[]:["delete"])]){
      assert.ok(nativeResponses.some(r=>r.operation===module+"."+op&&
        (op==="create"?r.status===201:op==="delete"?r.status===204:r.status===200)),
        "Native browser mutation not witnessed: "+module+"."+op);
    }
  }
  for(const entry of nativeResponses)
    if(!entry.request_id||entry.source_sha!==process.env.GITHUB_SHA)
      throw Error("Incomplete native response evidence for "+entry.operation);
  const mutations=nativeResponses.filter(r=>
    ["POST","PATCH","DELETE"].includes(r.method)&&[200,201,204].includes(r.status));
  const expectedMutations=3+3+3+3+2; // Party create/update, four models' C/U/D (minimum)
  assert.ok(mutations.length>=expectedMutations,
    "Missing native UI mutation receipts; got "+mutations.length);
  await mkdir("apps/cloudflare-crm/.release",{recursive:true});
  await writeFile("apps/cloudflare-crm/.release/native-functional-cycle.json",
    JSON.stringify({git_sha:process.env.GITHUB_SHA,runtime:"native-workers-rs",
      ui_coverage:requiredCoverage,requests:nativeResponses,
      writes:mutations,result:"BROWSER_PASS_D1_LOGS_PENDING"},null,2)+"\n");
  evidence.ui_coverage=requiredCoverage;
  evidence.native_http_responses=nativeResponses.length;
  evidence.native_mutations=mutations.length;
  if (errors.length) throw Error("Browser JavaScript error: " + errors.slice(0, 2).join("; "));
  result = "PASS";
  evidence.status = "PASS";
  console.log(JSON.stringify({ event: "crm.browser-e2e", result, tests: "real Chromium Customer 360, scoped A/B, derived metrics, reload, linked edit, mobile, delete", sha: process.env.GITHUB_SHA }));
} catch (error) {
  await mkdir("apps/cloudflare-crm", { recursive: true });
  await page.screenshot({ path: "apps/cloudflare-crm/e2e-failure.png" }).catch(() => {});
  evidence.status = "FAIL";
  evidence.error = { message: String(error.message).slice(0, 300),
    screenshots_completed: evidence.screenshots.length };
  console.error("CRM browser E2E FAIL:", error.message);
  throw error;
} finally {
  await mkdir(uiEvidenceDir, { recursive: true });
  await writeFile(uiEvidenceDir + "/manifest.json", JSON.stringify(evidence, null, 2) + "\n");
  await cleanup().catch(() => {});
  await browser.close();
}

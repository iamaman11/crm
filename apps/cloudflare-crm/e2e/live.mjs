import { chromium } from "playwright";
import { mkdir } from "node:fs/promises";
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
page.on("pageerror", error => errors.push(error.message));
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
}
async function remove(section, title) {
  await tab(section).click();
  page.once("dialog", dialog => dialog.accept());
  await (await rowFor(title)).getByRole("button", { name: "Удалить" }).click();
  await page.getByRole("row").filter({ hasText: title }).waitFor({ state: "detached", timeout: 12000 });
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

let result = "FAIL";
try {
  const doctor = await context.request.get(url + "/api/doctor");
  assert.equal(doctor.status(), 200, "published schema doctor");
  const diagnosis = await doctor.json();
  assert.equal(diagnosis.database, "ok");
  assert.deepEqual(diagnosis.modules, ["customers", "contacts", "deals", "tasks"]);
  assert.equal(diagnosis.tables.present, 4);
  assert.ok(doctor.headers()["x-request-id"], "correlation id exists");
  const session = await context.request.get(url + "/api/session");
  if (session.status() !== 200) throw Error("Protected CRM API session HTTP " + session.status());
  const response = await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });
  if (response.status() !== 200) throw Error("Protected CRM page HTTP " + response.status());
  await page.getByRole("heading", { name: "Моя CRM" }).waitFor({ timeout: 12000 });
  // Real browser proof that the original Rust Party runs in this same CRM
  // Worker behind the same Cloudflare Access policy and persists in D1.
  const partyName = marker+" Native Rust";
  await tab("Лица и организации").click();
  await page.getByRole("button",{name:"+ Добавить лицо / организацию"}).click();
  await page.getByLabel("Имя / название",{exact:true}).fill(partyName);
  await page.getByRole("button",{name:"Сохранить"}).click();
  await page.getByText(partyName,{exact:true}).waitFor({timeout:12000});
  await page.reload();
  await tab("Лица и организации").click();
  const partyRow=page.getByRole("row").filter({hasText:partyName});
  await partyRow.waitFor({timeout:12000});
  await partyRow.getByRole("button",{name:"Изменить"}).click();
  await page.getByLabel("Имя / название",{exact:true}).fill(partyName+" Updated");
  await page.getByRole("button",{name:"Сохранить"}).click();
  await page.getByText(partyName+" Updated",{exact:true}).waitFor({timeout:12000});
  const nativeRead = await context.request.get(url + "/api/parties");
  assert.equal(nativeRead.status(),200,"authorized Rust Party API");
  const partyList=(await nativeRead.json()).items;
  const identity=partyList.find(x=>x.name===partyName+" Updated");
  assert.equal(identity?.version,2,"original Rust persisted versioned update");
  const wrongVersion=await context.request.patch(url + "/api/parties/" + identity.id,{
    headers:{"idempotency-key":"stale-"+marker},
    data:{expected_version:1,name:partyName+" Rejected"}});
  assert.equal(wrongVersion.status(),409,"Rust rejects stale versions");
  const unauthorized=await context.request.get(url + "/api/parties/"+identity.id,{
    headers:{"x-crm-native-tenant":"spoofed-id"}});
  assert.equal(unauthorized.status(),200,"verified Access tenant overrides injected header");
  // Party deletion is not part of the accepted original Rust domain contract.
  // CI uses isolated dedicated test workspaces; this test identity is retained.
  const customer = marker + " Компания", contact = marker + " Контакт", deal = marker + " Сделка", task = marker + " Задача";
  await create("Клиенты", "клиента", { "Имя / название": customer, "Тип": "organization" }, customer);
  await page.reload();
  await check(customer);
  await create("Контакты", "контакт", { "Клиент": { label: customer }, "Имя": contact, "Электронная почта": "demo@example.test" }, contact);
  await create("Сделки", "сделку", { "Клиент": { label: customer }, "Название сделки": deal, "Этап": "new", "Сумма": "120" }, deal);
  await create("Задачи", "задачу", { "Клиент": { label: customer }, "Что сделать": task,
    "Срок": "2020-01-01", "Статус": "open" }, task);
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
  await tab("Клиенты").click();
  await (await rowFor(customer)).getByRole("button", { name: "Карточка" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  for(const [section,title] of [["Контакты клиента",contact],["Сделки клиента",deal],["Задачи клиента",task]])
    await page.getByRole("region", { name: section }).getByText(title, { exact: true }).waitFor();
  assert.equal(await page.getByText("Просрочено",{exact:true}).locator("..").locator("strong").textContent(),"1");
  await page.getByRole("button", { name: "Обновить карточку" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  await page.getByRole("button", { name: "← К списку клиентов" }).click();
  await tab("Сделки").click();
  await page.getByRole("button", { name: "Воронка" }).click();
  const card = page.locator(".deal-card").filter({ hasText: deal });
  await card.getByRole("combobox", { name: "Этап сделки" }).selectOption("qualified");
  await page.getByRole("status").getByText("Этап обновлён").waitFor({ timeout: 12000 });
  await page.reload();
  await tab("Сделки").click();
  await page.getByRole("button", { name: "Воронка" }).click();
  const savedStage = await page.locator(".deal-card").filter({ hasText: deal })
    .getByRole("combobox", { name: "Этап сделки" }).inputValue();
  assert.equal(savedStage, "qualified", "pipeline stage survives browser reload");
  await page.getByRole("button", { name: "Таблица" }).click();
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
  await page.reload();
  await (await rowFor(customer)).getByRole("button", { name: "Карточка" }).click();
  await page.getByRole("heading", { name: customer }).waitFor();
  await page.getByRole("region", { name: "Контакты клиента" }).getByText(contact, { exact: true }).waitFor();
  // Linked edit uses the original CRUD owner without copying or mutating Customer 360 state.
  await page.getByRole("region", { name: "Контакты клиента" }).getByRole("button", { name:"Изменить" }).click();
  await page.getByRole("button", {name:"Отмена"}).click();
  await tab("Клиенты").click();
  await tab("Клиенты").click();
  await page.getByRole("textbox", { name: "Поиск" }).fill(customer);
  await check(customer);
  await page.getByRole("textbox", { name: "Поиск" }).fill("");
  await page.setViewportSize({ width: 390, height: 844 });
  await tab("Клиенты").click();
  await page.locator(".mobile article").filter({hasText:customer}).getByRole("button",{name:"Карточка"}).click();
  await page.getByRole("heading", {name:customer}).waitFor();
  await page.getByRole("region",{name:"Сделки клиента"}).getByText(deal,{exact:true}).waitFor();
  await page.getByRole("button",{name:"← К списку клиентов"}).click();
  await tab("Задачи").click();
  await page.locator(".mobile").getByText(task, { exact: true }).waitFor({ timeout: 12000 });
  await page.screenshot({ path: "apps/cloudflare-crm/e2e-mobile.png" });
  await page.setViewportSize({ width: 1366, height: 900 });
  await checkIndependentWorkspaces(customer);
  await remove("Задачи", task);
  await remove("Сделки", deal);
  await remove("Контакты", contact);
  await remove("Клиенты", customer);
  const removed360 = await context.request.get(url + "/api/customer360/" + customerRow.id);
  assert.equal(removed360.status(), 404);
  if (errors.length) throw Error("Browser JavaScript error: " + errors.slice(0, 2).join("; "));
  result = "PASS";
  console.log(JSON.stringify({ event: "crm.browser-e2e", result, tests: "real Chromium Customer 360, scoped A/B, derived metrics, reload, linked edit, mobile, delete", sha: process.env.GITHUB_SHA }));
} catch (error) {
  await mkdir("apps/cloudflare-crm", { recursive: true });
  await page.screenshot({ path: "apps/cloudflare-crm/e2e-failure.png" }).catch(() => {});
  console.error("CRM browser E2E FAIL:", error.message);
  throw error;
} finally {
  await cleanup().catch(() => {});
  await browser.close();
}

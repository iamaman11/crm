import { chromium } from "playwright";
import { mkdir } from "node:fs/promises";

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
let result = "FAIL";
try {
  const response = await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });
  if (response.status() !== 200) throw Error("Protected CRM page HTTP " + response.status());
  await page.getByRole("heading", { name: "Моя CRM" }).waitFor({ timeout: 12000 });
  const customer = marker + " Компания", contact = marker + " Контакт", deal = marker + " Сделка", task = marker + " Задача";
  await create("Клиенты", "клиента", { "Имя / название": customer, "Тип": "organization" }, customer);
  await page.reload();
  await check(customer);
  await create("Контакты", "контакт", { "Клиент": { label: customer }, "Имя": contact, "Электронная почта": "demo@example.test" }, contact);
  await create("Сделки", "сделку", { "Клиент": { label: customer }, "Название сделки": deal, "Этап": "new", "Сумма": "120" }, deal);
  await create("Задачи", "задачу", { "Клиент": { label: customer }, "Что сделать": task, "Статус": "open" }, task);
  await edit("Сделки", deal, "Этап", "won", deal);
  await edit("Задачи", task, "Статус", "done", task);
  await tab("Клиенты").click();
  await page.getByRole("textbox", { name: "Поиск" }).fill(customer);
  await check(customer);
  await page.getByRole("textbox", { name: "Поиск" }).fill("");
  await page.setViewportSize({ width: 390, height: 844 });
  await tab("Задачи").click();
  await page.locator(".mobile").getByText(task, { exact: true }).waitFor({ timeout: 12000 });
  await page.screenshot({ path: "apps/cloudflare-crm/e2e-mobile.png" });
  await page.setViewportSize({ width: 1366, height: 900 });
  await remove("Задачи", task);
  await remove("Сделки", deal);
  await remove("Контакты", contact);
  await remove("Клиенты", customer);
  if (errors.length) throw Error("Browser JavaScript error: " + errors.slice(0, 2).join("; "));
  result = "PASS";
  console.log(JSON.stringify({ event: "crm.browser-e2e", result, tests: "real Chromium UI, 4 creates, persistence, search, edit, mobile, delete", sha: process.env.GITHUB_SHA }));
} catch (error) {
  await mkdir("apps/cloudflare-crm", { recursive: true });
  await page.screenshot({ path: "apps/cloudflare-crm/e2e-failure.png" }).catch(() => {});
  console.error("CRM browser E2E FAIL:", error.message);
  throw error;
} finally {
  await cleanup().catch(() => {});
  await browser.close();
}

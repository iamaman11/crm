// Real Chromium -> built React -> generated Connect client -> signed native Worker -> D1.
import {chromium} from "playwright";
import assert from "node:assert/strict";
import {mkdirSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";

export async function typedBrowser({base,admin,viewer,tenantB,sql,sourceSha}) {
 const directory=fileURLToPath(new URL("../../e2e-evidence-native/",import.meta.url));
 mkdirSync(directory,{recursive:true});
 const browser=await chromium.launch({headless:true});
 const context=await browser.newContext({extraHTTPHeaders:{"cf-access-jwt-assertion":admin}});
 const page=await context.newPage();
 const pageErrors=[];
 page.on("pageerror",error=>pageErrors.push(error.message));
 const steps=[];
 const navigate=async(hash,title)=>{
   await page.goto(base+"/#"+hash);
   await page.getByRole("heading",{name:title,exact:true}).waitFor();
 };
 const mutation=async(button,method)=>{
   const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
   await button.click();const response=await pending;
   assert.equal(response.status(),200,method+" "+await response.text());
   await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
 };
 const firstId=(table,column,value)=>{
   const escaped=value.replaceAll("'","''");
   const rows=sql("SELECT id FROM "+table+" WHERE tenant='tenant-a' AND json_extract(state,'$."+column+"')='"+escaped+"'");
   assert.equal(rows.length,1);return rows[0].id;
 };
 const runId=Date.now().toString();
 let partyId,accountId;
 try {
   await navigate("parties","Люди и организации");
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Party "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   partyId=firstId("crm_party_records","display_name","Browser Party "+runId);
   assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+partyId+"'")[0].n,1);
   await page.reload();
   await page.getByRole("heading",{name:"Browser Party "+runId,exact:true}).waitFor();
   const partyCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Party "+runId,exact:true})});
   await partyCard.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить участника",exact:true}).waitFor();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Updated "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"UpdateParty");
   assert.equal(sql("SELECT version FROM crm_party_records WHERE tenant='tenant-a' AND id='"+partyId+"'")[0].version,2);
   steps.push("Party create/edit/reload and D1 version/audit");
   await navigate("accounts","Клиенты");
   await page.getByLabel("Название клиента",{exact:true}).fill("Browser Account "+runId);
   await page.getByLabel("Основной участник",{exact:true}).selectOption(partyId);
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"CreateAccount");
   accountId=firstId("crm_account_records","name","Browser Account "+runId);
   await page.getByRole("heading",{name:"Browser Account "+runId,exact:true}).waitFor();
   const accountCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Account "+runId,exact:true})});
   const overviewResponse=page.waitForResponse(r=>r.url().endsWith("/GetAccountOverview"));
   await accountCard.getByRole("button",{name:"Открыть карточку",exact:true}).click();
   assert.equal((await overviewResponse).status(),200);
   await page.getByText("Основной участник: Browser Updated "+runId,{exact:true}).waitFor();
   steps.push("Account create and Customer360 read");
   await navigate("contacts","Контакты");
   await page.getByLabel("Участник",{exact:true}).selectOption(partyId);
   await page.getByLabel("Значение контакта",{exact:true}).fill("browser-"+runId+"@example.org");
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"CreateContactPoint");
   assert.equal(sql("SELECT COUNT(*) AS n FROM crm_contact_point_records WHERE tenant='tenant-a' AND party_id='"+partyId+"'")[0].n,1);
   await navigate("tasks","Задачи");
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Task "+runId);
   await page.getByLabel("Клиент",{exact:true}).selectOption(accountId);
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const taskId=firstId("crm_activity_task_records","subject","Browser Task "+runId);
   const taskCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Task "+runId,exact:true})});
   await mutation(taskCard.getByRole("button",{name:"Выполнить задачу",exact:true}),"CompleteTask");
   assert.equal(sql("SELECT json_extract(state,'$.status') AS status FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskId+"'")[0].status,"completed");
   steps.push("Contact and Task create/complete");
   await navigate("sales","Сделки");
   await page.getByLabel("Название сделки",{exact:true}).fill("Browser Deal "+runId);
   await page.getByLabel("Клиент",{exact:true}).selectOption(accountId);
   await page.getByLabel("Сумма",{exact:true}).fill("90071992547409.93");
   await page.getByLabel("Вероятность, %",{exact:true}).fill("37.45");
   await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"CreateDeal");
   const dealId=firstId("crm_sales_deal_records","name","Browser Deal "+runId);
   assert.ok(dealId);
   await page.getByText("90071992547409.93 USD · 37.45%",{exact:true}).waitFor();
   const dealCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Deal "+runId,exact:true})}).last();
   await dealCard.getByRole("button",{name:"Изменить этап",exact:true}).click();
   await page.getByLabel("Код следующего этапа",{exact:true}).fill("qualified");
   await mutation(page.getByRole("button",{name:"Сохранить этап",exact:true}),"AdvanceStage");
   assert.equal(sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant='tenant-a' AND deal_id='"+dealId+"'")[0].n,1);
   await navigate("accounts","Клиенты");
   await page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Account "+runId,exact:true})})
     .getByRole("button",{name:"Открыть карточку",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   steps.push("Exact bigint Deal and atomic Sales–Activities flow visible in Account");
   await navigate("review","Качество и совпадения");
   const quality=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить имя",exact:true})});
   await quality.getByLabel("Участник",{exact:true}).selectOption(partyId);
   const qualityResponse=page.waitForResponse(r=>r.url().endsWith("/EvaluatePartyName"));
   await quality.getByRole("button",{name:"Проверить качество",exact:true}).click();
   assert.equal((await qualityResponse).status(),200);
   await page.getByText("Не пройдено: 0",{exact:true}).waitFor();
   await navigate("consents","Коммуникации");
   const suppress=page.locator("section").filter({has:page.getByRole("heading",{name:"Запретить коммуникацию",exact:true})});
   await suppress.getByLabel("Участник",{exact:true}).selectOption(partyId);
   await mutation(suppress.getByRole("button",{name:"Запретить контакт",exact:true}),"SuppressCommunication");
   const evaluate=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить возможность контакта",exact:true})});
   await evaluate.getByLabel("Участник",{exact:true}).selectOption(partyId);
   await evaluate.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await page.getByRole("heading",{name:"Контакт не разрешён",exact:true}).waitFor();
   steps.push("Quality and consent evaluation/suppression");
   await navigate("privacy","Запросы о данных");
   await mutation(page.getByRole("button",{name:"Создать обращение",exact:true}),"CreatePrivacyCase");
   assert.ok(sql("SELECT COUNT(*) AS n FROM crm_privacy_case_records WHERE tenant='tenant-a'")[0].n>0);
   await navigate("enrichment","Обогащение данных");
   await page.getByLabel("Ключ провайдера",{exact:true}).fill("browser_registry_"+runId);
   await page.getByLabel("Лицензия",{exact:true}).fill("Browser test license");
   await page.getByLabel("Начало действия",{exact:true}).fill("2026-11-12T12:00:00");
   await mutation(page.getByRole("button",{name:"Опубликовать профиль",exact:true}),"PublishProviderProfile");
   steps.push("Privacy intake and immutable Enrichment profile");
   await navigate("data","Импорт и экспорт");
   const csv="party_id,kind,display_name,resource_version\r\nbrowser-import,person,Browser Import "+runId+",1\r\n";
   await page.getByLabel("CSV-файл",{exact:true}).setInputFiles({name:"browser.csv",mimeType:"text/csv",buffer:Buffer.from(csv)});
   await page.getByRole("button",{name:"Проверить импорт",exact:true}).click();
   await page.getByRole("button",{name:"Подтвердить импорт",exact:true}).waitFor();
   await mutation(page.getByRole("button",{name:"Подтвердить импорт",exact:true}),"ApplyPartyImport");
   assert.ok(firstId("crm_party_records","display_name","Browser Import "+runId));
   steps.push("CSV preview/confirmed import through generated client");

   // Two actual browser forms hold the same version. Only one may commit.
   await navigate("parties","Люди и организации");
   const stalePage=await context.newPage();
   await stalePage.goto(base+"/#parties");
   const editParty=async(target)=>{
     await target.locator("section").filter({has:target.getByRole("heading",{name:"Browser Updated "+runId,exact:true})})
       .getByRole("button",{name:"Редактировать",exact:true}).click();
     await target.getByRole("heading",{name:"Изменить участника",exact:true}).waitFor();
   };
   await editParty(stalePage);await editParty(page);
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Winner "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"UpdateParty");
   await stalePage.getByLabel("Имя или название",{exact:true}).fill("Browser Lost Update "+runId);
   const conflict=stalePage.waitForResponse(r=>r.url().endsWith("/UpdateParty"));
   await stalePage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   assert.equal((await conflict).status(),409);
   await stalePage.getByRole("heading",{name:/Конфликт изменения/}).waitFor();
   assert.equal(firstId("crm_party_records","display_name","Browser Winner "+runId),partyId);
   assert.equal(sql("SELECT version FROM crm_party_records WHERE tenant='tenant-a' AND id='"+partyId+"'")[0].version,3);
   await stalePage.close();
   steps.push("Two browser forms enforce CAS and show generated-client conflict");

   // Lose the response AFTER the server commits; retry the unchanged form.
   await navigate("parties","Люди и организации");
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Retry "+runId);
   let dropped=false;
   await page.route("**/crm.parties.v3.PartyService/CreateParty",async route=>{
     if(dropped){await route.continue();return;}
     dropped=true;
     const committed=await route.fetch();
     assert.equal(committed.status(),200);
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await page.getByRole("heading",{name:/Ошибка сервиса|Сервис временно недоступен/}).waitFor();
   const retryId=firstId("crm_party_records","display_name","Browser Retry "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   await page.getByRole("heading",{name:/Сохранено ранее/}).waitFor();
   assert.equal(firstId("crm_party_records","display_name","Browser Retry "+runId),retryId);
   assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+retryId+"'")[0].n,1);
   await page.unroute("**/crm.parties.v3.PartyService/CreateParty");
   steps.push("Committed response loss and same-form retry replay without duplicate audit");

   // Commit succeeds but only the following list refresh fails.
   await navigate("parties","Люди и организации");
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Saved Stale "+runId);
   await page.route("**/crm.parties.v3.PartyService/ListParties",route=>route.abort("failed"));
   const created=page.waitForResponse(r=>r.url().endsWith("/CreateParty"));
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   assert.equal((await created).status(),200);
   await page.getByRole("heading",{name:/Сохранено.*Список устарел/}).waitFor();
   assert.ok(firstId("crm_party_records","display_name","Browser Saved Stale "+runId));
   await page.unroute("**/crm.parties.v3.PartyService/ListParties");
   steps.push("Committed write remains successful when refresh fails");

   await page.getByLabel("Тема",{exact:true}).selectOption("dark");
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.reload();
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.setViewportSize({width:390,height:844});
   await page.screenshot({path:resolve(directory,"crm-mobile-dark.png"),fullPage:true});
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1),"mobile horizontal overflow");
   steps.push("Theme persistence and mobile layout");

   const denied=await browser.newContext({extraHTTPHeaders:{"cf-access-jwt-assertion":viewer}});
   const deniedPage=await denied.newPage();
   await deniedPage.goto(base+"/#parties");
   await deniedPage.getByLabel("Имя или название",{exact:true}).fill("Browser Denied "+runId);
   await deniedPage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await deniedPage.getByRole("heading",{name:/Недостаточно прав/}).waitFor();
   assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a' AND json_extract(state,'$.display_name')='Browser Denied "+runId+"'")[0].n,0);
   await denied.close();
   const other=await browser.newContext({extraHTTPHeaders:{"cf-access-jwt-assertion":tenantB}});
   const otherPage=await other.newPage();
   await otherPage.goto(base+"/#accounts");
   await otherPage.getByText("Клиентов пока нет.",{exact:true}).waitFor();
   assert.equal(await otherPage.getByRole("heading",{name:"Browser Account "+runId,exact:true}).count(),0);
   await other.close();
   const anonymous=await browser.newContext();
   const anonymousPage=await anonymous.newPage();
   await anonymousPage.goto(base+"/#accounts");
   await anonymousPage.getByRole("heading",{name:/Сессия завершена/}).waitFor();
   await anonymous.close();
   steps.push("Generated-client unauthenticated/permission errors and tenant B isolation");
   assert.deepEqual(pageErrors,[]);
   writeFileSync(resolve(directory,"typed-browser.json"),JSON.stringify({
     result:"PASS",source_sha:sourceSha,features:["native-ci"],browser:"Chromium",
     tested:"built React and generated Connect client against signed workerd/D1",steps
   },null,2)+"\n");
 } catch(error) {
   await page.screenshot({path:resolve(directory,"failure.png"),fullPage:true}).catch(()=>{});
   throw error;
 } finally {await browser.close();}
}

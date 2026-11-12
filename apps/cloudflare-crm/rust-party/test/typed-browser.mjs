// Real Chromium -> built React -> generated Connect client -> signed native Worker -> D1.
import {chromium} from "playwright";
import assert from "node:assert/strict";
import {mkdirSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {readFile} from "node:fs/promises";
import {fileURLToPath} from "node:url";

export async function typedBrowser({base,admin,viewer,tenantB,sql,sourceSha,
 adminHeaders={"cf-access-jwt-assertion":admin},
 viewerHeaders={"cf-access-jwt-assertion":viewer},
 tenantBHeaders={"cf-access-jwt-assertion":tenantB},tenantA="tenant-a",
 features=["native-ci"],beforeViewer=async()=>async()=>{},
 directory=fileURLToPath(new URL("../../e2e-evidence-native/",import.meta.url))}) {
 const tenantSql=tenantA.replaceAll("'","''");
 const startedAt=Date.now();
 const requests=[],writes=[],pageErrors=[],steps=[];
 let restoreViewer=async()=>{};
 const recordResponse=response=>{
   const path=new URL(response.url()).pathname;
   if(!path.startsWith("/crm.")&&!path.endsWith("/parties.csv"))return;
   const headers=response.headers();
   requests.push({path,status:response.status(),request_id:headers["x-request-id"],
     source_sha:headers["x-crm-source-sha"],runtime:headers["x-crm-runtime"]});
 };
 const watch=context=>{
   context.on("page",p=>p.on("pageerror",error=>pageErrors.push(error.message)));
   context.on("response",recordResponse);
 };
 mkdirSync(directory,{recursive:true});
 const browser=await chromium.launch({headless:true});
 const context=await browser.newContext({extraHTTPHeaders:adminHeaders,timezoneId:"UTC"});
 watch(context);
 const page=await context.newPage();

 const navigate=async(hash,title)=>{
   await page.goto(base+"/#"+hash);
   await page.getByRole("heading",{name:title,exact:true}).waitFor();
 };
 const assertMutation=async(response,method,{replay=false}={})=>{
   assert.equal(response.status(),200,method+" "+await response.text());
   const requestId=response.headers()["x-request-id"];
   assert.match(requestId??"",/^[a-f0-9-]{36}$/i);
   const audit=await sql("SELECT action,subject_id FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND request_id='"+requestId+"'");
   if(replay)assert.equal(audit.length,0,"Replay must not append audit");
   else {
     assert.ok(audit.length>0,method+" must commit audit for the browser request");
     writes.push({path:new URL(response.url()).pathname,request_id:requestId,audit});
   }
 };
 const mutation=async(button,method,options)=>{
   const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
   await button.click();const response=await pending;
   await assertMutation(response,method,options);
   await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
 };
 const firstId=async(table,column,value)=>{
   const escaped=value.replaceAll("'","''");
   const rows=await sql("SELECT id FROM "+table+" WHERE tenant='"+tenantSql+"' AND json_extract(state,'$."+column+"')='"+escaped+"'");
   assert.equal(rows.length,1);return rows[0].id;
 };
 const runId=Date.now().toString();
 let partyId,accountId;
 try {
   await navigate("parties","Люди и организации");
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Party "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   partyId=await firstId("crm_party_records","display_name","Browser Party "+runId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+partyId+"'"))[0].n,1);
   await page.reload();
   await page.getByRole("heading",{name:"Browser Party "+runId,exact:true}).waitFor();
   const partyCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Party "+runId,exact:true})});
   await partyCard.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить участника",exact:true}).waitFor();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Updated "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"UpdateParty");
   assert.equal((await sql("SELECT version FROM crm_party_records WHERE tenant='"+tenantSql+"' AND id='"+partyId+"'"))[0].version,2);
   steps.push("Party create/edit/reload and D1 version/audit");
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Other "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   const otherPartyId=await firstId("crm_party_records","display_name","Browser Other "+runId);
   await navigate("relationships","Связи");
   await page.getByLabel("Первый участник",{exact:true}).selectOption(partyId);
   await page.getByLabel("Второй участник",{exact:true}).selectOption(otherPartyId);
   await page.getByLabel("Код отношения",{exact:true}).fill("browser_relation_"+runId);
   await mutation(page.getByRole("button",{name:"Сохранить связь",exact:true}),"CreatePartyRelationship");
   await page.locator("section").filter({has:page.getByRole("heading",{name:"browser_relation_"+runId,exact:true})})
     .getByRole("button",{name:"Редактировать связь",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить связь",exact:true}).waitFor();
   await page.getByLabel("Статус",{exact:true}).selectOption("2");
   await mutation(page.getByRole("button",{name:"Сохранить связь",exact:true}),"UpdatePartyRelationship");
   const relationship=(await sql("SELECT version,json_extract(state,'$.status') AS status FROM crm_party_relationship_records WHERE tenant='"+tenantSql+"' AND from_party='"+partyId+"' AND to_party='"+otherPartyId+"'"))[0];
   assert.equal(relationship.version,2);assert.equal(relationship.status,"inactive");
   steps.push("Relationship create/edit through original owner with D1 version/status");
   await navigate("accounts","Клиенты");
   await page.getByLabel("Название клиента",{exact:true}).fill("Browser Account "+runId);
   await page.getByLabel("Основной участник",{exact:true}).selectOption(partyId);
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"CreateAccount");
   accountId=await firstId("crm_account_records","name","Browser Account "+runId);
   await page.getByRole("heading",{name:"Browser Account "+runId,exact:true}).waitFor();
   const accountCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Account "+runId,level:2,exact:true})});
   const overviewResponse=page.waitForResponse(r=>r.url().endsWith("/GetAccountOverview"));
   await accountCard.getByRole("button",{name:"Открыть карточку",exact:true}).click();
   assert.equal((await overviewResponse).status(),200);
   await page.getByText("Основной участник: Browser Updated "+runId,{exact:true}).waitFor();
   await accountCard.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить клиента",exact:true}).waitFor();
   await page.getByLabel("Статус",{exact:true}).selectOption("2");
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"UpdateAccount");
   assert.equal((await sql("SELECT version FROM crm_account_records WHERE tenant='"+tenantSql+"' AND id='"+accountId+"'"))[0].version,2);
   await page.getByLabel("Статус клиента",{exact:true}).selectOption("1");
   await accountCard.waitFor({state:"detached"});
   await page.getByLabel("Статус клиента",{exact:true}).selectOption("2");
   await accountCard.waitFor();
   steps.push("Account create and Customer360 read");
   await navigate("contacts","Контакты");
   await page.getByLabel("Участник",{exact:true}).selectOption(partyId);
   await page.getByLabel("Значение контакта",{exact:true}).fill("browser-"+runId+"@example.org");
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"CreateContactPoint");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_contact_point_records WHERE tenant='"+tenantSql+"' AND party_id='"+partyId+"'"))[0].n,1);
   await page.locator("section").filter({has:page.getByRole("heading",{name:"browser-"+runId+"@example.org",exact:true})})
     .getByRole("button",{name:"Редактировать контакт",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить контакт",exact:true}).waitFor();
   await page.getByLabel("Предпочтительный",{exact:true}).check();
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"UpdateContactPoint");
   assert.equal((await sql("SELECT version FROM crm_contact_point_records WHERE tenant='"+tenantSql+"' AND party_id='"+partyId+"'"))[0].version,2);
   await navigate("tasks","Задачи");
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Task "+runId);
   await page.getByLabel("Клиент",{exact:true}).selectOption(accountId);
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const taskId=await firstId("crm_activity_task_records","subject","Browser Task "+runId);
   const taskCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Task "+runId,exact:true})});
   await mutation(taskCard.getByRole("button",{name:"Выполнить задачу",exact:true}),"CompleteTask");
   assert.equal((await sql("SELECT json_extract(state,'$.status') AS status FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+taskId+"'"))[0].status,"completed");
   steps.push("Contact and Task create/complete");
   await navigate("sales","Сделки");
   await page.getByLabel("Название сделки",{exact:true}).fill("Browser Deal "+runId);
   await page.getByLabel("Клиент",{exact:true}).selectOption(accountId);
   await page.getByLabel("Сумма",{exact:true}).fill("90071992547409.93");
   await page.getByLabel("Вероятность, %",{exact:true}).fill("37.45");
   await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"CreateDeal");
   const dealId=await firstId("crm_sales_deal_records","name","Browser Deal "+runId);
   assert.ok(dealId);
   await page.getByText("90071992547409.93 USD · 37.45%",{exact:true}).waitFor();
   const dealCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Deal "+runId,exact:true})}).last();
   await dealCard.getByRole("button",{name:"Изменить сделку",exact:true}).click();
   // GetDeal completes asynchronously and replaces the create form. Editing the
   // old field before this heading appears would lose the entered value.
   await page.getByRole("heading",{name:"Изменить сделку",exact:true}).waitFor();
   assert.equal(await page.getByLabel("Вероятность, %",{exact:true}).inputValue(),"37.45");
   await page.getByLabel("Вероятность, %",{exact:true}).fill("38.5");
   assert.equal(await page.getByLabel("Вероятность, %",{exact:true}).inputValue(),"38.5");
   await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"UpdateDeal");
   const savedDeal=(await sql("SELECT version,json_extract(state,'$.probability_basis_points') AS probability FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.equal(savedDeal.version,2);assert.equal(savedDeal.probability,3850);
   await page.getByText("90071992547409.93 USD · 38.50%",{exact:true}).waitFor();
   await dealCard.getByRole("button",{name:"Изменить этап",exact:true}).click();
   await page.getByLabel("Код следующего этапа",{exact:true}).fill("qualified");
   await mutation(page.getByRole("button",{name:"Сохранить этап",exact:true}),"AdvanceStage");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant='"+tenantSql+"' AND deal_id='"+dealId+"'"))[0].n,1);
   await navigate("accounts","Клиенты");
   await page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Account "+runId,level:2,exact:true})})
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
   const compare=page.locator("section").filter({has:page.getByRole("heading",{name:"Сравнить участников",exact:true})});
   await compare.getByLabel("Первый участник",{exact:true}).selectOption(partyId);
   await compare.getByLabel("Второй участник",{exact:true}).selectOption(otherPartyId);
   const compared=page.waitForResponse(r=>r.url().endsWith("/CompareParties"));
   await compare.getByRole("button",{name:"Сравнить",exact:true}).click();
   assert.equal((await compared).status(),200);
   await compare.getByText(/разные записи; личность не подтверждена/).waitFor();
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
   const privacyBefore=new Set((await sql("SELECT id FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   await mutation(page.getByRole("button",{name:"Создать обращение",exact:true}),"CreatePrivacyCase");
   const newCases=(await sql("SELECT id FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"'")).filter(x=>!privacyBefore.has(x.id));
   assert.equal(newCases.length,1);
   const privacyId=newCases[0].id;
   const privacyCard=page.locator("section").filter({has:page.getByText("ID: "+privacyId,{exact:true})});
   await privacyCard.getByRole("button",{name:"Открыть запрос",exact:true}).click();
   await mutation(page.getByRole("button",{name:"Подать запрос",exact:true}),"SubmitPrivacyCase");
   await privacyCard.getByRole("button",{name:"Открыть запрос",exact:true}).click();
   await mutation(page.getByRole("button",{name:"Отменить запрос",exact:true}),"CancelPrivacyCase");
   const privacyState=(await sql("SELECT version,json_extract(state,'$.status.code') AS status FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"' AND id='"+privacyId+"'"))[0];
   assert.equal(privacyState.version,3);assert.equal(privacyState.status,"cancelled");
   await navigate("enrichment","Обогащение данных");
   await page.getByLabel("Ключ провайдера",{exact:true}).fill("browser_registry_"+runId);
   await page.getByLabel("Лицензия",{exact:true}).fill("Browser test license");
   await page.getByLabel("Начало действия",{exact:true}).fill("2026-11-12T12:00");
   assert.equal(await page.getByLabel("Начало действия",{exact:true}).inputValue(),"2026-11-12T12:00");
   await mutation(page.getByRole("button",{name:"Опубликовать профиль",exact:true}),"PublishProviderProfile");
   const profileId=await firstId("crm_enrichment_profiles","provider_key","browser_registry_"+runId);
   await page.getByRole("button",{name:"browser_registry_"+runId+" · 1.0.0",exact:true}).click();
   await page.getByText("ID: "+profileId,{exact:true}).waitFor();
   await page.getByLabel("Версия профиля",{exact:true}).selectOption(profileId);
   await page.getByLabel("Ключ сопоставления",{exact:true}).fill("browser_mapping_"+runId);
   await mutation(page.getByRole("button",{name:"Опубликовать сопоставление",exact:true}),"PublishMapping");
   const mappingId=await firstId("crm_enrichment_mappings","mapping_key","browser_mapping_"+runId);
   await page.getByRole("button",{name:"browser_mapping_"+runId,exact:true}).click();
   await page.getByText("ID: "+mappingId+"; поле: organization.legal_name; профиль: "+profileId,{exact:true}).waitFor();
   steps.push("Privacy draft/submit/cancel and immutable Enrichment profile/mapping readback");
   await navigate("data","Импорт и экспорт");
   const csv="party_id,kind,display_name,resource_version\r\nbrowser-import,person,Browser Import "+runId+",1\r\n";
   await page.getByLabel("CSV-файл",{exact:true}).setInputFiles({name:"browser.csv",mimeType:"text/csv",buffer:Buffer.from(csv)});
   await page.getByRole("button",{name:"Проверить импорт",exact:true}).click();
   await page.getByRole("button",{name:"Подтвердить импорт",exact:true}).waitFor();
   await mutation(page.getByRole("button",{name:"Подтвердить импорт",exact:true}),"ApplyPartyImport");
   assert.ok(await firstId("crm_party_records","display_name","Browser Import "+runId));
   const exportReply=page.waitForResponse(r=>r.url().endsWith("/PreviewPartyExport"));
   await page.getByRole("button",{name:"Проверить экспорт",exact:true}).click();
   assert.equal((await exportReply).status(),200);
   const downloadPending=page.waitForEvent("download");
   await page.getByRole("link",{name:"Скачать CSV",exact:true}).click();
   const download=await downloadPending;
   assert.equal(await download.failure(),null);
   const exported=await readFile(await download.path(),"utf8");
   assert.ok(exported.includes("Browser Import "+runId));
   assert.ok(exported.includes("Browser Updated "+runId));
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
   assert.equal(await firstId("crm_party_records","display_name","Browser Winner "+runId),partyId);
   assert.equal((await sql("SELECT version FROM crm_party_records WHERE tenant='"+tenantSql+"' AND id='"+partyId+"'"))[0].version,3);
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
     // route.fetch receives the committed response which the browser will lose.
     recordResponse(committed);
     await assertMutation(committed,"CreateParty");
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await page.getByRole("heading",{name:/Ошибка сервиса|Сервис временно недоступен/}).waitFor();
   const retryId=await firstId("crm_party_records","display_name","Browser Retry "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty",{replay:true});
   await page.getByRole("heading",{name:/Сохранено ранее/}).waitFor();
   assert.equal(await firstId("crm_party_records","display_name","Browser Retry "+runId),retryId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+retryId+"'"))[0].n,1);
   await page.unroute("**/crm.parties.v3.PartyService/CreateParty");
   steps.push("Committed response loss and same-form retry replay without duplicate audit");

   // Commit succeeds but only the following list refresh fails.
   await navigate("parties","Люди и организации");
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Saved Stale "+runId);
   await page.route("**/crm.parties.v3.PartyService/ListParties",route=>route.abort("failed"));
   const created=page.waitForResponse(r=>r.url().endsWith("/CreateParty"));
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await assertMutation(await created,"CreateParty");
   await page.getByRole("heading",{name:/Сохранено.*Список устарел/}).waitFor();
   assert.ok(await firstId("crm_party_records","display_name","Browser Saved Stale "+runId));
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

   restoreViewer=await beforeViewer();
   const denied=await browser.newContext({extraHTTPHeaders:viewerHeaders});
   watch(denied);
   const deniedPage=await denied.newPage();
   await deniedPage.goto(base+"/#parties");
   await deniedPage.getByLabel("Имя или название",{exact:true}).fill("Browser Denied "+runId);
   await deniedPage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await deniedPage.getByRole("heading",{name:/Недостаточно прав/}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+"' AND json_extract(state,'$.display_name')='Browser Denied "+runId+"'"))[0].n,0);
   await denied.close();
   await restoreViewer();restoreViewer=async()=>{};
   const other=await browser.newContext({extraHTTPHeaders:tenantBHeaders});
   watch(other);
   const otherPage=await other.newPage();
   await otherPage.goto(base+"/#accounts");
   await otherPage.getByText("Клиентов пока нет.",{exact:true}).waitFor();
   assert.equal(await otherPage.getByRole("heading",{name:"Browser Account "+runId,exact:true}).count(),0);
   await other.close();
   const anonymous=await browser.newContext();
   const anonymousPage=await anonymous.newPage();
   if(features.includes("native-ci")){
     const refused=anonymousPage.waitForResponse(r=>r.url().endsWith("/ListAccounts"));
     await anonymousPage.goto(base+"/#accounts");
     assert.equal((await refused).status(),401);
     // Both the account list and its Party picker can report expired access.
     await anonymousPage.getByRole("heading",{name:/Сессия завершена/}).first().waitFor();
   }else{
     const response=await anonymous.request.get(base+"/api/session",{maxRedirects:0});
     assert.equal(response.status(),302,"Anonymous production request must stop at Access");
     assert.match(new URL(response.headers().location).hostname,/\.cloudflareaccess\.com$/);
   }
   await anonymous.close();
   steps.push("Generated-client unauthenticated/permission errors and tenant B isolation");
   assert.deepEqual(pageErrors,[]);
   assert.ok(writes.length>0);
   for(const response of requests){
     assert.equal(response.source_sha,sourceSha,"Every browser response must use the tested source");
     assert.equal(response.runtime,"native-workers-rs");
     assert.match(response.request_id??"",/^[a-f0-9-]{36}$/i);
   }
   const evidence={result:"PASS",source_sha:sourceSha,features,browser:"Chromium",
     started_at:startedAt,finished_at:Date.now(),tenant:tenantA,
     tested:"built React and generated Connect client against Rust Worker/D1",steps,requests,writes};
   writeFileSync(resolve(directory,"typed-browser.json"),JSON.stringify(evidence,null,2)+"\n");
   return evidence;
 } catch(error) {
   await page.screenshot({path:resolve(directory,"failure.png"),fullPage:true}).catch(()=>{});
   throw error;
 } finally {
   try {await restoreViewer();}finally{await browser.close();}
 }
}

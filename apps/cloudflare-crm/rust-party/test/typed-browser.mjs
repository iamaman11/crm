// Real Chromium -> built React -> generated Connect client -> signed native Worker -> D1.
import {chromium} from "playwright";
import {expect} from "playwright/test";
import assert from "node:assert/strict";
import {mkdirSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {readFile} from "node:fs/promises";
import {fileURLToPath} from "node:url";

export async function typedBrowser({base,admin,viewer,tenantB,sql,sourceSha,adminIdentity,
 adminHeaders={"cf-access-jwt-assertion":admin},
 viewerHeaders={"cf-access-jwt-assertion":viewer},
 tenantBHeaders={"cf-access-jwt-assertion":tenantB},tenantA="tenant-a",
 features=["native-ci"],beforeViewer=async()=>async()=>{},
 directory=fileURLToPath(new URL("../../e2e-evidence-native/",import.meta.url))}) {
 const tenantSql=tenantA.replaceAll("'","''");
 assert.ok(adminIdentity,"Expected signed actor is required for browser audit assertions");
 const startedAt=Date.now();
 const requests=[],writes=[],pageErrors=[],steps=[];
 const headerReads=[];
 let restoreViewer=async()=>{};
 const recordResponse=response=>{
   const path=new URL(response.url()).pathname;
   if(!path.startsWith("/crm.")&&!path.endsWith("/parties.csv"))return;
   const entry={path,status:response.status()};
   requests.push(entry);
   const capture=headers=>Object.assign(entry,{
     request_id:headers["x-request-id"],error_code:headers["x-crm-error-code"]??null,
     source_sha:headers["x-crm-source-sha"],runtime:headers["x-crm-runtime"]});
   // Browser Response headers may arrive with later network extra-info. Await
   // Playwright's complete headers; route.fetch() returns an APIResponse instead.
   if(typeof response.allHeaders==="function"){
     headerReads.push(response.allHeaders().then(capture).catch(error=>{
       entry.header_error=String(error.message);
     }));
   }else capture(response.headers());
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
 await page.addInitScript(()=>{
   const events=[];
   Object.defineProperty(window,"__crmRouteEvents",{value:events});
   const documentId=crypto.randomUUID();
   const capture=(event)=>{
     const root=document.querySelector("[data-crm-route-kind]");
     events.push({event:event.type,document_id:documentId,
       hash:window.location.hash,at:performance.now(),
       persisted:"persisted" in event?event.persisted:null,
       rendered_kind:root?.getAttribute("data-crm-route-kind")??null,
       rendered_account_status:root?.getAttribute("data-crm-account-status")??null,
       history_length:history.length});
     if(events.length>80)events.shift();
   };
   for(const name of ["pageshow","pagehide","popstate","hashchange","visibilitychange"])
     window.addEventListener(name,capture);
 });

 const navigate=async(hash,title)=>{
   await page.goto(base+"/#"+hash);
   await page.getByRole("heading",{name:title,exact:true}).waitFor();
 };
 const search=async(label,value,method)=>{
   const input=page.getByLabel(label,{exact:true});
   await input.fill(value);
   const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
   await input.press("Enter");
   const response=await pending;
   assert.equal(response.status(),200);
   assert.equal((await response.allHeaders())["x-crm-source-sha"],sourceSha);
 };
 const assertMutation=async(response,method,{replay=false}={})=>{
   assert.equal(response.status(),200,method+" "+await response.text());
   const requestId=response.headers()["x-request-id"];
   assert.match(requestId??"",/^[a-f0-9-]{36}$/i);
   const audit=await sql("SELECT action,subject_id,actor FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND request_id='"+requestId+"'");
   if(replay)assert.equal(audit.length,0,"Replay must not append audit");
   else {
     assert.ok(audit.length>0,method+" must commit audit for the browser request");
     if(["CreateParty","UpdateParty","ApplyPartyImport"].includes(method)){
       for(const row of audit)assert.equal(row.actor,adminIdentity,method+" must preserve the signed actor in D1");
     }
     writes.push({path:new URL(response.url()).pathname,request_id:requestId,audit});
   }
 };
 const mutation=async(button,method,options)=>{
   const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
   await button.click();const response=await pending;
   await assertMutation(response,method,options);
   await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
 };
 // Pickers show one bounded page of options; randomly generated IDs
 // need not be on the first page. Use real UI pagination, not sleeps.
 const selectPagedPicker=async(scope,label,id,kind="party")=>{
  const select=scope.getByLabel(label,{exact:true});
  const picker=select.locator("xpath=../.."); // SelectField div -> picker div
  const nextLabel=kind==="party"?"Другие участники":"Другие клиенты";
  const method=kind==="party"?"/ListParties":"/ListAccounts";
  for(let pageNumber=0;pageNumber<30;pageNumber++){
   await picker.locator("option").nth(1).waitFor({state:"attached"});
   await picker.getByRole("status").waitFor({state:"hidden"});
   if(await select.locator(`option[value="${id}"]`).count()){
    await select.selectOption(id);
    // A controlled React select settles after its change handler renders.
    // Assert the committed value, not the same-tick DOM observation.
    await expect(select,label+" must select the requested record").toHaveValue(id);
    return;
   }
   const next=picker.getByRole("button",{name:nextLabel,exact:true});
   assert.equal(await next.count(),1,
     label+" has no further page containing "+id+" (page "+pageNumber+")");
   const pending=page.waitForResponse(response=>
     response.url().endsWith(method)&&response.request().method()==="POST");
   await next.click();
   assert.equal((await pending).status(),200,label+" pagination RPC failed");
   await picker.getByRole("status").waitFor({state:"hidden"});
  }
  throw Error(label+" lookup exceeded 30 pages for "+id);
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
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Party "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   partyId=await firstId("crm_party_records","display_name","Browser Party "+runId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+partyId+"'"))[0].n,1);
   await page.reload();
   await page.keyboard.press("Tab");
   assert.equal(await page.evaluate(()=>document.activeElement?.classList.contains("crm-skip-link")),true);
   await page.keyboard.press("Enter");
   assert.equal(await page.evaluate(()=>document.activeElement?.id),"main-content",
     "skip-link must focus main without changing the typed route");
   assert.equal(new URL(await page.url()).hash,"#parties");
   await page.getByRole("heading",{name:"Browser Party "+runId,exact:true}).waitFor();
   const partyCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Party "+runId,exact:true})});
   const partyEdit=partyCard.getByRole("button",{name:"Редактировать",exact:true});
   await partyEdit.click();
   await page.getByRole("heading",{name:"Изменить участника",exact:true}).waitFor();
   assert.equal(await page.getByRole("dialog").count(),1);
   const scrollAtOpen=await page.evaluate(()=>window.scrollY);
   await page.keyboard.press("Escape");
   await page.getByRole("dialog").waitFor({state:"hidden"});
   assert.equal(await page.evaluate(()=>window.scrollY),scrollAtOpen,"closing a record must keep list scroll");
   await partyEdit.click();
   await page.getByRole("heading",{name:"Изменить участника",exact:true}).waitFor();
   assert.equal(await page.getByRole("dialog").count(),1);
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Updated "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"UpdateParty");
   assert.equal((await sql("SELECT version FROM crm_party_records WHERE tenant='"+tenantSql+"' AND id='"+partyId+"'"))[0].version,2);
   steps.push("Party create/edit/reload and D1 version/audit");
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Other "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   const otherPartyId=await firstId("crm_party_records","display_name","Browser Other "+runId);
   const searchAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await search("Поиск участников","Browser Updated "+runId,"ListParties");
   await page.getByRole("heading",{name:"Browser Updated "+runId,exact:true}).waitFor();
   assert.equal(await page.getByRole("heading",{name:"Browser Other "+runId,exact:true}).count(),0);
   await search("Поиск участников","missing-search-"+runId,"ListParties");
   await page.getByText("Участники не найдены.",{exact:true}).waitFor();
   await search("Поиск участников",partyId,"ListParties");
   await page.getByRole("heading",{name:"Browser Updated "+runId,exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,searchAuditBefore);
   // Party -> Account is one existing CreateAccount command, not a second submit owner.
   const accountFromParty=async()=>{
     await page.locator("section").filter({has:page.getByRole("heading",
       {name:"Browser Updated "+runId,exact:true})})
       .getByRole("button",{name:"Создать клиента",exact:true}).click();
     await page.getByRole("heading",{name:"Новый клиент",exact:true}).waitFor();
     assert.equal(new URL(await page.url()).hash,"#accounts/new?partyId="+partyId);
     await expect(page.getByLabel("Название клиента",{exact:true})).toHaveValue("Browser Updated "+runId);
     await expect(page.getByLabel("Основной участник",{exact:true})).toHaveValue(partyId);
   };
   const partyAccountAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await accountFromParty();
   await page.getByLabel("Название клиента",{exact:true}).fill("Not saved "+runId);
   page.once("dialog",dialog=>void dialog.dismiss());
   await page.getByRole("button",{name:"К участнику",exact:true}).click();
   assert.equal(new URL(await page.url()).hash,"#accounts/new?partyId="+partyId,
     "cancelled discard must keep the unsaved Account form");
   assert.equal(await page.getByLabel("Название клиента",{exact:true}).inputValue(),"Not saved "+runId);
   page.once("dialog",dialog=>void dialog.accept());
   await page.getByRole("button",{name:"К участнику",exact:true}).click();
   await expect(page).toHaveURL(/#parties\?partyId=/);
   await page.getByRole("heading",{name:"Browser Updated "+runId,exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     partyAccountAuditBefore,"Cancel/Back must not create an Account");
   await accountFromParty();
   await page.getByLabel("Название клиента",{exact:true}).fill("Browser Party Account "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"CreateAccount");
   const linkedAccountId=await firstId("crm_account_records","name","Browser Party Account "+runId);
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,"#accounts/"+linkedAccountId,
     "accepted CreateAccount must replace draft URL with a permalink");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_account_records WHERE tenant='"+tenantSql+"' AND id='"+linkedAccountId+"'"))[0].n,1);
   steps.push("Party -> shared CreateAccount: selected Party, dirty Cancel/Back and permalink");
   await navigate("relationships","Связи");
   await page.getByRole("button",{name:"Создать связь",exact:true}).click();
   await selectPagedPicker(page,"Первый участник",partyId);
   await selectPagedPicker(page,"Второй участник",otherPartyId);
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
   await page.getByRole("button",{name:"Создать клиента",exact:true}).click();
   await page.getByLabel("Название клиента",{exact:true}).fill("Browser Account "+runId);
   await search("Поиск: Основной участник",partyId,"ListParties");
   await selectPagedPicker(page,"Основной участник",partyId);
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"CreateAccount");
   accountId=await firstId("crm_account_records","name","Browser Account "+runId);
   await page.getByRole("heading",{name:"Browser Account "+runId,exact:true}).waitFor();
   await search("Поиск клиентов",accountId,"ListAccounts");
   await page.getByRole("heading",{name:"Browser Account "+runId,exact:true}).waitFor();
   const accountCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Account "+runId,level:2,exact:true})});
   const overviewResponse=page.waitForResponse(r=>r.url().endsWith("/GetAccountOverview"));
   await accountCard.getByRole("button",{name:"Открыть карточку",exact:true}).click();
   assert.equal((await overviewResponse).status(),200);
   await page.getByText("Основной участник: Browser Updated "+runId,{exact:true}).waitFor();
   assert.equal(await accountCard.getByRole("region",{name:"Browser Account "+runId}).count(),1,"overview is inline");
   await accountCard.getByRole("button",{name:"Свернуть",exact:true}).click();
   assert.equal(await accountCard.getByRole("region",{name:"Browser Account "+runId}).count(),0);
   await accountCard.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByLabel("Статус",{exact:true}).waitFor();
   assert.equal(await page.getByRole("dialog").count(),0,"edit stays inline");
   await page.getByLabel("Статус",{exact:true}).selectOption("2");
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"UpdateAccount");
   assert.equal((await sql("SELECT version FROM crm_account_records WHERE tenant='"+tenantSql+"' AND id='"+accountId+"'"))[0].version,2);
   await page.getByLabel("Статус клиента",{exact:true}).selectOption("1");
   // Assert the navigation commit itself: list disappearance can also be a pending refresh.
   await expect(page).toHaveURL(/(?:\\?|&)status=1(?:&|$)/);
   await expect(page.locator('[data-crm-route-kind="accounts"]')).toHaveAttribute("data-crm-account-status","1");
   await accountCard.waitFor({state:"detached"});
   await page.getByLabel("Статус клиента",{exact:true}).selectOption("2");
   await expect(page).toHaveURL(/(?:\\?|&)status=2(?:&|$)/);
   await expect(page.locator('[data-crm-route-kind="accounts"]')).toHaveAttribute("data-crm-account-status","2");
   await accountCard.waitFor();
   const listHash=new URL(await page.url()).hash;
   // Make the small isolated list scrollable to verify a nonzero real scroll position.
   await page.evaluate(()=>{document.documentElement.style.minHeight="2200px";window.scrollTo(0,360);});
   const accountScrollAtOpen=await page.evaluate(()=>window.scrollY);
   assert.ok(accountScrollAtOpen>=300,"the scroll restoration test must not be a zero-scroll no-op");
   const pageLink=accountCard.getByRole("link",{name:"Полная страница",exact:true});
   await pageLink.click();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,"#accounts/"+encodeURIComponent(accountId));
   await page.goBack();
   await page.getByRole("heading",{name:"Клиенты",exact:true}).waitFor();
   await expect.poll(()=>page.evaluate(()=>window.scrollY)).toBe(accountScrollAtOpen);
   await pageLink.click();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   await page.reload();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   await page.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByLabel("Название клиента",{exact:true}).fill("Unsaved "+runId);
   page.once("dialog",dialog=>void dialog.dismiss());
   await page.getByRole("button",{name:"К списку клиентов",exact:true}).click();
   assert.equal(new URL(await page.url()).hash,"#accounts/"+encodeURIComponent(accountId),"discard refusal must retain the page");
   assert.equal(await page.getByLabel("Название клиента",{exact:true}).inputValue(),"Unsaved "+runId);
   await page.getByLabel("Название клиента",{exact:true}).fill("Browser Account "+runId);

   await page.getByLabel("Статус",{exact:true}).selectOption("1");
   await mutation(page.getByRole("button",{name:"Сохранить клиента",exact:true}),"UpdateAccount");
   assert.equal((await sql("SELECT version FROM crm_account_records WHERE tenant='"+tenantSql+"' AND id='"+accountId+"'"))[0].version,3);
   await page.getByRole("button",{name:"К списку клиентов",exact:true}).click();
   await page.getByRole("heading",{name:"Клиенты",exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,listHash,"Back must restore search, status and cursor");
   // Hash navigation and React reconciliation are asynchronous: assert the settled
   // control value, not a transient value in the first paint after popstate.
   await expect(page.locator('[data-crm-route-kind="accounts"]')).toHaveAttribute("data-crm-account-status","2");
   await expect(page.getByLabel("Статус клиента",{exact:true})).toHaveValue("2",{timeout:10000});
   await expect(page.getByLabel("Поиск клиентов",{exact:true})).toHaveValue(accountId,{timeout:10000});
   await page.reload();
   await expect(page.locator('[data-crm-route-kind="accounts"]')).toHaveAttribute("data-crm-account-status","2");
   await expect(page.getByLabel("Статус клиента",{exact:true})).toHaveValue("2",{timeout:10000});
   await expect(page.getByLabel("Поиск клиентов",{exact:true})).toHaveValue(accountId,{timeout:10000});
   await page.getByLabel("Статус клиента",{exact:true}).selectOption("1");
   await page.getByRole("heading",{name:"Browser Account "+runId,exact:true}).waitFor();
   steps.push("Account one editor: inline, full-page, reload, mutation, Back and filter restoration");
   await page.goto(base+"/#accounts?status=999");
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   steps.push("Account URL filter rejects unknown generated enum values");
   steps.push("Account create and Customer360 read");
   steps.push("Party/Account literal search by name and ID, empty result and read-only D1");
   await navigate("contacts","Контакты");
   await page.getByRole("button",{name:"Создать контакт",exact:true}).click();
   await selectPagedPicker(page,"Участник",partyId);
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
   await page.getByRole("button",{name:"Новая задача",exact:true}).click();
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Task "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const taskId=await firstId("crm_activity_task_records","subject","Browser Task "+runId);
   const taskCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Task "+runId,exact:true})});
   await mutation(taskCard.getByRole("button",{name:"Выполнить задачу",exact:true}),"CompleteTask");
   assert.equal((await sql("SELECT json_extract(state,'$.status') AS status FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+taskId+"'"))[0].status,"completed");
   await page.getByRole("button",{name:"Новая задача",exact:true}).click();
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Urgent "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await page.getByLabel("Приоритет",{exact:true}).selectOption("4");
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const urgentId=await firstId("crm_activity_task_records","subject","Browser Urgent "+runId);
   assert.equal((await sql("SELECT json_extract(state,'$.priority') AS priority FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+urgentId+"'"))[0].priority,"urgent");
   const beforeTaskFilters=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   const filterTasks=async(label,value)=>{
     const response=page.waitForResponse(r=>r.url().endsWith("/ListTasks")&&r.request().method()==="POST");
     await page.getByLabel(label,{exact:true}).selectOption(value);
     assert.equal((await response).status(),200);
   };
   await filterTasks("Приоритет задач","4");
   await page.getByRole("heading",{name:"Browser Urgent "+runId,exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor({state:"hidden"});
   await filterTasks("Статус задачи","2");
   await page.getByText("Задач пока нет.",{exact:true}).waitFor();
   await filterTasks("Приоритет задач","2");
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Urgent "+runId,exact:true}).waitFor({state:"hidden"});
   await filterTasks("Статус задачи","");
   await filterTasks("Приоритет задач","");
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Urgent "+runId,exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,beforeTaskFilters);
   // Real React -> generated Connect UpdateTask -> D1 and audit -> reload.
   const urgentCard=page.locator("section").filter({has:page.getByRole("heading",
     {name:"Browser Urgent "+runId,exact:true})});
   await urgentCard.getByRole("button",{name:"Редактировать задачу",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить задачу",exact:true}).waitFor();
   await page.getByLabel("Тема задачи: изменение",{exact:true}).fill("Browser Edited Urgent "+runId);
   await page.getByLabel("Описание задачи: изменение",{exact:true}).fill("Edited from UI");
   await page.getByLabel("Приоритет: изменение",{exact:true}).selectOption("3");
   await mutation(page.getByRole("button",{name:"Сохранить изменения задачи",exact:true}),"UpdateTask");
   const savedEdit=(await sql("SELECT version,json_extract(state,'$.subject') AS subject,json_extract(state,'$.priority') AS priority FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+urgentId+"'"))[0];
   assert.equal(savedEdit.version,2);
   assert.equal(savedEdit.subject,"Browser Edited Urgent "+runId);
   assert.equal(savedEdit.priority,"high");
   assert.deepEqual((await sql("SELECT action FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+urgentId+"' ORDER BY rowid")).map(x=>x.action),
     ["activity_tasks.create","activity_tasks.update"]);
   await page.reload();
   await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   await filterTasks("Приоритет задач","4");
   await page.getByText("Задач пока нет.",{exact:true}).waitFor();
   await filterTasks("Приоритет задач","3");
   await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   await filterTasks("Приоритет задач","");
   steps.push("Task UpdateTask via React, D1 audit action, priority filter and reload");
   steps.push("Contact and Task create/complete; priority/status filters preserve records and audit");
   await navigate("sales","Сделки");
   await page.getByRole("button",{name:"Новая сделка",exact:true}).click();
   await page.getByLabel("Название сделки",{exact:true}).fill("Browser Deal "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
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
   await page.getByText("Основной участник: Browser Updated "+runId,{exact:true}).waitFor();
   steps.push("Exact bigint Deal and atomic Sales–Activities flow visible in Account");
   await navigate("review","Качество и совпадения");
   const quality=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить имя",exact:true})});
   await selectPagedPicker(quality,"Участник",partyId);
   const qualityResponse=page.waitForResponse(r=>r.url().endsWith("/EvaluatePartyName"));
   await quality.getByRole("button",{name:"Проверить качество",exact:true}).click();
   assert.equal((await qualityResponse).status(),200);
   await page.getByText("Не пройдено: 0",{exact:true}).waitFor();
   const compare=page.locator("section").filter({has:page.getByRole("heading",{name:"Сравнить участников",exact:true})});
   await selectPagedPicker(compare,"Первый участник",partyId);
   await selectPagedPicker(compare,"Второй участник",otherPartyId);
   const compared=page.waitForResponse(r=>r.url().endsWith("/CompareParties"));
   await compare.getByRole("button",{name:"Сравнить",exact:true}).click();
   assert.equal((await compared).status(),200);
   await compare.getByText(/разные записи; личность не подтверждена/).waitFor();
   await navigate("consents","Коммуникации");
   await page.getByRole("button",{name:"Запретить коммуникацию",exact:true}).click();
   const suppress=page.getByRole("dialog");
   await selectPagedPicker(suppress,"Участник",partyId);
   await mutation(suppress.getByRole("button",{name:"Запретить контакт",exact:true}),"SuppressCommunication");
   const evaluate=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить возможность контакта",exact:true})});
   await selectPagedPicker(evaluate,"Участник",partyId);
   await evaluate.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await page.getByRole("heading",{name:"Контакт не разрешён",exact:true}).waitFor();
   steps.push("Quality and consent evaluation/suppression");
   await navigate("privacy","Запросы о данных");
   await page.getByRole("button",{name:"Новое обращение",exact:true}).click();
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
   await page.getByRole("button",{name:"Новый профиль",exact:true}).click();
   await page.getByLabel("Ключ провайдера",{exact:true}).fill("browser_registry_"+runId);
   await page.getByLabel("Лицензия",{exact:true}).fill("Browser test license");
   await page.getByLabel("Начало действия",{exact:true}).fill("2026-11-12T12:00");
   assert.equal(await page.getByLabel("Начало действия",{exact:true}).inputValue(),"2026-11-12T12:00");
   await mutation(page.getByRole("button",{name:"Опубликовать профиль",exact:true}),"PublishProviderProfile");
   const profileId=await firstId("crm_enrichment_profiles","provider_key","browser_registry_"+runId);
   await page.getByRole("button",{name:"browser_registry_"+runId+" · 1.0.0",exact:true}).click();
   await page.getByText("ID: "+profileId,{exact:true}).waitFor();
   await page.getByRole("button",{name:"Закрыть панель",exact:true}).click();
   await page.getByRole("dialog").waitFor({state:"hidden"});
   await page.getByRole("button",{name:"Новое сопоставление",exact:true}).click();
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
   const conflictResponse=await conflict;
   assert.equal(conflictResponse.status(),409);
   assert.equal(conflictResponse.headers()["x-crm-error-code"],"aborted");
   const conflictRequestId=conflictResponse.headers()["x-request-id"];
   assert.match(conflictRequestId??"",/^[a-f0-9-]{36}$/i);
   await stalePage.getByRole("heading",{name:new RegExp("Конфликт изменения.*"+conflictRequestId)}).waitFor();
    assert.equal(await stalePage.getByRole("heading",{name:new RegExp("Конфликт изменения.*"+conflictRequestId)}).count(),1,
      "a conflict is shown once in the active panel, not behind it");
   assert.equal(await firstId("crm_party_records","display_name","Browser Winner "+runId),partyId);
   assert.equal((await sql("SELECT version FROM crm_party_records WHERE tenant='"+tenantSql+"' AND id='"+partyId+"'"))[0].version,3);
   await Promise.all(headerReads);
   await stalePage.close();
   steps.push("Two browser forms enforce CAS and show generated-client conflict");

   // Lose the response AFTER the server commits; retry the unchanged form.
   await navigate("parties","Люди и организации");
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
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
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Saved Stale "+runId);
   await page.route("**/crm.parties.v3.PartyService/ListParties",route=>route.abort("failed"));
   const created=page.waitForResponse(r=>r.url().endsWith("/CreateParty"));
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await assertMutation(await created,"CreateParty");
   await page.getByRole("heading",{name:/Сохранено.*Список устарел/}).waitFor();
   assert.ok(await firstId("crm_party_records","display_name","Browser Saved Stale "+runId));
   await page.unroute("**/crm.parties.v3.PartyService/ListParties");
   steps.push("Committed write remains successful when refresh fails");

   const navigationAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   for(const [link,screen,title,filter,value,method] of [
     ["Все контакты участника","contacts","Контакты","Фильтр по ID участника",partyId,"ListContactPoints"],
     ["Все связи участника","relationships","Связи","Фильтр по ID участника",partyId,"ListPartyRelationships"],
     ["Все сделки клиента","sales","Сделки","Фильтр по ID клиента",accountId,"ListDeals"],
     ["Все задачи клиента","tasks","Задачи","Фильтр по ID клиента",accountId,"ListTasks"],
   ]){
     await navigate("accounts","Клиенты");
     await search("Поиск клиентов",accountId,"ListAccounts");
     const [overviewResult]=await Promise.all([
       page.waitForResponse(r=>r.url().endsWith("/GetAccountOverview")),
       page.locator("section").filter({has:page.getByRole("heading",
         {name:"Browser Account "+runId,level:2,exact:true})})
         .getByRole("button",{name:"Открыть карточку",exact:true}).click(),
     ]);
     assert.equal(overviewResult.status(),200);
     const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
     await page.getByRole("link",{name:link,exact:true}).click();
     assert.equal((await pending).status(),200);
     await page.getByRole("heading",{name:title,exact:true}).waitFor();
     assert.equal(await page.getByLabel(filter,{exact:true}).inputValue(),value);
     assert.equal(new URL(await page.url()).hash,"#"+screen+"?"+(screen==="sales"||screen==="tasks"?"accountId":"partyId")+"="+encodeURIComponent(value));
     const reloaded=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
     await page.reload();assert.equal((await reloaded).status(),200);
     assert.equal(await page.getByLabel(filter,{exact:true}).inputValue(),value,"Reload must retain the linked scope");
     if(screen==="sales")await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
     if(screen==="tasks")await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   }
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,navigationAuditBefore);
   steps.push("Customer360 links to server-filtered full lists, reload retains scope, no writes");
   await page.getByLabel("Тема",{exact:true}).selectOption("dark");
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.reload();
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.setViewportSize({width:390,height:844});
   await page.screenshot({path:resolve(directory,"crm-mobile-dark.png"),fullPage:true});
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1),"mobile horizontal overflow");
   await navigate("accounts","Клиенты");
   await page.getByRole("button",{name:"Создать клиента",exact:true}).click();
   const mobilePanel=page.getByRole("dialog");
   await mobilePanel.waitFor();
   const bounds=await mobilePanel.evaluate(element=>({
     width:element.getBoundingClientRect().width,viewport:window.innerWidth
   }));
   assert.ok(bounds.width<=bounds.viewport+1 && bounds.width>=bounds.viewport-2,"record panel fills mobile viewport");
   await page.keyboard.press("Escape");
   await mobilePanel.waitFor({state:"hidden"});
   steps.push("Theme persistence and mobile layout");

   restoreViewer=await beforeViewer();
   const denied=await browser.newContext({extraHTTPHeaders:viewerHeaders});
   watch(denied);
   const deniedPage=await denied.newPage();
   await deniedPage.goto(base+"/#parties");
   await deniedPage.getByRole("button",{name:"Создать участника",exact:true}).click();
   await deniedPage.getByLabel("Имя или название",{exact:true}).fill("Browser Denied "+runId);
   await deniedPage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await deniedPage.getByRole("heading",{name:/Недостаточно прав/}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+"' AND json_extract(state,'$.display_name')='Browser Denied "+runId+"'"))[0].n,0);
   await Promise.all(headerReads);
   await denied.close();
   await restoreViewer();restoreViewer=async()=>{};
   const other=await browser.newContext({extraHTTPHeaders:tenantBHeaders});
   watch(other);
   const otherPage=await other.newPage();
   await otherPage.goto(base+"/#accounts");
   await otherPage.getByText("Клиентов пока нет.",{exact:true}).waitFor();
   assert.equal(await otherPage.getByRole("heading",{name:"Browser Account "+runId,exact:true}).count(),0);
   await Promise.all(headerReads);
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
   await Promise.all(headerReads);
   for(const response of requests){
     assert.equal(response.source_sha,sourceSha,"Every browser response must use the tested source: "+JSON.stringify(response));
     assert.equal(response.runtime,"native-workers-rs");
     assert.match(response.request_id??"",/^[a-f0-9-]{36}$/i);
   }
   const evidence={result:"PASS",source_sha:sourceSha,features,browser:"Chromium",
     started_at:startedAt,finished_at:Date.now(),tenant:tenantA,
     tested:"built React and generated Connect client against Rust Worker/D1",steps,requests,writes};
   writeFileSync(resolve(directory,"typed-browser.json"),JSON.stringify(evidence,null,2)+"\n");
   return evidence;
 } catch(error) {
   await Promise.all(headerReads);
   const navigation=await page.evaluate(()=>{
     const field=Array.from(document.querySelectorAll("label"))
       .find(label=>label.textContent?.trim()==="Статус клиента");
     const select=field?document.getElementById(field.htmlFor):null;
     const route=document.querySelector("[data-crm-route-kind]");
     return {hash:window.location.hash,
       document_url:new URL(window.location.href).pathname+window.location.hash,
       navigation_type:performance.getEntriesByType("navigation").at(-1)?.type??null,
       history_length:history.length,
       history_state:window.history.state,
       rendered_kind:route?.getAttribute("data-crm-route-kind")??null,
       rendered_account_status:route?.getAttribute("data-crm-account-status")??null,
       select_status:select instanceof HTMLSelectElement?select.value:null,
       events:window.__crmRouteEvents?.slice(-60)??[]};
   }).catch(e=>({capture_error:String(e)}));
   writeFileSync(resolve(directory,"failure.json"),JSON.stringify({result:"FAIL",source_sha:sourceSha,
     error:String(error.message),navigation,steps,requests,writes},null,2)+"\n");
   await page.screenshot({path:resolve(directory,"failure.png"),fullPage:true}).catch(()=>{});
   throw error;
 } finally {
   try {await restoreViewer();}finally{await browser.close();}
 }
}

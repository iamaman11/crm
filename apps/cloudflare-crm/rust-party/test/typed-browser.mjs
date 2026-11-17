// Real Chromium -> built React -> generated Connect client -> signed native Worker -> D1.
import {chromium} from "playwright";
import {expect} from "playwright/test";
import assert from "node:assert/strict";
import {mkdirSync,writeFileSync} from "node:fs";
import {resolve} from "node:path";
import {readFile} from "node:fs/promises";
import {fileURLToPath} from "node:url";
import {installUiDiagnostics,assertUiPanel} from "./ui-diagnostics.mjs";

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
 await installUiDiagnostics(page);
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
   const headers=typeof response.allHeaders==="function"?await response.allHeaders():response.headers();
   const requestId=headers["x-request-id"];
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
   const skipLink=page.locator(".crm-skip-link");
   assert.equal(await skipLink.getAttribute("href"),"#main-content");
   await skipLink.focus();
   assert.equal(await page.evaluate(()=>document.activeElement?.classList.contains("crm-skip-link")),true);
   await page.keyboard.press("Enter");
   assert.equal(await page.evaluate(()=>document.activeElement?.id),"main-content",
     "skip-link must focus main without changing the typed route");
   assert.equal(new URL(await page.url()).hash,"#parties");
   const partyHeading=page.getByRole("heading",{name:"Browser Party "+runId,exact:true});
   // Workers.dev route can transiently return its OWN HTML 404 on an
   // independently reloaded read-only Connect ListParties POST. Recover via
   // the shipped UI Retry control ONLY after proving the response did not
   // originate from the Rust Worker. Never retry a mutation, 409 or 401/403.
   let edgeReadRecoveries=0;
   for(let attempt=0;attempt<5;attempt++){
     try {await partyHeading.waitFor({timeout:8000});break;}
     catch(error){
       const notice=page.getByRole("heading",{name:"Ошибка сервиса. HTTP 404",exact:true});
       if(!await notice.isVisible())throw error;
       await Promise.all(headerReads);
       const last=requests.findLast(entry=>entry.path.endsWith("/ListParties"));
       if(last?.status!==404||last.source_sha||last.runtime)
         throw Error("ListParties failed inside CRM; no edge-only recovery allowed: "+
           JSON.stringify(last));
       const pending=page.waitForResponse(r=>r.url().endsWith("/ListParties")&&
         r.request().method()==="POST");
       await page.getByRole("button",{name:"Повторить чтение",exact:true}).click();
       const response=await pending;
       const headers=await response.allHeaders();
       if(response.status()===200)
         assert.equal(headers["x-crm-source-sha"],sourceSha,
           "Recovered read must come from the exact native Worker");
       else {
         assert.equal(response.status(),404,"Retry cannot bypass a Worker read error");
         assert.equal(headers["x-crm-source-sha"],undefined,
           "Worker-owned 404 must fail rather than retry");
       }
       edgeReadRecoveries++;
     }
   }
   await partyHeading.waitFor({timeout:10000});
   if(edgeReadRecoveries)steps.push("Cloudflare edge-only read 404 recovered through typed UI Retry");
   const partyCard=page.locator("section").filter({has:partyHeading});
   const partyEdit=partyCard.getByRole("button",{name:"Редактировать",exact:true});
   await partyEdit.click();
   await page.getByRole("heading",{name:"Изменить участника",exact:true}).waitFor();
   assert.equal(await page.getByRole("dialog").count(),1);
   const scrollAtOpen=await page.evaluate(()=>window.scrollY);
   const editorPanel=page.getByRole("dialog");
   const editorPanelId=await editorPanel.getAttribute("aria-labelledby");
   assert.ok(editorPanelId,"The native Party dialog must have a stable React useId identity");
   await page.keyboard.press("Escape");
   await assertUiPanel({page,panel:editorPanel,panelId:editorPanelId,title:"Изменить участника",
     expected:"closed",scenario:"party-edit-escape",evidenceDirectory:directory});
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
   const partyAccountResponse=page.waitForResponse(r=>
     r.url().endsWith("/CreateAccount")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Сохранить клиента",exact:true}).click();
   await assertMutation(await partyAccountResponse,"CreateAccount");
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
   // HTTP completion precedes React query commit. Assert the exact filtered
   // option entered the DOM before the generic paged picker checks cursors.
   // A bad server result still fails (the expected option never appears).
   await page.getByLabel("Основной участник",{exact:true})
     .locator(`option[value="${partyId}"]`).waitFor({state:"attached"});
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
   await page.evaluate(()=>{document.documentElement.style.minHeight="2200px";window.scrollTo(0,150);});
   const accountScrollAtOpen=await page.evaluate(()=>window.scrollY);
   assert.ok(accountScrollAtOpen>=100,"the scroll restoration test must not be a zero-scroll no-op");
   const pageLink=accountCard.getByRole("link",{name:"Полная страница",exact:true});
   await pageLink.click();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,"#accounts/"+encodeURIComponent(accountId));
   await page.goBack();
   await page.getByRole("heading",{name:"Клиенты",exact:true}).waitFor();
   const recordedScroll=await page.evaluate(()=>window.history.state?.crmAccountScroll);
   assert.ok(recordedScroll>=100,"open must record a nonzero list scroll state");
   await expect.poll(()=>page.evaluate(()=>window.scrollY)).toBe(recordedScroll);
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
   const browserSalesContactId=await firstId("crm_contact_point_records","normalized_value",
     "browser-"+runId+"@example.org");
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
   await page.getByRole("button",{name:"Создать воронку",exact:true}).click();
   await page.getByLabel("Название воронки",{exact:true}).fill("Browser Pipeline "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"CreatePipeline");
   const newPipelineId=await firstId("crm_sales_pipeline_records","name","Browser Pipeline "+runId);
   assert.equal((await sql("SELECT version FROM crm_sales_pipeline_records WHERE tenant='"+tenantSql+"' AND id='"+newPipelineId+"'"))[0].version,1);
   const recordedPipelines=await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+newPipelineId+"' AND action='sales_pipeline.create'");
   assert.equal(recordedPipelines[0].n,1);
   // The test owns its pipeline end-to-end; old test Deals are discarded in 0012.
   const pipelineState=(await sql("SELECT state FROM crm_sales_pipeline_records WHERE tenant='"+tenantSql+"' AND id='"+newPipelineId+"'"))[0].state;
   const ownStageIds=JSON.parse(pipelineState).stages.map(s=>s.stage_id);
   assert.equal(ownStageIds.length,2,"new user-managed pipeline must have two stages");
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
   await page.getByRole("button",{name:"Новая сделка",exact:true}).click();
   await page.getByLabel("Название сделки",{exact:true}).fill("Browser Deal "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   const dealContact=page.getByLabel("Основной контакт сделки",{exact:true});
   await dealContact.locator('option[value="'+browserSalesContactId+'"]').waitFor({state:"attached"});
   await dealContact.selectOption(browserSalesContactId);
   await page.getByLabel("Ожидаемая дата закрытия",{exact:true}).fill("2030-05-21");
   await page.getByLabel("Сумма",{exact:true}).fill("90071992547409.93");
   await page.getByLabel("Источник вероятности",{exact:true}).selectOption("1");
   await page.getByLabel("Категория прогноза",{exact:true}).selectOption("3");
   await page.getByLabel("Вероятность, %",{exact:true}).fill("37.45");
   await page.getByLabel("Название сделки",{exact:true}).fill("   ");
   const invalidCreate=page.waitForResponse(r=>r.url().endsWith("/CreateDeal")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Сохранить сделку",exact:true}).click();
   assert.equal((await invalidCreate).status(),400);
   await expect(page.getByRole("dialog").getByText(/deal\.name: deal name must be non-empty/)).toBeVisible();
   await page.getByLabel("Название сделки",{exact:true}).fill("Browser Deal "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"CreateDeal");
   const dealId=await firstId("crm_sales_deal_records","name","Browser Deal "+runId);
   assert.ok(dealId);
   const saleCreatedR72=(await sql("SELECT json_extract(state,'$.primary_contact.resource_id') AS contact_id, "+
     "json_extract(state,'$.expected_close_date.year') AS year, "+
     "json_extract(state,'$.expected_close_date.month') AS month, "+
     "json_extract(state,'$.expected_close_date.day') AS day "+
     "FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.deepEqual(saleCreatedR72,{contact_id:browserSalesContactId,year:2030,month:5,day:21});
   await page.getByText("90071992547409.93 USD · 37.45%",{exact:true}).waitFor();
   const leadTotals=page.getByRole("group",{name:"Итоги этапа Первичный контакт"});
   await leadTotals.getByText("90071992547409.93 USD · сделок с суммой: 1",
     {exact:true}).waitFor();
   const wholePipelineMetrics=page.getByRole("region",{name:"Аналитика воронки"});
   await wholePipelineMetrics.getByText(/^Всего сделок: 1 · срез по UTC: \d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/).waitFor();
   const metricStage=wholePipelineMetrics.getByRole("group",{name:"Первичный контакт · Открытые"});
   await metricStage.getByText("Открытые: 1 сделок",{exact:true}).waitFor();
   const operationalPanel=metricStage.getByRole("group",{name:"Операционные показатели"});
   await operationalPanel.getByText("позднее 1",{exact:false}).waitFor();
   await operationalPanel.getByText("дата не указана 0",{exact:false}).waitFor();
   await operationalPanel.getByText("подтверждённая история у 1",{exact:false}).waitFor();
   await metricStage.getByText(
     "90071992547409.93 USD · взвешено: 33731961209005.018785 USD · сделок с суммой: 1",
     {exact:true}).waitFor();
   const forecast=page.getByRole("region",{name:"Прогноз продаж"}).getByRole("group",{name:"Обязательство",exact:true});
   await forecast.getByText("Сделок: 1 · по этапу: 0 · вручную: 1",{exact:true}).waitFor();
   await forecast.getByText("90071992547409.93 USD · взвешено: 33731961209005.018785 USD · сделок с суммой: 1",{exact:true}).waitFor();
   const forecastRow=(await sql("SELECT json_extract(state,'$.probability_source') AS source,json_extract(state,'$.forecast_category') AS category FROM crm_sales_deal_records WHERE id='"+dealId+"'"))[0];
   assert.deepEqual(forecastRow,{source:"manual",category:"commit"});
   steps.push("R7.3 full-pipeline exact weighted multi-currency read model, React, D1");
   // R7.3 saved views through REAL React -> generated Connect -> Rust domain
   // -> D1 audit/receipt. Reload must not lose a named shared filter.
   const savedViewName="Browser Saved View "+runId;
   await page.getByLabel("Статус сделки",{exact:true}).selectOption("1");
   await page.getByRole("button",{name:"Сохранить как новое",exact:true}).click();
   await page.getByLabel("Название представления",{exact:true}).fill(savedViewName);
   await mutation(page.getByRole("button",{name:"Сохранить новое представление",exact:true}),
     "CreateSalesView");
   const viewId=await firstId("crm_sales_saved_view_records","name",savedViewName);
   const createdView=JSON.parse((await sql("SELECT state FROM crm_sales_saved_view_records "+
     "WHERE tenant='"+tenantSql+"' AND id='"+viewId+"'"))[0].state);
   assert.equal(createdView.pipeline_id,newPipelineId);
   assert.equal(createdView.status,"open");
   assert.equal(createdView.version,1);
   await page.reload();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   const viewPicker=page.getByLabel("Сохранённое представление",{exact:true});
   await viewPicker.locator('option[value="'+viewId+'"]').waitFor({state:"attached"});
   await viewPicker.selectOption(viewId);
   // Views and pipelines are independent asynchronous reads on reload.
   // Wait for the controlled React selection AND its catalog option, not a
   // same-tick inputValue before the ListPipelines response is rendered.
   await expect(viewPicker).toHaveValue(viewId);
   const selectedPipeline=page.getByLabel("Воронка продаж",{exact:true});
   await expect(selectedPipeline.locator('option[value="'+newPipelineId+'"]').first()).toBeAttached();
   await expect(selectedPipeline).toHaveValue(newPipelineId);
   await expect(page.getByLabel("Статус сделки",{exact:true})).toHaveValue("1");
   await page.getByRole("region",{name:"Аналитика воронки"})
     .getByText(/^Всего сделок: 1 · срез по UTC:/).waitFor();
   // Save the changed selected filters under the same stable view ID.
   await page.getByLabel("Статус сделки",{exact:true}).selectOption("2");
   await page.getByLabel("Название представления",{exact:true})
     .fill("Browser Won View "+runId);
   await mutation(page.getByRole("button",{name:"Обновить представление",exact:true}),
     "UpdateSalesView");
   const updatedView=JSON.parse((await sql("SELECT state FROM crm_sales_saved_view_records "+
     "WHERE tenant='"+tenantSql+"' AND id='"+viewId+"'"))[0].state);
   assert.equal(updatedView.version,2);
   assert.equal(updatedView.status,"won");
   assert.equal(updatedView.name,"Browser Won View "+runId);
   await mutation(page.getByRole("button",{name:"Архивировать представление",exact:true}),
     "UpdateSalesView");
   assert.equal((await sql("SELECT json_extract(state,'$.archived') AS archived "+
     "FROM crm_sales_saved_view_records WHERE tenant='"+tenantSql+"' AND id='"+viewId+"'"))[0].archived,1);
   const viewsAfterArchive=page.waitForResponse(r=>r.url().endsWith("/ListSalesViews")&&r.request().method()==="POST");
   await page.reload();
   assert.equal((await viewsAfterArchive).status(),200);
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   const viewsSection=page.getByRole("region",{name:"Сохранённые представления"});
   await expect(viewsSection.getByRole("status")).toHaveCount(0);
   await expect(viewsSection.getByLabel("Сохранённое представление",{exact:true})
     .locator('option[value="'+viewId+'"]'),
     "Archived view must remain audited but not appear as selectable").toHaveCount(0);
   assert.deepEqual((await sql("SELECT action FROM crm_audit WHERE tenant_id='"+tenantSql+
     "' AND subject_id='"+viewId+"' ORDER BY rowid")).map(x=>x.action),
     ["sales_view.create","sales_view.update","sales_view.update"]);
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
   await page.getByLabel("Статус сделки",{exact:true}).selectOption("");
   await page.getByText("90071992547409.93 USD · 37.45%",{exact:true}).waitFor();
   steps.push("R7.3 saved view create, reload/apply, update, archive and D1 audit");
   const dealCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Deal "+runId,exact:true})}).last();
   await dealCard.getByRole("button",{name:"Раскрыть карточку",exact:true}).click();
   await page.getByRole("region",{name:"Подробности сделки "+dealId}).getByText(
     "Ожидаемое закрытие: 2030-05-21",{exact:true}).waitFor();
   await page.getByRole("button",{name:"Свернуть",exact:true}).last().click();
   // Force the exact LIVE race: GetDeal has loaded but ContactPoint choices
   // have not. A pending read must never turn an unchanged association
   // into a missing FormData field or silently Clear the selected contact.
   let releaseContactRead=()=>{};
   const pendingContactRead=new Promise(resolve=>{releaseContactRead=resolve;});
   let blockedContactRead=false;
   const blockContactRead=async route=>{
     if(!blockedContactRead) {
       blockedContactRead=true;
       await pendingContactRead;
     }
     try {
       await route.continue();
     } catch(error) {
       // Successful Deal update unmounts the editor and cancels its
       // pending ContactPoint lookup. Playwright has already handled that
       // specific route; unrelated errors must still fail the test.
       if(!/Route is already handled!/.test(String(error)))throw error;
     }
   };
   await page.route("**/crm.accounts.v2.AccountService/GetAccount",blockContactRead);
   await dealCard.getByRole("button",{name:"Изменить сделку",exact:true}).click();
   // GetDeal completes asynchronously and replaces the create form. Editing the
   // old field before this heading appears would lose the entered value.
   await page.getByRole("heading",{name:"Изменить сделку",exact:true}).waitFor();
   assert.equal(await page.getByLabel("Вероятность, %",{exact:true}).inputValue(),"37.45");
   await expect(page.getByLabel("Основной контакт сделки",{exact:true})).toBeDisabled();
   assert.equal(blockedContactRead,true,"Account/Contact read must still be pending during edit");
   await page.getByLabel("Вероятность, %",{exact:true}).fill("38.5");
   await page.getByLabel("Ожидаемая дата закрытия",{exact:true}).fill("2030-06-01");
   assert.equal(await page.getByLabel("Вероятность, %",{exact:true}).inputValue(),"38.5");
   await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"UpdateDeal");
   releaseContactRead();
   await page.unroute("**/crm.accounts.v2.AccountService/GetAccount",blockContactRead);
   const savedDeal=(await sql("SELECT version,json_extract(state,'$.probability_basis_points') AS probability FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.equal(savedDeal.version,2);assert.equal(savedDeal.probability,3850);
   const expectedAfterUpdate=(await sql("SELECT json_extract(state,'$.expected_close_date.day') AS day, "+
     "json_extract(state,'$.primary_contact.resource_id') AS contact_id "+
     "FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.equal(expectedAfterUpdate.day,1);
   assert.equal(expectedAfterUpdate.contact_id,browserSalesContactId,
     "async Account/Contact lookup must not erase unchanged Deal primary contact");
   steps.push("R7.2 contact/date via real React, GetDeal/detail and original D1");
   await page.getByText("90071992547409.93 USD · 38.50%",{exact:true}).waitFor();
   // Keyboard/select fallback remains the original mutation form; verify
   // it opens and can be cancelled without a write before testing native DnD.
   await dealCard.getByRole("button",{name:"Изменить этап",exact:true}).focus();
   await page.keyboard.press("Enter");
   await page.getByRole("heading",{name:"Этап сделки: Browser Deal "+runId,exact:true}).waitFor();
   await page.getByLabel("Следующий этап",{exact:true}).selectOption(ownStageIds[1]);
   await page.getByRole("button",{name:"Отмена",exact:true}).last().click();
   assert.equal((await sql("SELECT version FROM crm_sales_deal_records WHERE tenant='"+
     tenantSql+"' AND id='"+dealId+"'"))[0].version,2,
     "Keyboard fallback cancellation must not mutate a Deal");

   // HTML5 DnD uses the always visible, sticky drop target on long
   // columns. The board itself is one keyboard-focusable scroll region.
   const kanban=page.getByRole("region",{name:"Kanban — этапы и сделки"});
   await expect(kanban).toHaveAttribute("tabindex","0");
   const dragHandle=dealCard.getByText("Перетащить",{exact:true});
   const targetDrop=page.getByRole("group",{name:"Зона переноса в этап Квалификация"});
   await expect(targetDrop).toBeVisible();
   await dragHandle.dragTo(targetDrop);
   await page.getByRole("heading",{name:"Этап сделки: Browser Deal "+runId,exact:true}).waitFor();
   assert.equal(await page.getByLabel("Следующий этап",{exact:true}).inputValue(),
     ownStageIds[1],"Native DnD must preselect the dropped stage");
   // A real Deal+Task atomic commit with a lost HTTP acknowledgement.
   // Retry must preserve the exact binary payload, not merely its key.
   const advanceRoute="**/crm.sales.v2.DealService/AdvanceStage";
   let committedIntent;
   let originalAdvanceRequestId;
   await page.route(advanceRoute,async route=>{
     const payload=route.request().postDataBuffer();
     assert.ok(payload,"AdvanceStage must send a binary request");
     if(committedIntent){
       assert.deepEqual(payload,committedIntent,"retry must preserve the exact original intent");
       await route.continue();return;
     }
     committedIntent=payload;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"AdvanceStage");
     originalAdvanceRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Сохранить этап",exact:true}).click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(page.getByLabel("Следующий этап",{exact:true})).toBeDisabled();
   await expect(page.getByRole("button",{name:"Сохранить этап",exact:true})).toBeDisabled();
   await page.keyboard.press("Escape");
   await expect(page.getByRole("dialog")).toBeVisible();
   const pendingHash=new URL(page.url()).hash;
   await page.getByRole("navigation",{name:"Primary navigation"}).getByRole("link",{name:"Клиенты",exact:true}).click();
   await expect(page).toHaveURL(url=>url.hash===pendingHash);
   await expect(page.getByRole("dialog")).toBeVisible();
   const retryAdvance=page.waitForResponse(r=>r.url().endsWith("/AdvanceStage")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Повторить исходную операцию",exact:true}).click();
   await assertMutation(await retryAdvance,"AdvanceStage",{replay:true});
   await page.getByRole("heading",{name:/Сохранено ранее/}).waitFor();
   assert.ok(originalAdvanceRequestId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id='"+originalAdvanceRequestId+"' AND action='sales_deals.advance'"))[0].n,1);
   await page.unroute(advanceRoute);
   steps.push("S1 lost Deal acknowledgement: locked editor, exact binary retry, original audit");
   await page.getByRole("group",{name:"Итоги этапа Квалификация"})
     .getByText("90071992547409.93 USD · сделок с суммой: 1",
       {exact:true}).waitFor();
   await leadTotals.getByText("Суммы не указаны",{exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant='"+tenantSql+"' AND deal_id='"+dealId+"'"))[0].n,1);
   // Closing is independent of stage movement: retain the real stage and do
   // not create a fictitious terminal stage or a second follow-up Task.
   const actualStage=(await sql("SELECT json_extract(state,'$.stage.stage_id') AS stage FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0].stage;
   const movedDealCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Deal "+runId,exact:true})}).last();
   await movedDealCard.getByRole("button",{name:"Завершить сделку",exact:true}).click();
   await page.getByRole("heading",{name:"Завершить сделку: Browser Deal "+runId,exact:true}).waitFor();
   await page.getByLabel("Причина закрытия",{exact:true}).fill("contract-signed");
   await mutation(page.getByRole("button",{name:"Завершить сделку",exact:true}).last(),"CloseDeal");
   const terminal=(await sql("SELECT version,json_extract(state,'$.status') AS status,json_extract(state,'$.stage.stage_id') AS stage FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.deepEqual(terminal,{version:4,status:"won",stage:actualStage});
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant='"+tenantSql+"' AND deal_id='"+dealId+"'"))[0].n,2);
   await movedDealCard.getByText("Выиграна · версия 4",{exact:true}).waitFor();
   assert.equal(await movedDealCard.getByRole("button",{name:"Изменить сделку",exact:true}).count(),0);
   steps.push("CloseDeal: Won at unchanged stage, terminal read-only, D1 linked receipt");
   // The same visible editor may reorder stages already used by closed Deals.
   await page.getByRole("button",{name:"Настроить воронку",exact:true}).click();
   await page.getByRole("button",{name:"Выше",exact:true}).last().click();
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"UpdatePipeline");
   const newCatalog=JSON.parse((await sql("SELECT state FROM crm_sales_pipeline_records WHERE tenant='"+
     tenantSql+"' AND id='"+newPipelineId+"'"))[0].state);
   assert.deepEqual(newCatalog.stages.map(s=>s.stage_id),[ownStageIds[1],ownStageIds[0]]);
   const rebased=(await sql("SELECT version,json_extract(state,'$.stage.ordinal') AS rank "+
     "FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.deepEqual(rebased,{version:4,rank:1},
     "Occupied reorder must not increment Deal versions");
   await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   steps.push("Occupied stage reorder in real React keeps Deal version and stable ID");
   const currentCatalog=page.waitForResponse(r=>r.url().endsWith("/GetPipeline")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Настроить воронку",exact:true}).click();
   assert.equal((await currentCatalog).status(),200,
     "Editor must read latest authoritative Pipeline version after reorder");
   await page.getByLabel("Архивировать воронку",{exact:true}).check();
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"UpdatePipeline");
   assert.equal(JSON.parse((await sql("SELECT state FROM crm_sales_pipeline_records WHERE tenant='"+tenantSql+"' AND id='"+newPipelineId+"'"))[0].state).archived,true);
   await expect(page.getByRole("button",{name:"Новая сделка",exact:true})).toBeDisabled();
   await page.getByRole("button",{name:"Настроить воронку",exact:true}).click();
   await page.getByLabel("Архивировать воронку",{exact:true}).uncheck();
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"UpdatePipeline");
   assert.equal(JSON.parse((await sql("SELECT state FROM crm_sales_pipeline_records WHERE tenant='"+tenantSql+"' AND id='"+newPipelineId+"'"))[0].state).archived,false);
   steps.push("Pipeline archive and restore using real React, Connect and D1");
   // Actual full Deal route: typed append-only history, linked Task and
   // explicit ReopenDeal. No second editor or client-side history inference.
   await page.goto(base+"/#sales/"+encodeURIComponent(dealId));
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await page.getByText("Сделка завершена",{exact:true}).waitFor();
   await page.getByText("contract-signed",{exact:false}).first().waitFor();
   await page.getByText("Связанная задача:",{exact:false}).first().waitFor();
   const linkedTask=page.locator('section[aria-label^="История сделки"] a[href^="#tasks?"]').first();
   const linkedTaskHref=await linkedTask.getAttribute("href");
   assert.ok(linkedTaskHref?.includes("&taskId=delivery-"),"History must link the exact Task, not merely the Account task list");
   const linkedTaskId=new URLSearchParams(linkedTaskHref.slice("#tasks?".length)).get("taskId");
   assert.ok(linkedTaskId);
   await linkedTask.click();
   await page.getByRole("heading",{name:"Карточка задачи",exact:true}).waitFor();
   await page.getByText("ID: "+linkedTaskId,{exact:false}).waitFor();
   await page.getByRole("heading",{name:"Follow up deal after stage change",exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   steps.push("R7.2 linked Task opens exact original GetTask through typed route, then Back");

   await page.getByRole("button",{name:"Открыть повторно",exact:true}).last().click();
   await page.getByRole("heading",{name:"Повторно открыть сделку: Browser Deal "+runId,exact:true}).waitFor();
   await page.getByLabel("Причина повторного открытия",{exact:true}).fill("reconsidered");
   await mutation(page.getByRole("button",{name:"Открыть повторно",exact:true}).last(),"ReopenDeal");
   await page.getByText("Сделка повторно открыта",{exact:true}).waitFor();
   const reopened=(await sql("SELECT version,json_extract(state,'$.status') AS status,"+
     "json_extract(state,'$.close_outcome') AS close_outcome FROM crm_sales_deal_records "+
     "WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.equal(reopened.version,5);
   assert.equal(reopened.status,"open");
   assert.equal(reopened.close_outcome,null);
   assert.deepEqual((await sql("SELECT action FROM crm_sales_deal_events "+
     "WHERE tenant='"+tenantSql+"' AND deal_id='"+dealId+"' ORDER BY deal_version DESC LIMIT 2"))
     .map(r=>r.action),["reopened","closed"]);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records "+
     "WHERE tenant='"+tenantSql+"' AND deal_id='"+dealId+"'"))[0].n,2);
   steps.push("R7.2 CloseDeal→immutable history→ReopenDeal→D1 and linked Task");
   await page.goto(base+"/#sales");
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();

   // The live tenant may have ZERO seeded pipelines. Create the second
   // pipeline through React; never depend on CI-only pipeline.enterprise.
   await page.getByRole("button",{name:"Создать воронку",exact:true}).click();
   await page.getByLabel("Название воронки",{exact:true}).fill("Browser Alternate Pipeline "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"CreatePipeline");
   const alternatePipelineId=await firstId("crm_sales_pipeline_records","name",
     "Browser Alternate Pipeline "+runId);
   assert.ok(alternatePipelineId,"Second pipeline must be persisted");
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(alternatePipelineId);
   await page.getByRole("button",{name:"Новая сделка",exact:true}).click();
   await page.getByLabel("Название сделки",{exact:true}).fill("Browser Alternate Deal "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await page.getByLabel("Сумма",{exact:true}).fill("100.00");
   await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"CreateDeal");
   await page.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
   steps.push("Second pipeline Deal under same Account persisted in D1");
   // Full-page Deal URL and inline expansion share the same original GetDeal
   // read component. This is a real route, not a second Deal editor/store.
   await page.getByRole("link",{name:"Полная страница",exact:true}).last().click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   assert.equal((await page.evaluate(()=>location.hash)).startsWith("#sales/"),true);
   await page.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   steps.push("R7.2 addressable full Deal card via shared GetDeal and Back");
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
   const conflictHeaders=await conflictResponse.allHeaders();
   assert.equal(conflictHeaders["x-crm-error-code"],"aborted");
   const conflictRequestId=conflictHeaders["x-request-id"];
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
     // The new global Sales overview is backed by ListDeals regardless of
     // tenant pipeline selection. Stage pages are fetched only when a board
     // is explicitly selected.
     const salesBoard=screen==="sales";
     const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
     await page.getByRole("link",{name:link,exact:true}).click();
     assert.equal((await pending).status(),200);
     await page.getByRole("heading",{name:title,exact:true}).waitFor();
     assert.equal(await page.getByLabel(filter,{exact:true}).inputValue(),value);
     assert.equal(new URL(await page.url()).hash,"#"+screen+"?"+(screen==="sales"||screen==="tasks"?"accountId":"partyId")+"="+encodeURIComponent(value));
     if(salesBoard){
       assert.equal(await page.getByLabel("Воронка продаж",{exact:true}).inputValue(),"",
         "Customer360 must open the all-pipelines overview by default");
       const all=page.locator('section[aria-label="Сделки клиента во всех воронках"]');
       await all.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
       await all.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
       // Selecting a specific Kanban must not hide Account Deals in other pipelines.
       const stageRead=page.waitForResponse(r=>r.url().endsWith("/ListStageDeals")&&
         r.request().method()==="POST");
       await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
       assert.equal((await stageRead).status(),200);
       await all.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
     }
     const reloaded=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
     await page.reload();assert.equal((await reloaded).status(),200);
     assert.equal(await page.getByLabel(filter,{exact:true}).inputValue(),value,"Reload must retain the linked scope");
     if(salesBoard){
       assert.equal(await page.getByLabel("Воронка продаж",{exact:true}).inputValue(),"",
         "Reload must keep the global all-pipelines default");
       const all=page.locator('section[aria-label="Сделки клиента во всех воронках"]');
       await all.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
       await all.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
     }
     if(screen==="tasks")await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   }
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,navigationAuditBefore);
   steps.push("Customer360 links to server-filtered full lists, reload retains scope, no writes");
   await page.getByLabel("Тема",{exact:true}).selectOption("dark");
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.reload();
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await page.setViewportSize({width:390,height:844});
   await navigate("sales","Сделки");
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
   const mobileKanban=page.getByRole("region",{name:"Kanban — этапы и сделки"});
   await expect(mobileKanban).toHaveAttribute("tabindex","0");
   const scrollMetrics=await mobileKanban.evaluate(element=>({
     clientWidth:element.clientWidth,scrollWidth:element.scrollWidth,
     clientHeight:element.clientHeight,scrollHeight:element.scrollHeight,
     viewportWidth:window.innerWidth,
     documentWidth:document.documentElement.scrollWidth
   }));
   assert.ok(scrollMetrics.viewportWidth<=391&&scrollMetrics.documentWidth<=391,
     "the mobile page itself must remain at 390 px: "+JSON.stringify(scrollMetrics));
   assert.ok(scrollMetrics.scrollWidth>scrollMetrics.clientWidth,
     "mobile Kanban must scroll horizontally inside its named region: "+JSON.stringify(scrollMetrics));
   await mobileKanban.focus();
   assert.equal(await mobileKanban.evaluate(element=>element===document.activeElement),true,
     "mobile Kanban scroll region must be keyboard focusable");
   await page.keyboard.press("ArrowRight");
   await expect.poll(()=>mobileKanban.evaluate(element=>element.scrollLeft))
     .toBeGreaterThan(0);
   await page.screenshot({path:resolve(directory,"crm-mobile-dark.png"),fullPage:true});
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1),"mobile horizontal overflow");
   steps.push("Mobile Kanban: named keyboard-focusable horizontal scroll region without viewport overflow");
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

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
 const requests=[],writes=[],recovery=[],ownerRecovery=[],syntheticPrivateProbes=[],pageErrors=[],steps=[];
 let lastCheckpointAt=startedAt;
 const checkpoint=label=>{
   const now=Date.now();
   steps.push(label);
   // Same evidence for local workerd and live Chromium: timings and scenario
   // labels only, never credentials, fixture payloads, tenant IDs or D1 data.
   console.log(JSON.stringify({event:"crm.browser-checkpoint",label,
     span_ms:now-lastCheckpointAt,elapsed_ms:now-startedAt}));
   lastCheckpointAt=now;
 };
 const headerReads=[];
 let restoreViewer=async()=>{};
 const recordResponse=response=>{
   const path=new URL(response.url()).pathname;
   if(!path.startsWith("/crm."))return;
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
 // Match the RPC pathname, not the serialized URL: privacy probes deliberately add query parameters.
 const responsePathEndsWith=(response,suffix)=>new URL(response.url()).pathname.endsWith(suffix);
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

 // Only browser document/assets, never authenticated response bodies or headers.
 // After a protected D1 migration, Workers.dev can briefly serve its own HTML
 // 404 before the static asset route has propagated to every edge.
 const shellResponses=[];
 page.on("response",response=>{
   const kind=response.request().resourceType();
   if(!["document","script","stylesheet"].includes(kind))return;
   const url=new URL(response.url());
   if(url.origin!==new URL(base).origin)return;
   const headers=response.headers();
   shellResponses.push({kind,path:url.pathname,status:response.status(),
     fromWorker:Boolean(headers["x-crm-runtime"])});
   if(shellResponses.length>40)shellResponses.shift();
 });
 let firstBrowserNavigation=true;
 const navigate=async(hash,title)=>{
   const first=firstBrowserNavigation;
   const limit=first?6:1;
   for(let attempt=0;attempt<limit;attempt++){
     const offset=shellResponses.length;
     // Mount the document once, then navigate through the real SPA hash router.
     // Repeated page.goto() re-downloaded the shell and assets for every module.
     const response=first?await page.goto(base+"/#"+hash):
       await page.evaluate(next=>{window.location.hash="#"+next;return null;},hash);
     try {
       await page.getByRole("heading",{name:title,exact:true})
         .waitFor({timeout:first?8000:30000});
       firstBrowserNavigation=false;
       return;
     }catch(error){
       const shell=shellResponses.slice(offset);
       const edgeOnly404=shell.some(x=>x.status===404&&!x.fromWorker);
       if(first&&edgeOnly404&&attempt+1<limit){
         await new Promise(resolve=>setTimeout(resolve,1500));
         continue;
       }
       // Fail closed on application errors, a missing React root, or a Worker
       // 404. The only recoverable case above is independently observed
       // edge-owned 404 on initial document or static-asset GET.
       const documentState=await page.evaluate(()=>({
         path:location.pathname,hash:location.hash,
         readyState:document.readyState,
         hasReactRoot:Boolean(document.getElementById("root")),
         hasRoute:Boolean(document.querySelector("[data-crm-route-kind]"))
       })).catch(()=>({unavailable:"navigation-in-progress"}));
       throw new Error("CRM_BROWSER_ROUTE_START_FAILED "+JSON.stringify({
         expected:title,first,attempt:attempt+1,
         documentStatus:response?.status()??null,
         shell,documentState
       }),{cause:error});
     }
   }
 };
 // Read-only SPA reloads have the same independent Workers.dev edge-asset
 // propagation hazard as first navigation. Retry *only* observed edge-owned
 // HTML/JS/CSS 404; never re-run a mutation, retry a Worker error, or conceal
 // failed React rendering behind a generic long timeout.
 const reloadDocument=async()=>{
   for(let attempt=1;attempt<=5;attempt++){
     const offset=shellResponses.length;
     const response=await page.reload();
     const shell=shellResponses.slice(offset);
     // The navigation response is authoritative even if Chromium did not
     // dispatch a separate "response" event to the shell observer.
     if(response) {
       const doc={kind:"document",path:new URL(response.url()).pathname,
         status:response.status(),fromWorker:Boolean(response.headers()["x-crm-runtime"])};
       if(!shell.some(item=>item.kind==="document"&&item.path===doc.path&&
         item.status===doc.status))shell.unshift(doc);
     }
     const invalid=shell.filter(item=>item.status>=400&&
       (item.status!==404||item.fromWorker));
     if(invalid.length)
       throw Error("CRM_BROWSER_RELOAD_HTTP_FAILED "+JSON.stringify({attempt,shell}));
     if(!shell.some(item=>item.status===404&&!item.fromWorker)){
       // A 200 document without a mounted React shell is an application
       // failure, not an edge 404. Fail early with bounded safe diagnostics.
       try {await page.locator(".crm-skip-link").waitFor({state:"attached",timeout:10000});}
       catch(error){
         const documentState=await page.evaluate(()=>({
           path:location.pathname,hash:location.hash,
           hasReactRoot:Boolean(document.getElementById("root")),
           hasRoute:Boolean(document.querySelector("[data-crm-route-kind]"))
         })).catch(()=>({unavailable:true}));
         throw Error("CRM_BROWSER_RELOAD_RENDER_FAILED "+JSON.stringify({
           attempt,documentStatus:response?.status()??null,
           shell,documentState,pageErrors:pageErrors.slice(-3)
         }),{cause:error});
       }
       if(attempt>1)checkpoint("Read-only SPA edge 404 recovered after "+attempt+" document requests");
       return response;
     }
     if(attempt===5)
       throw Error("CRM_BROWSER_RELOAD_EDGE_404_EXHAUSTED "+JSON.stringify({attempt,shell}));
     await new Promise(resolve=>setTimeout(resolve,1500));
   }
 };
 const search=async(label,value,method)=>{
   const input=page.getByLabel(label,{exact:true});
   await input.fill(value);
   const pending=page.waitForResponse(r=>responsePathEndsWith(r,"/"+method)&&r.request().method()==="POST");
   await input.press("Enter");
   const response=await pending;
   assert.equal(response.status(),200);
   assert.equal((await response.headers())["x-crm-source-sha"],sourceSha);
 };
 const assertMutation=async(response,method,{replay=false}={})=>{
   assert.equal(response.status(),200,method+" "+await response.text());
   const headers=typeof response.allHeaders==="function"?await response.headers():response.headers();
   const requestId=headers["x-request-id"];
   assert.match(requestId??"",/^[a-f0-9-]{36}$/i);
   const audit=await sql("SELECT action,subject_id,actor FROM crm_audit WHERE tenant_id=? AND request_id=?",tenantA,requestId);
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
   const pending=page.waitForResponse(r=>responsePathEndsWith(r,"/"+method)&&r.request().method()==="POST");
   await button.click();const response=await pending;
   await assertMutation(response,method,options);
   await page.getByRole("heading",{name:/^Сохранено/}).last().waitFor();
 };
 // One tenant-scoped D1 read proves the original audit and the absence
 // of any follow-up audit. Keep both exact-count checks for every owner.
 const assertAuditRecovery=async(originalId,readId,action,owner)=>{
   for(const id of [originalId,readId])
     assert.match(id??"",/^[a-f0-9-]{36}$/i,owner+" must have a signed request ID");
   assert.notEqual(originalId,readId,owner+" recovery must be a distinct request");
   assert.match(action,/^[a-z_]+\.[a-z_]+$/,owner+" must use a known audit action");
   const rows=await sql("SELECT request_id,action FROM crm_audit WHERE tenant_id=? AND request_id IN (?,?)",
     tenantA,originalId,readId);
   assert.equal(rows.filter(row=>row.request_id===originalId&&row.action===action).length,
     1,owner+" original commit must have one matching audit action");
   assert.equal(rows.filter(row=>row.request_id===readId).length,
     0,owner+" recovery request must not append audit");
 };
 const reloadAfterPending=async()=>{
   const accept=dialog=>void dialog.accept();
   page.on("dialog",accept);
   try{await reloadDocument();}finally{page.off("dialog",accept);}
 };
 const recoverOwnerWrite=async({owner,writeRoute,method,readSuffix,storageKey,
   auditAction,heading,trigger})=>{
   let calls=0,originalRequestId;
   const loseAcknowledgement=async route=>{
     calls++;
     const committed=await route.fetch();
     recordResponse(committed);await assertMutation(committed,method);
     originalRequestId=(await committed.headers())["x-request-id"];
     await route.abort("failed");
   };
   await page.route(writeRoute,loseAcknowledgement);
   await trigger();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).last().waitFor();
   const marker=await page.evaluate(key=>JSON.parse(sessionStorage.getItem(key)),storageKey);
   assert.deepEqual(Object.keys(marker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(marker.expectedVersion,"0");
   assert.match(marker.key,/^[a-f0-9-]{36}$/i);
   await reloadAfterPending();
   await page.getByRole("heading",{name:heading,exact:true}).waitFor();
   const check=page.getByRole("button",{name:"Проверить результат записи",exact:true}).last();
   await check.waitFor();
   const pending=page.waitForResponse(response=>
     responsePathEndsWith(response,readSuffix)&&response.request().method()==="POST");
   await check.click();
   const readResponse=await pending;
   assert.equal(readResponse.status(),200,owner+" receipt lookup");
   const readRequestId=(await readResponse.headers())["x-request-id"];
   assert.match(readRequestId??"",/^[a-f0-9-]{36}$/i);
   assert.ok(originalRequestId,owner+" original request id");
   assert.equal(calls,1,owner+" recovery must not re-execute the original write");
   await expect.poll(()=>page.evaluate(key=>sessionStorage.getItem(key),storageKey)).toBe(null);
   await assertAuditRecovery(originalRequestId,readRequestId,auditAction,owner);
   ownerRecovery.push({owner,kind:"lookup",original_request_id:originalRequestId,
     subsequent_request_id:readRequestId});
   await page.unroute(writeRoute,loseAcknowledgement);
   checkpoint("S5 "+owner+" lost acknowledgement: opaque marker, immutable receipt lookup, one D1 audit");
   return marker.resourceId;
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
     responsePathEndsWith(response,method)&&response.request().method()==="POST");
   await next.click();
   assert.equal((await pending).status(),200,label+" pagination RPC failed");
   await picker.getByRole("status").waitFor({state:"hidden"});
  }
  throw Error(label+" lookup exceeded 30 pages for "+id);
 };
 // Browser contract: UI/typed Connect proves visible business state;
 // D1 is read directly only for generated IDs and independent persistence,
 // audit, receipt, rollback or no-write invariants not proven by the UI.
 const firstId=async(table,column,value)=>{
   const rows=await sql("SELECT id FROM "+table+" WHERE tenant=? AND json_extract(state,?)=?",
     tenantA,"$."+column,value);
   assert.equal(rows.length,1);return rows[0].id;
 };
 const runId=Date.now().toString();
 let partyId,accountId;
 try {
   await navigate("parties","Люди и организации");
   // Use only synthetic markers; signed browser sends a real Cookie and a
   // single request header into the original Rust Worker, never real secrets.
   const privateCookie="crm-s3-cookie-"+runId;
   const privateHeader="crm-s3-header-"+runId;
   const privateAuthorization="Bearer crm-s3-authorization-"+runId;
   const privateJwt="eyJhbGciOiJub25lIn0.eyJzdWIiOiJjcm0tc3ludGhldGljIn0.signature-"+runId;
   const privateQuery="crm-s3-query-"+runId;
   await context.addCookies([{name:"crm_s3_test_cookie",value:privateCookie,url:base}]);
   let privateHeaderRequests=0;
   const privateRoute=async route=>{
     const headers=await route.request().headers();
     assert.ok((headers.cookie??"").includes("crm_s3_test_cookie="+privateCookie),
       "Synthetic test cookie must be present in the real signed request");
     privateHeaderRequests++;
     const url=new URL(route.request().url());
     url.searchParams.set("crm_s3_test",privateQuery);
     await route.continue({url:url.toString(),headers:{
       ...headers,"x-crm-s3-test-probe":privateHeader,
       authorization:privateAuthorization,"x-crm-s3-test-jwt":privateJwt
     }});
   };
   const createPartyRoute="**/crm.parties.v3.PartyService/CreateParty";
   await page.route(createPartyRoute,privateRoute);
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Party "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   await page.unroute(createPartyRoute,privateRoute);
   assert.equal(privateHeaderRequests,1,
     "Synthetic header must accompany exactly one real signed mutation");
   await context.clearCookies({name:"crm_s3_test_cookie"});
   const firstMutationId=writes.at(-1)?.request_id;
   syntheticPrivateProbes.push(
     {kind:"party-name",request_id:firstMutationId,value:"Browser Party "+runId},
     {kind:"cookie",request_id:firstMutationId,value:privateCookie},
     {kind:"header",request_id:firstMutationId,value:privateHeader},
     {kind:"authorization",request_id:firstMutationId,value:privateAuthorization},
     {kind:"jwt",request_id:firstMutationId,value:privateJwt},
     {kind:"query-string",request_id:firstMutationId,value:privateQuery});
   partyId=await firstId("crm_party_records","display_name","Browser Party "+runId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+partyId+"'"))[0].n,1);
   await reloadDocument();
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
       const pending=page.waitForResponse(r=>responsePathEndsWith(r,"/ListParties")&&
         r.request().method()==="POST");
       await page.getByRole("button",{name:"Повторить чтение",exact:true}).click();
       const response=await pending;
       const headers=await response.headers();
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
   if(edgeReadRecoveries)checkpoint("Cloudflare edge-only read 404 recovered through typed UI Retry");
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
   checkpoint("Party create/edit/reload and D1 version/audit");
   // S5a: original Party commit happens, its HTTP acknowledgement is lost,
   // and a fresh React instance MUST use GetParty + typed D1 receipt, not
   // blindly POST another CreateParty with a newly generated key.
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   const s5aPartyName="S5a Recovery "+runId;
   await page.getByLabel("Имя или название",{exact:true}).fill(s5aPartyName);
   const s5aRoute="**/crm.parties.v3.PartyService/CreateParty";
   let s5aOriginalRequestId, s5aCreateCalls=0;
   const loseS5aAcknowledgement=async route=>{
     s5aCreateCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"CreateParty");
     s5aOriginalRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   };
   await page.route(s5aRoute,loseS5aAcknowledgement);
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await page.getByRole("dialog").getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   const s5aPartyId=await firstId("crm_party_records","display_name",s5aPartyName);
   const s5aMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.party.pending.v1")));
   assert.deepEqual(Object.keys(s5aMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(s5aMarker.resourceId,s5aPartyId);
   assert.equal(s5aMarker.expectedVersion,"0");
   await expect(page.getByRole("button",{name:"Сохранить участника",exact:true})).toBeDisabled();
   const acceptS5aReload=dialog=>void dialog.accept();
   page.on("dialog",acceptS5aReload);
   try {await reloadDocument();} finally {page.off("dialog",acceptS5aReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   await expect(page.getByRole("button",{name:"Создать участника",exact:true})).toBeDisabled();
   const s5aGetRoute="**/crm.parties.v3.PartyService/GetParty";
   const failS5aRead=route=>route.abort("failed");
   await page.route(s5aGetRoute,failS5aRead);
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   await expect(page.getByRole("button",{name:"Проверить результат записи",exact:true})).toBeEnabled();
   assert.deepEqual(await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.party.pending.v1"))),s5aMarker);
   await page.unroute(s5aGetRoute,failS5aRead);
   const s5aProofRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetParty")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const s5aChecked=await s5aProofRead;
   assert.equal(s5aChecked.status(),200);
   const s5aReadRequestId=(await s5aChecked.headers())["x-request-id"];
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(s5aOriginalRequestId);
   assert.equal(s5aCreateCalls,1,"Recovery must not retry the original Party mutation after reload");
   await assertAuditRecovery(s5aOriginalRequestId,s5aReadRequestId,"parties.create","party");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.party.pending.v1")),null);
   ownerRecovery.push({owner:"party",kind:"lookup",original_request_id:s5aOriginalRequestId,
     subsequent_request_id:s5aReadRequestId});
   await page.unroute(s5aRoute,loseS5aAcknowledgement);
   checkpoint("S5a lost Party acknowledgment: opaque reload marker, signed GetParty receipt, no second commit");

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
   await recoverOwnerWrite({
     owner:"account",writeRoute:"**/CreateAccount",method:"CreateAccount",
     readSuffix:"/GetAccount",storageKey:"crm.account.create.pending.v1",
     auditAction:"accounts.create",heading:"Новый клиент",
     trigger:()=>page.getByRole("button",{name:"Сохранить клиента",exact:true}).click()
   });
   const linkedAccountId=await firstId("crm_account_records","name","Browser Party Account "+runId);
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,"#accounts/"+linkedAccountId,
     "accepted CreateAccount must replace draft URL with a permalink");
   checkpoint("Party -> shared CreateAccount: selected Party, dirty Cancel/Back and permalink");
   await navigate("relationships","Связи");
   await page.getByRole("button",{name:"Создать связь",exact:true}).click();
   await selectPagedPicker(page,"Первый участник",partyId);
   await selectPagedPicker(page,"Второй участник",otherPartyId);
   await page.getByLabel("Код отношения",{exact:true}).fill("browser_relation_"+runId);
   await recoverOwnerWrite({
     owner:"relationship",writeRoute:"**/CreatePartyRelationship",method:"CreatePartyRelationship",
     readSuffix:"/GetPartyRelationship",storageKey:"crm.relationship.pending.v1",
     auditAction:"party_relationships.create",heading:"Связи",
     trigger:()=>page.getByRole("button",{name:"Сохранить связь",exact:true}).click()
   });
   await page.locator("section").filter({has:page.getByRole("heading",{name:"browser_relation_"+runId,exact:true})})
     .getByRole("button",{name:"Редактировать связь",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить связь",exact:true}).waitFor();
   await page.getByLabel("Статус",{exact:true}).selectOption("2");
   await mutation(page.getByRole("button",{name:"Сохранить связь",exact:true}),"UpdatePartyRelationship");
   const relationship=(await sql("SELECT version,json_extract(state,'$.status') AS status FROM crm_party_relationship_records WHERE tenant='"+tenantSql+"' AND from_party='"+partyId+"' AND to_party='"+otherPartyId+"'"))[0];
   assert.equal(relationship.version,2);assert.equal(relationship.status,"inactive");
   checkpoint("Relationship create/edit through original owner with D1 version/status");
   const relationFilterAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   // Other Parties can have active relationships in this tenant.
   // Filter the *specific* updated record; do not demand an empty global list.
   const relationHeading=page.getByRole("heading",{name:"browser_relation_"+runId,exact:true});
   const activeRelationRead=page.waitForResponse(r=>responsePathEndsWith(r,"/ListPartyRelationships")&&
     r.request().method()==="POST");
   await page.getByLabel("Статус связей",{exact:true}).selectOption("1");
   assert.equal((await activeRelationRead).status(),200);
   await relationHeading.waitFor({state:"hidden"});
   const inactiveRelationRead=page.waitForResponse(r=>responsePathEndsWith(r,"/ListPartyRelationships")&&
     r.request().method()==="POST");
   await page.getByLabel("Статус связей",{exact:true}).selectOption("2");
   assert.equal((await inactiveRelationRead).status(),200);
   await relationHeading.waitFor();
   await page.getByLabel("Статус связей",{exact:true}).selectOption("");
   await page.getByRole("heading",{name:"browser_relation_"+runId,exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     relationFilterAuditBefore,"Relationship list filtering never writes audit");
   checkpoint("M1-05 real status-filtered Relationship list preserves Party and D1 audit");
   // S-M1-A: the relationship must show authorized names, and its Party
   // permalink must load GetParty directly, not search the text index by ID.
   const namedRelationship=page.locator("section").filter({has:page.getByRole("heading",
     {name:"browser_relation_"+runId,exact:true})});
   await namedRelationship.getByRole("link",{name:"Browser Updated "+runId,exact:true}).waitFor();
   const linkedParty=namedRelationship.getByRole("link",{name:"Browser Other "+runId,exact:true});
   await linkedParty.waitFor();
   const directPartyRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetParty")&&
     r.request().method()==="POST");
   await linkedParty.click();
   assert.equal((await directPartyRead).status(),200);
   assert.equal(new URL(await page.url()).hash,"#parties?partyId="+encodeURIComponent(otherPartyId));
   await page.getByRole("heading",{name:"Browser Other "+runId,exact:true}).waitFor();
   await reloadDocument();
   await page.getByRole("heading",{name:"Browser Other "+runId,exact:true}).waitFor();
   await page.getByRole("link",{name:"К списку участников",exact:true}).click();
   await page.getByRole("heading",{name:"Люди и организации",exact:true}).waitFor();
   checkpoint("S-M1-A relationship names and direct Party Get permalink/reload");
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
   const overviewResponse=page.waitForResponse(r=>responsePathEndsWith(r,"/GetAccountOverview"));
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
   await reloadDocument();
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
   await page.getByRole("button",{name:"К списку клиентов",exact:true}).click();
   await page.getByRole("heading",{name:"Клиенты",exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,listHash,"Back must restore search, status and cursor");
   // Hash navigation and React reconciliation are asynchronous: assert the settled
   // control value, not a transient value in the first paint after popstate.
   await expect(page.locator('[data-crm-route-kind="accounts"]')).toHaveAttribute("data-crm-account-status","2");
   await expect(page.getByLabel("Статус клиента",{exact:true})).toHaveValue("2",{timeout:10000});
   await expect(page.getByLabel("Поиск клиентов",{exact:true})).toHaveValue(accountId,{timeout:10000});
   await reloadDocument();
   await expect(page.locator('[data-crm-route-kind="accounts"]')).toHaveAttribute("data-crm-account-status","2");
   await expect(page.getByLabel("Статус клиента",{exact:true})).toHaveValue("2",{timeout:10000});
   await expect(page.getByLabel("Поиск клиентов",{exact:true})).toHaveValue(accountId,{timeout:10000});
   await page.getByLabel("Статус клиента",{exact:true}).selectOption("1");
   await page.getByRole("heading",{name:"Browser Account "+runId,exact:true}).waitFor();
   checkpoint("Account one editor: inline, full-page, reload, mutation, Back and filter restoration");
   await page.goto(base+"/#accounts?status=999");
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   checkpoint("Account URL filter rejects unknown generated enum values");
   checkpoint("Account create and Customer360 read");
   checkpoint("Party/Account literal search by name and ID, empty result and read-only D1");
   await navigate("contacts","Контакты");
   await page.getByRole("button",{name:"Создать контакт",exact:true}).click();
   await selectPagedPicker(page,"Участник",partyId);
   await page.getByLabel("Значение контакта",{exact:true}).fill("browser-"+runId+"@example.org");
   await recoverOwnerWrite({
     owner:"contact-point",writeRoute:"**/CreateContactPoint",method:"CreateContactPoint",
     readSuffix:"/GetContactPoint",storageKey:"crm.contact.pending.v1",
     auditAction:"contact_points.create",heading:"Контакты",
     trigger:()=>page.getByRole("button",{name:"Сохранить контакт",exact:true}).click()
   });
   syntheticPrivateProbes.push({kind:"contact-email",request_id:writes.at(-1)?.request_id,
     value:"browser-"+runId+"@example.org"});
   const browserSalesContactId=await firstId("crm_contact_point_records","normalized_value",
     "browser-"+runId+"@example.org");
   await page.locator("section").filter({has:page.getByRole("heading",{name:"browser-"+runId+"@example.org",exact:true})})
     .getByRole("button",{name:"Редактировать контакт",exact:true}).click();
   await page.getByRole("heading",{name:"Изменить контакт",exact:true}).waitFor();
   await page.getByLabel("Предпочтительный",{exact:true}).check();
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"UpdateContactPoint");
   // Filter by the known user-facing name, never by copying the Party ID.
   await search("Поиск: Фильтр по участнику","Browser Updated "+runId,"ListParties");
   await page.getByLabel("Фильтр по участнику",{exact:true})
     .locator('option[value="'+partyId+'"]').waitFor({state:"attached"});
   await page.getByLabel("Фильтр по участнику",{exact:true}).selectOption(partyId);
   await page.getByRole("heading",{name:"browser-"+runId+"@example.org",exact:true}).waitFor();
   const contactCard=page.locator("section").filter({has:page.getByRole("heading",
     {name:"browser-"+runId+"@example.org",exact:true})});
   await contactCard.getByRole("link",{name:"Browser Updated "+runId,exact:true}).waitFor();
   checkpoint("S-M1-A named Contact filter and authorized Party link");
   // Use the owner-tested fictional NANP 555-0102 fixture, not the UK drama range.
   const privatePhone="+12025550102";
   await page.getByRole("button",{name:"Создать контакт",exact:true}).click();
   await selectPagedPicker(page,"Участник",partyId);
   await page.getByLabel("Тип контакта",{exact:true}).selectOption("2");
   await page.getByLabel("Значение контакта",{exact:true}).fill(privatePhone);
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"CreateContactPoint");
   syntheticPrivateProbes.push({kind:"contact-phone",request_id:writes.at(-1)?.request_id,value:privatePhone});
   const contactFilterAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await page.getByLabel("Способ связи",{exact:true}).selectOption("2");
   await page.getByRole("heading",{name:privatePhone,exact:true}).waitFor();
   await page.getByRole("heading",{name:"browser-"+runId+"@example.org",exact:true})
     .waitFor({state:"hidden"});
   await page.getByLabel("Статус контактов",{exact:true}).selectOption("2");
   await page.getByText("Контактов пока нет.",{exact:true}).waitFor();
   await page.getByLabel("Статус контактов",{exact:true}).selectOption("1");
   await page.getByRole("heading",{name:privatePhone,exact:true}).waitFor();
   await page.getByLabel("Способ связи",{exact:true}).selectOption("");
   await page.getByRole("heading",{name:"browser-"+runId+"@example.org",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     contactFilterAuditBefore,"Contact filter requests must not write audit");
   checkpoint("M1-04 typed Contact kind+status filters before page limit, Party scope unchanged");
   // M1-06: Account360 must link straight to the exact owned ContactPoint
   // and either end of a Relationship, never to a 25-row list page where
   // the record may not be visible. Direct Get checks both tenant and Party.
   const linkedRelationshipRows=await sql("SELECT id FROM crm_party_relationship_records "+
     "WHERE tenant='"+tenantSql+"' AND from_party='"+partyId+"' AND to_party='"+otherPartyId+"'");
   assert.equal(linkedRelationshipRows.length,1);
   const linkedRelationshipId=linkedRelationshipRows[0].id;
   const overviewNavigationAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await navigate("accounts/"+accountId,"Карточка клиента");
   const emailFromAccount=page.getByRole("link",{name:"browser-"+runId+"@example.org",exact:true});
   await emailFromAccount.waitFor();
   assert.equal(await emailFromAccount.getAttribute("href"),
     "#contacts?partyId="+partyId+"&contactPointId="+browserSalesContactId);
   const contactFromOverviewRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetContactPoint")&&
     r.request().method()==="POST");
   await emailFromAccount.click();
   assert.equal((await contactFromOverviewRead).status(),200);
   await page.getByRole("heading",{name:"Карточка контакта",exact:true}).waitFor();
   await page.getByText("ID контакта: "+browserSalesContactId,{exact:true}).waitFor();
   assert.equal(new URL(await page.url()).hash,
     "#contacts?partyId="+partyId+"&contactPointId="+browserSalesContactId);
   await reloadDocument();
   await page.getByRole("heading",{name:"Карточка контакта",exact:true}).waitFor();
   await page.getByText("ID контакта: "+browserSalesContactId,{exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   const relationshipFromAccount=page.getByRole("link",{name:"browser_relation_"+runId,exact:true});
   await relationshipFromAccount.waitFor();
   assert.equal(await relationshipFromAccount.getAttribute("href"),
     "#relationships?partyId="+partyId+"&relationshipId="+linkedRelationshipId);
   const relationshipFromOverviewRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetPartyRelationship")&&
     r.request().method()==="POST");
   await relationshipFromAccount.click();
   assert.equal((await relationshipFromOverviewRead).status(),200);
   await page.getByRole("heading",{name:"Карточка связи",exact:true}).waitFor();
   await page.getByRole("heading",{name:"browser_relation_"+runId,exact:true}).waitFor();
   await page.getByText(/Неактивна · с/).waitFor();
   assert.equal(new URL(await page.url()).hash,
     "#relationships?partyId="+partyId+"&relationshipId="+linkedRelationshipId);
   await reloadDocument();
   await page.getByRole("heading",{name:"Карточка связи",exact:true}).waitFor();
   await page.getByRole("heading",{name:"browser_relation_"+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   checkpoint("M1-06 Account360 direct Contact/Relationship Get, exact named links, reload and Back");
   // A real contact for another Party (or a real relation between other
   // Parties) must never be rendered as belonging to the scoped Party.
   await navigate("contacts?partyId="+otherPartyId+"&contactPointId="+browserSalesContactId,
     "Карточка контакта");
   await page.getByRole("heading",{name:/Контакт не принадлежит выбранному участнику/}).waitFor();
   await expect(page.getByRole("heading",{name:"browser-"+runId+"@example.org",exact:true}))
     .toHaveCount(0);
   await navigate("relationships?partyId="+accountId+"&relationshipId="+linkedRelationshipId,
     "Карточка связи");
   await page.getByRole("heading",{name:/Связь не относится к выбранному участнику/}).waitFor();
   await expect(page.getByRole("heading",{name:"browser_relation_"+runId,exact:true}))
     .toHaveCount(0);
   await navigate("contacts?partyId="+partyId+"&contactPointId=bad%2Fid","Страница не найдена");
   await navigate("relationships?partyId="+partyId+"&relationshipId="+linkedRelationshipId+
     "&relationshipId="+linkedRelationshipId,"Страница не найдена");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     overviewNavigationAudit,"Account360 and focused Get navigation may never write audit");
   checkpoint("M1-06 wrong-party, malformed and duplicate routes fail closed, no write");
   await navigate("tasks","Задачи");
   await page.getByRole("button",{name:"Новая задача",exact:true}).click();
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Task "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await recoverOwnerWrite({
     owner:"task",writeRoute:"**/CreateTask",method:"CreateTask",
     readSuffix:"/GetTask",storageKey:"crm.task.pending.v1",
     auditAction:"activity_tasks.create",heading:"Задачи",
     trigger:()=>page.getByRole("button",{name:"Создать задачу",exact:true}).click()
   });
   syntheticPrivateProbes.push({kind:"task-subject",request_id:writes.at(-1)?.request_id,
     value:"Browser Task "+runId});
   const taskId=await firstId("crm_activity_task_records","subject","Browser Task "+runId);
   const taskCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Task "+runId,exact:true})});
   // M1-08: signed React uses the existing original Task owner to set,
   // reschedule and no-op an existing Task reminder (not a second scheduler).
   await taskCard.getByRole("button",{name:"Назначить напоминание",exact:true}).click();
   await page.getByRole("heading",{name:"Напоминание задачи",exact:true}).waitFor();
   await page.getByLabel("Время напоминания",{exact:true}).fill("2030-06-01T12:00");
   await mutation(page.getByRole("button",{name:"Сохранить напоминание",exact:true}),"ScheduleReminder");
   assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+taskId+"'"))[0].version,2);
   const scheduledAudits=async()=>(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+
     "' AND subject_id='"+taskId+"' AND action='activity_tasks.reminder_scheduled'"))[0].n;
   assert.equal(await scheduledAudits(),1,"one real schedule receipt and audit");
   await taskCard.getByRole("button",{name:"Изменить напоминание",exact:true}).click();
   await page.getByLabel("Время напоминания",{exact:true}).fill("2030-06-02T12:00");
   await mutation(page.getByRole("button",{name:"Сохранить напоминание",exact:true}),"ScheduleReminder");
   assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+taskId+"'"))[0].version,3);
   assert.equal(await scheduledAudits(),2,"rescheduling writes exactly one new audit");
   await taskCard.getByRole("button",{name:"Изменить напоминание",exact:true}).click();
   await page.getByLabel("Время напоминания",{exact:true}).fill("2030-06-02T12:00");
   await page.getByRole("button",{name:"Сохранить напоминание",exact:true}).click();
   await page.getByRole("heading",{name:/Изменений не было/}).waitFor();
   assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+taskId+"'"))[0].version,3);
   assert.equal(await scheduledAudits(),2,"same-time no-op has no D1 audit or receipt");
   await reloadDocument();
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   await taskCard.getByText(/Напоминание:.*2030/).waitFor();
   checkpoint("M1-08 original Task ScheduleReminder set/reschedule/no-op/reload and D1 audit");
   await mutation(taskCard.getByRole("button",{name:"Выполнить задачу",exact:true}),"CompleteTask");
   assert.equal((await sql("SELECT json_extract(state,'$.status') AS status FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+taskId+"'"))[0].status,"completed");
   await search("Поиск: Фильтр по клиенту","Browser Account "+runId,"ListAccounts");
   await page.getByLabel("Фильтр по клиенту",{exact:true})
     .locator('option[value="'+accountId+'"]').waitFor({state:"attached"});
   await page.getByLabel("Фильтр по клиенту",{exact:true}).selectOption(accountId);
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   const taskAccount=page.locator("section").filter({has:page.getByRole("heading",
     {name:"Browser Task "+runId,exact:true})});
   await taskAccount.getByRole("link",{name:"Browser Account "+runId,exact:true}).waitFor();
   checkpoint("S-M1-A named Task Account filter and detail link");
   await page.getByRole("button",{name:"Новая задача",exact:true}).click();
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Urgent "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await page.getByLabel("Приоритет",{exact:true}).selectOption("4");
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const urgentId=await firstId("crm_activity_task_records","subject","Browser Urgent "+runId);
   const beforeTaskFilters=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   // Find a real Task by subject without manually paging through opaque IDs.
   await page.getByLabel("Поиск задач",{exact:true}).fill("Browser Urgent "+runId);
   await page.getByRole("button",{name:"Найти задачу по названию",exact:true}).click();
   assert.ok(new URL(page.url()).hash.includes("search="),"Task search owns the native URL");
   await page.getByRole("heading",{name:"Browser Urgent "+runId,exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor({state:"hidden"});
   await reloadDocument();
   await page.getByRole("heading",{name:"Browser Urgent "+runId,exact:true}).waitFor();
   await page.getByRole("button",{name:"Сбросить поиск задач",exact:true}).click();
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   const filterTasks=async(label,value)=>{
     const response=page.waitForResponse(r=>responsePathEndsWith(r,"/ListTasks")&&r.request().method()==="POST");
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
   const rowAccountReads=[];
   const trackAccountRead=request=>{
     if(new URL(request.url()).pathname.endsWith("/GetAccount"))rowAccountReads.push(request.url());
   };
   page.on("request",trackAccountRead);
   await filterTasks("Статус задачи","");
   await filterTasks("Приоритет задач","");
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Urgent "+runId,exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,beforeTaskFilters);
   await expect(taskCard.locator('a[href="#accounts/'+accountId+'"]')).toHaveText(/.+/);
   page.off("request",trackAccountRead);
   assert.equal(rowAccountReads.length,0,"rendered task lists must use the account labels from ListTasks");
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
   await reloadDocument();
   await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   await filterTasks("Приоритет задач","4");
   await page.getByText("Задач пока нет.",{exact:true}).waitFor();
   await filterTasks("Приоритет задач","3");
   await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   await filterTasks("Приоритет задач","");
   checkpoint("Task UpdateTask via React, D1 audit action, priority filter and reload");
   // An actual salesperson must find and resolve overdue work without paging
   // across technical Task IDs; all reads use the existing signed owner.
   await page.getByRole("button",{name:"Новая задача",exact:true}).click();
   await page.getByLabel("Тема задачи",{exact:true}).fill("Browser Overdue "+runId);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await page.getByLabel("Срок",{exact:true}).fill("2020-01-01T12:00");
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const overdueId=await firstId("crm_activity_task_records","subject","Browser Overdue "+runId);
   const overdueCard=page.locator("section").filter({has:page.getByRole("heading",{
     name:"Browser Overdue "+runId,exact:true})});
   await overdueCard.getByText("Просрочена",{exact:true}).waitFor();
   const overdueReadAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await page.getByLabel("Только просроченные задачи",{exact:true}).check();
   assert.equal(new URL(page.url()).hash,"#tasks?accountId="+accountId+"&overdue=1");
   await page.getByRole("heading",{name:"Browser Overdue "+runId,exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor({state:"hidden"});
   await reloadDocument();
   await expect(page.getByLabel("Только просроченные задачи",{exact:true})).toBeChecked();
   await overdueCard.getByRole("link",{name:"Открыть карточку задачи",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка задачи",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Overdue "+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Browser Overdue "+runId,exact:true}).waitFor();
   assert.equal(new URL(page.url()).hash,"#tasks?accountId="+accountId+"&overdue=1");
   await navigate("tasks?accountId=missing-account&overdue=1","Задачи");
   await page.getByText("Просроченных задач нет.",{exact:true}).waitFor();
   await navigate("tasks?accountId="+accountId+"&overdue=1","Задачи");
   await page.getByRole("heading",{name:"Browser Overdue "+runId,exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     overdueReadAudit,"Overdue filtering, card navigation and reload are read-only");
   await navigate("tasks?overdue=1&overdue=1","Страница не найдена");
   await navigate("tasks?overdue=invalid","Страница не найдена");
   await navigate("tasks?overdue=1","Задачи");
   await mutation(overdueCard.getByRole("button",{name:"Выполнить задачу",exact:true}),"CompleteTask");
   // Other overdue tasks (including the native fixture) must remain visible:
   // completion removes ONLY this task, not the entire overdue list.
   await page.getByRole("heading",{name:"Browser Overdue "+runId,exact:true}).waitFor({state:"hidden"});
   await page.getByLabel("Только просроченные задачи",{exact:true}).uncheck();
   await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   checkpoint("Task due-date triage: overdue-only owner filter/URL/reload/back/complete, no read audit");
   checkpoint("Contact and Task create/complete; priority/status filters preserve records and audit");
   await navigate("sales","Сделки");
   await page.getByRole("button",{name:"Создать воронку",exact:true}).click();
   await page.getByLabel("Название воронки",{exact:true}).fill("Browser Pipeline "+runId);
   const pipelineCreateRoute="**/crm.sales.v2.DealService/CreatePipeline";
   let pipelineCreateCalls=0,originalPipelineCreateRequestId;
   await page.route(pipelineCreateRoute,async route=>{
     pipelineCreateCalls++;
     const committed=await route.fetch();
     recordResponse(committed);await assertMutation(committed,"CreatePipeline");
     originalPipelineCreateRequestId=(await committed.headers())["x-request-id"];
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Сохранить воронку",exact:true}).click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   const pipelineMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.sales.pipeline.pending.v1")));
   assert.deepEqual(Object.keys(pipelineMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(pipelineMarker.expectedVersion,"0");
   assert.match(pipelineMarker.key,/^[a-f0-9-]{36}$/i);
   await reloadAfterPending();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   const pipelineLookup=page.waitForResponse(r=>responsePathEndsWith(r,"/GetPipeline")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const pipelineLookupResponse=await pipelineLookup;
   assert.equal(pipelineLookupResponse.status(),200);
   const pipelineLookupRequestId=(await pipelineLookupResponse.headers())["x-request-id"];
   assert.match(pipelineLookupRequestId??"",/^[a-f0-9-]{36}$/i);
   const newPipelineId=await firstId("crm_sales_pipeline_records","name","Browser Pipeline "+runId);
   assert.equal(pipelineMarker.resourceId,newPipelineId);
   assert.equal(pipelineCreateCalls,1,"Reload recovery must not re-execute CreatePipeline");
   assert.ok(originalPipelineCreateRequestId);
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(originalPipelineCreateRequestId);
   await assertAuditRecovery(originalPipelineCreateRequestId,pipelineLookupRequestId,"sales_pipeline.create","sales-pipeline");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.sales.pipeline.pending.v1")),null);
   ownerRecovery.push({owner:"sales-pipeline",kind:"lookup",
     original_request_id:originalPipelineCreateRequestId,subsequent_request_id:pipelineLookupRequestId});
   await page.unroute(pipelineCreateRoute);
   await expect(page.getByLabel("Воронка продаж",{exact:true})).toHaveValue(newPipelineId);
   assert.ok(new URL(page.url()).hash.includes("pipelineId="+newPipelineId),
     "confirmed pipeline selection must have a native URL");
   await reloadDocument();
   await expect(page.getByLabel("Воронка продаж",{exact:true})).toHaveValue(newPipelineId);
   await expect(page.getByRole("region",{name:"Kanban — этапы и сделки"})).toBeVisible();
   checkpoint("S5 pipeline lost acknowledgement: opaque marker, immutable receipt lookup, one D1 audit");
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
   const preferredEmailOption=dealContact.locator('option[value="'+browserSalesContactId+'"]');
   await preferredEmailOption.waitFor({state:"attached"});
   await expect(preferredEmailOption).toContainText("Email · предпочтительный");
   await expect(preferredEmailOption).not.toContainText(browserSalesContactId);
   await dealContact.selectOption(browserSalesContactId);
   await page.getByLabel("Ожидаемая дата закрытия",{exact:true}).fill("2030-05-21");
   await page.getByLabel("Сумма",{exact:true}).fill("90071992547409.93");
   await page.getByLabel("Источник вероятности",{exact:true}).selectOption("1");
   await page.getByLabel("Категория прогноза",{exact:true}).selectOption("3");
   await page.getByLabel("Вероятность, %",{exact:true}).fill("37.45");
   await page.getByLabel("Название сделки",{exact:true}).fill("   ");
   const invalidCreate=page.waitForResponse(r=>responsePathEndsWith(r,"/CreateDeal")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Сохранить сделку",exact:true}).click();
   assert.equal((await invalidCreate).status(),400);
   await expect(page.getByRole("dialog").getByText(/^deal\.name:/)).toBeVisible();
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
   // Manual board selection survives the real full Deal route and Back.
   const boardAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await page.locator('a[href="#sales/'+dealId+'"]').first().click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   const detailsFromBoard=page.getByRole("region",{name:"Подробности сделки "+dealId});
   await expect(detailsFromBoard.getByRole("link",{name:"Browser Account "+runId,exact:true}))
     .toHaveAttribute("href","#accounts/"+accountId);
   // SK-05: the actual user records fact-time interactions from the Deal,
   // sees both in chronological order after reload and an Account-only note.
   const dealHistory=detailsFromBoard.getByRole("region",
     {name:"История взаимодействий "+dealId});
   const saveInteraction=async(panel,kind,when,body)=>{
     await panel.getByLabel("Вид взаимодействия",{exact:true}).selectOption(String(kind));
     await panel.getByLabel("Время события",{exact:true}).fill(when);
     await panel.getByLabel("Что произошло",{exact:true}).fill(body);
     await mutation(panel.getByRole("button",{name:"Добавить в историю",exact:true}),
       "CreateInteraction");
     await panel.getByText(body,{exact:true}).waitFor();
   };
   const laterCall="Browser SK05 call "+runId;
   const earlierMeeting="Browser SK05 meeting "+runId;
   const accountNote="Browser SK05 note "+runId;
   await saveInteraction(dealHistory,1,"2026-05-19T12:30",laterCall);
   await saveInteraction(dealHistory,2,"2026-05-18T11:00",earlierMeeting);
   await reloadDocument();
   const dealReloadHistory=page.getByRole("region",
     {name:"История взаимодействий "+dealId});
   await dealReloadHistory.getByText(laterCall,{exact:true}).waitFor();
   await dealReloadHistory.getByText(earlierMeeting,{exact:true}).waitFor();
   const dealText=await dealReloadHistory.locator("ol li").allTextContents();
   assert.ok(dealText[0].includes(laterCall)&&dealText[1].includes(earlierMeeting),
     "SK05 must sort by fact time, not insertion order");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE tenant=? AND deal_id=?",
     tenantA,dealId))[0].n,2);
   await navigate("accounts/"+accountId,"Карточка клиента");
   const accountTimeline=page.getByRole("region",{name:"История взаимодействий "+accountId});
   await accountTimeline.getByText(laterCall,{exact:true}).waitFor();
   await accountTimeline.getByText(earlierMeeting,{exact:true}).waitFor();
   await saveInteraction(accountTimeline,3,"2026-05-17T10:00",accountNote);
   await reloadDocument();
   const accountReload=page.getByRole("region",{name:"История взаимодействий "+accountId});
   for(const text of [laterCall,earlierMeeting,accountNote])
     await accountReload.getByText(text,{exact:true}).waitFor();
   const accountRows=await accountReload.locator("ol li").allTextContents();
   assert.ok(accountRows[0].includes(laterCall)&&accountRows[1].includes(earlierMeeting)&&
     accountRows[2].includes(accountNote),"Account history must contain deal events and own note");
   const receipts=await sql("SELECT count(*) AS n FROM crm_activity_interaction_receipts WHERE tenant=?",
     tenantA);
   assert.ok(receipts[0].n>=3,"each UI write has an original immutable owner receipt");
   await navigate("sales/"+dealId,"Карточка сделки");
   const dealAfterAccountNote=page.getByRole("region",
     {name:"История взаимодействий "+dealId});
   await dealAfterAccountNote.getByText(laterCall,{exact:true}).waitFor();
   assert.equal(await dealAfterAccountNote.getByText(accountNote,{exact:true}).count(),0,
     "Account-only note must not be misattributed to Deal");
   const sk05BackAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   assert.equal(sk05BackAuditBefore,boardAuditBefore+3,
     "three SK-05 interactions must have exactly three new audit records");
   checkpoint("SK-05 React Account+Deal typed interaction history, authored facts, reload, ordering and D1 receipts");
   const dealContactLink=detailsFromBoard.getByRole("link",{
     name:"browser-"+runId+"@example.org",exact:true
   });
   await expect(dealContactLink).toHaveAttribute("href",
     "#contacts?partyId="+partyId+"&contactPointId="+browserSalesContactId);
   const exactContactRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetContactPoint")&&
     r.request().method()==="POST");
   await dealContactLink.click();
   assert.equal((await exactContactRead).status(),200);
   await page.getByRole("heading",{name:"Карточка контакта",exact:true}).waitFor();
   await page.getByText("ID контакта: "+browserSalesContactId,{exact:true}).waitFor();
   await reloadDocument();
   await page.getByRole("heading",{name:"Карточка контакта",exact:true}).waitFor();
   await page.getByText("ID контакта: "+browserSalesContactId,{exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await detailsFromBoard.getByRole("link",{
     name:"browser-"+runId+"@example.org",exact:true
   }).waitFor();
   // SK-05 adds Account -> Deal history entries after the original board.
   // Back must restore each visited card before returning to Kanban.
   await page.goBack();
   await page.getByRole("heading",{name:"Карточка клиента",exact:true}).waitFor();
   assert.equal(new URL(page.url()).hash,"#accounts/"+accountId);
   await accountReload.getByText(accountNote,{exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   assert.equal(new URL(page.url()).hash,"#sales/"+dealId);
   await dealAfterAccountNote.getByText(laterCall,{exact:true}).waitFor();
   await page.goBack();
   await expect(page.getByLabel("Воронка продаж",{exact:true})).toHaveValue(newPipelineId);
   await page.getByRole("region",{name:"Kanban — этапы и сделки"}).getByRole(
     "heading",{name:/Первичный контакт · 1/}).waitFor();
   await reloadDocument();
   await expect(page.getByLabel("Воронка продаж",{exact:true})).toHaveValue(newPipelineId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     sk05BackAuditBefore,"manual board navigation must not write D1 audit");
   checkpoint("M2-01 native board and M2-02 exact Deal Contact direct Get survive reload/Back without read-audit");
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
   await page.getByLabel("Начало периода",{exact:true}).fill("2000-01-01");
   await page.getByLabel("Конец периода",{exact:true}).fill("2000-01-01");
   await page.getByRole("button",{name:"Применить период",exact:true}).click();
   await page.getByRole("region",{name:"Прогноз продаж",exact:true}).getByText(/2000-01-01 — 2000-01-01/).waitFor();
   await forecast.getByText("Сделок: 0 · по этапу: 0 · вручную: 0",{exact:true}).waitFor();
   await page.getByRole("region",{name:"Фактическая конверсия",exact:true}).getByText(/знаменатель: 0/).waitFor();
   await page.getByLabel("Начало периода",{exact:true}).fill("2030-05-21");
   await page.getByLabel("Конец периода",{exact:true}).fill("2030-05-21");
   await page.getByRole("button",{name:"Применить период",exact:true}).click();
   await page.getByRole("region",{name:"Прогноз продаж",exact:true}).getByText(/2030-05-21 — 2030-05-21/).waitFor();
   await forecast.getByText("Сделок: 1 · по этапу: 0 · вручную: 1",{exact:true}).waitFor();
   await page.getByRole("button",{name:"Без ограничения по сроку",exact:true}).click();
   await page.getByRole("region",{name:"Прогноз продаж",exact:true}).getByText(/без ограничения по сроку/).waitFor();
   await forecast.getByText("Сделок: 1 · по этапу: 0 · вручную: 1",{exact:true}).waitFor();
   checkpoint("R7.3 full-pipeline exact weighted multi-currency read model, React, D1");
   // R7.3 saved views through REAL React -> generated Connect -> Rust domain
   // -> D1 audit/receipt. Reload must not lose a named shared filter.
   const savedViewName="Browser Saved View "+runId;
   await page.getByLabel("Статус сделки",{exact:true}).selectOption("1");
   await page.getByRole("button",{name:"Сохранить как новое",exact:true}).click();
   await page.getByLabel("Название представления",{exact:true}).fill(savedViewName);
   const createViewRoute="**/crm.sales.v2.DealService/CreateSalesView";
   let createViewCalls=0,originalViewCreateRequestId;
   await page.route(createViewRoute,async route=>{
     createViewCalls++;
     const committed=await route.fetch();
     recordResponse(committed);await assertMutation(committed,"CreateSalesView");
     originalViewCreateRequestId=(await committed.headers())["x-request-id"];
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Сохранить новое представление",exact:true}).click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   const viewMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.sales.view.pending.v1")));
   assert.deepEqual(Object.keys(viewMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(viewMarker.expectedVersion,"0");
   assert.match(viewMarker.key,/^[a-f0-9-]{36}$/i);
   await reloadAfterPending();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   const viewLookup=page.waitForResponse(r=>responsePathEndsWith(r,"/GetSalesView")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const viewLookupResponse=await viewLookup;
   assert.equal(viewLookupResponse.status(),200);
   const viewLookupRequestId=(await viewLookupResponse.headers())["x-request-id"];
   assert.match(viewLookupRequestId??"",/^[a-f0-9-]{36}$/i);
   const viewId=await firstId("crm_sales_saved_view_records","name",savedViewName);
   assert.equal(viewMarker.resourceId,viewId);
   assert.equal(createViewCalls,1,"Reload recovery must not re-execute CreateSalesView");
   assert.ok(originalViewCreateRequestId);
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(originalViewCreateRequestId);
   await assertAuditRecovery(originalViewCreateRequestId,viewLookupRequestId,"sales_view.create","sales-view");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.sales.view.pending.v1")),null);
   ownerRecovery.push({owner:"sales-view",kind:"lookup",
     original_request_id:originalViewCreateRequestId,subsequent_request_id:viewLookupRequestId});
   await page.unroute(createViewRoute);
   await expect(page.getByLabel("Сохранённое представление",{exact:true})).toHaveValue(viewId);
   assert.ok(new URL(page.url()).hash.includes("viewId="+viewId),
     "Committed saved view must become a typed native deep link");
   await expect(page.getByLabel("Статус сделки",{exact:true})).toHaveValue("1");
   checkpoint("S5 saved-view lost acknowledgement: opaque marker, immutable receipt lookup, one D1 audit");
   const createdView=JSON.parse((await sql("SELECT state FROM crm_sales_saved_view_records "+
     "WHERE tenant='"+tenantSql+"' AND id='"+viewId+"'"))[0].state);
   assert.equal(createdView.pipeline_id,newPipelineId);
   assert.equal(createdView.status,"open");
   assert.equal(createdView.version,1);
   await reloadDocument();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   const viewPicker=page.getByLabel("Сохранённое представление",{exact:true});
   await viewPicker.locator('option[value="'+viewId+'"]').waitFor({state:"attached"});
   // No manual reselect after reload: restore from the native URL.
   await expect(viewPicker).toHaveValue(viewId);
   assert.ok(new URL(page.url()).hash.includes("viewId="+viewId));
   // Views and pipelines are independent asynchronous reads on reload.
   // Wait for the controlled React selection AND its catalog option, not a
   // same-tick inputValue before the ListPipelines response is rendered.
   await expect(viewPicker).toHaveValue(viewId);
   const selectedPipeline=page.getByLabel("Воронка продаж",{exact:true});
   await expect(selectedPipeline.locator('option[value="'+newPipelineId+'"]').first()).toBeAttached();
   await expect(selectedPipeline).toHaveValue(newPipelineId);
   await expect(page.getByLabel("Статус сделки",{exact:true})).toHaveValue("1");
   // Native Deal → Back and search preserve the named view, not just its D1 row.
   await page.locator('a[href="#sales/'+dealId+'"]').first().click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await page.goBack();
   await expect(page.getByLabel("Сохранённое представление",{exact:true})).toHaveValue(viewId);
   await expect(page.getByLabel("Статус сделки",{exact:true})).toHaveValue("1");
   await page.getByLabel("Найти сделку",{exact:true}).fill("Browser Deal "+runId);
   await page.getByRole("button",{name:"Найти сделку по названию",exact:true}).click();
   assert.ok(new URL(page.url()).hash.includes("viewId="+viewId));
   assert.ok(new URL(page.url()).hash.includes("search="));
   await expect(page.getByLabel("Сохранённое представление",{exact:true})).toHaveValue(viewId);
   await page.getByRole("button",{name:"Сбросить поиск сделок",exact:true}).click();
   await expect(page.getByLabel("Сохранённое представление",{exact:true})).toHaveValue(viewId);
   assert.ok(!new URL(page.url()).hash.includes("search="));
   await page.getByRole("region",{name:"Аналитика воронки"})
     .getByText(/^Всего сделок: 1 · срез по UTC:/).waitFor();
   // Hold an obsolete read to prove no previous-key data is displayed.
   const metricsRoute="**/crm.sales.v2.DealService/GetPipelineMetrics";
   let startedOldMetrics,releaseOldMetrics;
   const oldMetricsStarted=new Promise(resolve=>{startedOldMetrics=resolve;});
   const oldMetricsGate=new Promise(resolve=>{releaseOldMetrics=resolve;});
   let oldMetricsRequest;
   const blockOldMetrics=async route=>{
     if(!oldMetricsRequest){
       oldMetricsRequest=route.request();startedOldMetrics();await oldMetricsGate;
     }
     try{await route.continue();}
     catch(error){if(!/Route is already handled!/.test(String(error)))throw error;}
   };
   await page.route(metricsRoute,blockOldMetrics);
   const statusFilter=page.getByLabel("Статус сделки",{exact:true});
   try {
     await statusFilter.selectOption("2");
     await oldMetricsStarted;
     await expect(page.getByRole("region",{name:"Аналитика воронки"})
       .getByText(/^Всего сделок: 1 · срез по UTC:/)).toHaveCount(0);
     const superseded=page.waitForEvent("requestfailed",{predicate:req=>req===oldMetricsRequest,timeout:15000});
     await statusFilter.selectOption("1");
     assert.ok((await superseded).failure(),"obsolete request must be cancelled");
     await page.getByRole("region",{name:"Аналитика воронки"})
       .getByText(/^Всего сделок: 1 · срез по UTC:/).waitFor();
   } finally {
     releaseOldMetrics();
     await page.unroute(metricsRoute,blockOldMetrics);
   }
   checkpoint("S1 read: old key hidden, obsolete request cancelled, latest key rendered");
   // Save the changed selected filters under the same stable view ID.
   await statusFilter.selectOption("2");
   await page.getByLabel("Название представления",{exact:true})
     .fill("Browser Won View "+runId);
   const updateViewRoute="**/crm.sales.v2.DealService/UpdateSalesView";
   let updateViewCalls=0,originalViewUpdateRequestId;
   await page.route(updateViewRoute,async route=>{
     updateViewCalls++;
     const committed=await route.fetch();
     recordResponse(committed);await assertMutation(committed,"UpdateSalesView");
     originalViewUpdateRequestId=(await committed.headers())["x-request-id"];
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Обновить представление",exact:true}).click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   const viewUpdateMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.sales.view.pending.v1")));
   assert.equal(viewUpdateMarker.resourceId,viewId);
   assert.equal(viewUpdateMarker.expectedVersion,"1");
   await reloadAfterPending();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   const viewUpdateLookup=page.waitForResponse(r=>responsePathEndsWith(r,"/GetSalesView")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const viewUpdateLookupResponse=await viewUpdateLookup;
   assert.equal(viewUpdateLookupResponse.status(),200);
   const viewUpdateLookupRequestId=(await viewUpdateLookupResponse.headers())["x-request-id"];
   assert.equal(updateViewCalls,1,"Reload recovery must not re-execute UpdateSalesView");
   assert.ok(originalViewUpdateRequestId);
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(originalViewUpdateRequestId);
   await assertAuditRecovery(originalViewUpdateRequestId,viewUpdateLookupRequestId,"sales_view.update","sales-view-update");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.sales.view.pending.v1")),null);
   await page.unroute(updateViewRoute);
   await expect(page.getByLabel("Сохранённое представление",{exact:true})).toHaveValue(viewId);
   await expect(page.getByLabel("Статус сделки",{exact:true})).toHaveValue("2");
   const updatedView=JSON.parse((await sql("SELECT state FROM crm_sales_saved_view_records "+
     "WHERE tenant='"+tenantSql+"' AND id='"+viewId+"'"))[0].state);
   assert.equal(updatedView.version,2);
   assert.equal(updatedView.status,"won");
   assert.equal(updatedView.name,"Browser Won View "+runId);
   await mutation(page.getByRole("button",{name:"Архивировать представление",exact:true}),
     "UpdateSalesView");
   const viewsAfterArchive=page.waitForResponse(r=>responsePathEndsWith(r,"/ListSalesViews")&&r.request().method()==="POST");
   await reloadDocument();
   assert.equal((await viewsAfterArchive).status(),200);
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   const viewsSection=page.getByRole("region",{name:"Сохранённые представления"});
   await expect(viewsSection.getByRole("status")).toHaveCount(0);
   await expect(viewsSection.getByLabel("Сохранённое представление",{exact:true})
     .locator('option[value="'+viewId+'"]'),
     "Archived view must remain audited but not appear as selectable").toHaveCount(0);
   assert.ok(!new URL(page.url()).hash.includes("viewId="),
     "Archiving a view must drop its native route selection");
   await page.goto(base+"/#sales?viewId="+viewId+"&viewId="+viewId);
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   await page.goto(base+"/#sales?viewId="+encodeURIComponent("wrong/id"));
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   await page.goto(base+"/#sales?viewId="+viewId);
   await page.getByText("Сохранённое представление недоступно.",{exact:false}).waitFor();
   await page.goto(base+"/#sales");
   assert.deepEqual((await sql("SELECT action FROM crm_audit WHERE tenant_id='"+tenantSql+
     "' AND subject_id='"+viewId+"' ORDER BY rowid")).map(x=>x.action),
     ["sales_view.create","sales_view.update","sales_view.update"]);
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
   await page.getByLabel("Статус сделки",{exact:true}).selectOption("");
   await page.getByText("90071992547409.93 USD · 37.45%",{exact:true}).waitFor();
   checkpoint("R7.3 saved view create, reload/apply, update, archive and D1 audit");
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
   checkpoint("R7.2 contact/date via real React, GetDeal/detail and original D1");
   await page.getByText("90071992547409.93 USD · 38.50%",{exact:true}).waitFor();
   // Keyboard/select fallback remains the original mutation form; verify
   // it opens and can be cancelled without a write before testing native DnD.
   const keyboardStageButton=dealCard.getByRole("button",{name:"Изменить этап",exact:true});
   const keyboardStageRead=page.waitForResponse(response=>
     responsePathEndsWith(response,"/GetDeal")&&response.request().method()==="POST");
   await keyboardStageButton.press("Enter");
   const stageReadResponse=await keyboardStageRead;
   assert.equal(stageReadResponse.status(),200,
     "Keyboard fallback must load the current Deal from its original owner");
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
   const beforePendingNavigation=page.url();
   // Simulate a native hash navigation while the modal owns input.
   await page.evaluate(()=>{window.location.hash="#accounts";});
   await expect(page).toHaveURL(beforePendingNavigation);
   await expect(page.getByRole("dialog")).toBeVisible();
   await expect(page.getByRole("button",{name:"Повторить исходную операцию",exact:true})).toBeVisible();
   // The rejected hash entry remains in native history. Forward must restore
   // the same accepted entry, too, not merely repair the first hash change.
   assert.equal(await page.evaluate(()=>window.navigation.canGoForward),true);
   const attemptedForward=await page.evaluate(()=>new Promise(resolve=>{
     window.addEventListener("popstate",()=>resolve(location.hash),{once:true});
     history.forward();
   }));
   assert.equal(attemptedForward,"#accounts");
   await expect(page).toHaveURL(beforePendingNavigation);
   await expect(page.getByRole("dialog")).toBeVisible();
   const retryAdvance=page.waitForResponse(r=>responsePathEndsWith(r,"/AdvanceStage")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Повторить исходную операцию",exact:true}).click();
   const retryAdvanceResponse=await retryAdvance;
   await assertMutation(retryAdvanceResponse,"AdvanceStage",{replay:true});
   const retryAdvanceRequestId=(await retryAdvanceResponse.headers())["x-request-id"];
   assert.match(retryAdvanceRequestId??"",/^[a-f0-9-]{36}$/i);
   assert.notEqual(retryAdvanceRequestId,originalAdvanceRequestId);
   recovery.push({kind:"replay",original_request_id:originalAdvanceRequestId,
     subsequent_request_id:retryAdvanceRequestId});
   await page.getByRole("heading",{name:/Сохранено ранее/}).waitFor();
   assert.ok(originalAdvanceRequestId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id='"+originalAdvanceRequestId+"' AND action='sales_deals.advance'"))[0].n,1);
   await page.unroute(advanceRoute);
   checkpoint("S1 lost Deal acknowledgement: locked editor, exact binary retry, original audit");
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
   await expect(page.getByText(/^Код причины без пробелов:/)).toBeVisible();
   await page.getByLabel("Причина закрытия",{exact:true}).fill("contract-signed");
   // A different real write loses its response and the page is reloaded.
   // Only opaque identifiers survive; lookup must not issue CloseDeal again.
   const closeRoute="**/crm.sales.v2.DealService/CloseDeal";
   let closeCalls=0, originalCloseRequestId;
   await page.route(closeRoute,async route=>{
     closeCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"CloseDeal");
     originalCloseRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Завершить сделку",exact:true}).last().click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   const mark=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.deal.pending.v1")));
   assert.deepEqual(Object.keys(mark).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(mark.resourceId,dealId);
   assert.equal(mark.expectedVersion,"3");
   assert.match(mark.key,/^[a-f0-9-]{36}$/i);
   const acceptReload=dialog=>void dialog.accept();
   page.on("dialog",acceptReload);
   try{await reloadDocument();}finally{page.off("dialog",acceptReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
   await expect(page.getByRole("button",{name:"Новая сделка",exact:true})).toBeDisabled();
   // A failed read cannot clear the marker or start another write.
   const getDealRoute="**/crm.sales.v2.DealService/GetDeal";
   const failLookup=route=>route.abort("failed");
   await page.route(getDealRoute,failLookup);
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   await expect(page.getByRole("button",{name:"Проверить результат записи",exact:true})).toBeEnabled();
   assert.deepEqual(await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.deal.pending.v1"))),mark);
   assert.equal(closeCalls,1);
   await expect(page.getByRole("button",{name:"Новая сделка",exact:true})).toBeDisabled();
   await page.unroute(getDealRoute,failLookup);
   const lookupResponse=page.waitForResponse(r=>responsePathEndsWith(r,"/GetDeal")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const checked=await lookupResponse;
   assert.equal(checked.status(),200);
   const checkId=(await checked.headers())["x-request-id"];
   assert.match(checkId??"",/^[a-f0-9-]{36}$/i);
   assert.ok(originalCloseRequestId);
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(originalCloseRequestId);
   assert.equal(closeCalls,1,"Reload recovery must not re-execute a write");
   await assertAuditRecovery(originalCloseRequestId,checkId,"sales_deals.close","deal-close");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.deal.pending.v1")),null);
   recovery.push({kind:"lookup",original_request_id:originalCloseRequestId,
     subsequent_request_id:checkId});
   await page.unroute(closeRoute);
   checkpoint("S1 reload: opaque marker, read original receipt, no second CloseDeal");
   const terminal=(await sql("SELECT version,json_extract(state,'$.status') AS status,json_extract(state,'$.stage.stage_id') AS stage FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.deepEqual(terminal,{version:4,status:"won",stage:actualStage});
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant='"+tenantSql+"' AND deal_id='"+dealId+"'"))[0].n,2);
   await movedDealCard.getByText("Выиграна · версия 4",{exact:true}).waitFor();
   assert.equal(await movedDealCard.getByRole("button",{name:"Изменить сделку",exact:true}).count(),0);
   checkpoint("CloseDeal: Won at unchanged stage, terminal read-only, D1 linked receipt");
   // The same visible editor may reorder stages already used by closed Deals.
   await page.getByRole("button",{name:"Настроить воронку",exact:true}).click();
   await page.getByRole("button",{name:"Выше",exact:true}).last().click();
   const updatePipelineRoute="**/crm.sales.v2.DealService/UpdatePipeline";
   let updatePipelineCalls=0,originalPipelineUpdateRequestId;
   await page.route(updatePipelineRoute,async route=>{
     updatePipelineCalls++;
     const committed=await route.fetch();
     recordResponse(committed);await assertMutation(committed,"UpdatePipeline");
     originalPipelineUpdateRequestId=(await committed.headers())["x-request-id"];
     await route.abort("failed");
   });
   await page.getByRole("button",{name:"Сохранить воронку",exact:true}).click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   const pipelineUpdateMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.sales.pipeline.pending.v1")));
   assert.equal(pipelineUpdateMarker.resourceId,newPipelineId);
   assert.equal(pipelineUpdateMarker.expectedVersion,"1");
   await reloadAfterPending();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   const pipelineUpdateLookup=page.waitForResponse(r=>responsePathEndsWith(r,"/GetPipeline")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const pipelineUpdateLookupResponse=await pipelineUpdateLookup;
   assert.equal(pipelineUpdateLookupResponse.status(),200);
   const pipelineUpdateLookupRequestId=(await pipelineUpdateLookupResponse.headers())["x-request-id"];
   assert.equal(updatePipelineCalls,1,"Reload recovery must not re-execute UpdatePipeline");
   assert.ok(originalPipelineUpdateRequestId);
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(originalPipelineUpdateRequestId);
   await assertAuditRecovery(originalPipelineUpdateRequestId,pipelineUpdateLookupRequestId,"sales_pipeline.update","sales-pipeline-update");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.sales.pipeline.pending.v1")),null);
   await page.unroute(updatePipelineRoute);
   const newCatalog=JSON.parse((await sql("SELECT state FROM crm_sales_pipeline_records WHERE tenant='"+
     tenantSql+"' AND id='"+newPipelineId+"'"))[0].state);
   assert.deepEqual(newCatalog.stages.map(s=>s.stage_id),[ownStageIds[1],ownStageIds[0]]);
   const rebased=(await sql("SELECT version,json_extract(state,'$.stage.ordinal') AS rank "+
     "FROM crm_sales_deal_records WHERE tenant='"+tenantSql+"' AND id='"+dealId+"'"))[0];
   assert.deepEqual(rebased,{version:4,rank:1},
     "Occupied reorder must not increment Deal versions");
   await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   checkpoint("Occupied stage reorder in real React keeps Deal version and stable ID");
   const currentCatalog=page.waitForResponse(r=>responsePathEndsWith(r,"/GetPipeline")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Настроить воронку",exact:true}).click();
   assert.equal((await currentCatalog).status(),200,
     "Editor must read latest authoritative Pipeline version after reorder");
   await page.getByLabel("Архивировать воронку",{exact:true}).check();
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"UpdatePipeline");
   await expect(page.getByRole("button",{name:"Новая сделка",exact:true})).toBeDisabled();
   await page.getByRole("button",{name:"Настроить воронку",exact:true}).click();
   await page.getByLabel("Архивировать воронку",{exact:true}).uncheck();
   await mutation(page.getByRole("button",{name:"Сохранить воронку",exact:true}),"UpdatePipeline");
   await expect(page.getByRole("button",{name:"Новая сделка",exact:true})).toBeEnabled();
   checkpoint("Pipeline archive and restore using real React, Connect and audit");
   // Actual full Deal route: typed append-only history, linked Task and
   // explicit ReopenDeal. No second editor or client-side history inference.
   await page.goto(base+"/#sales/"+encodeURIComponent(dealId));
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   const namedDetails=page.getByRole("region",{name:"Подробности сделки "+dealId});
   const namedHistory=page.getByRole("region",{name:"История сделки "+dealId});
   await namedDetails.getByText("Воронка: Browser Pipeline "+runId+" · этап: Квалификация",{exact:true}).waitFor();
   await namedHistory.getByText("Этап: Первичный контакт → Квалификация",{exact:true}).waitFor();
   await namedHistory.getByText(/Названия этапов показаны по текущему справочнику/).waitFor();
   const auditBeforeNamedRead=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await reloadDocument();
   await namedDetails.getByText("Воронка: Browser Pipeline "+runId+" · этап: Квалификация",{exact:true}).waitFor();
   await namedHistory.getByText("Этап: Первичный контакт → Квалификация",{exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     auditBeforeNamedRead,"Named stage catalog/history rendering must never create audit writes");
   checkpoint("M2-01 original immutable stage IDs shown as named pipeline/stage in full Deal and history after reload");
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
   checkpoint("R7.2 linked Task opens exact original GetTask through typed route, then Back");

   await page.getByRole("button",{name:"Открыть повторно",exact:true}).last().click();
   await page.getByRole("heading",{name:"Повторно открыть сделку: Browser Deal "+runId,exact:true}).waitFor();
   await expect(page.getByText(/^Код причины без пробелов:/)).toBeVisible();
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
   checkpoint("R7.2 CloseDeal→immutable history→ReopenDeal→D1 and linked Task");
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
   const alternateDealCard=page.locator("section").filter({
     has:page.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true})
   }).last();
   await alternateDealCard.getByRole("button",{name:"Раскрыть карточку",exact:true}).click();
   await alternateDealCard.getByText(
     "Воронка: Browser Alternate Pipeline "+runId+" · этап: Первичный контакт",
     {exact:true}).last().waitFor();
   checkpoint("M2-01 inline Deal name matches stage catalog, no second pipeline or event read owner");
   checkpoint("Second pipeline Deal under same Account persisted in D1");
   // M2 sales lookup: find a Deal across pipelines even when a Kanban board
   // is selected. The name search must not change the board or metrics.
   const metricsBeforeSearch=await page.getByRole("region",{name:"Аналитика воронки"})
     .innerText();
   assert.ok(new URL(page.url()).hash.includes("pipelineId="+alternatePipelineId));
   await page.getByLabel("Найти сделку",{exact:true}).fill("Browser Deal "+runId);
   await page.getByRole("button",{name:"Найти сделку по названию",exact:true}).click();
   assert.ok(new URL(page.url()).hash.includes("pipelineId="+alternatePipelineId),
     "Deal search must not drop the manual Kanban selection");
   const searchResults=page.getByRole("region",{name:"Результаты поиска сделок"});
   await searchResults.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await expect(searchResults.getByRole("heading",{name:"Browser Alternate Deal "+runId,
     exact:true})).toHaveCount(0);
   assert.equal(await page.getByRole("region",{name:"Аналитика воронки"}).innerText(),
     metricsBeforeSearch,"search must not mutate the selected Kanban metrics");
   await searchResults.getByRole("link",{name:"Полная страница",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await expect(page.getByLabel("Найти сделку",{exact:true})).toHaveValue("Browser Deal "+runId);
   await searchResults.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await reloadDocument();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await expect(page.getByLabel("Найти сделку",{exact:true})).toHaveValue("Browser Deal "+runId);
   await searchResults.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await page.getByLabel("Найти сделку",{exact:true}).fill("No matching Deal "+runId);
   await page.getByRole("button",{name:"Найти сделку по названию",exact:true}).click();
   await searchResults.getByText("Сделки по запросу не найдены.",{exact:true}).waitFor();
   await page.getByRole("button",{name:"Сбросить поиск сделок",exact:true}).click();
   await expect(searchResults).toHaveCount(0);
   checkpoint("M2 Deal search across pipelines, typed name filter, result→full page→Back, no-results and reset");

   // L-08: original typed Party/Account/Deal tenant reads, not a search index.
   const unifiedAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await navigate("search","Поиск по CRM");
   await page.getByLabel("Поиск по CRM",{exact:true}).fill(runId);
   await page.getByRole("button",{name:"Найти",exact:true}).click();
   assert.equal(new URL(page.url()).hash,"#search?query="+encodeURIComponent(runId));
   const foundParties=page.getByRole("region",{name:"Найденные участники"});
   const foundAccounts=page.getByRole("region",{name:"Найденные клиенты"});
   const foundDeals=page.getByRole("region",{name:"Найденные сделки"});
   const foundTasks=page.getByRole("region",{name:"Найденные задачи"});
   await foundParties.getByRole("link",{name:"Browser Updated "+runId,exact:true}).waitFor();
   await foundAccounts.getByRole("link",{name:"Browser Account "+runId,exact:true}).waitFor();
   await foundDeals.getByRole("link",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await foundTasks.getByRole("link",{name:"Browser Task "+runId,exact:true}).waitFor();
   await foundTasks.getByRole("link",{name:"Browser Task "+runId,exact:true}).click();
   await page.getByRole("heading",{name:"Карточка задачи",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Task "+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Поиск по CRM",exact:true}).waitFor();
   await foundDeals.getByRole("link",{name:"Browser Deal "+runId,exact:true}).click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Поиск по CRM",exact:true}).waitFor();
   await expect(page.getByLabel("Поиск по CRM",{exact:true})).toHaveValue(runId);
   await foundAccounts.getByRole("link",{name:"Browser Account "+runId,exact:true}).waitFor();
   await reloadDocument();
   await page.getByRole("heading",{name:"Поиск по CRM",exact:true}).waitFor();
   await foundParties.getByRole("link",{name:"Browser Updated "+runId,exact:true}).waitFor();
   await page.getByLabel("Поиск по CRM",{exact:true}).fill("missing-unified-"+runId);
   await page.getByRole("button",{name:"Найти",exact:true}).click();
   await foundParties.getByText("Участники не найдены.",{exact:true}).waitFor();
   await foundAccounts.getByText("Клиенты не найдены.",{exact:true}).waitFor();
   await foundDeals.getByText("Сделки не найдены.",{exact:true}).waitFor();
   await foundTasks.getByText("Задачи не найдены.",{exact:true}).waitFor();
   await page.goto(base+"/#search?query=a&query=b");
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   await page.goto(base+"/#search?query="+("x".repeat(201)));
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     unifiedAuditBefore,"L-08 search, reload, links and malformed URLs must not write audit");
   await page.goto(base+"/#sales");
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   checkpoint("L-08 bounded four-owner Party/Account/Deal/Task discovery, native URL/Back/reload, empty/malformed, zero read audit");
   // Full-page Deal URL and inline expansion share the same original GetDeal
   // read component. The preceding search/back route can reset the selected
   // pipeline, so target the actual Deal card, not whichever link is last.
   const alternateCardAfterSearch=page.locator("section").filter({
     has:page.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true})
   }).last();
   await alternateCardAfterSearch.getByRole("link",{name:"Полная страница",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   assert.equal((await page.evaluate(()=>location.hash)).startsWith("#sales/"),true);
   await page.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   checkpoint("R7.2 addressable full Deal card via shared GetDeal and Back");
   await page.getByLabel("Воронка продаж",{exact:true}).selectOption("");
   assert.ok(!new URL(page.url()).hash.includes("pipelineId="),
     "All-pipelines overview must clear the manual URL selection");
   await page.goto(base+"/#sales?pipelineId="+alternatePipelineId);
   await expect(page.getByLabel("Воронка продаж",{exact:true})).toHaveValue(alternatePipelineId);
   await page.goto(base+"/#sales?pipelineId="+alternatePipelineId+"&pipelineId="+alternatePipelineId);
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   await page.goto(base+"/#sales?pipelineId="+alternatePipelineId+"&viewId=ambiguous-view");
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   await page.goto(base+"/#sales?pipelineId=bad%2Fid");
   await page.getByRole("heading",{name:"Страница не найдена",exact:true}).waitFor();
   await page.goto(base+"/#sales?pipelineId=nonexistent-manual-board");
   await page.getByText("Воронка по ссылке недоступна.",{exact:false}).waitFor();
   await page.goto(base+"/#sales");
   checkpoint("M2-01 manual board deep link, clear selection, fail-closed URLs");
   await navigate("accounts","Клиенты");
   await page.locator("section").filter({has:page.getByRole("heading",{name:"Browser Account "+runId,level:2,exact:true})})
     .getByRole("button",{name:"Открыть карточку",exact:true}).click();
   await page.getByText("Основной участник: Browser Updated "+runId,{exact:true}).waitFor();
   const customer360Deal=page.getByRole("listitem").filter({
     has:page.getByRole("link",{name:"Browser Deal "+runId,exact:true})
   });
   await customer360Deal.getByText(
     "Воронка: Browser Pipeline "+runId+" · этап: Квалификация",{exact:false}
   ).waitFor();
   const customer360Task=page.getByRole("listitem").filter({
     has:page.getByRole("link",{name:"Browser Edited Urgent "+runId,exact:true})
   });
   await customer360Task.getByText(/Открыта/).waitFor();
   const customer360Audit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await navigate("accounts/"+accountId,"Карточка клиента");
   await customer360Deal.getByText("Воронка: Browser Pipeline "+runId+" · этап: Квалификация",{exact:false}).waitFor();
   await reloadDocument();
   await customer360Deal.getByText("Воронка: Browser Pipeline "+runId+" · этап: Квалификация",{exact:false}).waitFor();
   await customer360Task.getByText(/Открыта/).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     customer360Audit,"Customer360 stage names and Task status reads must not write audit");
   checkpoint("N-03 Customer360 Deal names from original archived-capable Sales catalog, open Task status, reload and no read audit");
   checkpoint("Exact bigint Deal and atomic Sales–Activities flow visible in Account");
   await navigate("review","Качество и совпадения");
   const quality=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить имя",exact:true})});
   await selectPagedPicker(quality,"Участник",partyId);
   const qualityResponse=page.waitForResponse(r=>responsePathEndsWith(r,"/EvaluatePartyName"));
   await quality.getByRole("button",{name:"Проверить качество",exact:true}).click();
   assert.equal((await qualityResponse).status(),200);
   await page.getByText("Не пройдено: 0",{exact:true}).waitFor();
   const compare=page.locator("section").filter({has:page.getByRole("heading",{name:"Сравнить участников",exact:true})});
   await selectPagedPicker(compare,"Первый участник",partyId);
   await selectPagedPicker(compare,"Второй участник",otherPartyId);
   const compared=page.waitForResponse(r=>responsePathEndsWith(r,"/CompareParties"));
   await compare.getByRole("button",{name:"Сравнить",exact:true}).click();
   assert.equal((await compared).status(),200);
   await compare.getByText(/разные записи; личность не подтверждена/).waitFor();
   // N-05: exercise the already typed original candidate read -> same-pair
   // comparison, without registration, identity confirmation or any merge.
   await navigate("contacts","Контакты");
   await page.getByRole("button",{name:"Создать контакт",exact:true}).click();
   await selectPagedPicker(page,"Участник",otherPartyId);
   await page.getByLabel("Значение контакта",{exact:true}).fill("browser-"+runId+"@example.org");
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"CreateContactPoint");
   const candidateReadAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await navigate("review","Качество и совпадения");
   const candidateCard=page.locator("section").filter({has:page.getByRole("heading",{name:"Совпадающие email",exact:true})});
   const candidateRow=candidateCard.getByRole("listitem").filter({
     has:page.getByRole("link",{name:"Browser Other "+runId,exact:true})
   });
   await candidateRow.getByRole("link",{name:"Browser Updated "+runId,exact:true}).waitFor();
   await candidateRow.getByText(/есть неподтверждённые контакты/).waitFor();
   const candidateCompare=page.waitForResponse(r=>responsePathEndsWith(r,"/CompareParties"));
   await candidateRow.getByRole("button",{name:"Сравнить пару",exact:true}).click();
   assert.equal((await candidateCompare).status(),200);
   await candidateRow.getByText(/разные записи; личность не подтверждена/).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     candidateReadAudit,"Candidate discovery and comparison must never write audit");
   await candidateRow.getByRole("link",{name:"Browser Other "+runId,exact:true}).first().click();
   assert.equal(new URL(await page.url()).hash,"#parties?partyId="+encodeURIComponent(otherPartyId));
   await page.getByRole("heading",{name:"Browser Other "+runId,exact:true}).waitFor();
   await page.goBack();
   await candidateCard.getByRole("listitem").filter({has:page.getByRole("link",{
     name:"Browser Other "+runId,exact:true
   })}).getByRole("button",{name:"Сравнить пару",exact:true}).waitFor();
   await reloadDocument();
   await candidateCard.getByRole("listitem").filter({has:page.getByRole("link",{
     name:"Browser Other "+runId,exact:true
   })}).getByRole("button",{name:"Сравнить пару",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     candidateReadAudit,"Navigating candidate Party and returning/reloading must remain read-only");
   checkpoint("N-05 original shared-email candidate -> direct typed CompareParties -> unconfirmed verdict, Party links/reload, zero read audit");

   // L-03: user-visible explicit identity confirmation, original Rust
   // merge/unmerge lineage, and no destructive owner reference changes.
   const l03Tables=["crm_account_records","crm_contact_point_records",
     "crm_party_relationship_records","crm_sales_deal_records",
     "crm_activity_task_records","crm_consent_records"];
   const l03Before=await Promise.all(l03Tables.map(async table=>
     (await sql("SELECT COUNT(*) AS n FROM "+table+" WHERE tenant='"+tenantSql+"'"))[0].n));
   const l03AuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await candidateRow.getByRole("button",{name:"Сравнить пару",exact:true}).click();
   const canonicalCard=page.locator("section").filter({
     has:page.getByRole("heading",{name:"Каноническая запись и история объединений",exact:true})
   });
   await canonicalCard.getByLabel("Запись-дубликат (неосновная)",{exact:true}).selectOption(otherPartyId);
   await expect(canonicalCard.getByRole("button",{name:"Подтвердить объединение",exact:true})).toBeDisabled();
   await canonicalCard.getByLabel(/Я вручную проверил личность/).check();
   await mutation(canonicalCard.getByRole("button",{name:"Подтвердить объединение",exact:true}),"MergeParty");
   const l03Merge=(await sql("SELECT id,source_party,survivor_party,version,status "+
     "FROM crm_identity_merge_records WHERE tenant='"+tenantSql+"' AND source_party='"+otherPartyId+"' "+
     "AND survivor_party='"+partyId+"' ORDER BY rowid DESC LIMIT 1"))[0];
   assert.ok(l03Merge);
   assert.equal(l03Merge.version,1);
   assert.equal(l03Merge.status,"active");
   await navigate("parties?partyId="+encodeURIComponent(otherPartyId),"Люди и организации");
   await page.getByRole("link",{name:"Открыть каноническую запись",exact:true}).waitFor();
   await page.getByRole("heading",{name:"Browser Other "+runId,exact:true}).waitFor();
   await navigate("review?partyId="+encodeURIComponent(otherPartyId),"Качество и совпадения");
   await canonicalCard.getByRole("button",{name:"Отменить объединение",exact:true}).waitFor();
   await mutation(canonicalCard.getByRole("button",{name:"Отменить объединение",exact:true}),"UnmergeParty");
   const l03Unmerged=(await sql("SELECT version,status FROM crm_identity_merge_records "+
     "WHERE tenant='"+tenantSql+"' AND id='"+l03Merge.id+"'"))[0];
   assert.deepEqual(l03Unmerged,{version:2,status:"unmerged"});
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     l03AuditBefore+2,"Merge+Unmerge must write exactly 2 audits");
   const l03After=await Promise.all(l03Tables.map(async table=>
     (await sql("SELECT COUNT(*) AS n FROM "+table+" WHERE tenant='"+tenantSql+"'"))[0].n));
   assert.deepEqual(l03After,l03Before,"Lineage cannot lose downstream references");
   await reloadDocument();
   await canonicalCard.getByText(/Отменено:/).waitFor();
   await navigate("parties?partyId="+encodeURIComponent(otherPartyId),"Люди и организации");
   await page.getByRole("heading",{name:"Browser Other "+runId,exact:true}).waitFor();
   await expect(page.getByRole("link",{name:"Открыть каноническую запись",exact:true})).toHaveCount(0);
   checkpoint("L-03 original typed domain reversible canonical Merge/Unmerge, explicit identity decision, exact D1 audit/refs/reload");
   // N-04: the original quality read and Party update are already present.
   // Exercise only the missing user journey: exact Party -> edit -> exact review.
   await navigate("parties?partyId="+encodeURIComponent(otherPartyId),"Люди и организации");
   await page.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("A");
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"UpdateParty");
   const qualityAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await page.getByRole("link",{name:"Повторно проверить качество",exact:true}).click();
   assert.equal(new URL(await page.url()).hash,"#review?partyId="+encodeURIComponent(otherPartyId));
   await reloadDocument();
   const badQuality=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить имя",exact:true})});
   await expect(badQuality.getByLabel("Участник",{exact:true})).toHaveValue(otherPartyId);
   await expect(badQuality.getByLabel("Участник",{exact:true})).toBeEnabled();
   const badQualityRead=page.waitForResponse(r=>responsePathEndsWith(r,"/EvaluatePartyName"));
   await badQuality.getByRole("button",{name:"Проверить качество",exact:true}).click();
   assert.equal((await badQualityRead).status(),200);
   await badQuality.getByText("Не пройдено: 1",{exact:true}).waitFor();
   await badQuality.getByRole("link",{name:"Открыть карточку и исправить",exact:true}).click();
   assert.equal(new URL(await page.url()).hash,"#parties?partyId="+encodeURIComponent(otherPartyId));
   await page.getByRole("heading",{name:"A",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     qualityAuditBefore,"Data Quality reads and deep links must not write audit");
   await page.getByRole("button",{name:"Редактировать",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Other "+runId);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"UpdateParty");
   const repairedAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await page.getByRole("link",{name:"Повторно проверить качество",exact:true}).click();
   assert.equal(new URL(await page.url()).hash,"#review?partyId="+encodeURIComponent(otherPartyId));
   const goodQuality=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить имя",exact:true})});
   await expect(goodQuality.getByLabel("Участник",{exact:true})).toHaveValue(otherPartyId);
   await expect(goodQuality.getByLabel("Участник",{exact:true})).toBeEnabled();
   await goodQuality.getByRole("button",{name:"Проверить качество",exact:true}).click();
   await goodQuality.getByText("Не пройдено: 0",{exact:true}).waitFor();
   await expect(goodQuality.getByRole("link",{name:"Открыть карточку и исправить",exact:true})).toHaveCount(0);
   await reloadDocument();
   await expect(goodQuality.getByLabel("Участник",{exact:true})).toHaveValue(otherPartyId);
   await expect(goodQuality.getByLabel("Участник",{exact:true})).toBeEnabled();
   await goodQuality.getByRole("button",{name:"Проверить качество",exact:true}).click();
   await goodQuality.getByText("Не пройдено: 0",{exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     repairedAudit,"Repeated evaluation and reload must remain read-only");
   checkpoint("N-04 existing DataQuality + typed Party repair + exact review URL/reload, no read audit");
   // N-06 human and service-agent proofs are separate original Rust paths.
   // An enrolled service: identity is NOT an email and must not be self-attested
   // as a human. The dev test agent verifies a distinct valid test Email.
   const consentEmail=adminIdentity.startsWith("service:")
     ?"browser-consent-"+runId+"@example.org":adminIdentity;
   await navigate("contacts","Контакты");
   await page.getByRole("button",{name:"Создать контакт",exact:true}).click();
   await selectPagedPicker(page,"Участник",partyId);
   await page.getByLabel("Значение контакта",{exact:true}).fill(consentEmail);
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"CreateContactPoint");
   const verifiedContactRows=await sql("SELECT id FROM crm_contact_point_records "+
     "WHERE tenant='"+tenantSql+"' AND party_id='"+partyId+"' AND json_extract(state,'$.normalized_value')='"+
     consentEmail.replaceAll("'","''")+"'");
   assert.equal(verifiedContactRows.length,1,"verified owner email is scoped to the selected Party");
   const consentVerifiedContactId=verifiedContactRows[0].id;
   await selectPagedPicker(page,"Фильтр по участнику",partyId);
   const consentContactCard=page.locator("section").filter({
     has:page.getByRole("heading",{name:consentEmail,exact:true})
   });
   await mutation(consentContactCard.getByRole("button",{
     name:"Подтвердить email (Access / тестовый администратор)",exact:true
   }),"VerifyOwnEmailContactPoint");
   await navigate("consents","Коммуникации");
   // N-06: consume only original Party/ContactPoint owner reads instead of opaque IDs.
   await selectPagedPicker(page,"Фильтр по участнику",partyId);
   await page.getByRole("button",{name:"Выдать согласие",exact:true}).click();
   const grantPickerPanel=page.getByRole("dialog");
   await selectPagedPicker(grantPickerPanel,"Участник",partyId);
   const unverifiedEmail=grantPickerPanel.getByLabel("Подтверждённый email",{exact:true});
   const unverifiedOption=unverifiedEmail.locator('option[value="'+browserSalesContactId+'"]');
   await unverifiedOption.waitFor({state:"attached"});
   await expect(unverifiedOption).toBeDisabled();
   await expect(unverifiedOption).toContainText("не подтверждён");
   const verifiedOption=unverifiedEmail.locator('option[value="'+consentVerifiedContactId+'"]');
   await verifiedOption.waitFor({state:"attached"});
   await expect(verifiedOption).toBeEnabled();
   await unverifiedEmail.selectOption(consentVerifiedContactId);
   const n06GrantPurpose="n06.newsletter."+runId;
   await grantPickerPanel.getByLabel("Цель согласия",{exact:true}).fill(n06GrantPurpose);
   await mutation(grantPickerPanel.getByRole("button",{
     name:"Выдать согласие на email-коммуникацию",exact:true
   }),"GrantVerifiedEmailConsent");
   const grantCard=page.locator("section").filter({hasText:n06GrantPurpose});
   await grantCard.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await grantCard.getByRole("heading",{name:"Контакт разрешён",exact:true}).waitFor();
   await mutation(grantCard.getByRole("button",{name:"Отозвать согласие",exact:true}),"WithdrawConsentAuthorization");
   await grantCard.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await grantCard.getByRole("heading",{name:"Контакт не разрешён",exact:true}).waitFor();
   await grantCard.getByText("Согласие отозвано",{exact:true}).waitFor();
   checkpoint("N-06 original signed-human verified Email -> typed Grant -> allowed -> CAS Withdraw -> denied");
   // The original signed Email grant guard is independently tested in R3a.
   // This browser session may use a non-human service principal: never self-grant.
   await page.getByRole("button",{name:"Запретить коммуникацию",exact:true}).click();
   const suppress=page.getByRole("dialog");
   await selectPagedPicker(suppress,"Участник",partyId);
   const n06DenyPurpose="n06.deny."+runId;
   await suppress.getByLabel("Цель",{exact:true}).fill(n06DenyPurpose);
   await mutation(suppress.getByRole("button",{name:"Запретить контакт",exact:true}),"SuppressCommunication");
   const authCard=page.locator("section").filter({hasText:n06DenyPurpose});
   await authCard.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await authCard.getByRole("heading",{name:"Контакт не разрешён",exact:true}).waitFor();
   await authCard.getByText("Действующий запрет",{exact:true}).waitFor();
   const consentReadAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await reloadDocument();
   const authCardAfterReload=page.locator("section").filter({hasText:n06DenyPurpose});
   await authCardAfterReload.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await authCardAfterReload.getByRole("heading",{name:"Контакт не разрешён",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     consentReadAudit,"Consent card Evaluate must remain read-only after reload");
   checkpoint("N-06 named Party filter and original ContactPoint chooser, unverified Email disabled; exact consent card Evaluate after reload with no read audit");
   const evaluate=page.locator("section").filter({has:page.getByRole("heading",{name:"Проверить возможность контакта",exact:true})});
   await selectPagedPicker(evaluate,"Участник",partyId);
   const scopeContact=evaluate.getByLabel("Контакт (необязательно)",{exact:true});
   await scopeContact.locator('option[value="'+browserSalesContactId+'"]').waitFor({state:"attached"});
   await scopeContact.selectOption(browserSalesContactId);
   await evaluate.getByLabel("Цель",{exact:true}).fill(n06DenyPurpose);
   await evaluate.getByRole("button",{name:"Проверить разрешение",exact:true}).click();
   await page.getByRole("heading",{name:"Контакт не разрешён",exact:true}).waitFor();
   checkpoint("Quality and consent evaluation/suppression");
   // #471 S5b Consent DENY: verify immutable original commit after HTTP ack loss.
   // The recovery must never issue a second suppression or create a GRANT.
   await page.getByRole("button",{name:"Запретить коммуникацию",exact:true}).click();
   const s5bConsentDialog=page.getByRole("dialog");
   await selectPagedPicker(s5bConsentDialog,"Участник",partyId);
   await s5bConsentDialog.getByLabel("Цель",{exact:true}).fill("s5b_recovery_"+runId);
   const s5bConsentBefore=new Set((await sql("SELECT id FROM crm_consent_records WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   let s5bConsentCalls=0,s5bConsentRequestId;
   const loseConsentAck=async route=>{
     s5bConsentCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"SuppressCommunication");
     s5bConsentRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   };
   const s5bConsentPath="**/crm.consents.v2.ConsentAuthorizationService/SuppressCommunication";
   await page.route(s5bConsentPath,loseConsentAck);
   await s5bConsentDialog.getByRole("button",{name:"Запретить контакт",exact:true}).click();
   await s5bConsentDialog.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(s5bConsentDialog.getByRole("button",{name:"Запретить контакт",exact:true})).toBeDisabled();
   const s5bConsentMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.consents.pending.v1")));
   assert.deepEqual(Object.keys(s5bConsentMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(s5bConsentMarker.expectedVersion,"0");
   const s5bConsentCreated=(await sql("SELECT id FROM crm_consent_records WHERE tenant='"+tenantSql+"'"))
     .filter(x=>!s5bConsentBefore.has(x.id));
   assert.equal(s5bConsentCreated.length,1);
   assert.equal(s5bConsentMarker.resourceId,s5bConsentCreated[0].id);
   const acceptConsentReload=dialog=>void dialog.accept();
   page.on("dialog",acceptConsentReload);
   try{await reloadDocument();}finally{page.off("dialog",acceptConsentReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   await expect(page.getByRole("button",{name:"Запретить коммуникацию",exact:true})).toBeDisabled();
   const s5bConsentRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetConsentAuthorization")
     &&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const s5bConsentChecked=await s5bConsentRead;
   assert.equal(s5bConsentChecked.status(),200);
   const s5bConsentReadId=(await s5bConsentChecked.headers())["x-request-id"];
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(s5bConsentRequestId);
   assert.equal(s5bConsentCalls,1);
   await assertAuditRecovery(s5bConsentRequestId,s5bConsentReadId,"consents.operator_suppress","consent");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.consents.pending.v1")),null);
   ownerRecovery.push({owner:"consent",kind:"lookup",original_request_id:s5bConsentRequestId,
     subsequent_request_id:s5bConsentReadId});
   await page.unroute(s5bConsentPath,loseConsentAck);
   checkpoint("S5b consent DENY lost acknowledgment: read-only receipt after reload, no duplicate write");
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
   // A service token is deliberately not a human identity. Run the positive
   // signed-human E2E only in the native workerd session; live service-token
   // deployment still checks its unchanged published release and fails closed.
   if(!adminIdentity.startsWith("service:")){
   // #352 R4a: one real Chromium flow, not just a new RPC:
   // signed human email -> original ContactPoint verification -> submitted
   // PrivacyCase -> original Rust subject binding -> D1 audit/readback.
   // The earlier Account scenario uses an Organization. Privacy subjects must
   // be a *Person*, so create one through the real generated-client UI.
   const privacyPersonName="Browser Privacy Person "+runId;
   await navigate("parties","Люди и организации");
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   await page.getByLabel("Тип",{exact:true}).selectOption("1");
   await page.getByLabel("Имя или название",{exact:true}).fill(privacyPersonName);
   await mutation(page.getByRole("button",{name:"Сохранить участника",exact:true}),"CreateParty");
   const [privacyPerson]=await sql("SELECT id FROM crm_party_records "+
     "WHERE tenant=? AND json_extract(state,'$.display_name')=?",tenantA,privacyPersonName);
   assert.ok(privacyPerson?.id);
   const privacyPartyId=privacyPerson.id;
   await navigate("contacts","Контакты");
   await page.getByRole("button",{name:"Создать контакт",exact:true}).click();
   await selectPagedPicker(page,"Участник",privacyPartyId);
   await page.getByLabel("Значение контакта",{exact:true}).fill(adminIdentity);
   await mutation(page.getByRole("button",{name:"Сохранить контакт",exact:true}),"CreateContactPoint");
   await selectPagedPicker(page,"Фильтр по участнику",privacyPartyId);
   await page.getByRole("heading",{name:adminIdentity,exact:true}).waitFor();
   const ownContactSection=page.locator("section").filter({
     has:page.getByRole("heading",{name:adminIdentity,exact:true})
   });
   await mutation(ownContactSection.getByRole("button",{
     name:"Подтвердить email (Access / тестовый администратор)",exact:true
   }),"VerifyOwnEmailContactPoint");
   const ownContactRows=await sql(
     "SELECT id FROM crm_contact_point_records WHERE tenant=? AND party_id=? "+
     "AND json_extract(state,'$.normalized_value')=?",tenantA,privacyPartyId,adminIdentity);
   assert.equal(ownContactRows.length,1);
   const ownContactId=ownContactRows[0].id;
   await navigate("privacy","Запросы о данных");
   const r4CaseBefore=new Set((await sql("SELECT id FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   await page.getByRole("button",{name:"Новое обращение",exact:true}).click();
   await mutation(page.getByRole("button",{name:"Создать обращение",exact:true}),"CreatePrivacyCase");
   const r4Cases=(await sql("SELECT id FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"'"))
     .filter(x=>!r4CaseBefore.has(x.id));
   assert.equal(r4Cases.length,1);
   const r4CaseId=r4Cases[0].id;
   const r4Card=page.locator("section").filter({has:page.getByText("ID: "+r4CaseId,{exact:true})});
   await r4Card.getByRole("button",{name:"Открыть запрос",exact:true}).click();
   await mutation(page.getByRole("button",{name:"Подать запрос",exact:true}),"SubmitPrivacyCase");
   await r4Card.getByRole("button",{name:"Открыть запрос",exact:true}).click();
   await selectPagedPicker(page,"Мой участник (Person)",privacyPartyId);
   await page.getByLabel("ID проверенного email ContactPoint",{exact:true}).fill(ownContactId);
   await mutation(page.getByRole("button",{name:"Подтвердить свою личность",exact:true}),
     "VerifyOwnPrivacyCaseSubject");
   const [r4State]=await sql("SELECT version,json_extract(state,'$.status.code') AS status, "+
     "json_extract(state,'$.subject_binding.canonical_party_id') AS party_id "+
     "FROM crm_privacy_case_records WHERE tenant=? AND id=?",tenantA,r4CaseId);
   assert.equal(r4State.version,3);assert.equal(r4State.status,"subject_verified");
   assert.equal(r4State.party_id,privacyPartyId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? "+
     "AND subject_id=? AND action='customer_privacy.subject.verify_own'",tenantA,r4CaseId))[0].n,1);
   checkpoint("R4a signed human ContactPoint -> original PrivacyCase subject verification, D1 receipt and audit");
   // Only the verified human can inspect the current two-owner preview.
   // It must remain incomplete (seven other canonical owner contributions)
   // and must not create a D1 case receipt or audit write.
   await r4Card.getByRole("button",{name:"Открыть запрос",exact:true}).click();
   const beforeScopeReads=(await sql("SELECT COUNT(*) AS n FROM crm_audit "+
     "WHERE tenant_id=? AND subject_id=?",tenantA,r4CaseId))[0].n;
   await page.getByRole("button",{
     name:"Посмотреть предварительный состав моих данных",exact:true
   }).click();
   await page.getByText("Неполный предварительный состав",{exact:true}).waitFor();
   await page.getByText(/Проверено владельцев: 2 из 9/).waitFor();
   await expect(page.getByText(/Ещё не охвачены: crm\./)).toBeVisible();
   const afterScopeReads=(await sql("SELECT COUNT(*) AS n FROM crm_audit "+
     "WHERE tenant_id=? AND subject_id=?",tenantA,r4CaseId))[0].n;
   assert.equal(afterScopeReads,beforeScopeReads,"Privacy preview must be read-only in real Chromium");
   await page.getByRole("button",{name:"Закрыть панель",exact:true}).click();
   checkpoint("R4a own-scope preview: original 2/9 owner contributions, seven explicitly missing, D1 read-only");

   }else{
     checkpoint("R4a human-only positive verification reserved for signed human browser; service principal cannot verify");
   }

   // #471 S5b Privacy: a committed intake write loses its browser acknowledgment.
   // Reload must use original signed GetPrivacyCase receipt provenance, never
   // repeat CreatePrivacyCase or store the subject's payload in sessionStorage.
   await page.getByRole("button",{name:"Новое обращение",exact:true}).click();
   const s5bPrivacyBefore=new Set((await sql("SELECT id FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   let s5bPrivacyCalls=0, s5bPrivacyRequestId;
   const losePrivacyAck=async route=>{
     s5bPrivacyCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"CreatePrivacyCase");
     s5bPrivacyRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   };
   await page.route("**/crm.customer_privacy.v2.PrivacyCaseService/CreatePrivacyCase",losePrivacyAck);
   await page.getByRole("button",{name:"Создать обращение",exact:true}).click();
   await page.getByRole("dialog").getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(page.getByRole("button",{name:"Создать обращение",exact:true})).toBeDisabled();
   const s5bPrivacyMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.privacy.pending.v1")));
   assert.deepEqual(Object.keys(s5bPrivacyMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(s5bPrivacyMarker.expectedVersion,"0");
   const s5bPrivacyNew=(await sql("SELECT id FROM crm_privacy_case_records WHERE tenant='"+tenantSql+"'"))
     .filter(x=>!s5bPrivacyBefore.has(x.id));
   assert.equal(s5bPrivacyNew.length,1);
   assert.equal(s5bPrivacyMarker.resourceId,s5bPrivacyNew[0].id);
   const acceptPrivacyReload=dialog=>void dialog.accept();
   page.on("dialog",acceptPrivacyReload);
   try {await reloadDocument();} finally {page.off("dialog",acceptPrivacyReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   await expect(page.getByRole("button",{name:"Новое обращение",exact:true})).toBeDisabled();
   const s5bPrivacyRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetPrivacyCase")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const s5bPrivacyChecked=await s5bPrivacyRead;
   assert.equal(s5bPrivacyChecked.status(),200);
   const s5bPrivacyReadId=(await s5bPrivacyChecked.headers())["x-request-id"];
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(s5bPrivacyRequestId);
   assert.equal(s5bPrivacyCalls,1);
   await assertAuditRecovery(s5bPrivacyRequestId,s5bPrivacyReadId,"customer_privacy.create","privacy");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.privacy.pending.v1")),null);
   ownerRecovery.push({owner:"privacy",kind:"lookup",original_request_id:s5bPrivacyRequestId,
     subsequent_request_id:s5bPrivacyReadId});
   await page.unroute("**/crm.customer_privacy.v2.PrivacyCaseService/CreatePrivacyCase",losePrivacyAck);
   checkpoint("S5b Privacy lost acknowledgement: reload + immutable GetPrivacyCase provenance without duplicate create");
   await navigate("enrichment","Обогащение данных");
   const providerKey="browser_registry_"+runId;
   const profileBefore=new Set((await sql("SELECT id FROM crm_enrichment_profiles WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   let s5bProfileCalls=0,s5bProfileRequestId;
   const loseProfileAck=async route=>{
     s5bProfileCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"PublishProviderProfile");
     s5bProfileRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   };
   const s5bProfilePath="**/crm.customer_enrichment.v2.EnrichmentConfigurationService/PublishProviderProfile";
   await page.route(s5bProfilePath,loseProfileAck);
   await page.getByRole("button",{name:"Новый профиль",exact:true}).click();
   await page.getByLabel("Ключ провайдера",{exact:true}).fill(providerKey);
   await page.getByLabel("Лицензия",{exact:true}).fill("Browser test license");
   await page.getByLabel("Имена ссылок на учётные данные (не секреты)",{exact:true})
     .fill("browser-credential-handle-"+runId);
   await page.getByLabel("Начало действия",{exact:true}).fill("2026-11-12T12:00");
   assert.equal(await page.getByLabel("Начало действия",{exact:true}).inputValue(),"2026-11-12T12:00");
   const profileDialog=page.getByRole("dialog");
   await profileDialog.getByRole("button",{name:"Опубликовать профиль",exact:true}).click();
   await profileDialog.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(profileDialog.getByRole("button",{name:"Опубликовать профиль",exact:true})).toBeDisabled();
   const s5bProfileMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.enrichment.profile.pending.v1")));
   assert.deepEqual(Object.keys(s5bProfileMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(s5bProfileMarker.expectedVersion,"0");
   assert.equal(s5bProfileMarker.resourceId,providerKey);
   const s5bProfileCreated=(await sql("SELECT id FROM crm_enrichment_profiles WHERE tenant='"+tenantSql+"'"))
     .filter(x=>!profileBefore.has(x.id));
   assert.equal(s5bProfileCreated.length,1);
   syntheticPrivateProbes.push(
     {kind:"enrichment-provider-key",request_id:s5bProfileRequestId,value:providerKey},
     {kind:"enrichment-license",request_id:s5bProfileRequestId,value:"Browser test license"},
     {kind:"enrichment-credential-handle",request_id:s5bProfileRequestId,
       value:"browser-credential-handle-"+runId});
   const acceptProfileReload=dialog=>void dialog.accept();
   page.on("dialog",acceptProfileReload);
   try{await reloadDocument();}finally{page.off("dialog",acceptProfileReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   await expect(page.getByRole("button",{name:"Новый профиль",exact:true})).toBeDisabled();
   const s5bProfileRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetProviderProfile")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const s5bProfileChecked=await s5bProfileRead;
   assert.equal(s5bProfileChecked.status(),200);
   const s5bProfileReadId=(await s5bProfileChecked.headers())["x-request-id"];
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(s5bProfileRequestId);
   assert.equal(s5bProfileCalls,1,"Enrichment profile recovery must not publish twice");
   await assertAuditRecovery(s5bProfileRequestId,s5bProfileReadId,"customer_enrichment.profile_publish","enrichment-profile");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.enrichment.profile.pending.v1")),null);
   ownerRecovery.push({owner:"enrichment-profile",kind:"lookup",original_request_id:s5bProfileRequestId,
     subsequent_request_id:s5bProfileReadId});
   await page.unroute(s5bProfilePath,loseProfileAck);
   const profileId=await firstId("crm_enrichment_profiles","provider_key",providerKey);
   await page.getByRole("button",{name:providerKey+" · 1.0.0",exact:true}).click();
   await page.getByText("ID: "+profileId,{exact:true}).waitFor();
   await page.getByRole("button",{name:"Закрыть панель",exact:true}).click();
   await page.getByRole("dialog").waitFor({state:"hidden"});

   const mappingKey="browser_mapping_"+runId;
   const mappingBefore=new Set((await sql("SELECT id FROM crm_enrichment_mappings WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   let s5bMappingCalls=0,s5bMappingRequestId;
   const loseMappingAck=async route=>{
     s5bMappingCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"PublishMapping");
     s5bMappingRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   };
   const s5bMappingPath="**/crm.customer_enrichment.v2.EnrichmentConfigurationService/PublishMapping";
   await page.route(s5bMappingPath,loseMappingAck);
   await page.getByRole("button",{name:"Новое сопоставление",exact:true}).click();
   await page.getByLabel("Версия профиля",{exact:true}).selectOption(profileId);
   await page.getByLabel("Ключ сопоставления",{exact:true}).fill(mappingKey);
   const mappingDialog=page.getByRole("dialog");
   await mappingDialog.getByRole("button",{name:"Опубликовать сопоставление",exact:true}).click();
   await mappingDialog.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(mappingDialog.getByRole("button",{name:"Опубликовать сопоставление",exact:true})).toBeDisabled();
   const s5bMappingMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.enrichment.mapping.pending.v1")));
   assert.deepEqual(Object.keys(s5bMappingMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(s5bMappingMarker.expectedVersion,"0");
   assert.equal(s5bMappingMarker.resourceId,mappingKey);
   const s5bMappingCreated=(await sql("SELECT id FROM crm_enrichment_mappings WHERE tenant='"+tenantSql+"'"))
     .filter(x=>!mappingBefore.has(x.id));
   assert.equal(s5bMappingCreated.length,1);
   syntheticPrivateProbes.push({kind:"enrichment-mapping-key",request_id:s5bMappingRequestId,value:mappingKey});
   const acceptMappingReload=dialog=>void dialog.accept();
   page.on("dialog",acceptMappingReload);
   try{await reloadDocument();}finally{page.off("dialog",acceptMappingReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   await expect(page.getByRole("button",{name:"Новое сопоставление",exact:true})).toBeDisabled();
   const s5bMappingRead=page.waitForResponse(r=>responsePathEndsWith(r,"/GetMapping")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const s5bMappingChecked=await s5bMappingRead;
   assert.equal(s5bMappingChecked.status(),200);
   const s5bMappingReadId=(await s5bMappingChecked.headers())["x-request-id"];
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(s5bMappingRequestId);
   assert.equal(s5bMappingCalls,1,"Enrichment mapping recovery must not publish twice");
   await assertAuditRecovery(s5bMappingRequestId,s5bMappingReadId,"customer_enrichment.mapping_publish","enrichment-mapping");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.enrichment.mapping.pending.v1")),null);
   ownerRecovery.push({owner:"enrichment-mapping",kind:"lookup",original_request_id:s5bMappingRequestId,
     subsequent_request_id:s5bMappingReadId});
   await page.unroute(s5bMappingPath,loseMappingAck);
   const mappingId=await firstId("crm_enrichment_mappings","mapping_key",mappingKey);
   await page.getByRole("button",{name:mappingKey,exact:true}).click();
   await page.getByText("ID: "+mappingId+"; поле: organization.legal_name; профиль: "+profileId,{exact:true}).waitFor();

   await navigate("data","Импорт и экспорт");
   const brokenCsv="party_id,kind,display_name,resource_version\r\n"+
     "same-key,invalid,Wrong,0\r\n"+
     "same-key,person,Second,1\r\n"+
     "empty-name,organization,,1\r\n";
   const n01AuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   const n01PartiesBefore=(await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+"'"))[0].n;
   await page.getByLabel("CSV-файл",{exact:true}).setInputFiles({
     name:"invalid.csv",mimeType:"text/csv",buffer:Buffer.from(brokenCsv)});
   await page.getByRole("button",{name:"Проверить импорт",exact:true}).click();
   const csvErrors=page.getByRole("region",{name:"Ошибки CSV"});
   await csvErrors.getByText(/Найдено ошибок: 4/).waitFor();
   await csvErrors.getByText(/Строка данных 1 · Версия исходной записи: Укажите положительное целое число/).waitFor();
   await csvErrors.getByText(/Строка данных 1 · Тип участника: Допустимы только person и organization/).waitFor();
   await csvErrors.getByText(/Строка данных 2 · ID исходной системы: ID пустой или повторяется/).waitFor();
   await csvErrors.getByText(/Строка данных 3 · Имя или название: Недопустимое или пустое/).waitFor();
   assert.equal(await page.getByRole("button",{name:"Подтвердить импорт",exact:true}).count(),0,
     "Invalid CSV preview must never offer an Apply action");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,n01AuditBefore);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+"'"))[0].n,n01PartiesBefore);
   checkpoint("N-01 invalid CSV shows all original row+field reasons, never enables Apply or writes");
   const csv="party_id,kind,display_name,resource_version\r\nbrowser-import,person,Browser Import "+runId+",1\r\n";
   const obsoleteCsv=csv.replace("Browser Import ","Browser Obsolete ");
   await page.getByLabel("CSV-файл",{exact:true}).setInputFiles({
     name:"obsolete.csv",mimeType:"text/csv",buffer:Buffer.from(obsoleteCsv)});
   await page.getByLabel("CSV-файл",{exact:true}).setInputFiles({
     name:"browser.csv",mimeType:"text/csv",buffer:Buffer.from(csv)});
   await page.getByRole("button",{name:"Проверить импорт",exact:true}).click();
   await page.getByRole("button",{name:"Подтвердить импорт",exact:true}).waitFor();
   const csvBefore=new Set((await sql("SELECT id FROM crm_party_records WHERE tenant='"+tenantSql+"'")).map(x=>x.id));
   let s5bCsvCalls=0,s5bCsvRequestId;
   const loseCsvAck=async route=>{
     s5bCsvCalls++;
     const committed=await route.fetch();
     recordResponse(committed);
     await assertMutation(committed,"ApplyPartyImport");
     s5bCsvRequestId=committed.headers()["x-request-id"];
     await route.abort("failed");
   };
   const s5bCsvPath="**/crm.customer_data_operations.v2.PartyCsvService/ApplyPartyImport";
   await page.route(s5bCsvPath,loseCsvAck);
   await page.getByRole("button",{name:"Подтвердить импорт",exact:true}).click();
   await page.getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(page.getByRole("button",{name:"Подтвердить импорт",exact:true})).toBeDisabled();
   const s5bCsvMarker=await page.evaluate(()=>JSON.parse(sessionStorage.getItem("crm.data.csv-import.pending.v1")));
   assert.deepEqual(Object.keys(s5bCsvMarker).sort(),["expectedVersion","key","resourceId"]);
   assert.equal(s5bCsvMarker.expectedVersion,"0");
   const s5bCsvReceipt=(await sql("SELECT source_sha256,row_count,request_id FROM crm_party_import_receipts WHERE tenant='"+
     tenantSql+"' AND key='"+s5bCsvMarker.key+"'"));
   assert.equal(s5bCsvReceipt.length,1);
   assert.equal(s5bCsvMarker.resourceId,s5bCsvReceipt[0].source_sha256);
   assert.equal(s5bCsvReceipt[0].row_count,1);
   assert.equal(s5bCsvReceipt[0].request_id,s5bCsvRequestId);
   syntheticPrivateProbes.push(
     {kind:"csv-party-name",request_id:s5bCsvRequestId,value:"Browser Import "+runId},
     {kind:"csv-payload",request_id:s5bCsvRequestId,value:csv});
   const csvCreated=(await sql("SELECT id FROM crm_party_records WHERE tenant='"+tenantSql+"'"))
     .filter(x=>!csvBefore.has(x.id));
   assert.equal(csvCreated.length,1);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+
     "' AND json_extract(state,'$.display_name')='Browser Obsolete "+runId+"'"))[0].n,0,
     "A replaced CSV file must never commit its superseded records");
   const storedSession=await page.evaluate(()=>JSON.stringify(
     Object.keys(sessionStorage).sort().map(key=>[key,sessionStorage.getItem(key)])));
   assert.ok(!storedSession.includes("Browser Import "+runId));
   assert.ok(!storedSession.includes("party_id,kind,display_name"));
   const acceptCsvReload=dialog=>void dialog.accept();
   page.on("dialog",acceptCsvReload);
   try{await reloadDocument();}finally{page.off("dialog",acceptCsvReload);}
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).waitFor();
   assert.equal(await page.getByRole("button",{name:"Подтвердить импорт",exact:true}).count(),0,
     "CSV payload is not restored after reload; only receipt lookup is available");
   const s5bCsvRead=page.waitForResponse(r=>responsePathEndsWith(r,"/PreviewPartyImport")&&r.request().method()==="POST");
   await page.getByRole("button",{name:"Проверить результат записи",exact:true}).click();
   const s5bCsvChecked=await s5bCsvRead;
   assert.equal(s5bCsvChecked.status(),200);
   const s5bCsvReadId=(await s5bCsvChecked.headers())["x-request-id"];
   await expect(page.getByRole("heading",{name:/Сохранено ранее. Подтверждение найдено/}))
     .toContainText(s5bCsvRequestId);
   assert.equal(s5bCsvCalls,1,"CSV recovery must not apply the import twice");
   await assertAuditRecovery(s5bCsvRequestId,s5bCsvReadId,"parties.create","party-csv-import");
   assert.equal(await page.evaluate(()=>sessionStorage.getItem("crm.data.csv-import.pending.v1")),null);
   ownerRecovery.push({owner:"party-csv-import",kind:"lookup",original_request_id:s5bCsvRequestId,
     subsequent_request_id:s5bCsvReadId});
   await page.unroute(s5bCsvPath,loseCsvAck);
   assert.ok(await firstId("crm_party_records","display_name","Browser Import "+runId));

   // New-user flow: download the two-column template and apply it using the
   // existing typed N-01 owner, not manually invented source IDs or versions.
   const templatePromise=page.waitForEvent("download");
   await page.getByRole("link",{name:"Скачать простой шаблон CSV",exact:true}).click();
   const template=await templatePromise;
   assert.equal(await template.failure(),null);
   assert.ok((await readFile(await template.path(),"utf8")).startsWith("kind,display_name\r\n"));
   const simpleCsv="kind,display_name\r\nperson,Browser Simple Person "+runId+
     '\r\norganization,"Browser, Simple Organization '+runId+'"\r\n';
   const simpleAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   const simplePartyBefore=(await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+"'"))[0].n;
   await page.getByLabel("CSV-файл",{exact:true}).setInputFiles({
     name:"simple-people.csv",mimeType:"text/csv",buffer:Buffer.from(simpleCsv)});
   await page.getByRole("button",{name:"Проверить импорт",exact:true}).click();
   await page.getByText(/записей 2, ранее импортировано 0/).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,simpleAuditBefore,
     "Simple CSV preview must not write");
   await mutation(page.getByRole("button",{name:"Подтвердить импорт",exact:true}),"ApplyPartyImport");
   assert.ok(await firstId("crm_party_records","display_name","Browser Simple Person "+runId));
   assert.ok(await firstId("crm_party_records","display_name","Browser, Simple Organization "+runId));
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='"+tenantSql+"'"))[0].n,simplePartyBefore+2);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,simpleAuditBefore+2);
   checkpoint("Simple two-column CSV template -> original Rust preview -> atomic Party apply / D1 audit");

   const exportReply=page.waitForResponse(r=>responsePathEndsWith(r,"/PreviewPartyExport"));
   const downloadPending=page.waitForEvent("download");
   await page.getByRole("button",{name:"Скачать CSV",exact:true}).click();
   assert.equal((await exportReply).status(),200);
   const download=await downloadPending;
   assert.equal(download.suggestedFilename(),"crm-parties.csv");
   assert.equal(await download.failure(),null);
   const exported=await readFile(await download.path(),"utf8");
   assert.ok(exported.includes("Browser Import "+runId));
   assert.ok(exported.includes("Browser Updated "+runId));
   checkpoint("S5b Enrichment profile/mapping and CSV import: lost acknowledgments, reload, read-only receipt lookup, no duplicate writes or payload storage");
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
   const conflict=stalePage.waitForResponse(r=>responsePathEndsWith(r,"/UpdateParty"));
   await stalePage.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   const conflictResponse=await conflict;
   assert.equal(conflictResponse.status(),409);
   const conflictHeaders=await conflictResponse.headers();
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
   checkpoint("Two browser forms enforce CAS and show generated-client conflict");

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
   await page.getByRole("dialog").getByRole("heading",{name:/Результат записи не подтверждён/}).waitFor();
   await expect(page.getByRole("button",{name:"Сохранить участника",exact:true})).toBeDisabled();
   const retryId=await firstId("crm_party_records","display_name","Browser Retry "+runId);
   // Only the explicit recovery action may replay the original immutable command.
   await mutation(page.getByRole("dialog").getByRole("button",
     {name:"Повторить исходную операцию",exact:true}),"CreateParty",{replay:true});
   await page.getByRole("heading",{name:/Сохранено ранее/}).waitFor();
   assert.equal(await firstId("crm_party_records","display_name","Browser Retry "+runId),retryId);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"' AND subject_id='"+retryId+"'"))[0].n,1);
   await page.unroute("**/crm.parties.v3.PartyService/CreateParty");
   checkpoint("Committed response loss and explicit same-key recovery replay without duplicate audit");

   // Commit succeeds but only the following list refresh fails.
   await navigate("parties","Люди и организации");
   await page.getByRole("button",{name:"Создать участника",exact:true}).click();
   await page.getByLabel("Имя или название",{exact:true}).fill("Browser Saved Stale "+runId);
   await page.route("**/crm.parties.v3.PartyService/ListParties",route=>route.abort("failed"));
   const created=page.waitForResponse(r=>responsePathEndsWith(r,"/CreateParty"));
   await page.getByRole("button",{name:"Сохранить участника",exact:true}).click();
   await assertMutation(await created,"CreateParty");
   await page.getByRole("heading",{name:/Сохранено.*Список устарел/}).waitFor();
   assert.ok(await firstId("crm_party_records","display_name","Browser Saved Stale "+runId));
   await page.unroute("**/crm.parties.v3.PartyService/ListParties");
   checkpoint("Committed write remains successful when refresh fails");

   const navigationAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   for(const [link,screen,title,filter,value,method] of [
     ["Все контакты участника","contacts","Контакты","Фильтр по участнику",partyId,"ListContactPoints"],
     ["Все связи участника","relationships","Связи","Фильтр по участнику",partyId,"ListPartyRelationships"],
     ["Все сделки клиента","sales","Сделки","Фильтр по клиенту",accountId,"ListDeals"],
     ["Все задачи клиента","tasks","Задачи","Фильтр по клиенту",accountId,"ListTasks"],
   ]){
     await navigate("accounts","Клиенты");
     await search("Поиск клиентов",accountId,"ListAccounts");
     const [overviewResult]=await Promise.all([
       page.waitForResponse(r=>responsePathEndsWith(r,"/GetAccountOverview")),
       page.locator("section").filter({has:page.getByRole("heading",
         {name:"Browser Account "+runId,level:2,exact:true})})
         .getByRole("button",{name:"Открыть карточку",exact:true}).click(),
     ]);
     assert.equal(overviewResult.status(),200);
     // The new global Sales overview is backed by ListDeals regardless of
     // tenant pipeline selection. Stage pages are fetched only when a board
     // is explicitly selected.
     const salesBoard=screen==="sales";
     const pending=page.waitForResponse(r=>responsePathEndsWith(r,"/"+method)&&r.request().method()==="POST");
     await page.getByRole("link",{name:link,exact:true}).click();
     assert.equal((await pending).status(),200);
     await page.getByRole("heading",{name:title,exact:true}).waitFor();
     assert.equal(await page.getByLabel(filter,{exact:true}).inputValue(),value);
     await expect(page.getByLabel(filter,{exact:true}).locator("option:checked"))
       .toHaveText((screen==="sales"||screen==="tasks"?
         "Browser Account ":"Browser Winner ")+runId);
     assert.equal(new URL(await page.url()).hash,"#"+screen+"?"+(screen==="sales"||screen==="tasks"?"accountId":"partyId")+"="+encodeURIComponent(value));
     if(salesBoard){
       assert.equal(await page.getByLabel("Воронка продаж",{exact:true}).inputValue(),"",
         "Customer360 must open the all-pipelines overview by default");
       const all=page.locator('section[aria-label="Сделки клиента во всех воронках"]');
       await all.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
       await all.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
       // Selecting a specific Kanban must not hide Account Deals in other pipelines.
       const stageRead=page.waitForResponse(r=>responsePathEndsWith(r,"/ListStageDeals")&&
         r.request().method()==="POST");
       await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
       assert.equal((await stageRead).status(),200);
       await all.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
     }
     const reloaded=page.waitForResponse(r=>responsePathEndsWith(r,"/"+method)&&r.request().method()==="POST");
     await reloadDocument();assert.equal((await reloaded).status(),200);
     assert.equal(await page.getByLabel(filter,{exact:true}).inputValue(),value,"Reload must retain the linked scope");
     await expect(page.getByLabel(filter,{exact:true}).locator("option:checked"))
       .toHaveText((screen==="sales"||screen==="tasks"?
         "Browser Account ":"Browser Winner ")+runId);
     if(salesBoard){
       assert.equal(await page.getByLabel("Воронка продаж",{exact:true}).inputValue(),newPipelineId,
         "Reload must restore the board explicitly selected from Customer360");
       assert.equal(new URLSearchParams(new URL(page.url()).hash.split("?")[1]).get("pipelineId"),
         newPipelineId,"Customer360 scope and the explicit Kanban must share the native route");
       const all=page.locator('section[aria-label="Сделки клиента во всех воронках"]');
       await all.getByRole("heading",{name:"Browser Deal "+runId,exact:true}).waitFor();
       await all.getByRole("heading",{name:"Browser Alternate Deal "+runId,exact:true}).waitFor();
     }
     // S-M1-B: reopening a create dialog must restore the Account scope
     // from Customer360. Neither picker should retain a cancelled choice.
     if(salesBoard||screen==="tasks"){
       const action=page.getByRole("button",{
         name:salesBoard?"Новая сделка":"Новая задача",exact:true
       });
       if(salesBoard)await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
       await action.click();
       const draftAccount=page.getByRole("dialog").getByLabel("Клиент",{exact:true});
       await expect(draftAccount).toHaveValue(accountId);
       if(salesBoard)await page.getByLabel("Основной контакт сделки",{exact:true})
         .locator('option[value="'+browserSalesContactId+'"]').waitFor({state:"attached"});
       await draftAccount.selectOption("");
       await page.getByRole("dialog").getByRole("button",{name:"Закрыть панель"}).click();
       await action.click();
       await expect(page.getByRole("dialog").getByLabel("Клиент",{exact:true})).toHaveValue(accountId);
       await page.getByRole("dialog").getByRole("button",{name:"Закрыть панель"}).click();
       assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
         navigationAuditBefore,"Cancelled scoped create must not commit an operation");
     }
     if(screen==="tasks")await page.getByRole("heading",{name:"Browser Edited Urgent "+runId,exact:true}).waitFor();
   }
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,navigationAuditBefore);
   checkpoint("Customer360 links to server-filtered full lists, reload retains scope, no writes");
   // Product-first M1-08: a Deal can share an Account with several other
   // Deals and manual Tasks. Its page must expose its OWN linked Tasks,
   // not silently show all Account tasks or a generic resource-type picker.
   const dealTaskId=(await sql("SELECT task_id FROM crm_sales_link_delivery_records WHERE tenant='"+
     tenantSql+"' AND deal_id='"+dealId+"' AND disposition='applied' LIMIT 1"))[0]?.task_id;
   assert.ok(dealTaskId,"one original stage-linked Task must exist");
   const beforeDealTaskNavigation=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+
     tenantSql+"'"))[0].n;
   await page.goto(base+"/#sales/"+encodeURIComponent(dealId));
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   await page.getByRole("link",{name:"Задачи этой сделки",exact:true}).click();
   await page.getByRole("heading",{name:"Задачи сделки",exact:true}).waitFor();
   await page.getByText("Browser Deal "+runId,{exact:true}).waitFor();
   assert.equal(new URL(page.url()).hash,"#tasks?accountId="+encodeURIComponent(accountId)+
     "&dealId="+encodeURIComponent(dealId));
   const scopedTaskLink=page.getByRole("link",{name:"Открыть карточку задачи",exact:true});
   await scopedTaskLink.waitFor({state:"visible"});
   assert.equal(await scopedTaskLink.count(),1,
     "The Deal shows only its own linked Task, not all Account Tasks");
   assert.equal(new URL(await scopedTaskLink.getAttribute("href"),base).hash,
     "#tasks?accountId="+encodeURIComponent(accountId)+"&taskId="+encodeURIComponent(dealTaskId));
   await reloadDocument();
   await page.getByRole("heading",{name:"Задачи сделки",exact:true}).waitFor();
   await scopedTaskLink.waitFor({state:"visible"});
   assert.equal(await scopedTaskLink.count(),1);
   await scopedTaskLink.click();
   await page.getByRole("heading",{name:"Карточка задачи",exact:true}).waitFor();
   await page.getByRole("link",{name:"Открыть сделку",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+
     tenantSql+"'"))[0].n,beforeDealTaskNavigation,
     "Deal→Task→Deal browsing must never create an audit write");
   checkpoint("Deal→original linked Task→Deal uses exact typed owner filter and reload without audit");

   // M1-08: a real, manually created Task belongs to this Deal rather than
   // merely to its Account. The form fixes the tenant-bound Account context.
   await page.getByRole("link",{name:"Задачи этой сделки",exact:true}).click();
   await page.getByRole("heading",{name:"Задачи сделки",exact:true}).waitFor();
   await page.getByRole("button",{name:"Новая задача сделки",exact:true}).click();
   const dealTaskForm=page.getByRole("dialog");
   await expect(dealTaskForm.getByLabel("Клиент",{exact:true})).toHaveCount(0);
   await dealTaskForm.getByLabel("Тема задачи",{exact:true}).fill("Browser Manual Deal Task "+runId);
   await mutation(dealTaskForm.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const manualTaskId=await firstId("crm_activity_task_records","subject","Browser Manual Deal Task "+runId);
   const manualState=(await sql("SELECT account_id,json_extract(state,'$.related_resources') AS refs FROM crm_activity_task_records WHERE tenant='"+
     tenantSql+"' AND id='"+manualTaskId+"'"))[0];
   assert.equal(manualState.account_id,accountId);
   assert.ok(JSON.parse(manualState.refs).some(ref=>ref.resource_type==="sales.deal"&&ref.resource_id===dealId));
   const manualTaskCard=page.getByRole("heading",{name:"Browser Manual Deal Task "+runId,exact:true});
   await manualTaskCard.waitFor();
   await reloadDocument();
   await page.getByRole("heading",{name:"Задачи сделки",exact:true}).waitFor();
   await manualTaskCard.waitFor();
   assert.equal(await page.getByRole("link",{name:"Открыть карточку задачи",exact:true}).count(),2);
   await page.locator('a[href="#tasks?accountId='+encodeURIComponent(accountId)+'&taskId='+encodeURIComponent(manualTaskId)+'"]').click();
   await page.getByRole("heading",{name:"Карточка задачи",exact:true}).waitFor();
   await page.getByRole("link",{name:"Открыть сделку",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка сделки",exact:true}).waitFor();
   checkpoint("Manual Deal Task→original Task owner→Deal survives reload, exactly one receipt and audit");

   // SK-06: the user sees one due-ordered, tenant-authorized workday,
   // follows the original Task record and returns to the SAME selected date.
   const workdayReadAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await navigate("workday?date=2026-11-23","Рабочий день");
   await expect(page.getByLabel("Дата рабочего дня",{exact:true})).toHaveValue("2026-11-23");
   const workdayPast=page.getByRole("region",{name:"До выбранной даты"});
   const workdayFuture=page.getByRole("region",{name:"Ближайшие после даты"});
   const workdayUndated=page.getByRole("region",{name:"Без срока"});
   const workdayRecent=page.getByRole("region",{name:"Последние взаимодействия"});
   await workdayPast.getByRole("link",{name:"Native overdue Z",exact:true}).waitFor();
   await workdayFuture.getByRole("link",{name:"SK06 Workday earliest",exact:true}).waitFor();
   await workdayUndated.getByRole("heading",{name:"Без срока",exact:true}).waitFor();
   await workdayRecent.getByText(accountNote,{exact:true}).waitFor();
   assert.ok(await workdayRecent.getByRole("link",{name:"Browser Account "+runId,exact:true}).count()>0);
   await workdayFuture.getByRole("link",{name:"SK06 Workday earliest",exact:true}).click();
   await page.getByRole("heading",{name:"Карточка задачи",exact:true}).waitFor();
   await page.goBack();
   await page.getByRole("heading",{name:"Рабочий день",exact:true}).waitFor();
   await expect(page.getByLabel("Дата рабочего дня",{exact:true})).toHaveValue("2026-11-23");
   await reloadDocument();
   await workdayRecent.getByText(accountNote,{exact:true}).waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     workdayReadAudit,"SK-06 workday Task/Interaction reads and Back/reload must not write audit");
   await page.screenshot({path:resolve(directory,"crm-workday.png"),fullPage:true});
   checkpoint("SK-06 typed workday: due-before-LIMIT, linked Task/Account/Deal, recent across tenant, Back/reload, read-only");

   // SK-07: complete portfolio analysis from existing Sales owner metrics,
   // not the visible first Deal pages. Audited read-only, URL-owned period.
   const intelligenceAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n;
   await navigate("insights","Аналитика бизнеса");
   const portfolio=page.getByRole("region",{name:"Общий портфель"});
   const risks=page.getByRole("region",{name:"Операционные риски"});
   const portfolioForecast=page.getByRole("region",{name:"Прогноз портфеля"});
   const conversion=page.getByRole("region",{name:"Фактическая конверсия портфеля"});
   await portfolio.getByText(/Сделок:/).waitFor();
   await risks.getByText(/Просрочена ожидаемая дата закрытия:/).waitFor();
   await portfolioForecast.getByRole("heading",{name:"Прогноз и категории"}).waitFor();
   await conversion.getByText(/Знаменатель:/).waitFor();
   const portfolioLink=page.getByRole("link",{name:"Открыть воронку Browser Pipeline "+runId,exact:true});
   await portfolioLink.waitFor();
   await portfolioLink.click();
   await page.getByRole("heading",{name:"Сделки",exact:true}).waitFor();
   await expect(page.getByLabel("Воронка продаж",{exact:true})).toHaveValue(newPipelineId);
   await page.goBack();
   await page.getByRole("heading",{name:"Аналитика бизнеса",exact:true}).waitFor();
   await page.getByLabel("Начало периода",{exact:true}).fill("2026-11-01");
   await page.getByLabel("Конец периода",{exact:true}).fill("2026-11-30");
   await page.getByRole("button",{name:"Применить период",exact:true}).click();
   await expect(page.getByLabel("Начало периода",{exact:true})).toHaveValue("2026-11-01");
   assert.ok(new URL(page.url()).hash.includes("start=2026-11-01"));
   await page.getByLabel("Сравнить с предыдущим периодом той же длительности").check();
   await conversion.getByText(/Предыдущий период:/).waitFor();
   await reloadDocument();
   await conversion.getByText(/Предыдущий период:/).waitFor();
   await page.getByRole("heading",{name:"Аналитика бизнеса",exact:true}).waitFor();
   await expect(page.getByLabel("Конец периода",{exact:true})).toHaveValue("2026-11-30");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+tenantSql+"'"))[0].n,
     intelligenceAudit,"Portfolio reads/compare/Back/reload must never write audit");
   await navigate("insights?start=2026-02-30&end=2026-03-01","Страница не найдена");
   await page.goBack();
   await page.getByRole("heading",{name:"Аналитика бизнеса",exact:true}).waitFor();
   await page.screenshot({path:resolve(directory,"crm-portfolio-intelligence.png"),fullPage:true});
   await page.setViewportSize({width:390,height:844});
   const portfolioTableScroll=page.getByRole("region",{name:"Портфель воронок — таблица"});
   await portfolioTableScroll.waitFor();
   const mobilePortfolio=await portfolioTableScroll.evaluate(element=>({
     viewport:window.innerWidth,document:document.documentElement.scrollWidth,
     local:element.clientWidth,content:element.scrollWidth
   }));
   assert.ok(mobilePortfolio.viewport<=391&&mobilePortfolio.document<=391,
     "SK-07 cannot overflow the 390px mobile viewport: "+JSON.stringify(mobilePortfolio));
   assert.ok(mobilePortfolio.content>mobilePortfolio.local,
     "Enterprise portfolio wide table must scroll INSIDE named region on mobile");
   await portfolioTableScroll.focus();
   assert.equal(await portfolioTableScroll.evaluate(element=>element===document.activeElement),true,
     "Portable table scroll region must be keyboard focusable");
   await page.keyboard.press("ArrowRight");
   await expect.poll(()=>portfolioTableScroll.evaluate(element=>element.scrollLeft)).toBeGreaterThan(0);
   await page.setViewportSize({width:1280,height:720});
   checkpoint("SK-07 complete portfolio, period comparison, read-only D1, Back/reload and accessible mobile tables");

   // SK-07.B: human roster + original TaskOwner assignment through the same
   // compiled Rust Access + Activities owners. This is signed CI human proof,
   // not a claim that our live service principal is a person.
   const browserStaff="browser-sk07-"+runId+"@example.test";
   const beforeStaffAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+
     tenantSql+"'"))[0].n;
   await navigate("team","Команда и распределение работы");
   await page.getByRole("heading",{name:"Сотрудники и роли",exact:true}).waitFor();
   await page.getByLabel("Email нового сотрудника",{exact:true}).fill(browserStaff);
   await page.getByLabel("Роль нового сотрудника",{exact:true}).selectOption("1");
   await mutation(page.getByRole("button",{name:"Добавить участника",exact:true}),"ProvisionMember");
   const memberDb=(await sql("SELECT role,active,version FROM crm_users WHERE tenant_id='"+
     tenantSql+"' AND email='"+browserStaff+"'"))[0];
   assert.deepEqual(memberDb,{role:"viewer",active:1,version:1});
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id='"+
     tenantSql+"' AND subject_id='"+browserStaff+"' AND action='access.member.provision'"))[0].n,1);
   assert.ok((await sql("SELECT actor FROM crm_audit WHERE tenant_id='"+
     tenantSql+"' AND subject_id='"+browserStaff+"'"))[0].actor===adminIdentity);
   await page.getByRole("row").filter({hasText:browserStaff}).waitFor();
   await reloadDocument();
   await page.getByRole("row").filter({hasText:browserStaff}).waitFor();
   const delegatedTitle="Browser SK07 delegated "+runId;
   await navigate("tasks","Задачи");
   await page.getByRole("button",{name:"Новая задача",exact:true}).click();
   await page.getByLabel("Тема задачи",{exact:true}).fill(delegatedTitle);
   await selectPagedPicker(page,"Клиент",accountId,"account");
   await page.getByLabel("Назначить сотруднику",{exact:true}).selectOption(browserStaff);
   await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
   const delegatedId=await firstId("crm_activity_task_records","subject",delegatedTitle);
   assert.ok(delegatedId);
   assert.equal((await sql("SELECT json_extract(state,'$.owner.actor_id') AS owner "+
     "FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+delegatedId+"'"))[0].owner,
     browserStaff,"Original domain state persists the human actor identity");
   const delegatedCard=page.locator("section").filter({has:page.getByRole("heading",
     {name:delegatedTitle,exact:true})});
   await delegatedCard.getByText("Ответственный: "+browserStaff,{exact:true}).waitFor();
   await page.getByLabel("Ответственный: фильтр",{exact:true}).selectOption(browserStaff);
   assert.ok(new URL(page.url()).hash.includes("assigneeEmail="+encodeURIComponent(browserStaff)));
   await page.getByRole("heading",{name:delegatedTitle,exact:true}).waitFor();
   await reloadDocument();
   await expect(page.getByLabel("Ответственный: фильтр",{exact:true})).toHaveValue(browserStaff);
   await delegatedCard.getByRole("button",{name:"Редактировать задачу",exact:true}).click();
   await page.getByLabel("Переназначить задачу",{exact:true}).selectOption("native-viewer@example.test");
   await mutation(page.getByRole("button",{name:"Сохранить изменения задачи",exact:true}),"UpdateTask");
   const delegatedState=(await sql("SELECT version,json_extract(state,'$.owner.actor_id') AS owner "+
     "FROM crm_activity_task_records WHERE tenant='"+tenantSql+"' AND id='"+delegatedId+"'"))[0];
   assert.deepEqual(delegatedState,{version:2,owner:"native-viewer@example.test"});
   assert.deepEqual((await sql("SELECT action FROM crm_audit WHERE tenant_id='"+tenantSql+
     "' AND subject_id='"+delegatedId+"' ORDER BY rowid")).map(x=>x.action),
     ["activity_tasks.create","activity_tasks.update"]);
   await page.getByRole("heading",{name:delegatedTitle,exact:true}).waitFor({state:"hidden"});
   await navigate("team","Команда и распределение работы");
   const workforce=page.getByRole("region",{name:"Реальная нагрузка команды"});
   await workforce.getByText("native-viewer@example.test",{exact:false}).first().waitFor();
   const staffRow=page.getByRole("row").filter({hasText:browserStaff}).first();
   await staffRow.getByRole("button",{name:"Изменить доступ"}).click();
   await page.getByLabel("Активный доступ",{exact:true}).uncheck();
   await mutation(page.getByRole("button",{name:"Сохранить доступ",exact:true}),"UpdateMember");
   assert.deepEqual((await sql("SELECT active,version FROM crm_users WHERE tenant_id='"+
     tenantSql+"' AND email='"+browserStaff+"'")),[{active:0,version:2}]);
   await page.getByRole("row").filter({hasText:browserStaff}).getByText("Деактивирован",{exact:true}).waitFor();
   const workforceReadAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+
     tenantSql+"'"))[0].n;
   await reloadDocument();
   await workforce.getByText("native-viewer@example.test",{exact:false}).first().waitFor();
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='"+
     tenantSql+"'"))[0].n,workforceReadAudit,"Team/report reload may never create audit");
   assert.equal(workforceReadAudit-beforeStaffAudit,4,
     "SK-07 browser: provision + TaskCreate + reassignment + deactivate = four writes");
   await page.setViewportSize({width:390,height:844});
   const memberTable=page.getByRole("region",{name:"Участники tenant — таблица"});
   await memberTable.waitFor();
   const memberMobile=await memberTable.evaluate(el=>({
     viewport:window.innerWidth,page:document.documentElement.scrollWidth,
     local:el.clientWidth,content:el.scrollWidth
   }));
   assert.ok(memberMobile.viewport<=391&&memberMobile.page<=391,
     "SK-07 Team table cannot overflow mobile viewport: "+JSON.stringify(memberMobile));
   await page.setViewportSize({width:1280,height:720});
   checkpoint("SK-07.B typed workforce, human membership, owner assignment/reassign, ACL audit, D1, reload and mobile");

   if(features.includes("native-ci")){
     const teamName="Browser SK07 team "+runId;
     await navigate("team","Команда и распределение работы");
     await page.getByRole("button",{name:"Создать команду",exact:true}).click();
     await page.getByLabel("Название команды",{exact:true}).fill(teamName);
     const teamForm=page.getByRole("form",{name:"Настройка команды"});
     await teamForm.getByLabel("native-viewer@example.test",{exact:true}).check();
     await mutation(teamForm.getByRole("button",{name:"Сохранить команду",exact:true}),"CreateTeam");
     const teamId=(await sql("SELECT id FROM crm_access_teams WHERE tenant=? AND json_extract(state,'$.name')=?",tenantA,teamName))[0].id;
     const teamRow=page.getByRole("row").filter({hasText:teamName});
     await teamRow.getByRole("link",{name:"Задачи команды",exact:true}).click();
     await page.getByRole("button",{name:"Новая задача",exact:true}).click();
     const poolName="Browser SK07 pool "+runId;
     await page.getByLabel("Тема задачи",{exact:true}).fill(poolName);
     await selectPagedPicker(page,"Клиент",accountId,"account");
     await page.getByLabel("Назначить сотруднику",{exact:true}).selectOption("team:"+teamId);
     await mutation(page.getByRole("button",{name:"Создать задачу",exact:true}),"CreateTask");
     const poolTask=page.locator("section").filter({has:page.getByRole("heading",{name:poolName,exact:true})});
     await poolTask.getByText("Ответственный: Команда: "+teamName,{exact:true}).waitFor();
     await reloadDocument();
     await poolTask.getByText("Ответственный: Команда: "+teamName,{exact:true}).waitFor();
     await poolTask.getByRole("button",{name:"Редактировать задачу",exact:true}).click();
     await page.getByLabel("Переназначить задачу",{exact:true}).selectOption("native-viewer@example.test");
     await mutation(page.getByRole("button",{name:"Сохранить изменения задачи",exact:true}),"UpdateTask");
     await poolTask.waitFor({state:"hidden"});
     // Deal owner scope is shared by the list, board, saved views and portfolio.
     await navigate("team","Команда и распределение работы");
     await teamRow.getByRole("link",{name:"Сделки команды",exact:true}).click();
     await page.getByLabel("Воронка продаж",{exact:true}).selectOption(newPipelineId);
     await page.getByRole("button",{name:"Новая сделка",exact:true}).click();
     const ownedDealName="Browser SK07 owned deal "+runId;
     await page.getByLabel("Название сделки",{exact:true}).fill(ownedDealName);
     await selectPagedPicker(page,"Клиент",accountId,"account");
     await page.getByLabel("Ответственный за сделку",{exact:true}).selectOption("team:"+teamId);
     await page.getByLabel("Сумма",{exact:true}).fill("100.01");
     await page.getByLabel("Валюта",{exact:true}).selectOption("EUR");
     await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"CreateDeal");
     const ownedDealId=await firstId("crm_sales_deal_records","name",ownedDealName);
     assert.ok(ownedDealId);
     assert.equal((await sql("SELECT json_extract(state,'$.owner.team_id') AS owner FROM crm_sales_deal_records WHERE tenant=? AND id=?",tenantA,ownedDealId))[0].owner,teamId);
     await expect(page.getByLabel("Ответственный за сделки: фильтр",{exact:true})).toHaveValue("team:"+teamId);
     await reloadDocument();
     await expect(page.getByLabel("Ответственный за сделки: фильтр",{exact:true})).toHaveValue("team:"+teamId);
     await page.getByRole("region",{name:"Аналитика воронки"}).getByText("Всего сделок: 1",{exact:true}).waitFor();
     const ownerViewName="Browser SK07 owner view "+runId;
     await page.getByRole("button",{name:"Сохранить как новое",exact:true}).click();
     await page.getByLabel("Название представления",{exact:true}).fill(ownerViewName);
     await mutation(page.getByRole("button",{name:"Сохранить новое представление",exact:true}),"CreateSalesView");
     await reloadDocument();
     await expect(page.getByLabel("Ответственный за сделки: фильтр",{exact:true})).toHaveValue("team:"+teamId);
     await navigate("team","Команда и распределение работы");
     await teamRow.getByRole("link",{name:"Портфель команды",exact:true}).click();
     await page.getByRole("region",{name:"Общий портфель"}).getByText(/Сделок: 1 · Открыто: 1/).waitFor();
     await reloadDocument();
     await expect(page.getByLabel("Ответственный за портфель",{exact:true})).toHaveValue("team:"+teamId);
     await page.getByRole("region",{name:"Общий портфель"}).getByText(/Сделок: 1 · Открыто: 1/).waitFor();
     await navigate("sales?dealId="+encodeURIComponent(ownedDealId),"Карточка сделки");
     await page.getByText("Ответственный: Команда: "+teamName,{exact:true}).waitFor();
     await page.getByRole("button",{name:"Редактировать сделку",exact:true}).click();
     await page.getByLabel("Ответственный за сделку",{exact:true}).selectOption("actor:native-viewer@example.test");
     await mutation(page.getByRole("button",{name:"Сохранить сделку",exact:true}),"UpdateDeal");
     await page.getByText("Ответственный: native-viewer@example.test",{exact:true}).waitFor();
     await navigate("team","Команда и распределение работы");
     await teamRow.getByRole("link",{name:"Портфель команды",exact:true}).click();
     await page.getByRole("region",{name:"Общий портфель"}).getByText(/Сделок: 0 · Открыто: 0/).waitFor();
     checkpoint("SK-07.C Deal ownership, saved owner scope, portfolio, reassignment and reload");
     await navigate("team","Команда и распределение работы");
     await teamRow.getByRole("button",{name:"Изменить команду",exact:true}).click();
     await teamForm.getByLabel("Активная команда",{exact:true}).uncheck();
     await mutation(teamForm.getByRole("button",{name:"Сохранить команду",exact:true}),"UpdateTeam");
     await teamRow.getByText("Архив",{exact:true}).waitFor();
     checkpoint("SK-07.C UI team roster, pool assignment, reload, reassignment and archive");
   }

   await page.getByLabel("Тема",{exact:true}).selectOption("dark");
   assert.equal(await page.locator("html").getAttribute("data-theme"),"dark");
   await reloadDocument();
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
   checkpoint("Mobile Kanban: named keyboard-focusable horizontal scroll region without viewport overflow");
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
   checkpoint("Theme persistence and mobile layout");

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
   // Native signed viewer can operate an assigned Task, but cannot delegate
   // or use the administrator-only creation/edit controls.
   if(features.includes("native-ci")){
     await deniedPage.goto(base+"/#tasks?assigneeEmail=native-viewer%40example.test");
     const mine=deniedPage.locator("section").filter({has:deniedPage.getByRole("heading",
       {name:delegatedTitle,exact:true})});
     const done=mine.getByRole("button",{name:"Выполнить задачу",exact:true});
     await done.waitFor();
     await expect(deniedPage.getByRole("button",{name:"Новая задача",exact:true})).toHaveCount(0);
     await expect(mine.getByRole("button",{name:"Редактировать задачу",exact:true})).toHaveCount(0);
     const completing=deniedPage.waitForResponse(r=>responsePathEndsWith(r,"/CompleteTask")&&r.request().method()==="POST");
     await done.click();
     assert.equal((await completing).status(),200);
     await mine.getByText(/Выполнена/).waitFor();
     await deniedPage.reload();
     await mine.getByText(/Выполнена/).waitFor();
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND actor=? AND action=?",
       tenantA,delegatedId,"native-viewer@example.test","activity_tasks.complete"))[0].n,1);
     checkpoint("SK-07.B signed viewer completes only own Task, no admin controls, reload and D1 audit");
   }
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
     const refused=anonymousPage.waitForResponse(r=>responsePathEndsWith(r,"/ListAccounts"));
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
   checkpoint("Generated-client unauthenticated/permission errors and tenant B isolation");
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
     tested:"built React and generated Connect client against Rust Worker/D1",steps,requests,writes,recovery,
     synthetic_private_probes:syntheticPrivateProbes,owner_recovery:ownerRecovery};
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

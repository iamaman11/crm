import assert from "node:assert/strict";
import {writeFileSync} from "node:fs";
import {join} from "node:path";
import {chromium} from "playwright";
import {fixtureRpc,fixtureRpcError,fixtureRpcPath,collectFixturePages} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {PartyService,PartyKind} from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import {AccountService,AccountStatus} from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import {installUiDiagnostics,assertUiPanel} from "./ui-diagnostics.mts";
import {importNfcPartyFixture} from "./party-csv.mts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type SearchFixture = {
 dispatch: RpcDispatch; sql: (query: string,...params: string[])=>Promise<Record<string,unknown>[]>;
 partySnapshot: () => Promise<Record<string,unknown>[]>;
 cwd: string; base: string; sourceSha: string;
 id: string; admin: string; viewer: string; tenantB: string;
};
type SearchMethod = typeof PartyService.method.listParties | typeof AccountService.method.listAccounts;
export async function verifyPartyAccountSearch({
 dispatch,sql,partySnapshot,cwd,base,sourceSha,id,admin,viewer,tenantB,
}: SearchFixture) {
 const parties=PartyService.method,accounts=AccountService.method;
 const allParties=(input: {search?: string; kind?: PartyKind})=>collectFixturePages(async pageToken=>{
   const {body}=await fixtureRpc(dispatch,parties.listParties,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:body.parties,nextPageToken:body.pageInfo?.nextPageToken};
 });
 const allAccounts=(input: {search?: string; status?: AccountStatus})=>collectFixturePages(async pageToken=>{
   const {body}=await fixtureRpc(dispatch,accounts.listAccounts,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:body.accounts,nextPageToken:body.pageInfo?.nextPageToken};
 });
 // Search/pagination needs >100 nonmatching rows, not 202 repeated RPC
 // creation tests. Create one canonical Party+Account through the original
 // typed Rust owners, then copy only their persisted state in isolated D1.
 // No parallel business codec, schema or extra test framework is introduced.
 const searchFixtureStarted=Date.now();
 const paddingParty="zz-search-padding-000",paddingAccount=paddingParty;
 assert.equal((await fixtureRpc(dispatch,parties.createParty,admin,{partyId:paddingParty,kind:1,
   displayName:"Search padding 000",
   precondition:{idempotencyKey:"search-party-padding-000"}})).status,200);
 assert.equal((await fixtureRpc(dispatch,accounts.createAccount,admin,{accountId:paddingAccount,
   name:"Search padding 000",primaryPartyId:id,
   precondition:{idempotencyKey:"search-account-padding-000"}})).status,200);
 // D1 triggers retain their normal receipt/audit ownership. Only the synthetic
 // search fixture uses cloning; all tested business mutations still use typed RPC.
 for(const [table,idField,labelField,receiptPrefix,requestPrefix] of [
   ["crm_party_records","party_id","display_name","search-party-padding","8001"],
   ["crm_account_records","account_id","name","search-account-padding","8002"]
 ]){
   const account=table==="crm_account_records";
   const source=account?"source.primary_party,":"";
   const columns=account?"tenant,id,primary_party,state,version,idem_key,idem_digest,request_id,actor,search_name":
     "tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name";
   await sql("WITH RECURSIVE pad(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM pad WHERE n<100) "+
     "INSERT INTO "+table+"("+columns+") "+
     "SELECT source.tenant,'zz-search-padding-'||printf('%03d',pad.n),"+source+
     "json_set(source.state,'$."+idField+"','zz-search-padding-'||printf('%03d',pad.n),"+
       "'$."+labelField+"','Search padding '||printf('%03d',pad.n)),source.version,"+
     "'"+receiptPrefix+"-'||printf('%03d',pad.n),'fixture-only',"+
     "printf('00000000-0000-4000-"+requestPrefix+"-%012d',pad.n),source.actor,"+
     "'Search padding '||printf('%03d',pad.n) "+
     "FROM pad CROSS JOIN "+table+" AS source "+
     "WHERE source.tenant='tenant-a' AND source.id=?",
     account?paddingAccount:paddingParty);
   const [proof]=await sql("SELECT COUNT(*) AS total,COUNT(DISTINCT id) AS distinct_ids,"+
     "SUM(CASE WHEN json_valid(state)=1 AND "+
     "json_extract(state,'$."+idField+"')=id AND search_name=json_extract(state,'$."+labelField+"') "+
     "THEN 0 ELSE 1 END) AS bad "+
     "FROM "+table+" WHERE tenant='tenant-a' AND id LIKE 'zz-search-padding-%'");
   assert.deepEqual(proof,{total:101,distinct_ids:101,bad:0},
     "Search padding must preserve owner-produced typed state in "+table);
 }
 console.log(JSON.stringify({event:"crm.native-search-padding-fixture",result:"PASS",
   typed_owner_creates:2,cloned_fixture_rows:200,elapsed_ms:Date.now()-searchFixtureStarted}));
 const searchIds=["zzz-search-first","zzz-search-gap","zzz-search-last"] as const;
 for(const [index,target] of searchIds.entries()){
   const name=index===1?"Nonmatching gap":"Поиск 100%_ O'Neil "+index;
   assert.equal((await fixtureRpc(dispatch,parties.createParty,admin,{partyId:target,kind:PartyKind.PERSON,displayName:name,
     precondition:{idempotencyKey:"search-party-"+index}})).status,200);
   assert.equal((await fixtureRpc(dispatch,accounts.createAccount,admin,{accountId:target,name,primaryPartyId:id,
     precondition:{idempotencyKey:"search-account-"+index}})).status,200);
 }
 const nfcPartyId="zzz-nfc-party",nfcAccountId="zzz-nfc-account";
 const originalNfcName="Cafe\u0301 И\u0306ошкар-Ола";
 const canonicalNfcName="Café Йошкар-Ола";
 const nfcParty=await fixtureRpc(dispatch,parties.createParty,admin,{partyId:nfcPartyId,kind:2,displayName:originalNfcName,
   precondition:{idempotencyKey:"nfc-party-create"}});
 assert.equal(nfcParty.status,200,formatRpcFailure(nfcParty));
 const nfcAccount=await fixtureRpc(dispatch,accounts.createAccount,admin,{accountId:nfcAccountId,name:canonicalNfcName,
   primaryPartyId:nfcPartyId,precondition:{idempotencyKey:"nfc-account-create"}});
 assert.equal(nfcAccount.status,200,formatRpcFailure(nfcAccount));
 for(const needle of [originalNfcName,canonicalNfcName]){
   const foundParties=await allParties({search:needle});
   assert.equal(foundParties.find(row=>row.partyId===nfcPartyId)?.displayName,originalNfcName);
   const foundAccounts=await allAccounts({search:needle});
   assert.equal(foundAccounts.find(row=>row.accountId===nfcAccountId)?.name,canonicalNfcName);
 }
 assert.deepEqual(await sql("SELECT search_name FROM crm_party_records WHERE id='zzz-nfc-party'"),
   [{search_name:canonicalNfcName}]);
 assert.deepEqual(await sql("SELECT search_name FROM crm_account_records WHERE id='zzz-nfc-account'"),
   [{search_name:canonicalNfcName}]);
 const nfcUpdateName="Renamed Cafe\u0301";
 const nfcUpdateCanonical="Renamed Café";
 const updatedNfcParty=await fixtureRpc(dispatch,parties.updateParty,admin,{partyId:nfcPartyId,displayName:nfcUpdateName,
   precondition:{idempotencyKey:"nfc-party-update",expectedVersion:1n}});
 assert.equal(updatedNfcParty.status,200,formatRpcFailure(updatedNfcParty));
 const updatedNfcAccount=await fixtureRpc(dispatch,accounts.updateAccount,admin,{accountId:nfcAccountId,name:nfcUpdateName,status:1,
   precondition:{idempotencyKey:"nfc-account-update",expectedVersion:1n}});
 assert.equal(updatedNfcAccount.status,200,formatRpcFailure(updatedNfcAccount));
 assert.equal((await allParties({search:canonicalNfcName})).length,0);
 assert.equal((await allAccounts({search:canonicalNfcName})).length,0);
 assert.equal((await allParties({search:nfcUpdateCanonical}))[0]?.displayName,nfcUpdateName);
 assert.equal((await allAccounts({search:nfcUpdateCanonical}))[0]?.name,nfcUpdateName);
 const nfcImportedPartyId=await importNfcPartyFixture(dispatch,admin);
 const importedNfc=await allParties({search:"Imported Café"});
 assert.equal(importedNfc.length,1);
 assert.equal(importedNfc[0]?.partyId,nfcImportedPartyId);
 assert.equal(importedNfc[0]?.displayName,"Imported Cafe\u0301");
 const searchSnapshot=async()=>({party:(await partySnapshot()),
   accounts:(await sql("SELECT id,state,version FROM crm_account_records ORDER BY tenant,id")),
   accountReceipts:(await sql("SELECT COUNT(*) AS n FROM crm_account_receipts"))});
 const searchBefore=(await searchSnapshot());
 type SearchPage = {search?: string; page?: {pageSize: number}};
 for(const scenario of [
   {
     query: async (identity: string, input: SearchPage, opposite=false) => {
       const result=await fixtureRpc(dispatch,parties.listParties,identity,
         {...input,...(opposite?{kind:PartyKind.ORGANIZATION}:{})});
       return {body:result.body, ids:result.body.parties.map(item=>item.partyId)};
     },
     found: async (search: string)=>(await allParties({search,kind:PartyKind.PERSON})).map(item=>item.partyId),
     reject: async (identity: string | null,input: SearchPage,status: number)=>{
       const result=await fixtureRpcError(dispatch,parties.listParties,identity,input);
       assert.equal(result.status,status);
       return result;
     },
   },
   {
     query: async (identity: string,input: SearchPage,opposite=false) => {
       const result=await fixtureRpc(dispatch,accounts.listAccounts,identity,
         {...input,...(opposite?{status:AccountStatus.INACTIVE}:{})});
       return {body:result.body, ids:result.body.accounts.map(item=>item.accountId)};
     },
     found: async (search: string)=>(await allAccounts({search,status:AccountStatus.ACTIVE})).map(item=>item.accountId),
     reject: async (identity: string | null,input: SearchPage,status: number)=>{
       const result=await fixtureRpcError(dispatch,accounts.listAccounts,identity,input);
       assert.equal(result.status,status);
       return result;
     },
   },
 ]){
   const firstPage=await scenario.query(viewer,{page:{pageSize:100}});
   assert.ok(!firstPage.ids.some(row=>searchIds.some(id=>id===row)));
   assert.deepEqual(await scenario.found("  Поиск 100%_ O'Neil  "),[searchIds[0],searchIds[2]],
     "Search must filter before LIMIT and retain every matching cursor");
   assert.deepEqual((await scenario.query(viewer,{search:"100%_"},true)).ids,[]);
   assert.deepEqual((await scenario.query(tenantB,{search:"Поиск 100%_"})).ids,[]);
   assert.deepEqual((await scenario.query(viewer,{search:"поиск 100%_"})).ids,[],"Literal search is case-sensitive");
   assert.deepEqual((await scenario.query(viewer,{search:"%' OR 1=1 --"})).ids,[]);
   assert.deepEqual((await scenario.query(viewer,{search:searchIds[2]})).ids,[searchIds[2]]);
   assert.deepEqual((await scenario.query(viewer,{search:"   ",page:{pageSize:100}})).body,firstPage.body);
   await scenario.query(viewer,{search:"я".repeat(200)});
   const tooLong=await scenario.reject(viewer,{search:"я".repeat(201)},400);
   assert.equal(tooLong.body.code,"invalid_argument");
   await scenario.reject(null,{search:"Поиск"},401);
 }
 assert.deepEqual((await searchSnapshot()),searchBefore,"Search cannot mutate state, receipts or audit");
 console.log(JSON.stringify({event:"crm.native-party-account-search",result:"PASS",
   before_limit:true,unicode_literal:true,tenant_ab:true,read_only:true,existing_rpcs:true}));
 // Actual React search and both pickers find a target beyond 100 existing rows.
 const searchBrowser=await chromium.launch({headless:true});
 try{
   const context=await searchBrowser.newContext({extraHTTPHeaders:{"cf-access-jwt-assertion":admin}});
   const page=await context.newPage();
   await installUiDiagnostics(page);
   const searchRequests: {method: string; status: number; request_id: string | undefined}[]=[];
   const panelTransitions=[];
   const enterSearch=async(label: string,text: string,method: SearchMethod)=>{
     const input=page.getByLabel(label,{exact:true});
     await input.fill(text);
     const pending=page.waitForResponse(r=>new URL(r.url()).pathname===fixtureRpcPath(method)&&r.request().method()==="POST");
     await input.press("Enter");
     const response=await pending,headers=await response.allHeaders();
     assert.equal(response.status(),200);assert.equal(headers["x-crm-source-sha"],sourceSha);
     searchRequests.push({method:method.name,status:response.status(),request_id:headers["x-request-id"]});
   };
   for(const [hash,title,label,method] of [
     ["parties","Люди и организации","Поиск участников",parties.listParties],
     ["accounts","Клиенты","Поиск клиентов",accounts.listAccounts],
   ] as const){
     await page.goto(base+"/#"+hash);
     await page.getByRole("heading",{name:title,exact:true}).waitFor();
     const nextPage=page.waitForResponse(r=>new URL(r.url()).pathname===fixtureRpcPath(method)&&r.request().method()==="POST");
     await page.getByRole("button",{name:"Следующая страница",exact:true}).click();
     assert.equal((await nextPage).status(),200);
     await enterSearch(label,searchIds[2],method);
     await page.getByRole("heading",{name:"Поиск 100%_ O'Neil 2",exact:true}).waitFor();
     await enterSearch(label,"zz-search-padding-000",method);
     await page.getByRole("heading",{name:"Search padding 000",exact:true}).waitFor();
   }
   // Searchable pickers are inside the shared create panel, not the list.
   // Use the real UI, preserving the same native-Rust List RPC assertions.
   for(const [hash,createAction,label,searchLabel,method] of [
     ["accounts","Создать клиента","Основной участник","Поиск: Основной участник",parties.listParties],
     ["tasks","Новая задача","Клиент","Поиск: Клиент",accounts.listAccounts],
   ] as const){
     await page.goto(base+"/#"+hash);
     await page.getByRole("button",{name:createAction,exact:true}).click();
     const panel=page.getByRole("dialog");
     await panel.waitFor();
     const panelId=await panel.getAttribute("aria-labelledby");
     assert.ok(panelId,"The native picker must retain a stable React useId identity");
     const select=panel.getByLabel(label,{exact:true});
     await enterSearch(searchLabel,searchIds[2],method);
     await select.selectOption(searchIds[2]);
     assert.equal(await select.inputValue(),searchIds[2]);
     await enterSearch(searchLabel,"missing-search-target",method);
     assert.equal(await select.inputValue(),"","Changed search must discard the old form selection");
     await enterSearch(searchLabel,searchIds[0],method);
     await select.selectOption(searchIds[0]);
     assert.equal(await select.inputValue(),searchIds[0]);
     // Finish the form before navigating to the next screen. The shared
     // AccountCreator correctly protects dirty drafts; an unhandled hash
     // transition must NOT bypass that guard or masquerade as a missing Task UI.
     if(hash==="accounts"){
       // Rejecting dirty-form closure must preserve BOTH the React intent
       // and the native dialog, rather than silently losing an unsaved draft.
       page.once("dialog",dialog=>void dialog.dismiss());
       await panel.getByRole("button",{name:"Отмена",exact:true}).click();
       const rejected=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"dirty-account-close-rejected",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const rejectedPanel=rejected.panels.find(p=>p.title==="Новый клиент");
       assert.equal(rejectedPanel?.dirty,"true","CreateAccount draft guard must be active");
       assert.equal(rejected.confirmations.at(-1)?.accepted,false,
         "Closing dirty Account after rejecting confirm must not discard draft");
       panelTransitions.push({title:"Новый клиент",result:"REJECTED",desired:rejectedPanel?.desired,
         nativeOpen:rejectedPanel?.nativeOpen,dirty:rejectedPanel?.dirty});
       // Escape must also enter the SAME dirty guard before the native dialog
       // closes. Reject first, then accept: no second navigation/close owner.
       page.once("dialog",dialog=>void dialog.dismiss());
       await page.keyboard.press("Escape");
       const escapeRejected=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"dirty-account-escape-rejected",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const escapePanel=escapeRejected.panels.find(p=>p.title==="Новый клиент");
       assert.equal(escapePanel?.closeSource,"escape");
       assert.equal(escapeRejected.confirmations.at(-1)?.accepted,false);
       assert.equal(escapePanel?.nativeOpen,true,"native Escape cannot bypass rejected dirty close");
       panelTransitions.push({title:"Новый клиент",result:"ESCAPE_REJECTED",
         desired:escapePanel?.desired,nativeOpen:escapePanel?.nativeOpen});
       // Regression for Chromium native cancel after repeated rejections:
       // the second Escape must also be prevented before the browser closes
       // the HTMLDialogElement (React desired and native open stay in sync).
       page.once("dialog",dialog=>void dialog.dismiss());
       await page.keyboard.press("Escape");
       const secondRejected=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"dirty-account-second-escape-rejected",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const secondPanel=secondRejected.panels.find(p=>p.panelId===panelId);
       assert.equal(secondRejected.confirmations.at(-1)?.accepted,false);
       assert.equal(secondPanel?.desired,"open");
       assert.equal(secondPanel?.nativeOpen,true);
       assert.equal(secondPanel?.closeSource,"escape");
       panelTransitions.push({title:"Новый клиент",result:"SECOND_ESCAPE_REJECTED",
         desired:secondPanel?.desired,nativeOpen:secondPanel?.nativeOpen});
       page.once("dialog",dialog=>void dialog.accept());
       await page.keyboard.press("Escape");
     }else{
       // A native <select> can consume Escape. Use the explicit close button
       // and inspect React intent and HTMLDialogElement.open independently.
       await panel.getByRole("button",{name:"Закрыть панель",exact:true}).click();
     }
     const title=hash==="accounts"?"Новый клиент":"Новая задача";
     const closed=await assertUiPanel({page,panel,panelId,title,
       expected:"closed",scenario:hash+"-picker-closed",
       evidenceDirectory:join(cwd,"../e2e-evidence-native")});
     const observed=closed.panels.find(p=>p.title===title);
     panelTransitions.push({title,result:"CLOSED",desired:observed?.desired,
       nativeOpen:observed?.nativeOpen,dirty:observed?.dirty,
       closeSource:observed?.closeSource});
     if(hash==="accounts"){
       // A dirty draft rejected twice and later discarded must never leave a
       // closed native dialog behind a stale React open=true on the next click.
       await page.getByRole("button",{name:createAction,exact:true}).click();
       const reopened=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"account-reopen-after-escape",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const reopenedPanel=reopened.panels.find(p=>p.panelId===panelId);
       assert.equal(reopenedPanel?.dirty,"false");
       assert.equal(await panel.getByLabel("Название клиента",{exact:true}).inputValue(),"");
       panelTransitions.push({title,result:"REOPENED",desired:reopenedPanel?.desired,
         nativeOpen:reopenedPanel?.nativeOpen});
       await panel.getByRole("button",{name:"Отмена",exact:true}).click();
       await assertUiPanel({page,panel,panelId,title:"Новый клиент",expected:"closed",
         scenario:"account-reopened-cancel",evidenceDirectory:join(cwd,"../e2e-evidence-native")});
     }
   }
   await page.setViewportSize({width:390,height:844});
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1));
   assert.deepEqual((await searchSnapshot()),searchBefore,"Search and Enter inside pickers must never submit the enclosing mutation form");
   writeFileSync(join(cwd,"../e2e-evidence-native/search-browser.json"),JSON.stringify({result:"PASS",source_sha:sourceSha,
     scenarios:["search after first unfiltered page","Party picker beyond 100 rows","Account picker beyond 100 rows",
       "selection reset","keyboard Enter does not submit mutation","mobile layout","read-only",
       "native dialog and React state matched on close","dirty draft close rejection retained dialog",
       "second rejected Escape preserves native modal before accepted close",
       "Account modal reopens cleanly after confirmed native Escape"],
     panel_transitions:panelTransitions,requests:searchRequests},null,2)+"\n");
 }finally{await searchBrowser.close();}
}

// Full generated binary Connect contract and actual React against signed workerd/D1.
import assert from "node:assert/strict";
import {verifyPartyNameQuality} from "./data-quality.mts";
import {verifyConsent} from "./consent.mts";
import {verifySavedViews} from "./saved-views.mts";
import {verifyPipeline} from "./pipeline.mts";
import {verifyOverdueTasks} from "./task-overdue.mts";
import {verifyTaskReminder} from "./task-reminder.mts";
import {verifyTasks} from "./tasks.mts";
import {verifyRelationships} from "./relationships.mts";
import {verifyAccounts} from "./accounts.mts";
import {verifyContacts} from "./contacts.mts";
import {verifyParties} from "./parties.mts";
import {verifyTaskDueOrdering} from "./task-due-order.mts";
import {verifyPrivacyIntake} from "./privacy-intake.mts";
import {verifyPrivacySubject} from "./privacy-subject.mts";
import {verifyRecentInteractions} from "./interaction-recent.mts";
import {verifyInteractions} from "./interactions.mts";
import {verifyAutomation} from "./automation.mts";
import {verifyEnrichment} from "./enrichment.mts";
import {verifyPartyCsv} from "./party-csv.mts";
import {verifyIdentityResolution} from "./identity-resolution.mts";
import {verifyAccountOverview} from "./account-overview.mts";
import {typedBrowser} from "./typed-browser.mts";
import {fixtureRpc,fixtureRpcError,collectFixturePages} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {number} from "../../scripts/json-boundary.mts";
import {verifySalesLeads} from "./sales-leads.mts";
import {verifySalesCreate} from "./sales-create.mts";
import {verifySalesUpdate} from "./sales-update.mts";
import {verifySalesPages} from "./sales-pages.mts";
import {verifySalesCoverage} from "./sales-coverage.mts";
import {verifySalesAdvance} from "./sales-advance.mts";
import {verifySalesSearch} from "./sales-search.mts";
import {verifySalesBoard} from "./sales-board.mts";
import {verifySalesOwner} from "./sales-owner.mts";
import {verifyPartyAccountSearch} from "./party-account-search.mts";
import {verifyWorkforce} from "./workforce.mts";
import {verifySalesLinkedTasks} from "./sales-linked-tasks.mts";
import {verifyLifecycle} from "./lifecycle.mts";
import {verifyCustomerReadParity} from "./customer-read-parity.mts";
import {verifyReceipts} from "./receipts.mts";
import {verifySalesOpportunity} from "./sales-opportunity.mts";
import {verifyForecast} from "./forecast.mts";
import {verifySalesScale} from "./sales-scale.mts";
import {verifySalesCommercial} from "./sales-commercial.mts";
import {verifySalesLifecycle} from "./sales-lifecycle.mts";
import {registerNativeTests} from "./native-harness.mts";

registerNativeTests(async ({cwd,base,worker,testHarness,sql,statement,batch,partySnapshot,rpcFetch,call,
 sourceSha,admin,viewer,tenantB,knownService,unknownService,jwt})=>{
 const describeQueryPlan=(value: unknown)=>JSON.stringify(value,
   (_key,item)=>typeof item==="bigint"?item.toString():item);
 const {id,creation,created,pagingPersonId}=await verifyParties({dispatch:rpcFetch,sql,partySnapshot,admin,viewer,tenantB,knownService,unknownService});

 // Accounts v2: original Account aggregate and D1 trigger, not a pilot record.
 const {accountId,accountInput,accountCreated,accountNextId,accountRetryInput}=await verifyAccounts({dispatch:rpcFetch,sql,id,admin,viewer,tenantB,knownService,unknownService});

 // ContactPoints v2 uses the original Party-owned contact aggregate, NEVER
 // the removed pilot crm_contacts table. No browser-written verified=true.
 const {contactId,contactInput,contactCreated,contactLastId,contactRetryInput}=await verifyContacts({dispatch:rpcFetch,sql,id,pagingPersonId,admin,viewer,tenantB,knownService,unknownService});

 // G3 Relationships v2: original Rust domain owner and one D1, binary
 // Protobuf descriptor, signed JWT + tenant A/B + viewer rights.
 const {rMid,rLast,midRelation,midCreated}=await verifyRelationships({dispatch:rpcFetch,sql,id,pagingPersonId,admin,viewer,tenantB,knownService});

 // Activities v2: real signed workerd binary Protobuf against the ORIGINAL
 // crm-activities Task state, native one-D1 account FK, receipt and audit.
 const {taskOther,taskMid,taskLast,taskMidInput,taskCreated,taskActorSnapshot,taskAudits}=await verifyTasks({dispatch:rpcFetch,sql,accountId,accountNextId,admin,viewer,tenantB,knownService});
 await verifyTaskReminder({dispatch:rpcFetch,sql,taskActorSnapshot,taskAudits,taskOther,taskMid,admin,viewer,tenantB,knownService});

 await verifyOverdueTasks({dispatch:rpcFetch,sql,accountId,accountNextId,admin,viewer,tenantB});

 // Original read-only DataQuality and Identity Resolution, signed binary
 // Connect from the exact generated descriptor, with tenant A/B and viewer.
 await verifyPartyNameQuality(rpcFetch,sql,id,admin,viewer,tenantB);

 await verifyIdentityResolution(rpcFetch,sql,id,pagingPersonId,admin,viewer,tenantB);

 const {r3Contact}=await verifyConsent({dispatch:rpcFetch,sql,id,contactId,pagingPersonId,admin,viewer,tenantB,knownService});

 // Original crm-sales v2: generated binary Connect, original Deal aggregate,
// one tenant D1, exact money, CAS, receipt replay, and atomic stage->Activity.
const sales=DealService.method;
const pipelineSeed=await verifyPipeline({dispatch:rpcFetch,sql,admin,viewer,tenantB});
await verifySavedViews({dispatch:rpcFetch,sql,accountId,admin,viewer,tenantB});
const {dealId,saleCreate,foreignSaleContactId,salesCreated}=await verifySalesCreate({
 dispatch:rpcFetch,sql,admin,viewer,tenantB,accountId,contactId,pagingPersonId
});
await verifySalesOpportunity({dispatch:rpcFetch,sql,statement,batch,admin,viewer,tenantB,saleCreate});
const {saleUpdated}=await verifySalesUpdate({
 dispatch:rpcFetch,sql,admin,viewer,knownService,dealId,foreignSaleContactId,saleCreate,salesCreated
});
const {advancedSale,closedSale,closeSale}=await verifySalesAdvance({
 dispatch:rpcFetch,sql,admin,viewer,accountId,dealId
});
const {dealOther,dealLast}=await verifySalesPages({
 dispatch:rpcFetch,sql,admin,viewer,tenantB,accountId,dealId
});
await verifySalesSearch({dispatch:rpcFetch,sql,viewer,tenantB,accountId,dealId,dealOther,dealLast});
await verifySalesBoard({dispatch:rpcFetch,sql,admin,viewer,tenantB,accountId,dealLast});
await verifySalesScale({dispatch:rpcFetch,cwd,accountId,viewer,admin,sql,statement,batch,describeQueryPlan});
assert.equal((await fixtureRpcError(rpcFetch,sales.listStageDeals,tenantB,{
  pipelineId:"pipeline.enterprise",stageId:"lead"
})).status,404,"tenant B cannot read tenant A pipeline");
assert.equal((await fixtureRpcError(rpcFetch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.enterprise",stageId:"not-a-stage"
})).status,404,"unmanaged stage is not a valid board column");
assert.equal((await fixtureRpc(rpcFetch,sales.listDeals,viewer,{
  page:{pageSize:1,pageToken:dealLast},pipelineId:"pipeline.enterprise"})
  ).body.deals.length,0);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
  "WHERE tenant='tenant-a' AND key IN ('connect-sales-create-main',"+
  "'connect-sales-update-main','connect-sales-advance-main','connect-sales-close-main')"
  ))[0]?.n,4,"original Deal receipt owners and D1 triggers remain singular");
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0]?.n,4,
  "create + update + two stage transitions each write exactly one audit");

  assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+dealId+"' ORDER BY rowid")),[
    {action:"sales_deals.create",actor:"native-a@example.test",
      request_id:salesCreated.headers.get("x-request-id")},
    {action:"sales_deals.update",actor:"service:ci-fixture.access",
      request_id:saleUpdated.headers.get("x-request-id")},
    {action:"sales_deals.advance",actor:"native-a@example.test",
      request_id:advancedSale.headers.get("x-request-id")},
    {action:"sales_deals.close",actor:"native-a@example.test",
      request_id:closedSale.headers.get("x-request-id")}
  ],"Sales create/update/stage actor and action survive cross-actor replay");
  const linked=(await sql("SELECT delivery_id,task_id,disposition,CASE WHEN task_id IS NULL THEN 1 ELSE 0 END AS no_task,actor,request_id FROM crm_sales_link_delivery_records "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"' ORDER BY deal_version"));
  assert.equal(linked.length,2);
  // One signed server request atomically writes Deal, linked Task (if any),
  // and linked-delivery audits. Distinguish their actions AND subject IDs.
  for(const [i,row] of linked.entries()){
    assert.equal(row.actor,"native-a@example.test");
    assert.equal(row.request_id,(i===0?advancedSale:closedSale).headers.get("x-request-id"));
    assert.equal(row.disposition,i===0?"applied":"ignored");
    assert.equal(row.no_task,i===0?0:1,
      "Ignored delivery must not create a Task; validate SQL NULL, not serialized string");
    const expected=[
      {action:i===0?"sales_deals.advance":"sales_deals.close",subject_id:dealId,
        actor:"native-a@example.test",request_id:row.request_id},
      {action:i===0?"sales_activities_link.applied":"sales_activities_link.ignored",
        subject_id:row.delivery_id,actor:"native-a@example.test",
        request_id:row.request_id},
      ...(row.disposition==="applied"?[{action:"activity_tasks.create",subject_id:row.task_id,
        actor:"native-a@example.test",request_id:row.request_id}]:[])
    ].sort((a,b)=>a.action.localeCompare(b.action));
    const actual=(await sql("SELECT action,subject_id,actor,request_id FROM crm_audit "+
      "WHERE tenant_id='tenant-a' AND request_id='"+row.request_id+"' ORDER BY action"));
    assert.deepEqual(actual,expected,
      "One AdvanceStage request must commit attributed Deal + Task + delivery audit together");
  }
  await verifySalesLinkedTasks({dispatch:rpcFetch,sql,linked,admin,viewer,tenantB,knownService,
   dealId,dealLast,accountId,accountNextId,taskMid});
  await verifySalesLifecycle({dispatch:rpcFetch,sql,statement,batch,accountId,dealId,dealLast,
    admin,viewer,pipelineSeed,closeSale});
 console.log(JSON.stringify({event:"crm.native-g3-party-v3",
   result:"PASS",generated_binary:true,owner:"crm.parties",
   rpc_methods:["CreateParty","UpdateParty","GetParty","ListParties"],
   signed_access:true,tenant_ab:true,viewer_mutation_denied:true,
   original_d1_cas:true,d1_receipts:true,d1_audit:true,
   published_active:true}));

 assert.equal((await call("/crm.future.v9.FutureService/NotRegistered",viewer,"POST")).status,501);

 // Privacy intake: execute original domain transitions through binary Connect.

 await verifyPrivacyIntake({dispatch:rpcFetch,sql,admin,viewer,tenantB,knownService,unknownService});

 await verifyPrivacySubject({dispatch:rpcFetch,sql,id,pagingPersonId,r3Contact,admin,viewer,tenantB,knownService});

 // Immutable configuration only: no provider I/O or Party mutation.
 await verifyEnrichment({dispatch:rpcFetch,sql,admin,viewer,tenantB,knownService,unknownService});

 const {linkedTaskId}=await verifyAccountOverview({
   dispatch:rpcFetch,sql,admin,viewer,tenantB,accountId,partyId:id,contactId,
   relationshipId:rMid,dealId,taskId:taskMid,otherTaskId:taskOther,
 });
 await verifyPartyCsv({dispatch:rpcFetch,rpcFetch,sql,partySnapshot,admin,viewer,tenantB,knownService,unknownService});
 await verifyCustomerReadParity({dispatch:rpcFetch,sql,admin,viewer,tenantB,id,accountId,contactId,rMid,dealId,linkedTaskId});
 // SK-05: the SAME binary Connect + signed worker + original Activities D1
 // validates completed calls/meetings/notes independently of Sales stage audit.
 const runScheduled=worker.scheduled.bind(worker);
 const {eventLater,eventEarlier,eventNote,failedId}=await verifyInteractions({
   dispatch:rpcFetch,sql,accountId,accountNextId,dealId,sourceSha,admin,viewer,tenantB,
   runScheduledProbe:async()=>{
 // Wrangler's documented native harness dispatches the compiled scheduled
 // entrypoint of THIS SAME Rust Worker. Neither cron nor a rule is enabled
 // in production: the boundary is read-only until complete run/lease acceptance.
 const scheduledAuditBefore=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n;
 const logStart=testHarness.getLogs().length;
 for(let attempt=0;attempt<2;attempt++){
   const result=await runScheduled({
     cron:"*/5 * * * *",scheduledTime:new Date("2026-11-24T00:05:00.000Z")
   });
   assert.equal(result.outcome,"ok","native scheduled Worker must dispatch without an exception");
   assert.equal(result.noRetry,false);
 }
 assert.ok(testHarness.getLogs().slice(logStart).some(entry=>
   String(entry.message).includes("CRM_AUTOMATION_SCHEDULED_DISPATCH_DISABLED")),
   "actual Rust scheduled callback must log its disabled-dispatch boundary");
 assert.equal((await sql(
   "SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE tenant=?","tenant-a"))[0]?.n,3,
   "two cron invocations cannot consume or duplicate source events");
 assert.equal((await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n,
   scheduledAuditBefore,"scheduled compatibility probe must not write audit");
 console.log(JSON.stringify({event:"crm.native-sk08a-scheduled-boundary",result:"PASS",
   native_workerd_handler:true,repeat_count:2,no_cross_tenant_read:true,
   no_task_creation:true,no_new_audit:true,production_cron_disabled:true}));

   },
   cleanupLongestInteraction:async(cleanupId,cleanupKey)=>{
 await batch([
   statement("DELETE FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
     "tenant-a",cleanupId),
   statement("DELETE FROM crm_activity_interaction_receipts WHERE tenant=? AND key=?",
     "tenant-a",cleanupKey),
   statement("DELETE FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action=?",
     "tenant-a",cleanupId,"activities.interaction.recorded"),
   statement("DELETE FROM crm_activity_interactions WHERE tenant=? AND id=?",
     "tenant-a",cleanupId)
 ]);
   },
 });

 await verifyTaskDueOrdering({dispatch:rpcFetch,sql,accountNextId,admin,viewer,tenantB});

 await verifyRecentInteractions({dispatch:rpcFetch,sql,accountId,accountNextId,dealId,eventLater,eventEarlier,eventNote,failedId,sourceSha,admin,viewer,tenantB});

 await verifyWorkforce({dispatch:rpcFetch,sql,jwt,accountId,admin,viewer,tenantB,knownService});
 await typedBrowser({base,admin,viewer,tenantB,sql,sourceSha,runScheduled,adminIdentity:"native-a@example.test"});
 const salesAuditBeforeFilter=number((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0]?.n);
 const collectDeals=(pageSize:number,filter:{accountId?:string}={})=>collectFixturePages(async pageToken=>{
   const {body}=await fixtureRpc(rpcFetch,sales.listDeals,viewer,{...filter,page:{pageSize,pageToken}});
   return {items:body.deals,nextPageToken:body.pageInfo?.nextPageToken};
 },pageSize);
 assert.deepEqual((await collectDeals(1,{accountId})).map(d=>d.dealId),
   (await collectDeals(100)).filter(d=>d.accountId===accountId).map(d=>d.dealId));
 assert.deepEqual((await fixtureRpc(rpcFetch,sales.listDeals,tenantB,{accountId})).body.deals,[]);
 assert.deepEqual((await fixtureRpc(rpcFetch,sales.listDeals,viewer,{accountId:"missing-account"})).body.deals,[]);
 assert.equal((await fixtureRpcError(rpcFetch,sales.listDeals,viewer,{accountId:"bad/id"})).status,400);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0]?.n,salesAuditBeforeFilter);
 await verifyPartyAccountSearch({dispatch:rpcFetch,sql,partySnapshot,cwd,base,sourceSha,id,admin,viewer,tenantB});
 await verifyForecast({dispatch:rpcFetch,sql,accountId,admin,viewer,tenantB});
 await verifyReceipts({dispatch:rpcFetch,sql,call,admin,viewer,tenantB,saleCreate,accountRetryInput,contactRetryInput,
   party:{id,other:"binary-rpc-proof-01",key:creation.precondition.idempotencyKey,headers:created.headers},
   account:{id:accountId,other:accountNextId,key:accountInput.precondition.idempotencyKey,headers:accountCreated.headers},
   contact:{id:contactId,other:contactLastId,key:contactInput.precondition.idempotencyKey,headers:contactCreated.headers},
   relationship:{id:rMid,other:rLast,key:midRelation.precondition.idempotencyKey,headers:midCreated.headers},
   task:{id:taskMid,other:taskLast,key:taskMidInput.precondition.idempotencyKey,headers:taskCreated.headers}});
 await verifySalesOwner({dispatch:rpcFetch,sql,accountId,admin,viewer,tenantB,jwt});
 await verifySalesCoverage({dispatch:rpcFetch,sql,accountId,admin,viewer,tenantB});
 await verifySalesCommercial({dispatch:rpcFetch,sql,statement,batch,accountId,admin,viewer,tenantB});
 await verifyLifecycle({dispatch:rpcFetch,sql,jwt,accountId,admin,viewer,tenantB});
 const automationFixture={dispatch:rpcFetch,sql,accountId,admin,viewer,tenantB,runScheduled};
 await verifyAutomation(automationFixture);
 await verifySalesLeads({dispatch:rpcFetch,sql,admin,viewer,tenantB});

});

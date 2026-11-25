import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { InteractionService } from "../../../../packages/client/gen/crm/activities/v2/interaction_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records, string } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type InteractionFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 runScheduledProbe: () => Promise<void>;
 cleanupLongestInteraction: (id: string, key: string) => Promise<void>;
 accountId: string; accountNextId: string; dealId: string; sourceSha: string;
 admin: string; viewer: string; tenantB: string;
};
export async function verifyInteractions({dispatch,sql,runScheduledProbe,cleanupLongestInteraction,accountId,accountNextId,dealId,sourceSha,admin,viewer,tenantB}: InteractionFixture) {
 const interaction=InteractionService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>) => {
   assert.equal(failed.status,503);
   assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,"ACTIVITIES_INTERACTION_WRITE_UNCONFIRMED");
   assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 const eventLater="sk05-later-"+randomUUID();
 const eventEarlier="sk05-earlier-"+randomUUID();
 const eventNote="sk05-note-"+randomUUID();
 const occurrence=(seconds: number)=>({seconds:BigInt(seconds),nanos:123456789});
 const interactionInput=(interactionId: string,kind: number,seconds: number,deal="")=>({
   interactionId,accountId,dealId:deal,kind,
   text:"SK05 validated interaction "+interactionId,
   occurredAt:occurrence(seconds),
   precondition:{idempotencyKey:"sk05-"+interactionId}
 });
 const laterInput=interactionInput(eventLater,1,1780000000,dealId);
 const earlierInput=interactionInput(eventEarlier,2,1770000000,dealId);
 const noteInput=interactionInput(eventNote,3,1775000000);
 const interactionAuditBefore=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='activities.interaction.recorded'"))[0]?.n;
 assert.ok(typeof interactionAuditBefore==="number");
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,null,laterInput)).status,401);
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,viewer,laterInput)).status,403);
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,tenantB,laterInput)).status,404,
   "cross-tenant Account must not accept events");
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,admin,{
   ...laterInput,accountId:accountNextId,
   precondition:{idempotencyKey:"sk05-mismatch-"+eventLater}})).status,400,
   "Deal and Account must have identical tenant-scoped parent");
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,admin,{
   ...laterInput,dealId:"sk05-nonexistent-deal",
   precondition:{idempotencyKey:"sk05-absent-"+eventLater}})).status,404);
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,admin,{
   ...laterInput,kind:0,
   precondition:{idempotencyKey:"sk05-unknown-kind-"+eventLater}})).status,400);
 const recordedLater=await fixtureRpc(dispatch,interaction.createInteraction,admin,laterInput);
 assert.equal(recordedLater.status,200,formatRpcFailure(recordedLater));
 assert.equal(recordedLater.body.interaction?.author,"native-a@example.test");
 assert.equal(recordedLater.body.interaction?.dealId,dealId);
 assert.equal(recordedLater.body.interaction?.accountId,accountId);
 assert.equal(recordedLater.body.interaction?.kind,1);
 assert.equal(recordedLater.body.interaction?.occurredAt?.seconds,1780000000n);
 assert.equal(recordedLater.body.receipt?.resourceVersion,1n);
 const recordedEarlier=await fixtureRpc(dispatch,interaction.createInteraction,admin,earlierInput);
 const recordedNote=await fixtureRpc(dispatch,interaction.createInteraction,admin,noteInput);
 for(const result of [recordedEarlier,recordedNote]){
   assert.equal(result.status,200,formatRpcFailure(result));
   assert.equal(result.body.receipt?.resourceVersion,1n);
 }
 assert.equal(recordedNote.body.interaction?.kind,3);
 assert.equal(recordedNote.body.interaction?.dealId,"");
 const replayed=await fixtureRpc(dispatch,interaction.createInteraction,admin,laterInput);
 assert.equal(replayed.status,200,formatRpcFailure(replayed));
 assert.equal(replayed.body.receipt?.replayed,true);
 assert.equal(replayed.body.receipt?.originalRequestId,recordedLater.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch,interaction.createInteraction,admin,{
   ...laterInput,text:"Modified on replay"
 })).status,409);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE tenant=? AND id=?",
   "tenant-a",eventLater))[0]?.n,1);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interaction_receipts WHERE tenant='tenant-a'"))[0]?.n,3);
 // The original Interaction INSERT, receipt, audit and Activities-owned
 // immutable automation source event are one physical D1 commit. Replays,
 // forged/denied requests and other tenants never emit a second event.
 const sourceEvents=await sql(
   "SELECT event_id,event_type,source_id,source_version,account_id,deal_id,source_actor,origin_request_id,origin_intent_digest,recorded_at_key "+
   "FROM crm_activity_automation_source_events WHERE tenant=? AND source_id IN (?,?,?) ORDER BY source_id",
   "tenant-a",eventLater,eventEarlier,eventNote);
 assert.equal(sourceEvents.length,3);
 for(const row of sourceEvents){
   assert.equal(row.event_type,"crm.activities.interaction_created.v1");
   assert.equal(row.event_id,"crm.activities.e1:"+row.origin_intent_digest);
   assert.ok(row.event_id.length<=180,"source EventId must match SDK bound");
   assert.equal(row.source_version,1);
   assert.equal(row.account_id,accountId);
   assert.equal(row.source_actor,"native-a@example.test");
   assert.match(string(row.origin_request_id),/^[0-9a-f-]{36}$/);
   assert.match(string(row.origin_intent_digest),/^[a-f0-9]{64}$/);
   assert.match(string(row.recorded_at_key),/^[0-9]{20}$/);
   assert.equal(row.deal_id,row.source_id===eventNote?null:dealId);
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE tenant=?","tenant-b"))[0]?.n,0);
 await runScheduledProbe();


 assert.equal((await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='activities.interaction.recorded'"))[0]?.n,
   interactionAuditBefore+3,"replay and rejected writes must not create another audit");
 const getAdmin=await fixtureRpc(dispatch,interaction.getInteraction,admin,{
   interactionId:eventLater,receiptIdempotencyKey:laterInput.precondition.idempotencyKey});
 const getViewer=await fixtureRpc(dispatch,interaction.getInteraction,viewer,{
   interactionId:eventLater,receiptIdempotencyKey:laterInput.precondition.idempotencyKey});
 assert.equal(getAdmin.status,200);
 assert.equal(getViewer.status,200);
 assert.equal(getViewer.body.interaction?.author,"native-a@example.test");
 assert.deepEqual(getAdmin.body.interaction,getViewer.body.interaction);
 assert.equal(getAdmin.body.confirmedMutation?.originalRequestId,
   recordedLater.headers.get("x-request-id"));
 assert.equal(getAdmin.body.confirmedMutation?.resourceVersion,1n);
 assert.equal((await fixtureRpc(dispatch,interaction.getInteraction,viewer,{
   interactionId:eventEarlier,receiptIdempotencyKey:laterInput.precondition.idempotencyKey
 })).body.confirmedMutation,undefined,"other record receipt must not be disclosed");
 assert.equal((await fixtureRpcError(dispatch,interaction.getInteraction,tenantB,{
   interactionId:eventLater})).status,404);
 const firstPage=await fixtureRpc(dispatch,interaction.listInteractions,viewer,{accountId,page:{pageSize:2}});
 assert.equal(firstPage.status,200);
 assert.deepEqual(firstPage.body.interactions.map(row=>row.interactionId),[eventLater,eventNote],
   "late backdated event must sort by fact time, not recorded/audit time");
 assert.ok(firstPage.body.pageInfo?.nextPageToken);
 const secondPage=await fixtureRpc(dispatch,interaction.listInteractions,viewer,{
   accountId,page:{pageSize:2,pageToken:firstPage.body.pageInfo.nextPageToken}
 });
 assert.deepEqual(secondPage.body.interactions.map(row=>row.interactionId),[eventEarlier]);
 assert.equal(secondPage.body.pageInfo?.nextPageToken??"","");
 const listAdmin=await fixtureRpc(dispatch,interaction.listInteractions,admin,{accountId,page:{pageSize:2}});
 assert.deepEqual(listAdmin.body,firstPage.body,
   "viewer sees all accessible original business fields and chronology");
 const dealPage=await fixtureRpc(dispatch,interaction.listInteractions,viewer,{accountId,dealId,page:{pageSize:10}});
 assert.deepEqual(dealPage.body.interactions.map(row=>row.interactionId),
   [eventLater,eventEarlier],"Deal history cannot show Account-only note");
 assert.deepEqual((await fixtureRpc(dispatch,interaction.listInteractions,tenantB,{
   accountId,page:{pageSize:10}})).body.interactions,[]);
 assert.deepEqual((await fixtureRpc(dispatch,interaction.listInteractions,viewer,{
   accountId:accountNextId,dealId,page:{pageSize:10}})).body.interactions,[]);
 assert.equal((await fixtureRpcError(dispatch,interaction.listInteractions,viewer,{
   accountId,page:{pageSize:2,pageToken:"invalid"}
 })).status,400);
 const interactionAuditBeforeReads=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 await fixtureRpc(dispatch,interaction.getInteraction,viewer,{interactionId:eventNote});
 await fixtureRpc(dispatch,interaction.listInteractions,viewer,{accountId,page:{pageSize:5}});
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   interactionAuditBeforeReads,"read-only history must not write audit");
 const failedId="sk05-failed-"+randomUUID();
 const failedInput=interactionInput(failedId,3,1773000000);
 await sql("CREATE TRIGGER fixture_fail_sk05_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+failedId+"' BEGIN SELECT RAISE(ABORT,'fixture audit'); END;");
 try{
   assertUnconfirmed(await fixtureRpcError(dispatch,interaction.createInteraction,admin,failedInput));
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE id=?",
     failedId))[0]?.n,0);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interaction_receipts WHERE key=?",
     failedInput.precondition.idempotencyKey))[0]?.n,0);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE source_id=?",
     failedId))[0]?.n,0);
 }finally{await sql("DROP TRIGGER fixture_fail_sk05_audit");}
 const recovered=await fixtureRpc(dispatch,interaction.createInteraction,admin,failedInput);
 assert.equal(recovered.status,200);
 assert.equal(recovered.body.receipt?.replayed,false);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE subject_id=?",failedId))[0]?.n,1);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE source_id=?",failedId))[0]?.n,1);
 // The original RecordId maximum must never overflow EventId. The
 // Activity owner writes an opaque bounded event ID from the signed intent
 // digest, not a potentially 180-character user-supplied source ID.
 const longestInteractionId="i".repeat(180);
 const longKey="sk08a-long-"+randomUUID();
 const longestResult=await fixtureRpc(dispatch,interaction.createInteraction,admin,{
   ...noteInput,interactionId:longestInteractionId,
   precondition:{idempotencyKey:longKey}
 });
 assert.equal(longestResult.status,200,formatRpcFailure(longestResult));
 const longSource=(await sql(
   "SELECT event_id,origin_intent_digest FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
   "tenant-a",longestInteractionId));
 assert.equal(longSource.length,1);
 assert.ok(longSource[0]);
 assert.equal(longSource[0].event_id,"crm.activities.e1:"+longSource[0].origin_intent_digest);
 assert.ok(string(longSource[0].event_id).length<=180);
 // This boundary-length fixture is intentionally not part of the SK-05
 // Account chronology; remove only its exact four linked D1 records.
 // All real Interaction/read/replay/rollback evidence above remains intact.
 await cleanupLongestInteraction(longestInteractionId,longKey);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE tenant=? AND id=?",
   "tenant-a",longestInteractionId))[0]?.n,0,"long-ID fixture must not alter the next scenario");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
   "tenant-a",longestInteractionId))[0]?.n,0);

 console.log(JSON.stringify({event:"crm.native-sk08a-interaction-source",result:"PASS",atomic:true,
   original_activities_owner:true,exact_origin_request:true,tenant_ab:true,
   replay_single_event:true,failed_audit_rollback:true,bounded_event_id:true,
   automation_execution_disabled:true}));
 console.log(JSON.stringify({event:"crm.native-sk05-interaction",result:"PASS",
   source_sha:sourceSha,original_owner:"crm.activities",binary_connect:true,tenant_ab:true,
   viewer_fields_equal:true,viewer_write_denied:true,account_deal_fk:true,
   chronological_keyset:true,idempotent_replay:true,
   immutable_receipt_and_audit:true,failed_audit_rollback:true}));


 return {eventLater,eventEarlier,eventNote,failedId};
}

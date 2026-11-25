import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { IdentityResolutionService } from "../../../../packages/client/gen/crm/identity_resolution/v2/identity_pb.js";
import { ContactPointKind, ContactPointService } from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import { PartyKind, PartyService } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";

type Sql = (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;

export async function verifyIdentityResolution(
 dispatch: RpcDispatch, sql: Sql, id: string, pagingPersonId: string,
 admin: string, viewer: string, tenantB: string,
) {
 const methods = IdentityResolutionService.method;
 const count = async (statement: string, ...bindings: string[]) => {
   const [row] = await sql(statement, ...bindings);
   assert.ok(typeof row?.n === "number", "Expected a D1 count");
   return row.n;
 };
 const assertUnconfirmed = async (response: Awaited<ReturnType<RpcDispatch>>, code: string) => {
   const body = record(await response.json());
   assert.equal(body.code, "unavailable");
   const detail = decodeFixtureDetail(DomainErrorDetailSchema, records(body.details));
   assert.equal(detail.commitOutcome, MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode, code);
   assert.equal(detail.requestId, response.headers.get("x-request-id"));
 };
 const beforeCandidates=await fixtureRpc(dispatch, methods.findSharedEmailCandidates,viewer,{page:{pageSize:1}});
 assert.equal(beforeCandidates.body.candidates.length,0);
 const auditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'");
 for(const [index,partyId] of [pagingPersonId,"binary-rpc-proof-01"].entries()){
   const contact=await fixtureRpc(dispatch, ContactPointService.method.createContactPoint, admin,{
     contactPointId:"shared-email-"+index+"-"+randomUUID(),
     partyId,kind:ContactPointKind.EMAIL,value:"later-timestamps@example.org",preferred:false,
     precondition:{idempotencyKey:"shared-email-create-"+index}});
   assert.equal(contact.body.contactPoint?.verified,false);
 }
 const auditAfter=await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'");
 assert.equal(auditAfter,auditBefore+2);
 await sendFixtureRpc(dispatch, methods.findSharedEmailCandidates,null,{},401);
 await sendFixtureRpc(dispatch, methods.findSharedEmailCandidates,viewer,{page:{pageSize:101}},400);
 await sendFixtureRpc(dispatch, methods.findSharedEmailCandidates,viewer,{
   page:{pageSize:1,pageToken:"invalid"}},400);
 assert.equal((await fixtureRpc(dispatch, methods.findSharedEmailCandidates,tenantB,{
   page:{pageSize:1}})).body.candidates.length,0);
 const matches = await collectFixturePages(async pageToken => {
   const { body } = await fixtureRpc(dispatch, methods.findSharedEmailCandidates, viewer, {
     page: { pageSize: 1, pageToken },
   });
   return { items: body.candidates, nextPageToken: body.pageInfo?.nextPageToken };
 });
 assert.equal(matches.length,3,"three canonical pair observations");
 assert.ok(matches.every(c=>!c.bothContactsVerified));
 const pairs=matches.map(c=>[c.firstParty?.partyId,c.secondParty?.partyId].join("|"));
 assert.equal(new Set(pairs).size,3);
 for (const candidate of matches) {
   assert.ok(candidate.firstParty && candidate.secondParty);
   assert.ok(candidate.firstParty.partyId < candidate.secondParty.partyId);
 }
 const compare={firstPartyId:pagingPersonId,secondPartyId:id};
 await sendFixtureRpc(dispatch, methods.compareParties,null,compare,401);
 await sendFixtureRpc(dispatch, methods.compareParties,tenantB,compare,404);
 await sendFixtureRpc(dispatch, methods.compareParties,viewer,{
   firstPartyId:id,secondPartyId:id},400);
 await sendFixtureRpc(dispatch, methods.compareParties,viewer,{
   firstPartyId:id,secondPartyId:"missing-party"},404);
 const compared=await fixtureRpc(dispatch, methods.compareParties,viewer,compare);
 assert.equal(compared.body.firstParty?.partyId,pagingPersonId);
 assert.equal(compared.body.secondParty?.partyId,id);
 assert.equal(compared.body.samePartyId,false);
 assert.equal(compared.body.identityConfirmed,false);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"),
   auditAfter,"read-only identity operations must not write audit");
 console.log(JSON.stringify({event:"crm.native-g3-identity-resolution-v2",
   result:"PASS",rpc_methods:["FindSharedEmailCandidates","CompareParties"],
   generated_binary:true,original_owner:"crm.identity-resolution",
   tenant_ab:true,viewer_read:true,pagination:true,no_identity_verdict:true,
   no_writes:true,published_active:true}));

 // L-03: a governed canonical merge is reversible lineage, not data reparenting.
 // The unchanged downstream D1 owner tables prove no Account/Contact/Deal/Task loss.
 const mergeSourceId="native-merge-source-"+randomUUID();
 const mergeSurvivorId="native-merge-survivor-"+randomUUID();
 for(const [p,name] of [[mergeSourceId,"Merge source"],[mergeSurvivorId,"Merge survivor"]] as const){
   await fixtureRpc(dispatch, PartyService.method.createParty, admin,{
     partyId:p,kind:PartyKind.PERSON,displayName:name,
     precondition:{idempotencyKey:"identity-l03-party-"+randomUUID()}});
 }
 const mergeOpId="native-merge-"+randomUUID();
 const mergeKey="identity-l03-merge-"+randomUUID();
 const intent={
   operationId:mergeOpId,sourcePartyId:mergeSourceId,sourcePartyVersion:1n,
   survivorPartyId:mergeSurvivorId,survivorPartyVersion:1n,
   reason:"manual_identity_review",precondition:{idempotencyKey:mergeKey}
 };
 const mergeBefore=async()=>({
   operation:await count("SELECT COUNT(*) AS n FROM crm_identity_merge_records WHERE tenant=? AND id=?",
     "tenant-a",mergeOpId),
   receipt:await count("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",mergeKey),
   audit:await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='identity_resolution.merge'",
     "tenant-a",mergeOpId)
 });
 const beforeMergeAdapterFailure=await mergeBefore();
 await sql("ALTER TABLE crm_identity_merge_receipts RENAME TO fixture_unavailable_identity_merge_receipts");
 try {
   const failed=await sendFixtureRpc(dispatch,methods.mergeParty,admin,intent,503);
   await assertUnconfirmed(failed,"IDENTITY_MERGE_WRITE_UNAVAILABLE");
   assert.match(failed.headers.get("x-request-id")??"",/^[a-f0-9-]{36}$/i);
   assert.equal(await count("SELECT COUNT(*) AS n FROM crm_identity_merge_records WHERE tenant=? AND id=?",
     "tenant-a",mergeOpId),beforeMergeAdapterFailure.operation);
   assert.equal(await count("SELECT COUNT(*) AS n FROM fixture_unavailable_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",mergeKey),beforeMergeAdapterFailure.receipt);
   assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='identity_resolution.merge'",
     "tenant-a",mergeOpId),beforeMergeAdapterFailure.audit);
 } finally {
   await sql("ALTER TABLE fixture_unavailable_identity_merge_receipts RENAME TO crm_identity_merge_receipts");
 }
 const downstreamTables=["crm_account_records","crm_contact_point_records",
   "crm_party_relationship_records","crm_sales_deal_records",
   "crm_activity_task_records","crm_consent_records"];
 const beforeDownstream=await Promise.all(downstreamTables.map(async table=>
   await count("SELECT COUNT(*) AS n FROM "+table)));
 const beforeLineageAudit=await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'");
 await sendFixtureRpc(dispatch, methods.mergeParty,viewer,intent,403);
 await sendFixtureRpc(dispatch, methods.mergeParty,tenantB,intent,404);
 await sendFixtureRpc(dispatch, methods.mergeParty,admin,{
   ...intent,sourcePartyVersion:2n,
   precondition:{idempotencyKey:"l03-stale-"+randomUUID()}
 },409);
 await sql("CREATE TRIGGER fixture_identity_rollback BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action='identity_resolution.merge' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;");
 try {
   await assertUnconfirmed(await sendFixtureRpc(dispatch, methods.mergeParty, admin, intent, 503), "IDENTITY_MERGE_COMMIT_UNCONFIRMED");
   assert.equal(await count("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",intent.precondition.idempotencyKey),0);
 } finally {await sql("DROP TRIGGER fixture_identity_rollback");}
 const merged=await fixtureRpc(dispatch, methods.mergeParty,admin,intent);
 assert.equal(merged.body.operation?.active,true);
 assert.equal(merged.body.operation?.version,1n);
 assert.equal(merged.body.receipt?.replayed,false);
 assert.equal(merged.body.receipt?.resourceVersion,1n);
 assert.equal((await fixtureRpc(dispatch, methods.mergeParty,admin,intent)).body.receipt?.replayed,true);
 await sendFixtureRpc(dispatch, methods.mergeParty,admin,{...intent,
   reason:"different_reason"},409);
 // active source cannot be merged twice
 await sendFixtureRpc(dispatch, methods.mergeParty,admin,{...intent,
   operationId:"l03-double-"+randomUUID(),
   precondition:{idempotencyKey:"identity-l03-duplicate-"+randomUUID()}
 },409);
 // cycle or redirected survivor must fail
 await sendFixtureRpc(dispatch, methods.mergeParty,admin,{
   ...intent,operationId:"l03-reverse-"+randomUUID(),
   sourcePartyId:mergeSurvivorId,survivorPartyId:mergeSourceId,
   precondition:{idempotencyKey:"identity-l03-reverse-"+randomUUID()}
 },409);
 const got=await fixtureRpc(dispatch, methods.getMergeOperation,viewer,{
   operationId:mergeOpId,receiptIdempotencyKey:mergeKey
 });
 assert.equal(got.body.operation?.operationId,mergeOpId);
 assert.equal(got.body.confirmedMutation?.resourceVersion,1n);
 assert.ok(got.body.confirmedMutation?.originalRequestId);
 await sendFixtureRpc(dispatch, methods.getMergeOperation,tenantB,{operationId:mergeOpId},404);
 const history=await fixtureRpc(dispatch, methods.listMergeOperationsByParty,viewer,{
   partyId:mergeSourceId,page:{pageSize:10}
 });
 assert.equal(history.body.operations.length,1);
 assert.equal(history.body.operations[0]?.operationId,mergeOpId);
 await sendFixtureRpc(dispatch, methods.listMergeOperationsByParty,tenantB,{
   partyId:mergeSourceId,page:{pageSize:10}
 },404);
 const resolved=await fixtureRpc(dispatch, methods.resolveCanonicalParty,viewer,{partyId:mergeSourceId});
 assert.equal(resolved.body.canonicalPartyId,mergeSurvivorId);
 assert.deepEqual(resolved.body.partyPath,[mergeSourceId,mergeSurvivorId]);
 assert.deepEqual(resolved.body.mergeOperationPath,[mergeOpId]);
 await sendFixtureRpc(dispatch, methods.resolveCanonicalParty,tenantB,{
   partyId:mergeSourceId
 },404);
 const unmergeKey="identity-l03-unmerge-"+randomUUID();
 const undo={operationId:mergeOpId,sourcePartyVersion:1n,survivorPartyVersion:1n,
   reason:"incorrect_merge",precondition:{expectedVersion:1n,idempotencyKey:unmergeKey}};
 await sendFixtureRpc(dispatch, methods.unmergeParty,viewer,undo,403);
 await sendFixtureRpc(dispatch, methods.unmergeParty,admin,{
   ...undo,precondition:{expectedVersion:2n,idempotencyKey:"l03-undo-stale-"+randomUUID()}
 },409);
 const unmergeBefore=async()=>({
   operation:await count("SELECT COUNT(*) AS n FROM crm_identity_merge_records WHERE tenant=? AND id=? AND status='active' AND version=1",
     "tenant-a",mergeOpId),
   receipt:await count("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",unmergeKey),
   audit:await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='identity_resolution.unmerge'",
     "tenant-a",mergeOpId)
 });
 const beforeUnmergeAdapterFailure=await unmergeBefore();
 await sql("ALTER TABLE crm_identity_merge_receipts RENAME TO fixture_unavailable_identity_merge_receipts");
 try {
   const failed=await sendFixtureRpc(dispatch,methods.unmergeParty,admin,undo,503);
   await assertUnconfirmed(failed,"IDENTITY_UNMERGE_WRITE_UNAVAILABLE");
   assert.match(failed.headers.get("x-request-id")??"",/^[a-f0-9-]{36}$/i);
   assert.equal(await count("SELECT COUNT(*) AS n FROM crm_identity_merge_records WHERE tenant=? AND id=? AND status='active' AND version=1",
     "tenant-a",mergeOpId),beforeUnmergeAdapterFailure.operation);
   assert.equal(await count("SELECT COUNT(*) AS n FROM fixture_unavailable_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",unmergeKey),beforeUnmergeAdapterFailure.receipt);
   assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='identity_resolution.unmerge'",
     "tenant-a",mergeOpId),beforeUnmergeAdapterFailure.audit);
 } finally {
   await sql("ALTER TABLE fixture_unavailable_identity_merge_receipts RENAME TO crm_identity_merge_receipts");
 }
 await sql("CREATE TRIGGER fixture_identity_rollback BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action='identity_resolution.unmerge' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;");
 try {
   await assertUnconfirmed(await sendFixtureRpc(dispatch, methods.unmergeParty, admin, undo, 503), "IDENTITY_UNMERGE_COMMIT_UNCONFIRMED");
   assert.equal(await count("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",undo.precondition.idempotencyKey),0);
 } finally {await sql("DROP TRIGGER fixture_identity_rollback");}
 const undone=await fixtureRpc(dispatch, methods.unmergeParty,admin,undo);
 assert.equal(undone.body.operation?.active,false);
 assert.equal(undone.body.operation?.version,2n);
 assert.equal((await fixtureRpc(dispatch, methods.unmergeParty,admin,undo)).body.receipt?.replayed,true);
 const reverted=await fixtureRpc(dispatch, methods.resolveCanonicalParty,viewer,{partyId:mergeSourceId});
 assert.equal(reverted.body.canonicalPartyId,mergeSourceId);
 assert.deepEqual(reverted.body.partyPath,[mergeSourceId]);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_identity_merge_records "+
   "WHERE tenant='tenant-a' AND id='"+mergeOpId+"' AND status='unmerged' AND version=2"),1);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts "+
   "WHERE tenant='tenant-a'"),2,"one immutable receipt per original mutation");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"),
   beforeLineageAudit+2,"one merge and one unmerge audit, reads and replays do not write");
 const afterDownstream=await Promise.all(downstreamTables.map(async table=>
   await count("SELECT COUNT(*) AS n FROM "+table)));
 assert.deepEqual(afterDownstream,beforeDownstream,"canonical lineage never rewires other owners");
 console.log(JSON.stringify({event:"crm.native-l03-merge-lineage",result:"PASS",
   generated_binary:true,role_policy:true,tenant_isolation:true,stale_party_guard:true,
   replay_receipt:true,immutable_history:true,canonical_resolve:true,
   reversible:true,downstream_unchanged:true,audit_atomic:true,
   adapter_failure_unconfirmed:true,same_key_recovery:true}));
}

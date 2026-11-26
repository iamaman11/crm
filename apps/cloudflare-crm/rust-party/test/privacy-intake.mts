import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { PrivacyCaseService } from "../../../../packages/client/gen/crm/customer_privacy/v2/case_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, encodeFixtureRpcInput, fixtureRpc, fixtureRpcError, fixtureRpcPath } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type PrivacyFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
 unknownService: string;
};
export async function verifyPrivacyIntake({dispatch,sql,admin,viewer,tenantB,knownService,unknownService}: PrivacyFixture) {
 const privacy=PrivacyCaseService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>) => {
   assert.equal(failed.status,503);
   assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,"PRIVACY_CASE_WRITE_UNCONFIRMED");
   assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 const collectCases=()=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,privacy.listPrivacyCases,viewer,{page:{pageSize:1,pageToken}});
   return {items:result.body.cases,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const caseId="privacy-case-"+randomUUID();
 const draftInput={caseId,kind:1,precondition:{idempotencyKey:"privacy-create-main"}};
 assert.equal((await fixtureRpcError(dispatch,privacy.createPrivacyCase,admin,{...draftInput,kind:0})).body.code,"invalid_argument");
 // Unknown enum 999 is an intentional malformed wire value, not a typed request.
 const unknownKind=await dispatch(fixtureRpcPath(privacy.createPrivacyCase),{method:"POST",headers:{
   "content-type":"application/proto","connect-protocol-version":"1","cf-access-jwt-assertion":admin},
   body:new Uint8Array([...encodeFixtureRpcInput(privacy.createPrivacyCase,{...draftInput,kind:0}),0x10,0xe7,0x07])});
 assert.equal(record(await unknownKind.json()).code,"invalid_argument");
 assert.equal((await fixtureRpcError(dispatch,privacy.createPrivacyCase,admin,{...draftInput,
   precondition:{idempotencyKey:"privacy-create-invalid",expectedVersion:1n}})).body.code,"invalid_argument");
 const draft=await fixtureRpc(dispatch,privacy.createPrivacyCase,admin,draftInput,{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test",
   "x-request-id":"forged-privacy-request"});
 assert.equal(draft.status,200,formatRpcFailure(draft));
 assert.equal(draft.body.privacyCase?.caseId,caseId);
 assert.equal(draft.body.privacyCase?.status,1);
 assert.equal(draft.body.privacyCase?.version,1n);
 assert.equal(draft.body.privacyCase?.subjectVerified,false);
 assert.equal(draft.body.receipt?.requestId,draft.headers.get("x-request-id"));
 assert.equal(draft.body.receipt?.replayed,false);
 const originalAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'"))[0]?.n;
 assert.equal(originalAudit,1);
 assert.equal((await fixtureRpc(dispatch,privacy.createPrivacyCase,admin,draftInput)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,privacy.createPrivacyCase,admin,{...draftInput,kind:4})).body.code,"aborted");
 assert.equal((await fixtureRpc(dispatch,privacy.getPrivacyCase,viewer,{caseId})).body.privacyCase?.version,1n);
 assert.equal((await fixtureRpcError(dispatch,privacy.getPrivacyCase,tenantB,{caseId})).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch,privacy.getPrivacyCase,viewer,{caseId:"bad/id"})).body.code,"invalid_argument");
 const submit={caseId,precondition:{expectedVersion:1n,idempotencyKey:"privacy-submit-main"}};
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,tenantB,submit)).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,admin,{...submit,
   precondition:{expectedVersion:9n,idempotencyKey:"privacy-submit-stale"}})).body.code,"aborted");
 const submitted=await fixtureRpc(dispatch,privacy.submitPrivacyCase,knownService,submit);
 assert.equal(submitted.status,200,formatRpcFailure(submitted));
 assert.equal(submitted.body.privacyCase?.status,2);
 assert.equal(submitted.body.receipt?.resourceVersion,2n);
 assert.equal((await fixtureRpc(dispatch,privacy.submitPrivacyCase,admin,submit)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,admin,{...submit,
   precondition:{expectedVersion:2n,idempotencyKey:"privacy-submit-again"}})).body.code,"failed_precondition");
 const cancel={caseId,precondition:{expectedVersion:2n,idempotencyKey:"privacy-cancel-main"}};
 assert.equal((await fixtureRpcError(dispatch,privacy.cancelPrivacyCase,tenantB,cancel)).body.code,"not_found");
 const cancelled=await fixtureRpc(dispatch,privacy.cancelPrivacyCase,admin,cancel);
 assert.equal(cancelled.status,200,formatRpcFailure(cancelled));
 assert.equal(cancelled.body.privacyCase?.status,15);
 assert.equal(cancelled.body.receipt?.resourceVersion,3n);
 assert.equal((await fixtureRpc(dispatch,privacy.cancelPrivacyCase,admin,cancel)).body.receipt?.replayed,true);
 // Replaying the earlier submit returns its original receipt, not latest state.
 const submitReplay=await fixtureRpc(dispatch,privacy.submitPrivacyCase,admin,submit);
 assert.equal(submitReplay.body.receipt?.replayed,true);
 assert.equal(submitReplay.body.privacyCase?.status,2);
 assert.equal((await fixtureRpc(dispatch,privacy.getPrivacyCase,viewer,{caseId})).body.privacyCase?.status,15);
 assert.equal((await sql("SELECT version FROM crm_privacy_case_records WHERE tenant='tenant-a' AND id='"+caseId+"'"))[0]?.version,3);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts WHERE tenant='tenant-a'"))[0]?.n,3);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'"))[0]?.n,3);
 assert.deepEqual((await sql("SELECT action,subject_id,request_id,actor FROM crm_audit WHERE "+
   "tenant_id='tenant-a' AND subject_id='"+caseId+"' ORDER BY rowid")),[
   {action:"customer_privacy.create",subject_id:caseId,
     request_id:draft.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"customer_privacy.submit",subject_id:caseId,
     request_id:submitted.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
   {action:"customer_privacy.cancel",subject_id:caseId,
     request_id:cancelled.headers.get("x-request-id"),actor:"native-a@example.test"},
 ],"Committed actions must preserve their own verified Access actor and request");
 assert.deepEqual((await sql("SELECT created_by,actor FROM crm_privacy_case_records "+
   "WHERE tenant='tenant-a' AND id='"+caseId+"'")),[
   {created_by:"native-a@example.test",actor:"native-a@example.test"},
 ],"Creator is immutable and the last transition actor is separate");
 const failingTransitionId="privacy-audit-transition-"+randomUUID();
 assert.equal((await fixtureRpc(dispatch,privacy.createPrivacyCase,admin,{
   caseId:failingTransitionId,kind:1,
   precondition:{idempotencyKey:"privacy-audit-ready-transition"}})).status,200);
 const privacySnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(tenant,id,state,version,created_by,actor)) "+
   "FROM crm_privacy_case_records) AS states,"+
   "(SELECT COUNT(*) FROM crm_privacy_case_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit WHERE tenant_id='tenant-a') AS audits"));
 const privacyBeforeDenials=(await privacySnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await fixtureRpcError(dispatch,privacy.createPrivacyCase,token,draftInput)).status>=400);
   assert.ok((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,token,submit)).status>=400);
   assert.ok((await fixtureRpcError(dispatch,privacy.cancelPrivacyCase,token,cancel)).status>=400);
 }
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,knownService,submit)).status,401,
   "Inactive service cannot replay a committed Privacy write");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,knownService,submit)).status,403,
   "Authorization must precede idempotent replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,tenantB,submit)).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch,privacy.submitPrivacyCase,admin,{
   ...submit,precondition:{expectedVersion:1n,idempotencyKey:"privacy-audit-denied-terminal"}
 })).body.code,"failed_precondition",
   "A cancelled case must reject a second Submit before version conflict");
 assert.deepEqual((await privacySnapshot()),privacyBeforeDenials,
   "Denied, cross-tenant and stale Privacy writes cannot alter state, receipts or audit");
 // Reject audit INSERT after the owner wrote state and receipt.
 // No failure injection is compiled into the production Worker.
 (await sql("CREATE TRIGGER fixture_fail_privacy_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('privacy-audit-failed-create','"+failingTransitionId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;"));
 try{
   assertUnconfirmed(await fixtureRpcError(dispatch,privacy.createPrivacyCase,admin,{
     caseId:"privacy-audit-failed-create",kind:1,
     precondition:{idempotencyKey:"privacy-audit-failed-create-key"}}));
   assertUnconfirmed(await fixtureRpcError(dispatch,privacy.submitPrivacyCase,knownService,{
     caseId:failingTransitionId,
     precondition:{expectedVersion:1n,idempotencyKey:"privacy-audit-failed-submit-key"}
   }));
   assert.deepEqual((await privacySnapshot()),privacyBeforeDenials,
     "Audit failure must roll back Privacy state, receipt and audit together");
 }finally{await sql("DROP TRIGGER fixture_fail_privacy_audit");}
 console.log(JSON.stringify({event:"crm.native-r2a-privacy-actor-audit",result:"PASS",
   human_create:true,service_submit:true,human_cancel:true,cross_actor_replay:true,
   authorization_before_replay:true,forged_headers_ignored:true,tenant_ab:true,
   stale_no_write:true,sql_failure_rollback:true}));
 for(const suffix of ["aaa","zzz"]){
   const result=await fixtureRpc(dispatch,privacy.createPrivacyCase,admin,{caseId:"privacy-"+suffix,kind:4,
     precondition:{idempotencyKey:"privacy-page-"+suffix}});
   assert.equal(result.status,200,formatRpcFailure(result));
 }
 assert.equal((await fixtureRpcError(dispatch,privacy.listPrivacyCases,viewer,{page:{pageSize:101}})).body.code,"invalid_argument");
 const allCases=await collectCases();
 assert.equal(allCases.length,4);
 assert.deepEqual(allCases.map(x=>x.caseId),
   ["privacy-aaa",caseId,"privacy-zzz",failingTransitionId].sort());
 assert.equal((await fixtureRpc(dispatch,privacy.listPrivacyCases,tenantB,{page:{pageSize:1}})).body.cases.length,0);
 assert.equal((await fixtureRpc(dispatch,privacy.listPrivacyCases,viewer,{page:{pageSize:1,pageToken:"privacy-zzz"}})).body.cases.length,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'"))[0]?.n,3,
   "read and denied requests must not write audit for the case");
 console.log(JSON.stringify({event:"crm.native-privacy-v2",result:"PASS",
   original_owner:"crm.customer-privacy",tenant_ab:true,cas:true,replay:true,
   page_bounds:true,d1_audit:true,subject_verification:"NOT_CONNECTED"}));

}

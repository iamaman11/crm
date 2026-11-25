import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { PrivacyCaseService } from "../../../../packages/client/gen/crm/customer_privacy/v2/case_pb.js";
import { ContactPointService } from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type PrivacySubjectFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 id: string;
 pagingPersonId: string;
 r3Contact: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
};
export async function verifyPrivacySubject({dispatch,sql,id,pagingPersonId,r3Contact,admin,viewer,tenantB,knownService}: PrivacySubjectFixture) {
 const privacy=PrivacyCaseService.method;
 const contact=ContactPointService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>) => {
   assert.equal(failed.status,503);
   assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,"PRIVACY_CASE_WRITE_UNCONFIRMED");
   assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 // R4a human self-verification: signed Access email + independently verified
 // original ContactPoint + Person owner; never trust service or dev-agent proof.
 const r4Contact="privacy-own-contact-"+randomUUID();
 const r4Created=await fixtureRpc(dispatch,contact.createContactPoint,admin,{
   contactPointId:r4Contact,partyId:pagingPersonId,kind:1,
   value:"native-a@example.test",preferred:false,
   precondition:{idempotencyKey:"r4-privacy-contact-create-"+r4Contact}});
 assert.equal(r4Created.status,200,formatRpcFailure(r4Created));
 assert.equal((await fixtureRpc(dispatch,contact.verifyOwnEmailContactPoint,admin,{
   contactPointId:r4Contact,
   precondition:{expectedVersion:1n,idempotencyKey:"r4-privacy-contact-verify-"+r4Contact}
 })).status,200);
 const ownCaseId="privacy-own-"+randomUUID();
 const ownCreate=await fixtureRpc(dispatch,privacy.createPrivacyCase,admin,{
   caseId:ownCaseId,kind:2,
   precondition:{idempotencyKey:"r4-privacy-create-"+ownCaseId}});
 assert.equal(ownCreate.status,200,formatRpcFailure(ownCreate));
 const ownSubmit=await fixtureRpc(dispatch,privacy.submitPrivacyCase,admin,{
   caseId:ownCaseId,
   precondition:{expectedVersion:1n,idempotencyKey:"r4-privacy-submit-"+ownCaseId}});
 assert.equal(ownSubmit.status,200,formatRpcFailure(ownSubmit));
 const ownProof={
   caseId:ownCaseId,partyId:pagingPersonId,contactPointId:r4Contact,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-verify-"+ownCaseId}
 };
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,knownService,ownProof))
   .body.code,"permission_denied","Service cannot impersonate human");
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,viewer,ownProof))
   .body.code,"permission_denied","Another signed email cannot verify a subject");
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,tenantB,ownProof))
   .body.code,"permission_denied","Tenant B cannot verify tenant A's ContactPoint");
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,{
   ...ownProof,partyId:id,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-wrong-party"}
 })).body.code,"permission_denied","An Organization cannot be a verified Person");
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,{
   ...ownProof,contactPointId:r3Contact,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-wrong-contact"}
 })).body.code,"permission_denied","A verified email on another Party is not ownership");
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,{
   ...ownProof,contactPointId:"bad/id"
 })).body.code,"invalid_argument");
 assert.equal((await fixtureRpc(dispatch,privacy.getPrivacyCase,viewer,{caseId:ownCaseId}))
   .body.privacyCase?.subjectVerified,false);
 // A current-state scope read is unavailable before verification.
 assert.equal((await fixtureRpcError(dispatch,privacy.previewOwnPrivacyScope,admin,{caseId:ownCaseId}))
   .body.code,"failed_precondition");
 assert.equal((await fixtureRpcError(dispatch,privacy.previewOwnPrivacyScope,admin,{caseId:"bad/id"}))
   .body.code,"invalid_argument");
 const ownVerified=await fixtureRpc(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,ownProof,{
   "x-crm-tenant":"tenant-b","x-crm-actor":"forged@example.test",
   "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(ownVerified.status,200,formatRpcFailure(ownVerified));
 assert.equal(ownVerified.body.privacyCase?.status,3);
 assert.equal(ownVerified.body.privacyCase?.subjectVerified,true);
 assert.equal(ownVerified.body.privacyCase?.version,3n);
 assert.equal(ownVerified.body.receipt?.resourceVersion,3n);
 assert.equal(ownVerified.body.receipt?.replayed,false);
 assert.equal((await fixtureRpc(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,ownProof))
   .body.receipt?.replayed,true,"Exact intent replay uses original snapshot");
 assert.equal((await fixtureRpc(dispatch,privacy.getPrivacyCase,viewer,{caseId:ownCaseId}))
   .body.privacyCase?.subjectVerified,true);
 assert.equal((await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,{
   ...ownProof,precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-verify-stale"}
 })).body.code,"failed_precondition");
 // R4a phase 2: the exact original canonical registry contains nine owners.
 // Only Parties and ContactPoints have current read-through contributions.
 // No durable ScopeSnapshot can be finalized from these two.
 const previewAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit "+
   "WHERE tenant_id='tenant-a' AND subject_id='"+ownCaseId+"'"))[0]?.n;
 assert.equal((await fixtureRpcError(dispatch,privacy.previewOwnPrivacyScope,viewer,{caseId:ownCaseId}))
   .body.code,"permission_denied","Another human has no access to own-scope preview");
 assert.equal((await fixtureRpcError(dispatch,privacy.previewOwnPrivacyScope,knownService,{caseId:ownCaseId}))
   .body.code,"permission_denied","Service identity cannot impersonate the human");
 assert.equal((await fixtureRpcError(dispatch,privacy.previewOwnPrivacyScope,tenantB,{caseId:ownCaseId}))
   .body.code,"not_found","Tenant B must not see tenant A's scope");
 const preview=await fixtureRpc(dispatch,privacy.previewOwnPrivacyScope,admin,{caseId:ownCaseId},{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(preview.status,200,formatRpcFailure(preview));
 assert.equal(preview.body.complete,false,"Never claim a nine-owner discovery snapshot");
 assert.equal(preview.body.caseId,ownCaseId);
 assert.equal(preview.body.caseVersion,3n);
 assert.equal(preview.body.canonicalPartyId,pagingPersonId);
 assert.equal(preview.body.registryDigest?.length,32);
 assert.deepEqual(preview.body.owners.map(o=>o.ownerModuleId),
   ["crm.contact-points","crm.parties"],"Use the canonical owner registry order");
 assert.equal(preview.body.owners.length,2);
 assert.equal(preview.body.missingOwnerModuleIds.length,7);
 assert.ok(preview.body.missingOwnerModuleIds.includes("crm.consents"));
 assert.ok(preview.body.missingOwnerModuleIds.includes("crm.identity-resolution"));
 assert.ok(preview.body.owners.every(o=>o.contributionDigest?.length===32));
 assert.ok(preview.body.resources.some(x=>x.resourceId===pagingPersonId
   && x.ownerModuleId==="crm.parties"&&x.resourceVersion===1n));
 assert.ok(preview.body.resources.some(x=>x.resourceId===r4Contact
   && x.ownerModuleId==="crm.contact-points"&&x.resourceVersion===2n));
 assert.equal(preview.body.resources.length,
   preview.body.owners.reduce((sum,owner)=>sum+owner.resourceCount,0));
 assert.deepEqual((await fixtureRpc(dispatch,privacy.previewOwnPrivacyScope,admin,{caseId:ownCaseId})).body,
   preview.body,"Deterministic owner contributions when D1 state is unchanged");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit "+
   "WHERE tenant_id='tenant-a' AND subject_id='"+ownCaseId+"'"))[0]?.n,
   previewAuditBefore,"Read-only preview must never create an audit write");
 const r4Cancelled=await fixtureRpc(dispatch,privacy.cancelPrivacyCase,admin,{
   caseId:ownCaseId,
   precondition:{expectedVersion:3n,idempotencyKey:"r4-privacy-cancel-after-verify"}
 });
 assert.equal(r4Cancelled.status,200,formatRpcFailure(r4Cancelled));
 assert.equal(r4Cancelled.body.privacyCase?.status,15);
 assert.equal((await fixtureRpcError(dispatch,privacy.previewOwnPrivacyScope,admin,{caseId:ownCaseId}))
   .body.code,"failed_precondition","Cancelled case cannot drive export preview");
 assert.equal(r4Cancelled.body.privacyCase?.subjectVerified,true,
   "Cancelling an authenticated case retains its original verified binding");
 assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
   "WHERE tenant_id='tenant-a' AND subject_id='"+ownCaseId+"' ORDER BY rowid")).map(row=>row.action),
   ["customer_privacy.create","customer_privacy.submit",
     "customer_privacy.subject.verify_own","customer_privacy.cancel"]);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts "+
   "WHERE tenant='tenant-a' AND key='r4-privacy-verify-"+ownCaseId+"'"))[0]?.n,1);

 // Audit insertion failure must roll back the new state, receipt and actor
 // together, through the SAME D1 trigger as all other Privacy transitions.
 const failProofId="privacy-own-failure-"+randomUUID();
 assert.equal((await fixtureRpc(dispatch,privacy.createPrivacyCase,admin,{
   caseId:failProofId,kind:1,
   precondition:{idempotencyKey:"r4-privacy-fail-create-"+failProofId}})).status,200);
 assert.equal((await fixtureRpc(dispatch,privacy.submitPrivacyCase,admin,{
   caseId:failProofId,
   precondition:{expectedVersion:1n,idempotencyKey:"r4-privacy-fail-submit-"+failProofId}
 })).status,200);
 const failVerification={
   caseId:failProofId,partyId:pagingPersonId,contactPointId:r4Contact,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-fail-verify-"+failProofId}};
 await sql("CREATE TRIGGER fixture_r4_privacy_audit_fail BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+failProofId+"' AND NEW.action='customer_privacy.subject.verify_own' "+
   "BEGIN SELECT RAISE(ABORT,'test audit failure'); END");
 try {
   assertUnconfirmed(await fixtureRpcError(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,failVerification));
   assert.equal((await fixtureRpc(dispatch,privacy.getPrivacyCase,admin,{caseId:failProofId}))
     .body.privacyCase?.subjectVerified,false);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts "+
     "WHERE tenant='tenant-a' AND key='r4-privacy-fail-verify-"+failProofId+"'"))[0]?.n,0);
 }finally{await sql("DROP TRIGGER fixture_r4_privacy_audit_fail");}
 assert.equal((await fixtureRpc(dispatch,privacy.verifyOwnPrivacyCaseSubject,admin,failVerification)).status,200,
   "Retry after atomic rollback must commit exactly once");
 console.log(JSON.stringify({event:"crm.native-r4a-privacy-scope-preview",result:"PASS",
   scope_complete:false,covered_owners:2,missing_owners:7,
   original_owner_registry:true,owner_digests:true,tenant_isolation:true,
   unauthorized_denied:true,read_only_no_audit:true,not_export_eligible:true}));
 console.log(JSON.stringify({event:"crm.native-r4a-privacy-signed-subject",result:"PASS",
   owner:"crm.customer-privacy",trusted_contact:"access-email",human_self_only:true,
   subject_verified:true,service_denied:true,tenant_isolation:true,
   forged_headers_ignored:true,d1_cas:true,receipt_replay:true,
   old_legacy_runtime:false,audit_failure_atomic:true}));

}

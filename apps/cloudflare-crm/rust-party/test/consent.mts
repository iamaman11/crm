import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { ConsentAuthorizationService } from "../../../../packages/client/gen/crm/consents/v2/consent_pb.js";
import { ContactPointService } from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records, string } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type ConsentFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 id: string;
 contactId: string;
 pagingPersonId: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
};

export async function verifyConsent({dispatch,sql,id,contactId,pagingPersonId,admin,viewer,tenantB,knownService}: ConsentFixture) {
 const consent = ConsentAuthorizationService.method;
 const contact = ContactPointService.method;
 const assertUnconfirmed = async (response: Awaited<ReturnType<RpcDispatch>>, domainCode: string) => {
   const error = record(await response.json());
   assert.equal(error.code, "unavailable");
   const detail = decodeFixtureDetail(DomainErrorDetailSchema, records(error.details));
   assert.equal(detail.domainCode, domainCode);
   assert.equal(detail.commitOutcome, MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.requestId, response.headers.get("x-request-id"));
 };
// Original crm-consents v2: operator may DENY, never self-attest GRANT.
// Every RPC is generated binary Protobuf through signed native workerd/D1.
const consentScope={partyId:id,contactPointId:contactId,
  purpose:"marketing.newsletter",channel:1};
assert.equal((await sendFixtureRpc(dispatch, consent.evaluateConsent, null, consentScope, 401)).status,401);
assert.equal((await sendFixtureRpc(dispatch, consent.evaluateConsent, viewer, {
  ...consentScope,channel:0}, 400)).status,400);
assert.equal((await sendFixtureRpc(dispatch, consent.evaluateConsent, tenantB, consentScope, 400)).status,400);
const noGrant=await fixtureRpc(dispatch, consent.evaluateConsent, viewer, consentScope);
assert.equal(noGrant.status,200,formatRpcFailure(noGrant));
assert.equal(noGrant.body.allowed,false,"no source-backed GRANT is available");
assert.equal(noGrant.body.reason,4,"NO_APPLICABLE_GRANT");
const denyInput={authorizationId:"consent-mid-"+randomUUID(),
  ...consentScope,jurisdiction:"eu",
  precondition:{idempotencyKey:"connect-consent-deny-mid"}};
assert.equal((await sendFixtureRpc(dispatch, consent.suppressCommunication, null, denyInput, 401)).status,401);
assert.equal((await sendFixtureRpc(dispatch, consent.suppressCommunication, viewer, denyInput, 403)).status,403);
assert.equal((await sendFixtureRpc(dispatch, consent.suppressCommunication, tenantB, denyInput, 400)).status,400);
assert.equal((await sendFixtureRpc(dispatch, consent.suppressCommunication, admin, {
  ...denyInput,channel:0,
  precondition:{idempotencyKey:"connect-consent-invalid-channel"}}, 400)).status,400);
const suppressed=await fixtureRpc(dispatch, consent.suppressCommunication, admin, denyInput);
assert.equal(suppressed.status,200,formatRpcFailure(suppressed));
assert.equal(suppressed.body.authorization?.effect,2,"DENY only");
assert.equal(suppressed.body.authorization?.status,1,"ACTIVE");
assert.equal(suppressed.body.authorization?.partyId,id);
assert.equal(suppressed.body.receipt?.resourceVersion,1n);
assert.equal(suppressed.body.receipt?.replayed,false);
assert.equal(suppressed.body.receipt?.requestId,suppressed.headers.get("x-request-id"));
const consentReplay=await fixtureRpc(dispatch, consent.suppressCommunication, admin, denyInput);
assert.equal(consentReplay.status,200,formatRpcFailure(consentReplay));
assert.equal(consentReplay.body.receipt?.replayed,true);
assert.equal((await sendFixtureRpc(dispatch, consent.suppressCommunication, admin, {
  ...denyInput,purpose:"marketing.different"}, 409)).status,409);
const consentDenied=await fixtureRpc(dispatch, consent.evaluateConsent, viewer, consentScope);
assert.equal(consentDenied.status,200,formatRpcFailure(consentDenied));
assert.equal(consentDenied.body.allowed,false);
assert.equal(consentDenied.body.reason,2,"ACTIVE_DENY");
const otherScope=await fixtureRpc(dispatch, consent.evaluateConsent, admin, {
  ...consentScope,purpose:"marketing.unrelated"});
assert.equal(otherScope.status,200);
assert.equal(otherScope.body.allowed,false);
assert.equal(otherScope.body.reason,4);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+denyInput.authorizationId+"'"))[0]?.n,1,
  "repeat suppression and consent evaluation must not duplicate audit");
const originalList=await fixtureRpc(dispatch, consent.listConsentAuthorizations, viewer, {page:{pageSize:1}});
assert.equal(originalList.status,200,formatRpcFailure(originalList));
assert.equal(originalList.body.authorizations.length,1);
assert.equal((await fixtureRpc(dispatch, consent.listConsentAuthorizations, tenantB, {page:{pageSize:1}}))
  .body.authorizations.length,0);
assert.equal((await sendFixtureRpc(dispatch, consent.listConsentAuthorizations, viewer, {page:{pageSize:101}}, 400))
  .status,400);
const otherId="consent-aaa-other-"+randomUUID();
const lastId="consent-zzz-own-"+randomUUID();
for(const [index,authorizationId,partyId] of [
  [0,otherId,pagingPersonId],[1,lastId,id]
] as const){
  const inserted=await fixtureRpc(dispatch, consent.suppressCommunication, admin, {
    authorizationId,partyId,purpose:"marketing.other",channel:1,jurisdiction:"eu",
    precondition:{idempotencyKey:"connect-consent-additional-"+index}});
  assert.equal(inserted.status,200,formatRpcFailure(inserted));
}
const readAuthorizations = (partyId?: string) => collectFixturePages(async pageToken => {
  const { body } = await fixtureRpc(dispatch, consent.listConsentAuthorizations, viewer, {
    ...(partyId === undefined ? {} : {partyId}), page: {pageSize: 1, pageToken},
  });
  return {items: body.authorizations, nextPageToken: body.pageInfo?.nextPageToken};
});
const filteredConsents=await readAuthorizations(id);
assert.deepEqual(filteredConsents.map(x=>x.authorizationId), [denyInput.authorizationId,lastId]);
const allConsents=await readAuthorizations();
assert.equal(allConsents.length,3);
assert.equal((await fixtureRpc(dispatch, consent.listConsentAuthorizations, viewer, {
  page:{pageSize:1,pageToken:lastId},partyId:id})).body.authorizations.length,0);
assert.equal((await fixtureRpc(dispatch, consent.listConsentAuthorizations, viewer, {
  page:{pageSize:1},partyId:"missing-party"})).body.authorizations.length,0);
console.log(JSON.stringify({event:"crm.native-g3-consents-v2",result:"PASS",
  rpc_methods:["EvaluateConsent","SuppressCommunication","ListConsentAuthorizations"],
  generated_binary:true,original_owner:"crm.consents",signed_access:true,
  tenant_ab:true,viewer_read:true,viewer_write_denied:true,operator_deny_only:true,
  no_grant_ingress:true,before_limit_filtered_pagination:true,
  d1_receipts:true,no_extra_audit:true,published_active:true}));

// R3a: signed human self-consent and distinct enrolled dev test-agent path.
 const r3Contact="r3-contact-"+randomUUID();
 const r3Create=await fixtureRpc(dispatch, contact.createContactPoint, admin, {
   contactPointId:r3Contact,partyId:id,kind:1,value:"native-a@example.test",
   preferred:false,precondition:{idempotencyKey:"r3-create-"+r3Contact}});
 assert.equal(r3Create.status,200,formatRpcFailure(r3Create));
 const r3Verify={contactPointId:r3Contact,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-verify-"+r3Contact}};
 assert.equal((await sendFixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, viewer, r3Verify, 403)).status,403);
 await sql("CREATE TRIGGER fixture_fail_verify_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action='contact_points.verify' BEGIN SELECT RAISE(ABORT,'verification audit unavailable'); END;");
 try{
   await assertUnconfirmed(await sendFixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, admin, r3Verify, 503),"CONTACT_POINTS_WRITE_UNCONFIRMED");
 }finally{await sql("DROP TRIGGER fixture_fail_verify_audit");}
 const verified=await fixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, admin, r3Verify);
 assert.equal(verified.status,200,formatRpcFailure(verified));
 assert.equal(verified.body.contactPoint?.verified,true);
 assert.equal(verified.body.contactPoint?.version,2n);
 assert.equal((await fixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, admin, r3Verify)).body.receipt?.replayed,true);
 assert.equal((await sendFixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, admin, {
   ...r3Verify,precondition:{expectedVersion:1n,
     idempotencyKey:"r3-stale-verify-"+r3Contact}}, 409)).status,409,
   "Stale verified-contact version must reject without any extra mutation");
 const r3Id="r3-consent-"+randomUUID();
 const r3Scope={partyId:id,contactPointId:r3Contact,purpose:"r3.test.newsletter",channel:1};
 const r3Grant={authorizationId:r3Id,partyId:id,contactPointId:r3Contact,
   purpose:r3Scope.purpose,jurisdiction:"eu",precondition:{idempotencyKey:"r3-grant-"+r3Id}};
 assert.equal((await sendFixtureRpc(dispatch, consent.grantVerifiedEmailConsent, knownService, r3Grant, 403)).status,403);
 assert.equal((await sendFixtureRpc(dispatch, consent.grantVerifiedEmailConsent, viewer, r3Grant, 403)).status,403);
 const granted=await fixtureRpc(dispatch, consent.grantVerifiedEmailConsent, admin, r3Grant);
 assert.equal(granted.status,200,formatRpcFailure(granted));
 assert.equal(granted.body.authorization?.effect,1);
 assert.equal((await fixtureRpc(dispatch, consent.grantVerifiedEmailConsent, admin, r3Grant)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpc(dispatch, consent.evaluateConsent, viewer, r3Scope)).body.allowed,true);
 const r3Withdraw={authorizationId:r3Id,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-withdraw-"+r3Id}};
 assert.equal((await sendFixtureRpc(dispatch, consent.withdrawConsentAuthorization, viewer, r3Withdraw, 403)).status,403);
 const revoked=await fixtureRpc(dispatch, consent.withdrawConsentAuthorization, knownService, r3Withdraw);
 assert.equal(revoked.status,200,formatRpcFailure(revoked));
 assert.equal(revoked.body.authorization?.status,2);
 assert.equal((await fixtureRpc(dispatch, consent.withdrawConsentAuthorization, knownService, r3Withdraw)).body.receipt?.replayed,true);
 assert.equal((await sendFixtureRpc(dispatch, consent.withdrawConsentAuthorization, admin, {
   ...r3Withdraw,precondition:{expectedVersion:1n,
     idempotencyKey:"r3-stale-withdraw-"+r3Id}}, 409)).status,409,
   "Stale withdrawal cannot create another audit or receipt");
 const deniedAfter=await fixtureRpc(dispatch, consent.evaluateConsent, viewer, r3Scope);
 assert.equal(deniedAfter.body.allowed,false);
 assert.equal(deniedAfter.body.reason,3);
 const secondId="r3-consent-renew-"+randomUUID();
 const second={...r3Grant,authorizationId:secondId,
   precondition:{idempotencyKey:"r3-grant2-"+secondId}};
 assert.equal((await fixtureRpc(dispatch, consent.grantVerifiedEmailConsent, admin, second)).status,200);
 assert.equal((await fixtureRpc(dispatch, consent.evaluateConsent, viewer, r3Scope)).body.allowed,true);

 // #471 S5b Consent: the existing read endpoint proves the original receipt,
 // even after the authorization has advanced to a later version.
 const consentReceiptInput={authorizationId:r3Id,
   receiptIdempotencyKey:r3Grant.precondition.idempotencyKey};
 assert.equal((await sendFixtureRpc(dispatch, consent.getConsentAuthorization, null, consentReceiptInput, 401)).status,401);
 const consentAuditBefore=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE subject_id=?",r3Id))[0]?.n;
 const recoveredGrant=await fixtureRpc(dispatch, consent.getConsentAuthorization, viewer, consentReceiptInput);
 assert.equal(recoveredGrant.status,200,formatRpcFailure(recoveredGrant));
 assert.equal(recoveredGrant.body.authorization?.status,2,
   "Get returns the current withdrawn authorization");
 assert.equal(recoveredGrant.body.confirmedMutation?.originalRequestId,
   granted.headers.get("x-request-id"));
 assert.equal(recoveredGrant.body.confirmedMutation?.resourceVersion,1n,
   "receipt evidence retains the original grant version, not the current withdrawn version");
 assert.equal(recoveredGrant.body.confirmedMutation?.requestId,
   recoveredGrant.headers.get("x-request-id"));
 assert.equal((await sendFixtureRpc(dispatch, consent.getConsentAuthorization, tenantB, consentReceiptInput, 404)).status,404);
 assert.equal((await sendFixtureRpc(dispatch, consent.getConsentAuthorization, viewer, {
   ...consentReceiptInput,receiptIdempotencyKey:"short"}, 400)).status,400);
 const missingConsentReceipt=await fixtureRpc(dispatch, consent.getConsentAuthorization, viewer, {
   authorizationId:r3Id,receiptIdempotencyKey:"consent-missing-receipt-01"});
 assert.equal(missingConsentReceipt.status,200);
 assert.equal(missingConsentReceipt.body.confirmedMutation,undefined,
   "a missing key is not evidence of a committed mutation");
 const wrongConsentResource=await fixtureRpc(dispatch, consent.getConsentAuthorization, viewer, {
   authorizationId:secondId,receiptIdempotencyKey:r3Grant.precondition.idempotencyKey});
 assert.equal(wrongConsentResource.status,200);
 assert.equal(wrongConsentResource.body.confirmedMutation,undefined,
   "a receipt for another authorization cannot confirm this resource");
 await sql("UPDATE crm_consent_receipts SET original_request_id=NULL WHERE tenant=? AND key=?",
   "tenant-a",r3Grant.precondition.idempotencyKey);
 try {
   const legacyConsent=await fixtureRpc(dispatch, consent.getConsentAuthorization, viewer, consentReceiptInput);
   assert.equal(legacyConsent.status,200);
   assert.equal(legacyConsent.body.confirmedMutation,undefined,
     "legacy receipts without original provenance are not certified");
 } finally {
   await sql("UPDATE crm_consent_receipts SET original_request_id=? WHERE tenant=? AND key=?",
     string(granted.headers.get("x-request-id")),"tenant-a",r3Grant.precondition.idempotencyKey);
 }
 await sql("ALTER TABLE crm_consent_receipts RENAME TO fixture_unavailable_consent_receipts");
 try {
   const unavailableConsent=await sendFixtureRpc(dispatch, consent.getConsentAuthorization, viewer, consentReceiptInput, 503);
   assert.equal(unavailableConsent.status,503,
     "an inaccessible receipt source must remain unconfirmed");
   assert.equal((await fixtureRpc(dispatch, consent.getConsentAuthorization, viewer, {
     authorizationId:r3Id})).status,200,
     "ordinary Get remains available without receipt recovery");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_consent_receipts RENAME TO crm_consent_receipts");
 }
 const restoredConsent=await fixtureRpc(dispatch, consent.getConsentAuthorization, viewer, consentReceiptInput);
 assert.equal(restoredConsent.status,200);
 assert.equal(restoredConsent.body.confirmedMutation?.originalRequestId,
   granted.headers.get("x-request-id"));
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE subject_id=?",r3Id))[0]?.n,
   consentAuditBefore,"read-only receipt recovery must never append an audit row");
 console.log(JSON.stringify({event:"crm.native-s5b-consent-commit-provenance",result:"PASS",
   original_request_id:true,original_version:true,tenant_isolation:true,
   wrong_resource_rejected:true,legacy_null_fail_closed:true,
   receipt_read_failure_unconfirmed:true,read_only:true}));

 const failingGrantId="r3-audit-fault-"+randomUUID();
 const failingKey="r3-audit-fault-key-"+failingGrantId;
 const beforeGrantFault=(await sql("SELECT "+
   "(SELECT COUNT(*) FROM crm_consent_records) AS records,"+
   "(SELECT COUNT(*) FROM crm_consent_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 (await sql("CREATE TRIGGER fixture_fail_r3_grant_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+failingGrantId+"' AND NEW.action='consents.grant' "+
   "BEGIN SELECT RAISE(ABORT,'fixture R3 consent grant audit failure'); END;"));
 try{
   await assertUnconfirmed(await sendFixtureRpc(dispatch, consent.grantVerifiedEmailConsent, admin, {
     ...r3Grant,authorizationId:failingGrantId,
     precondition:{idempotencyKey:failingKey}}, 503),"CONSENTS_WRITE_UNCONFIRMED");
   assert.deepEqual((await sql("SELECT "+
     "(SELECT COUNT(*) FROM crm_consent_records) AS records,"+
     "(SELECT COUNT(*) FROM crm_consent_receipts) AS receipts,"+
     "(SELECT COUNT(*) FROM crm_audit) AS audits")),beforeGrantFault,
     "Consent state, receipt and audit must roll back together on late audit failure");
 } finally {await sql("DROP TRIGGER fixture_fail_r3_grant_audit");}
 const changed=await fixtureRpc(dispatch, contact.updateContactPoint, admin, {
   contactPointId:r3Contact,value:"other@example.org",status:1,preferred:false,
   precondition:{idempotencyKey:"r3-change-"+r3Contact,expectedVersion:2n}});
 assert.equal(changed.status,200,formatRpcFailure(changed));
 assert.equal(changed.body.contactPoint?.verified,false);
 assert.equal((await fixtureRpc(dispatch, consent.evaluateConsent, viewer, r3Scope)).body.allowed,false,
   "Old grant must be ineffective after email change");
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+r3Id+"' ORDER BY rowid")),[
   {action:"consents.grant",actor:"native-a@example.test"},
   {action:"consents.withdraw",actor:"service:ci-fixture.access"}
 ]);
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+r3Contact+"' ORDER BY rowid")),[
   {action:"contact_points.create",actor:"native-a@example.test"},
   {action:"contact_points.verify",actor:"native-a@example.test"},
   {action:"contact_points.update",actor:"native-a@example.test"}
 ]);
 console.log(JSON.stringify({event:"crm.native-r3-signed-email-consent",result:"PASS",
   human_email:true,service_denied:true,grant_withdraw:true,replay:true,
   changed_contact_invalidates_grant:true,actor_audit:true,
   atomic_grant_rollback:true,stale_versions_denied:true}));

 // The same real signed admin service can accept a QA cycle through the UI.
 // It must not use human Access evidence, nor grant on human-verified records.
 const agentContact="r3-agent-contact-"+randomUUID();
 const agentCreated=await fixtureRpc(dispatch, contact.createContactPoint, knownService, {
   contactPointId:agentContact,partyId:id,kind:1,value:"qa-agent@example.test",
   preferred:false,precondition:{idempotencyKey:"r3-agent-create-"+agentContact}});
 assert.equal(agentCreated.status,200,formatRpcFailure(agentCreated));
 const agentVerify={contactPointId:agentContact,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-agent-verify-"+agentContact}};
 assert.equal((await sendFixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, viewer, agentVerify, 403)).status,403);
 assert.equal((await sendFixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, admin, agentVerify, 403)).status,403);
 const agentVerified=await fixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, knownService, agentVerify);
 assert.equal(agentVerified.status,200,formatRpcFailure(agentVerified));
 assert.equal(agentVerified.body.contactPoint?.verified,true);
 assert.equal((await fixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, knownService, agentVerify)).body.receipt?.replayed,true);
 assert.equal((await sendFixtureRpc(dispatch, contact.verifyOwnEmailContactPoint, knownService, {
   ...agentVerify,precondition:{expectedVersion:1n,
     idempotencyKey:"r3-agent-stale-"+agentContact}}, 409)).status,409);
 const agentId="r3-agent-consent-"+randomUUID();
 const agentScope={partyId:id,contactPointId:agentContact,
   purpose:"r3.dev.agent",channel:1};
 const agentGrant={authorizationId:agentId,partyId:id,
   contactPointId:agentContact,purpose:agentScope.purpose,
   jurisdiction:"eu",precondition:{idempotencyKey:"r3-agent-grant-"+agentId}};
 assert.equal((await sendFixtureRpc(dispatch, consent.grantVerifiedEmailConsent, viewer, agentGrant, 403)).status,403);
 assert.equal((await sendFixtureRpc(dispatch, consent.grantVerifiedEmailConsent, admin, agentGrant, 403)).status,403);
 const agentGranted=await fixtureRpc(dispatch, consent.grantVerifiedEmailConsent, knownService, agentGrant);
 assert.equal(agentGranted.status,200,formatRpcFailure(agentGranted));
 assert.equal(agentGranted.body.authorization?.effect,1);
 assert.equal((await fixtureRpc(dispatch, consent.grantVerifiedEmailConsent, knownService, agentGrant)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpc(dispatch, consent.evaluateConsent, viewer, agentScope)).body.allowed,true);
 const agentState=string((await sql("SELECT state FROM crm_consent_records WHERE id=?",agentId))[0]?.state);
 assert.ok(agentState.includes("crm.dev.agent"),"Service grant requires distinct test-agent provenance");
 const agentWithdraw={authorizationId:agentId,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-agent-withdraw-"+agentId}};
 assert.equal((await fixtureRpc(dispatch, consent.withdrawConsentAuthorization, knownService, agentWithdraw)).status,200);
 assert.equal((await fixtureRpc(dispatch, consent.evaluateConsent, viewer, agentScope)).body.allowed,false);
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+agentId+"' ORDER BY rowid")),[
   {action:"consents.grant",actor:"service:ci-fixture.access"},
   {action:"consents.withdraw",actor:"service:ci-fixture.access"}
 ]);
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+agentContact+"' ORDER BY rowid")),[
   {action:"contact_points.create",actor:"service:ci-fixture.access"},
   {action:"contact_points.verify",actor:"service:ci-fixture.access"}
 ]);
 console.log(JSON.stringify({event:"crm.native-r3-dev-agent-consent",result:"PASS",
   signed_service_admin:true,test_provenance:true,non_impersonation:true,
   grant_withdraw:true,replay:true,cas:true,actor_audit:true}));

 return {r3Contact};
}

import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { ContactPointService } from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import type { ListContactPointsRequest } from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type ContactsFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 id: string;
 pagingPersonId: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
 unknownService: string;
};
export async function verifyContacts({dispatch,sql,id,pagingPersonId,admin,viewer,tenantB,knownService,unknownService}: ContactsFixture) {
 const contact=ContactPointService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>) => {
   assert.equal(failed.status,503);
   assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,"CONTACT_POINTS_WRITE_UNCONFIRMED");
   assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 const collectContacts=(input: Partial<Pick<ListContactPointsRequest, "partyId" | "kind" | "status">>)=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,contact.listContactPoints,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:result.body.contactPoints,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const contactId="contact-native-"+randomUUID();
 const contactInput={contactPointId:contactId,partyId:id,kind:1,
   value:"  contact@example.com  ",preferred:true,
   validFrom:{seconds:1700000000n,nanos:123000000},
   validUntil:{seconds:1800000000n,nanos:456000000},
   precondition:{idempotencyKey:"candidate-v2-contact-create-001"}};
 assert.equal((await fixtureRpcError(dispatch,contact.createContactPoint,null,contactInput)).status,401);
 assert.equal((await fixtureRpcError(dispatch,contact.createContactPoint,viewer,contactInput)).status,403);
 assert.equal((await fixtureRpcError(dispatch,contact.createContactPoint,admin,{
   ...contactInput,kind:0,
   precondition:{idempotencyKey:"candidate-v2-contact-invalid-kind"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,contact.createContactPoint,admin,{
   ...contactInput,partyId:"missing-party",
   precondition:{idempotencyKey:"candidate-v2-contact-invalid-party"}})).status,400);
 for(const [index,value] of [
   "Ada..Tag@example.test","Ada<>@example.test","Ada@exa_mple.test",
   "Ada@example.test.","Ada@[127.0.0.1]","Ада@example.test"
 ].entries()){
   const denied=await fixtureRpcError(dispatch,contact.createContactPoint,admin,{
     ...contactInput,value,precondition:{idempotencyKey:"contact-email-invalid-"+index}});
   assert.equal(denied.status,400,formatRpcFailure(denied));
 }
 for(const [index,value] of [
   "2025550102","+999123456789","+11111111111",
   "+1 202 555 0102 ext 42","+1 202 555 0102 junk"
 ].entries()){
   const denied=await fixtureRpcError(dispatch,contact.createContactPoint,admin,{
     ...contactInput,kind:2,value,precondition:{idempotencyKey:"contact-phone-invalid-"+index}});
   assert.equal(denied.status,400,formatRpcFailure(denied));
 }
 const contactCreated=await fixtureRpc(dispatch,contact.createContactPoint,admin,contactInput,{
    "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
    "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(contactCreated.status,200,formatRpcFailure(contactCreated));
 assert.equal(contactCreated.body.contactPoint?.contactPointId,contactId);
 assert.equal(contactCreated.body.contactPoint?.partyId,id);
 assert.equal(contactCreated.body.contactPoint?.verified,false);
 assert.equal(contactCreated.body.contactPoint?.normalizedValue,"contact@example.com");
 assert.equal(contactCreated.body.contactPoint?.validFrom?.seconds,1700000000n);
 assert.equal(contactCreated.body.contactPoint?.validFrom?.nanos,123000000);
 assert.equal(contactCreated.body.contactPoint?.validUntil?.seconds,1800000000n);
 assert.equal(contactCreated.body.contactPoint?.validUntil?.nanos,456000000);
 assert.equal(contactCreated.body.receipt?.resourceVersion,1n);
 assert.equal(contactCreated.body.receipt?.replayed,false);
 assert.equal(contactCreated.body.receipt?.requestId,contactCreated.headers.get("x-request-id"));
 const contactReplay=await fixtureRpc(dispatch,contact.createContactPoint,knownService,contactInput);
 assert.equal(contactReplay.status,200,formatRpcFailure(contactReplay));
 assert.equal(contactReplay.body.receipt?.replayed,true);
 assert.equal(contactReplay.body.receipt?.originalRequestId,contactCreated.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch,contact.createContactPoint,admin,{...contactInput,
   value:"different@example.com"})).status,409);
 assert.equal((await fixtureRpcError(dispatch,contact.getContactPoint,tenantB,{contactPointId:contactId})).status,404);
 assert.equal((await fixtureRpc(dispatch,contact.getContactPoint,viewer,{contactPointId:contactId}))
   .body.contactPoint?.verified,false);
 const contactList=await fixtureRpc(dispatch,contact.listContactPoints,viewer,{page:{pageSize:1},partyId:id});
 assert.equal(contactList.status,200,formatRpcFailure(contactList));
 assert.equal(contactList.body.contactPoints?.[0]?.contactPointId,contactId);
 assert.equal((await fixtureRpcError(dispatch,contact.listContactPoints,viewer,{page:{pageSize:101}})).status,400);
 assert.equal((await fixtureRpc(dispatch,contact.listContactPoints,viewer,{partyId:"missing"}))
   .body.contactPoints.length,0);
 const contactUpdate={contactPointId:contactId,value:"next@example.org",status:1,
   preferred:false,
   validFrom:{operation:{case:"set" as const,value:{seconds:1760000000n,nanos:987654321}}},
   validUntil:{operation:{case:"clear" as const,value:true}},
   precondition:{
     expectedVersion:1n,idempotencyKey:"candidate-v2-contact-update-001"}};
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,viewer,contactUpdate)).status,403);
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,admin,{...contactUpdate,status:0,
   precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-v2-contact-invalid-status"}})).status,400);
 const contactUpdated=await fixtureRpc(dispatch,contact.updateContactPoint,knownService,contactUpdate);
 assert.equal(contactUpdated.status,200,formatRpcFailure(contactUpdated));
 assert.equal(contactUpdated.body.contactPoint?.version,2n);
 assert.equal(contactUpdated.body.contactPoint?.verified,false);
 assert.equal(contactUpdated.body.contactPoint?.normalizedValue,"next@example.org");
 assert.equal(contactUpdated.body.contactPoint?.validFrom?.seconds,1760000000n);
 assert.equal(contactUpdated.body.contactPoint?.validFrom?.nanos,987654321);
 assert.equal(contactUpdated.body.contactPoint?.validUntil,undefined,
   "explicit clear timestamp patch must remove validUntil");
 assert.equal(contactUpdated.body.receipt?.resourceVersion,2n);
 const contactReupdate=await fixtureRpc(dispatch,contact.updateContactPoint,admin,contactUpdate);
 assert.equal(contactReupdate.status,200,formatRpcFailure(contactReupdate));
 assert.equal(contactReupdate.body.receipt?.replayed,true);
 assert.equal(contactReupdate.body.receipt?.originalRequestId,contactUpdated.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,admin,{...contactUpdate,
   precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-v2-contact-stale"}})).status,409);
 assert.equal((await sql("SELECT version FROM crm_contact_point_records WHERE tenant='tenant-a' "+
   "AND id='"+contactId+"'"))[0]?.version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_contact_point_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v2-contact-create-001','candidate-v2-contact-update-001')"))[0]?.n,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"' AND action LIKE 'contact_points.%'"))[0]?.n,2);
 assert.deepEqual((await sql("SELECT action,tenant_id,subject_id,request_id,actor FROM crm_audit WHERE subject_id='"+contactId+"' ORDER BY rowid")),[
   {action:"contact_points.create",tenant_id:"tenant-a",subject_id:contactId,
     request_id:contactCreated.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"contact_points.update",tenant_id:"tenant-a",subject_id:contactId,
     request_id:contactUpdated.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Cross-actor contact replay must preserve signed author/request ID");
 const contactSnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_contact_point_records) AS states,"+
   "(SELECT COUNT(*) FROM crm_contact_point_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 const beforeContactDenials=(await contactSnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await fixtureRpcError(dispatch,contact.createContactPoint,token,contactInput)).status>=400);
   assert.ok((await fixtureRpcError(dispatch,contact.updateContactPoint,token,contactUpdate)).status>=400);
 }
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,tenantB,contactUpdate)).status,404);
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,contact.createContactPoint,knownService,contactInput)).status,401,
   "Inactive service cannot replay ContactPoint create");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,knownService,contactUpdate)).status,403,
   "Current permission is required before ContactPoint replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.deepEqual((await contactSnapshot()),beforeContactDenials,
   "Denied, stale and validation errors cannot change contact state, receipts or audit");
 const contactRetryInput={contactPointId:"contact-audit-rollback",partyId:id,kind:1,value:"rollback@example.org",preferred:false,
   precondition:{idempotencyKey:"contact-audit-failed-create"}};
 (await sql("CREATE TRIGGER fixture_fail_contact_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('contact-audit-rollback','"+contactId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture contact audit failure'); END;"));
 try{
   assertUnconfirmed(await fixtureRpcError(dispatch,contact.createContactPoint,admin,contactRetryInput));
   assertUnconfirmed(await fixtureRpcError(dispatch,contact.updateContactPoint,admin,{
     ...contactUpdate,value:"rollback@example.org",
     precondition:{expectedVersion:2n,idempotencyKey:"contact-audit-failed-update"}}));
   assert.deepEqual((await contactSnapshot()),beforeContactDenials,
     "Contact audit SQL failure must roll back both state and receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_contact_audit");}
 console.log(JSON.stringify({event:"crm.native-contact-actor-audit",result:"PASS",
   human_create:true,service_update:true,cross_actor_replay:true,forged_headers_ignored:true,
   authorization_before_replay:true,tenant_ab:true,sql_failure_rollback:true}));
 // Filtered ContactPoint list uses the EXISTING D1 party_id column.
 // Nonmatches sort before our two matches, exposing the old LIMIT bug.
 const contactLastId="contact-zzz-ours-"+randomUUID();
 for(const [index,pointToCreate] of ([
   ["contact-aaa-other-"+randomUUID(),pagingPersonId],
   ["contact-aab-other-"+randomUUID(),pagingPersonId],
   [contactLastId,id]
 ] as const).entries()){
   const made=await fixtureRpc(dispatch,contact.createContactPoint,admin,{
     contactPointId:pointToCreate[0],partyId:pointToCreate[1],kind:index===0?2:1,
     value:index===0?"+1 202 555 0102":"page-"+index+"@example.org",preferred:false,
     precondition:{idempotencyKey:"page-contact-create-"+index}});
   assert.equal(made.status,200,formatRpcFailure(made));
   if(index===0){
     assert.equal(made.body.contactPoint?.normalizedValue,"+12025550102");
     assert.equal(made.body.contactPoint?.verified,false);
     const read=await fixtureRpc(dispatch,contact.getContactPoint,viewer,{contactPointId:pointToCreate[0]});
     assert.equal(read.status,200,formatRpcFailure(read));
     assert.equal(read.body.contactPoint?.normalizedValue,"+12025550102");
   }
 }
 const filteredContacts=await collectContacts({partyId:id});
 assert.deepEqual(filteredContacts.map(p=>p.contactPointId),[contactId,contactLastId]);
 assert.equal((await collectContacts({partyId:"missing-party"})).length,0);
 assert.equal((await fixtureRpc(dispatch,contact.listContactPoints,tenantB,{
   page:{pageSize:1},partyId:id})).body.contactPoints.length,0);
 const beyondContact=await fixtureRpc(dispatch,contact.listContactPoints,viewer,{
   partyId:id,page:{pageSize:1,pageToken:contactLastId}});
 assert.equal(beyondContact.status,200);
 assert.equal(beyondContact.body.contactPoints.length,0);
 assert.ok(!beyondContact.body.pageInfo?.nextPageToken);

 // Omitted timestamp patches mean KEEP. Replay the SAME request after a
 // later successful update which changes both current timestamp fields.
 const keepRequest={contactPointId:contactId,value:"keep-timestamps@example.org",
   status:1,preferred:false,
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-v2-contact-keep-001"}};
 const keep=await fixtureRpc(dispatch,contact.updateContactPoint,admin,keepRequest);
 assert.equal(keep.status,200,formatRpcFailure(keep));
 assert.equal(keep.body.contactPoint?.version,3n);
 assert.equal(keep.body.contactPoint?.validFrom?.seconds,1760000000n);
 assert.equal(keep.body.contactPoint?.validUntil,undefined);
 const later=await fixtureRpc(dispatch,contact.updateContactPoint,admin,{
   ...keepRequest,value:"later-timestamps@example.org",
   validFrom:{operation:{case:"set",value:{seconds:1770000000n,nanos:3}}},
   validUntil:{operation:{case:"set",value:{seconds:1870000000n,nanos:9}}},
   precondition:{expectedVersion:3n,idempotencyKey:"candidate-v2-contact-later-001"}});
 assert.equal(later.status,200,formatRpcFailure(later));
 assert.equal(later.body.contactPoint?.version,4n);
 const beforeReplay=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"'"))[0]?.n;
 const restoredReceipt=await fixtureRpc(dispatch,contact.updateContactPoint,admin,keepRequest);
 assert.equal(restoredReceipt.status,200,formatRpcFailure(restoredReceipt));
 assert.equal(restoredReceipt.body.receipt?.replayed,true);
 assert.equal(restoredReceipt.body.receipt?.resourceVersion,3n);
 assert.equal(restoredReceipt.body.contactPoint?.version,3n);
 assert.equal(restoredReceipt.body.contactPoint?.validFrom?.seconds,1760000000n);
 assert.equal(restoredReceipt.body.contactPoint?.validUntil,undefined);
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,admin,{
   ...keepRequest,value:"not-the-same-request@example.org"})).status,409);
 assert.equal((await fixtureRpcError(dispatch,contact.updateContactPoint,admin,{
   ...keepRequest,precondition:{expectedVersion:2n,
     idempotencyKey:"candidate-v2-contact-stale-after-later"}})).status,409);
 const currentPoint=await fixtureRpc(dispatch,contact.getContactPoint,viewer,{contactPointId:contactId});
 assert.equal(currentPoint.body.contactPoint?.version,4n);
 assert.equal(currentPoint.body.contactPoint?.validFrom?.seconds,1770000000n);
 assert.equal(currentPoint.body.contactPoint?.validUntil?.seconds,1870000000n);
 assert.equal((await sql("SELECT version FROM crm_contact_point_records WHERE tenant='tenant-a' "+
   "AND id='"+contactId+"'"))[0]?.version,4);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"'"))[0]?.n,beforeReplay);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_contact_point_receipts "+
   "WHERE tenant='tenant-a' AND key IN ("+
   "'candidate-v2-contact-keep-001','candidate-v2-contact-later-001')"))[0]?.n,2);
 console.log(JSON.stringify({event:"crm.native-g3-contact-pagination-interleaved-replay",
   result:"PASS",same_original_receipt:true,no_extra_write:true,tenant_isolated:true}));

 // Product-first M1-04: narrow by a specific channel AND lifecycle status,
 // not post-LIMIT filtering that loses items on later pages.
 const contactLastInactive=await fixtureRpc(dispatch,contact.updateContactPoint,admin,{
   contactPointId:contactLastId,value:"page-2@example.org",preferred:false,status:2,
   precondition:{expectedVersion:1n,idempotencyKey:"contact-filter-inactive-last"}});
 assert.equal(contactLastInactive.status,200,formatRpcFailure(contactLastInactive));
 const contactFiltersAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 assert.deepEqual((await collectContacts({
   partyId:id,kind:1,status:1
 })).map(p=>p.contactPointId),[contactId]);
 assert.deepEqual((await collectContacts({
   partyId:id,kind:1,status:2
 })).map(p=>p.contactPointId),[contactLastId]);
 assert.deepEqual((await collectContacts({
   partyId:id,kind:1
 })).map(p=>p.contactPointId),[contactId,contactLastId]);
 assert.equal((await collectContacts({
   partyId:id,kind:2,status:1
 })).length,0);
 const otherPhone=await collectContacts({
   partyId:pagingPersonId,kind:2,status:1
 });
 assert.equal(otherPhone.length,1);
 assert.equal(otherPhone[0]?.normalizedValue,"+12025550102");
 for(const filter of [{kind:0},{kind:99},{status:0},{status:99}]){
   const denied=await fixtureRpcError(dispatch,contact.listContactPoints,viewer,filter);
   assert.equal(denied.status,400,formatRpcFailure(denied));
 }
 assert.equal((await fixtureRpc(dispatch,contact.listContactPoints,tenantB,{partyId:id,kind:1,status:1}))
   .body.contactPoints.length,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   contactFiltersAudit,"Filtering ContactPoints must not write audit");
 console.log(JSON.stringify({event:"crm.native-m1-contact-kind-status",result:"PASS",
   exact_kind:true,exact_status:true,composed_before_cursor:true,tenant:true,
   invalid_enum_rejected:true,read_only:true}));
 console.log(JSON.stringify({event:"crm.native-g3-contact-points-v2",result:"PASS",
   generated_binary:true,owner:"crm.contact-points",
   rpc_methods:["CreateContactPoint","UpdateContactPoint","GetContactPoint","ListContactPoints"],
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   no_browser_verification:true,d1_cas:true,d1_receipts:true,d1_audit:true,
   published_active:true}));

 return {contactId,contactInput,contactCreated,contactLastId,contactRetryInput};
}

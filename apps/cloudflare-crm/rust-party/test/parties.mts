import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { PartyService } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import type { ListPartiesRequest } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type PartiesFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 partySnapshot: () => Promise<ReadonlyArray<Record<string, unknown>>>;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
 unknownService: string;
};
export async function verifyParties({dispatch,sql,partySnapshot,admin,viewer,tenantB,knownService,unknownService}: PartiesFixture) {
 const party=PartyService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>) => {
   assert.equal(failed.status,503);
   assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,"PARTIES_WRITE_UNCONFIRMED");
   assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 const collectParties=(input: Partial<Pick<ListPartiesRequest, "kind">>)=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,party.listParties,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:result.body.parties,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const id="native-v3-"+randomUUID();
 const creation={partyId:id,kind:2,displayName:"  Connect   Native  Rust ",
   precondition:{idempotencyKey:"candidate-v3-create-001"}};
 assert.equal((await fixtureRpcError(dispatch,party.createParty,null,creation)).status,401);
 assert.equal((await fixtureRpcError(dispatch,party.createParty,viewer,creation)).status,403);
 assert.equal((await fixtureRpcError(dispatch,party.createParty,admin,{...creation,kind:0,
   precondition:{idempotencyKey:"candidate-v3-invalid-kind"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,party.createParty,admin,{...creation,
   precondition:{idempotencyKey:"candidate-v3-invalid-pre",
     expectedVersion:1n}})).status,400);
 const created=await fixtureRpc(dispatch,party.createParty,admin,creation,{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test","x-request-id":"forged-request"});
 assert.equal(created.status,200,formatRpcFailure(created));
 assert.equal(created.body.party?.partyId,id);
 assert.equal(created.body.party?.displayName,"Connect Native Rust");
 assert.equal(created.body.receipt?.resourceVersion,1n);
 assert.equal(created.body.receipt?.replayed,false);
 assert.equal(created.body.receipt?.requestId,created.headers.get("x-request-id"));
 const replay=await fixtureRpc(dispatch,party.createParty,knownService,creation);
 assert.equal(replay.status,200);
 assert.equal(replay.body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,party.createParty,admin,{...creation,
   displayName:"different replay"})).status,409);
 assert.equal((await fixtureRpcError(dispatch,party.getParty,tenantB,{partyId:id})).status,404);
 assert.equal((await fixtureRpc(dispatch,party.getParty,knownService,{partyId:id})).status,200,
   "Enrolled service and human admin must read the same tenant");
 const read=await fixtureRpc(dispatch,party.getParty,viewer,{partyId:id});
 assert.equal(read.status,200);
 assert.equal(read.body.party?.displayName,"Connect Native Rust");
 const listing=await fixtureRpc(dispatch,party.listParties,viewer,{page:{pageSize:1}});
 assert.equal(listing.status,200,formatRpcFailure(listing));
 assert.equal(listing.body.parties.length,1);
 assert.ok(listing.body.pageInfo?.nextPageToken);
 assert.equal((await fixtureRpcError(dispatch,party.listParties,viewer,{page:{pageSize:101}})).status,400);
 const updatedInput={partyId:id,displayName:"Updated via native v3",
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-v3-update-001"}};
 assert.equal((await fixtureRpcError(dispatch,party.updateParty,viewer,updatedInput)).status,403);
 const updated=await fixtureRpc(dispatch,party.updateParty,knownService,updatedInput);
 assert.equal(updated.status,200,formatRpcFailure(updated));
 assert.equal(updated.body.party?.version,2n);
 assert.equal(updated.body.party?.displayName,"Updated via native v3");
 assert.equal(updated.body.receipt?.resourceVersion,2n);
 assert.equal(updated.body.receipt?.replayed,false);
 const again=await fixtureRpc(dispatch,party.updateParty,admin,updatedInput);
 assert.equal(again.status,200);
 assert.equal(again.body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,party.updateParty,admin,{...updatedInput,
   precondition:{expectedVersion:1n,
      idempotencyKey:"candidate-v3-stale-version"}})).status,409);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0]?.n,2,
   "Unknown signed service must not create another tenant");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_users"))[0]?.n,4,
   "Unknown signed service must not insert an admin actor");
 assert.equal((await sql("SELECT tenant_id FROM crm_users WHERE email='service:ci-fixture.access'"))[0]?.tenant_id,
   "tenant-a","Explicit service enrollment must retain the owner's tenant");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE id='binary-rpc-proof-01'"))[0]?.n,1,
   "Repeated migrations apply must preserve existing business data");
 assert.equal((await sql("SELECT version FROM crm_party_records WHERE id='"+id+"'"))[0]?.version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v3-create-001','candidate-v3-update-001')"))[0]?.n,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+id+"'"))[0]?.n,2);
 assert.equal((await fixtureRpc(dispatch,party.getParty,admin,{partyId:id})).body.party?.version,2n);
 assert.deepEqual((await sql("SELECT action,tenant_id,subject_id,request_id,actor FROM crm_audit WHERE subject_id='"+id+"' ORDER BY rowid")),[
   {action:"parties.create",tenant_id:"tenant-a",subject_id:id,
     request_id:created.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"parties.update",tenant_id:"tenant-a",subject_id:id,
     request_id:updated.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Cross-actor replay must retain the original audit identity and request");
 const partyBeforeDenials=(await partySnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await fixtureRpcError(dispatch,party.createParty,token,creation)).status>=400);
   assert.ok((await fixtureRpcError(dispatch,party.updateParty,token,updatedInput)).status>=400);
 }
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,party.createParty,knownService,creation)).status,401,
   "Inactive actor cannot retrieve a previously committed mutation by replay");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,party.updateParty,knownService,updatedInput)).status,403,
   "Current permission is required before replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,party.updateParty,tenantB,updatedInput)).status,404);
 assert.equal((await fixtureRpcError(dispatch,party.updateParty,admin,{...updatedInput,displayName:"   ",
   precondition:{expectedVersion:2n,idempotencyKey:"party-invalid-name"}})).status,400);
 assert.deepEqual((await partySnapshot()),partyBeforeDenials,"Denied/invalid operations must not change Party state, receipts or audit");

 // This fixture rejects the audit INSERT after the owner has written state and receipt.
 // No test hook or failure path is compiled into the production Worker.
 (await sql("CREATE TRIGGER fixture_fail_party_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('party-audit-failure','"+id+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;"));
 try{
   assertUnconfirmed(await fixtureRpcError(dispatch,party.createParty,admin,{partyId:"party-audit-failure",kind:1,displayName:"Rollback",
     precondition:{idempotencyKey:"party-audit-failed-create"}}));
   assertUnconfirmed(await fixtureRpcError(dispatch,party.updateParty,admin,{...updatedInput,displayName:"Must roll back",
     precondition:{expectedVersion:2n,idempotencyKey:"party-audit-failed-update"}}));
   assert.deepEqual((await partySnapshot()),partyBeforeDenials,"SQL audit failure must roll back the state and receipt too");
 }finally{await sql("DROP TRIGGER fixture_fail_party_audit");}
 console.log(JSON.stringify({event:"crm.native-party-actor-audit",result:"PASS",
   human_create:true,service_update:true,cross_actor_replay:true,authorization_before_replay:true,
   forged_headers_ignored:true,sql_failure_rollback:true,tenant_ab:true}));

 // Interleave nonmatching person rows between matching organizations.
 // The old Rust post-LIMIT filter produced an empty middle page here.
 const pagingPersonId="native-page-person-a-"+randomUUID();
 for(const [index,personId] of [pagingPersonId,"native-page-person-b-"+randomUUID()].entries()){
   const made=await fixtureRpc(dispatch,party.createParty,admin,{
     partyId:personId,kind:1,displayName:"Paging Person "+index,
     precondition:{idempotencyKey:"page-person-create-"+index}});
   assert.equal(made.status,200,formatRpcFailure(made));
 }
 const organizations=await collectParties({kind:2});
 assert.deepEqual(organizations.map(p=>p.partyId),["binary-rpc-proof-01",id]);
 const personPages=await collectParties({kind:1});
 assert.equal(personPages.length,2);
 assert.deepEqual(personPages.map(p=>p.partyId),
   [...personPages.map(p=>p.partyId)].sort());
 assert.ok(personPages.some(p=>p.partyId===pagingPersonId));
 const beyondParty=await fixtureRpc(dispatch,party.listParties,viewer,{kind:2,page:{pageSize:1,pageToken:id}});
 assert.equal(beyondParty.status,200);
 assert.equal(beyondParty.body.parties.length,0);
 assert.ok(!beyondParty.body.pageInfo?.nextPageToken);
 assert.equal((await fixtureRpc(dispatch,party.listParties,tenantB,{kind:2,page:{pageSize:1}}))
   .body.parties.length,0);
 console.log(JSON.stringify({event:"crm.native-g3-party-filtered-pagination",result:"PASS"}));
 return {id,creation,created,pagingPersonId};
}

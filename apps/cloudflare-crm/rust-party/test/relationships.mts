import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { PartyRelationshipService } from "../../../../packages/client/gen/crm/party_relationships/v2/relationship_pb.js";
import type { ListPartyRelationshipsRequest } from "../../../../packages/client/gen/crm/party_relationships/v2/relationship_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type RelationshipsFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 id: string;
 pagingPersonId: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
};
export async function verifyRelationships({dispatch,sql,id,pagingPersonId,admin,viewer,tenantB,knownService}: RelationshipsFixture) {
 const rel=PartyRelationshipService.method;
 const collectRelationships=(input: Partial<Pick<ListPartyRelationshipsRequest, "partyId" | "status">>)=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,rel.listPartyRelationships,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:result.body.relationships,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const rOther="relationship-aaa-other-"+randomUUID();
 const rMid="relationship-mid-ours-"+randomUUID();
 const rLast="relationship-zzz-ours-"+randomUUID();
 const relType={code:"employment",directionality:1,fromRole:"employer",toRole:"employee"};
 const relationInput=(relationshipId: string,fromPartyId: string,toPartyId: string,seq: string)=>({
   relationshipId,fromPartyId,toPartyId,type:relType,
   validFrom:{seconds:1700000000n,nanos:1000},
   validUntil:{seconds:1800000000n,nanos:2000},
   precondition:{idempotencyKey:"candidate-rel-create-"+seq}
 });
 const otherRelation=relationInput(rOther,"binary-rpc-proof-01",pagingPersonId,"other");
 const midRelation=relationInput(rMid,id,pagingPersonId,"mid");
 const lastRelation=relationInput(rLast,pagingPersonId,id,"last");
 assert.equal((await fixtureRpcError(dispatch,rel.createPartyRelationship,null,midRelation)).status,401);
 assert.equal((await fixtureRpcError(dispatch,rel.createPartyRelationship,viewer,midRelation)).status,403);
 assert.equal((await fixtureRpcError(dispatch,rel.createPartyRelationship,admin,{
   ...midRelation,fromPartyId:id,toPartyId:id,
   precondition:{idempotencyKey:"candidate-rel-self"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,rel.createPartyRelationship,admin,{
   ...midRelation,fromPartyId:"missing-party",
   precondition:{idempotencyKey:"candidate-rel-missing"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,rel.createPartyRelationship,admin,{
   ...midRelation,type:{...relType,directionality:0},
   precondition:{idempotencyKey:"candidate-rel-invalid-type"}})).status,400);
 const otherCreated=await fixtureRpc(dispatch,rel.createPartyRelationship,admin,otherRelation);
 assert.equal(otherCreated.status,200,formatRpcFailure(otherCreated));
 const midCreated=await fixtureRpc(dispatch,rel.createPartyRelationship,admin,midRelation);
 assert.equal(midCreated.status,200,formatRpcFailure(midCreated));
 assert.equal(midCreated.body.relationship?.relationshipId,rMid);
 assert.equal(midCreated.body.relationship?.type?.code,"employment");
 assert.equal(midCreated.body.relationship?.validFrom?.nanos,1000);
 assert.equal(midCreated.body.receipt?.resourceVersion,1n);
 assert.equal(midCreated.body.receipt?.requestId,midCreated.headers.get("x-request-id"));
 const lastCreated=await fixtureRpc(dispatch,rel.createPartyRelationship,admin,lastRelation);
 assert.equal(lastCreated.status,200,formatRpcFailure(lastCreated));
 const midReplay=await fixtureRpc(dispatch,rel.createPartyRelationship,admin,midRelation);
 assert.equal(midReplay.status,200,formatRpcFailure(midReplay));
 assert.equal(midReplay.body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,rel.createPartyRelationship,admin,{
   ...midRelation,toPartyId:"binary-rpc-proof-01"})).status,409);
 assert.equal((await fixtureRpcError(dispatch,rel.getPartyRelationship,tenantB,{relationshipId:rMid})).status,404);
 assert.equal((await fixtureRpc(dispatch,rel.getPartyRelationship,viewer,{relationshipId:rMid}))
   .body.relationship?.fromPartyId,id);
 const relList=await collectRelationships({partyId:id});
 assert.deepEqual(relList.map(r=>r.relationshipId),[rMid,rLast]);
 assert.equal((await collectRelationships(
   {partyId:"missing-party"})).length,0);
 assert.equal((await fixtureRpc(dispatch,rel.listPartyRelationships,tenantB,{
   partyId:id,page:{pageSize:1}})).body.relationships.length,0);
 assert.equal((await fixtureRpcError(dispatch,rel.listPartyRelationships,viewer,{
   partyId:id,page:{pageSize:101}})).status,400);
 const afterRel=await fixtureRpc(dispatch,rel.listPartyRelationships,viewer,{
   partyId:id,page:{pageSize:1,pageToken:rLast}});
 assert.equal(afterRel.status,200,formatRpcFailure(afterRel));
 assert.equal(afterRel.body.relationships.length,0);
 assert.ok(!afterRel.body.pageInfo?.nextPageToken);
 const relUpdate={relationshipId:rMid,status:2,
   validFrom:{operation:{case:"set" as const,value:{seconds:1710000000n,nanos:123}}},
   validUntil:{operation:{case:"clear" as const,value:true}},
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-rel-update-01"}};
 assert.equal((await fixtureRpcError(dispatch,rel.updatePartyRelationship,viewer,relUpdate)).status,403);
 assert.equal((await fixtureRpcError(dispatch,rel.updatePartyRelationship,admin,{
   ...relUpdate,status:0,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-rel-invalid-status"}})).status,400);
 const updatedRel=await fixtureRpc(dispatch,rel.updatePartyRelationship,knownService,relUpdate);
 assert.equal(updatedRel.status,200,formatRpcFailure(updatedRel));
 assert.equal(updatedRel.body.relationship?.version,2n);
 assert.equal(updatedRel.body.relationship?.status,2);
 assert.equal(updatedRel.body.relationship?.validFrom?.seconds,1710000000n);
 assert.equal(updatedRel.body.relationship?.validUntil,undefined);
 // After the update this relationship is INACTIVE; the other one is ACTIVE.
 const relationFilterAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 assert.deepEqual((await collectRelationships({
   partyId:id,status:1
 })).map(r=>r.relationshipId),[rLast]);
 assert.deepEqual((await collectRelationships({
   partyId:id,status:2
 })).map(r=>r.relationshipId),[rMid]);
 assert.equal((await collectRelationships({
   partyId:"missing-party",status:2
 })).length,0);
 for(const invalidStatus of [0,99]){
   const denied=await fixtureRpcError(dispatch,rel.listPartyRelationships,viewer,{status:invalidStatus});
   assert.equal(denied.status,400,formatRpcFailure(denied));
 }
 assert.equal((await fixtureRpc(dispatch,rel.listPartyRelationships,tenantB,{partyId:id,status:2}))
   .body.relationships.length,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   relationFilterAudit,"Relationship status reads never write audit");
 console.log(JSON.stringify({event:"crm.native-m1-relationship-status",result:"PASS",
   active_inactive_exact:true,before_cursor:true,tenant:true,
   invalid_enum_rejected:true,read_only:true}));
 const relFirstReplay=await fixtureRpc(dispatch,rel.updatePartyRelationship,admin,relUpdate);
 assert.equal(relFirstReplay.status,200);
 assert.equal(relFirstReplay.body.receipt?.replayed,true);
 const relKeep={relationshipId:rMid,status:1,
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-rel-keep-02"}};
 const relKeepApplied=await fixtureRpc(dispatch,rel.updatePartyRelationship,admin,relKeep);
 assert.equal(relKeepApplied.status,200,formatRpcFailure(relKeepApplied));
 assert.equal(relKeepApplied.body.relationship?.version,3n);
 assert.equal(relKeepApplied.body.relationship?.validFrom?.seconds,1710000000n);
 assert.equal(relKeepApplied.body.relationship?.validUntil,undefined);
 const relLater=await fixtureRpc(dispatch,rel.updatePartyRelationship,admin,{
   ...relKeep,status:2,validFrom:{operation:{case:"set",
     value:{seconds:1720000000n,nanos:987}}},
   validUntil:{operation:{case:"set",value:{seconds:1820000000n,nanos:654}}},
   precondition:{expectedVersion:3n,idempotencyKey:"candidate-rel-later-03"}});
 assert.equal(relLater.status,200,formatRpcFailure(relLater));
 assert.equal(relLater.body.relationship?.version,4n);
 const relAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"+
   " AND subject_id='"+rMid+"'"))[0]?.n;
 const relKeepReplay=await fixtureRpc(dispatch,rel.updatePartyRelationship,admin,relKeep);
 assert.equal(relKeepReplay.status,200,formatRpcFailure(relKeepReplay));
 assert.equal(relKeepReplay.body.receipt?.replayed,true);
 assert.equal(relKeepReplay.body.receipt?.resourceVersion,3n);
 assert.equal(relKeepReplay.body.relationship?.validFrom?.seconds,1710000000n);
 assert.equal(relKeepReplay.body.relationship?.validUntil,undefined);
 assert.equal((await fixtureRpcError(dispatch,rel.updatePartyRelationship,admin,{
   ...relKeep,status:2})).status,409,"same key with different intent rejected");
 assert.equal((await fixtureRpcError(dispatch,rel.updatePartyRelationship,admin,{
   ...relKeep,precondition:{expectedVersion:2n,
     idempotencyKey:"candidate-rel-stale-04"}})).status,409);
 const currentRel=await fixtureRpc(dispatch,rel.getPartyRelationship,viewer,{relationshipId:rMid});
 assert.equal(currentRel.body.relationship?.version,4n);
 assert.equal(currentRel.body.relationship?.validFrom?.seconds,1720000000n);
 assert.equal(currentRel.body.relationship?.validUntil?.seconds,1820000000n);
 assert.equal((await sql("SELECT version FROM crm_party_relationship_records WHERE tenant='tenant-a'"+
   " AND id='"+rMid+"'"))[0]?.version,4);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"+
   " AND subject_id='"+rMid+"'"))[0]?.n,relAuditBefore);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_relationship_receipts WHERE tenant='tenant-a'"+
   " AND key IN ('candidate-rel-create-mid','candidate-rel-update-01',"+
   " 'candidate-rel-keep-02','candidate-rel-later-03')"))[0]?.n,4);

  // R2a #408: real signed human/service authors, replay and atomic SQL rollback.
  assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+rMid+"' ORDER BY rowid")),[
    {action:"party_relationships.create",actor:"native-a@example.test",
      request_id:midCreated.headers.get("x-request-id")},
    {action:"party_relationships.update",actor:"service:ci-fixture.access",
      request_id:updatedRel.headers.get("x-request-id")},
    {action:"party_relationships.update",actor:"native-a@example.test",
      request_id:relKeepApplied.headers.get("x-request-id")},
    {action:"party_relationships.update",actor:"native-a@example.test",
      request_id:relLater.headers.get("x-request-id")}
  ],"Cross-actor replay never replaces original Relationship audit author");
  const relSnapshot=async()=>(await sql("SELECT "+
    "(SELECT state FROM crm_party_relationship_records WHERE tenant='tenant-a' AND id='"+rMid+"') AS state,"+
    "(SELECT COUNT(*) FROM crm_party_relationship_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"));
  const beforeRelFault=(await relSnapshot());
  (await sql("CREATE TRIGGER fixture_fail_rel_actor BEFORE INSERT ON crm_audit "+
    "WHEN NEW.subject_id='"+rMid+"' BEGIN SELECT RAISE(ABORT,'fixture relation audit'); END;"));
  try{
    const failed=await fixtureRpcError(dispatch,rel.updatePartyRelationship,admin,{relationshipId:rMid,status:1,
      precondition:{expectedVersion:4n,idempotencyKey:"candidate-rel-actor-failed"}});
    assert.equal(failed.status,503);
    assert.equal(failed.body.code,"unavailable");
    const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
    assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
    assert.equal(detail.domainCode,"PARTY_RELATIONSHIPS_WRITE_UNCONFIRMED");
    assert.equal(detail.requestId,failed.headers.get("x-request-id"));
    assert.deepEqual((await relSnapshot()),beforeRelFault);
  }finally{await sql("DROP TRIGGER fixture_fail_rel_actor");}
  console.log(JSON.stringify({event:"crm.native-r2a-relationship-actor-audit",result:"PASS",
    signed_cross_actor:true,failed_audit_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-relationship-v2",
   result:"PASS",rpc_methods:["CreatePartyRelationship","UpdatePartyRelationship",
     "GetPartyRelationship","ListPartyRelationships"],
   generated_binary:true,original_owner:"crm.party-relationships",
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,temporal_patch:true,replay_after_interleaving:true,
   no_extra_audit:true,published_active:true}));

 return {rMid,rLast,midRelation,midCreated};
}

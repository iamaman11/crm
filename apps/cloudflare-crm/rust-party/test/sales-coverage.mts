import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { DealService, SalesOverlayRole } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { MemberRole, MemberService } from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import { decodeFixtureDetail, dispatchFixtureRpc, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type CoverageFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string;
 admin: string;
 viewer: string;
 tenantB: string;
};
export async function verifySalesCoverage({dispatch,sql,accountId,admin,viewer,tenantB}: CoverageFixture) {
 const sales=DealService.method, member=MemberService.method;
   // SK-07.D1: signed generated RPCs against the original Sales owners.
   const suffix=randomUUID(),root="territory-root-"+suffix,child="territory-child-"+suffix;
   const email="coverage-"+suffix+"@example.test";
   const human={owner:{case:"assigneeIdentity",value:email}} as const;
   const primary={owner:{case:"assigneeIdentity",value:"native-a@example.test"}} as const;
   const pre=(label: string,version?: bigint)=>({idempotencyKey:"coverage-"+label+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{email,role:MemberRole.VIEWER,precondition:pre("member")})).status,200);
   const configuration={name:"Coverage root "+suffix,defaultOwner:primary};
   const rootCreate={territoryId:root,configuration,precondition:pre("root")};
   const created=await fixtureRpc(dispatch,sales.createTerritory,admin,rootCreate);
   assert.equal(created.status,200,formatRpcFailure(created));
   assert.equal(created.body.territory?.version,1n);
   assert.equal((await fixtureRpc(dispatch,sales.createTerritory,admin,rootCreate)).body.receipt?.replayed,true);
   const childConfig={name:"Coverage child "+suffix,parentId:root,defaultOwner:primary};
   assert.equal((await fixtureRpc(dispatch,sales.createTerritory,admin,{territoryId:child,configuration:childConfig,precondition:pre("child")})).status,200);
   const read=await fixtureRpc(dispatch,sales.getTerritory,viewer,{territoryId:root,receiptIdempotencyKey:rootCreate.precondition.idempotencyKey});
   assert.equal(read.status,200);
   assert.deepEqual(read.body.territory,created.body.territory);
   assert.equal(read.body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await fixtureRpcError(dispatch,sales.getTerritory,tenantB,{territoryId:root})).status,404);
   assert.equal((await fixtureRpc(dispatch,sales.listTerritories,tenantB,{})).body.territories.length,0);
   assert.equal((await fixtureRpc(dispatch,sales.listTerritories,viewer,{page:{pageSize:1}})).body.territories.length,1);
   assert.equal((await fixtureRpcError(dispatch,sales.updateTerritory,admin,{territoryId:root,
     configuration:{...configuration,parentId:child},precondition:pre("cycle",1n)})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.updateTerritory,admin,{territoryId:root,
     configuration:{...configuration,archived:true},precondition:pre("blocked-parent",1n)})).status,409);
   const assignmentConfig={territoryId:child,primaryOwner:human,overlays:[{role:SalesOverlayRole.SPECIALIST,owner:primary}]};
   const assignmentInput={accountId,configuration:assignmentConfig,precondition:pre("assignment")};
   assert.equal((await fixtureRpc(dispatch,sales.getSalesAccountAssignment,viewer,{accountId})).body.assignment,undefined);
   const assignment=await fixtureRpc(dispatch,sales.createSalesAccountAssignment,admin,assignmentInput);
   assert.equal(assignment.status,200,formatRpcFailure(assignment));
   assert.equal(assignment.body.assignment?.version,1n);
   assert.equal((await fixtureRpc(dispatch,sales.createSalesAccountAssignment,admin,assignmentInput)).body.receipt?.replayed,true);
   const aRead=await fixtureRpc(dispatch,sales.getSalesAccountAssignment,viewer,{accountId,receiptIdempotencyKey:assignmentInput.precondition.idempotencyKey});
   assert.deepEqual(aRead.body.assignment,assignment.body.assignment);
   assert.equal(aRead.body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesAccountAssignment,tenantB,{accountId})).status,404);
   assert.equal((await fixtureRpc(dispatch,sales.listSalesAccountAssignments,tenantB,{})).body.assignments.length,0);
   const filtered=await fixtureRpc(dispatch,sales.listSalesAccountAssignments,viewer,{territoryId:child,primaryOwnerFilter:human});
   assert.deepEqual(filtered.body.assignments.map(a=>a.accountId),[accountId]);
   assert.equal((await fixtureRpcError(dispatch,sales.updateTerritory,admin,{territoryId:child,
     configuration:{...childConfig,archived:true},precondition:pre("blocked-child",1n)})).status,409);
   assert.equal((await fixtureRpcError(dispatch,member.updateMember,admin,{email,active:false,precondition:pre("blocked-member",1n)})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,
     configuration:{...assignmentConfig,overlays:[{role:SalesOverlayRole.SPECIALIST,owner:human}]},precondition:pre("duplicate",1n)})).status,400);
   const before=await sql("SELECT state,version FROM crm_sales_account_assignment_records WHERE tenant=? AND id=?","tenant-a",accountId);
   const count=(await sql("SELECT COUNT(*) n FROM crm_sales_account_assignment_receipts WHERE tenant=?","tenant-a"))[0]?.n;
   await sql("CREATE TRIGGER fixture_fail_coverage_audit BEFORE INSERT ON crm_audit WHEN NEW.action='sales.account_assignment.update' BEGIN SELECT RAISE(ABORT,'fixture coverage audit rollback'); END;");
   try{
     const fail=await fixtureRpcError(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,configuration:{...assignmentConfig,archived:true},precondition:pre("fault",1n)});
     assert.equal(fail.status,503);
     assert.equal(fail.body.code,"unavailable");
     const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(fail.body).details));
     assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
     assert.equal(detail.domainCode,"SALES_COVERAGE_WRITE_UNCONFIRMED");
     assert.equal(detail.requestId,fail.headers.get("x-request-id"));
     assert.deepEqual(await sql("SELECT state,version FROM crm_sales_account_assignment_records WHERE tenant=? AND id=?","tenant-a",accountId),before);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_account_assignment_receipts WHERE tenant=?","tenant-a"))[0]?.n,count);
   }finally{await sql("DROP TRIGGER fixture_fail_coverage_audit");}
  const auditCount=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action LIKE 'sales.account_assignment.%'",
    "tenant-a",accountId))[0]?.n;
  assert.equal(auditCount,1,"the existing assignment audit must be present before the adapter-failure probe");
  await sql("ALTER TABLE crm_sales_account_assignment_receipts RENAME TO fixture_unavailable_assignment_receipts");
  try{
    const unavailable=await fixtureRpcError(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,
      configuration:{...assignmentConfig,archived:true},precondition:pre("unavailable",1n)});
    assert.equal(unavailable.status,503);
    assert.equal(unavailable.body.code,"unavailable");
    const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(unavailable.body).details));
    assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
    assert.equal(detail.domainCode,"SALES_COVERAGE_WRITE_FAILED");
    assert.equal(detail.requestId,unavailable.headers.get("x-request-id"));
    assert.deepEqual(await sql("SELECT state,version FROM crm_sales_account_assignment_records WHERE tenant=? AND id=?","tenant-a",accountId),before);
    assert.equal((await sql("SELECT COUNT(*) n FROM fixture_unavailable_assignment_receipts WHERE tenant=?","tenant-a"))[0]?.n,count);
    assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action LIKE 'sales.account_assignment.%'",
      "tenant-a",accountId))[0]?.n,auditCount);
  }finally{await sql("ALTER TABLE fixture_unavailable_assignment_receipts RENAME TO crm_sales_account_assignment_receipts");}
   const archived=await fixtureRpc(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,
     configuration:{...assignmentConfig,archived:true},precondition:pre("archive-assignment",1n)});
   assert.equal(archived.status,200,formatRpcFailure(archived));
   assert.equal(archived.body.assignment?.version,2n);
   assert.equal((await fixtureRpcError(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,
     configuration:assignmentConfig,precondition:pre("stale",1n)})).status,409);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{email,active:false,precondition:pre("deactivate",1n)})).status,200);
   assert.equal((await fixtureRpcError(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,
     configuration:assignmentConfig,precondition:pre("inactive-owner",2n)})).status,409);
   assert.equal((await fixtureRpc(dispatch,sales.updateTerritory,admin,{territoryId:child,
     configuration:{...childConfig,archived:true},precondition:pre("archive-child",1n)})).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.updateTerritory,admin,{territoryId:root,
     configuration:{...configuration,archived:true},precondition:pre("archive-root",1n)})).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.listTerritories,viewer,{})).body.territories.length,0);
   const archivedTerritories=(await fixtureRpc(dispatch,sales.listTerritories,viewer,{includeArchived:true})).body.territories;
   assert.deepEqual(archivedTerritories.filter(t=>[root,child].includes(t.territoryId)).map(t=>t.configuration?.archived),[true,true]);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id IN (?,?) AND action LIKE 'sales.territory.%'","tenant-a",root,child))[0]?.n,4);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action LIKE 'sales.account_assignment.%'","tenant-a",accountId))[0]?.n,2);
   const raceTeam="coverage-team-"+suffix,raceTerritory="coverage-race-"+suffix;
   const teamOwner={owner:{case:"teamId",value:raceTeam}} as const;
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:raceTeam,name:"Coverage race "+suffix,
     roster:{memberEmails:["native-a@example.test"]},precondition:pre("race-team")})).status,200);
   const raceConfig={name:"Coverage race "+suffix,defaultOwner:teamOwner};
   const [assigned,teamArchived]=await Promise.all([
     dispatchFixtureRpc(dispatch,sales.createTerritory,admin,{territoryId:raceTerritory,configuration:raceConfig,precondition:pre("race-territory")}),
     dispatchFixtureRpc(dispatch,member.updateTeam,admin,{teamId:raceTeam,active:false,precondition:pre("race-archive",1n)})
   ]);
   assert.equal(Number(assigned.status===200)+Number(teamArchived.status===200),1);
   if(assigned.status===200){
     assert.equal(teamArchived.status,409);
     assert.equal((await fixtureRpc(dispatch,sales.updateTerritory,admin,{territoryId:raceTerritory,
       configuration:{...raceConfig,archived:true},precondition:pre("race-cleanup",1n)})).status,200);
     assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{teamId:raceTeam,active:false,precondition:pre("race-archive",1n)})).status,200);
   }else{
     assert.equal(assigned.status,409);
   }
   console.log(JSON.stringify({event:"crm.native-sk07d1-coverage",result:"PASS",
     typed_ownership:true,hierarchy:true,tenant_ab:true,archive_guard:true,overlay:true,cas_replay:true,atomic_audit_rollback:true}));
}

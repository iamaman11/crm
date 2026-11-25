import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { dispatchFixtureRpc, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { MemberService } from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type SalesOwnerFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string;
 admin: string;
 viewer: string;
 tenantB: string;
 jwt: (email: string) => string;
};

export async function verifySalesOwner({dispatch,sql,accountId,admin,viewer,tenantB,jwt}: SalesOwnerFixture) {
 const sales=DealService.method, member=MemberService.method, task=TaskService.method;
   const salesTeam="sk07-sales-team-"+randomUUID();
   const salesTeamCreated=await fixtureRpc(dispatch,member.createTeam,admin,{
     teamId:salesTeam,name:"Sales owner QA "+salesTeam,roster:{memberEmails:["native-viewer@example.test"]},
     precondition:{idempotencyKey:"team-"+salesTeam}
   });
   assert.equal(salesTeamCreated.status,200,formatRpcFailure(salesTeamCreated));
   const ownerFilter={owner:{case:"teamId",value:salesTeam}} as const;
   const actorOwner={owner:{case:"assigneeIdentity",value:"native-viewer@example.test"}} as const;
   const pId="sk07-sales-owner-"+randomUUID();
   const pMade=await fixtureRpc(dispatch,sales.createPipeline,admin,{pipelineId:pId,name:"Owner filter QA "+pId,
     stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:5000}],
     precondition:{idempotencyKey:"pipeline-"+pId}});
   assert.equal(pMade.status,200,formatRpcFailure(pMade));
   const input=(id: string,owner: {owner: {case: "teamId"; value: string} | {case: "assigneeIdentity"; value: string}},currency: string,units: bigint)=>({dealId:id,name:"Sales owner QA "+id,accountId,
     stage:{pipelineId:pId,stageId:"lead",ordinal:1},owner,amount:{currencyCode:currency,minorUnits:units},
     precondition:{idempotencyKey:"owner-create-"+id}});
   const ids=["sk07-owner-a-"+randomUUID(),"sk07-owner-b-"+randomUUID(),"sk07-owner-c-"+randomUUID()];
   const inputs=[input(ids[0]!,ownerFilter,"EUR",10001n),input(ids[1]!,ownerFilter,"USD",20000n),input(ids[2]!,actorOwner,"EUR",40000n)];
   for(const request of inputs){
     const result=await fixtureRpc(dispatch,sales.createDeal,admin,request);
     assert.ok(request.owner.owner);
     assert.equal(result.status,200,formatRpcFailure(result));
     assert.equal(result.body.deal?.owner?.owner.case,request.owner.owner.case);
     assert.equal(result.body.deal?.owner?.owner.value,request.owner.owner.value);
   }
   assert.equal((await fixtureRpc(dispatch,sales.createDeal,admin,inputs[0]!)).body.receipt?.replayed,true);
   const missing=input("sk07-owner-bad-"+randomUUID(),{owner:{case:"teamId",value:"missing-team"}},"EUR",1n);
   assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,missing)).status,400);
   assert.equal((await fixtureRpcError(dispatch,sales.createDeal,viewer,{...inputs[0]!,dealId:"sk07-owner-denied-"+randomUUID()})).status,403);
   const list=await fixtureRpc(dispatch,sales.listDeals,viewer,{ownerFilter,pipelineId:pId,page:{pageSize:1}});
   assert.equal(list.status,200,formatRpcFailure(list));
   assert.equal(list.body.deals[0]?.dealId,ids[0]!);
   assert.ok(list.body.pageInfo?.nextPageToken);
   const next=await fixtureRpc(dispatch,sales.listDeals,viewer,{ownerFilter,pipelineId:pId,page:{pageSize:1,pageToken:list.body.pageInfo.nextPageToken}});
   assert.equal(next.body.deals[0]?.dealId,ids[1]!);
   assert.equal((await fixtureRpc(dispatch,sales.listDeals,tenantB,{ownerFilter})).body.deals.length,0);
   const board=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{pipelineId:pId,stageId:"lead",ownerFilter,page:{pageSize:1}});
   assert.equal(board.body.totalCount,2n);
   assert.deepEqual(board.body.currencyTotals.map(x=>[x.currencyCode,x.totalMinorUnits]),[["EUR","10001"],["USD","20000"]]);
   const metrics=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{pipelineId:pId,ownerFilter});
   assert.equal(metrics.status,200,formatRpcFailure(metrics));
   assert.equal(metrics.body.totalDealCount,2n);
   assert.deepEqual(metrics.body.forecastCurrencies.map(x=>[x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints]),
     [["EUR","10001","50005000"],["USD","20000","100000000"]]);
   const viewId="sk07-owner-view-"+randomUUID();
   const viewCreated=await fixtureRpc(dispatch,sales.createSalesView,admin,{viewId,name:"Team owner view",pipelineId:pId,ownerFilter,
     precondition:{idempotencyKey:"view-"+viewId}});
   assert.equal(viewCreated.status,200,formatRpcFailure(viewCreated));
   const viewRead=await fixtureRpc(dispatch,sales.getSalesView,viewer,{viewId});
   assert.equal(viewRead.body.view?.ownerFilter?.owner.value,salesTeam);
   const noChange={teamId:salesTeam,active:true,precondition:{expectedVersion:1n,idempotencyKey:"noop-"+salesTeam}};
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,noChange)).status,400);
   const archive={teamId:salesTeam,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+salesTeam}};
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,archive)).status,409);
   for(const id of ids.slice(0,2)){
     const change={dealId:id,owner:actorOwner,precondition:{expectedVersion:1n,idempotencyKey:"owner-update-"+id}};
     assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,change)).status,403);
     const result=await fixtureRpc(dispatch,sales.updateDeal,admin,change);
     assert.equal(result.status,200,formatRpcFailure(result));
     assert.equal(result.body.deal?.owner?.owner.value,"native-viewer@example.test");
     assert.equal((await fixtureRpc(dispatch,sales.updateDeal,admin,change)).body.receipt?.replayed,true);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",id))[0]?.n,2);
   }
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,archive)).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{pipelineId:pId,ownerFilter})).body.totalDealCount,0n);
   assert.equal((await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{pipelineId:pId,ownerFilter:actorOwner})).body.totalDealCount,3n);
   console.log(JSON.stringify({event:"crm.native-sk07c-sales-owner",result:"PASS",typed_owner:true,tenant_ab:true,
     list_before_limit:true,exact_currency:true,forecast:true,saved_view:true,archive_guard:true,replay:true}));
   // SK-07.C4: no last-human removal/revocation can strand an open pool.
   const suffix=randomUUID(),lastEmail="qa-last-"+suffix+"@example.test",lastTeam="sk07-last-"+suffix;
   const lastJwt=jwt(lastEmail);
   assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{email:lastEmail,role:1,
     precondition:{idempotencyKey:"member-"+suffix}})).status,200);
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:lastTeam,name:"Last member QA "+suffix,
     roster:{memberEmails:[lastEmail]},precondition:{idempotencyKey:"team-"+suffix}})).status,200);
   const lastOwner={owner:{case:"teamId",value:lastTeam}} as const;
   const lastTask="sk07-last-task-"+suffix,lastDeal="sk07-last-deal-"+suffix;
   assert.equal((await fixtureRpc(dispatch,task.createTask,admin,{taskId:lastTask,accountId,subject:"Last member QA",priority:2,
     owner:lastOwner,precondition:{idempotencyKey:"task-"+suffix}})).status,200);
   const empty={teamId:lastTeam,roster:{memberEmails:[]},precondition:{expectedVersion:1n,idempotencyKey:"empty-"+suffix}};
   const deactivate={email:lastEmail,active:false,precondition:{expectedVersion:1n,idempotencyKey:"revoke-"+suffix}};
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,empty)).status,409);
   assert.equal((await fixtureRpcError(dispatch,member.updateMember,admin,deactivate)).status,409);
   assert.equal((await fixtureRpc(dispatch,sales.createDeal,admin,input(lastDeal,lastOwner,"EUR",12500n))).status,200);
   const claim={taskId:lastTask,precondition:{expectedVersion:1n,idempotencyKey:"claim-"+suffix}};
   const [claimed,revoked]=await Promise.all([
     dispatchFixtureRpc(dispatch,task.claimTask,lastJwt,claim),
     dispatchFixtureRpc(dispatch,member.updateMember,admin,deactivate)]);
   assert.equal(claimed.status,200);
   assert.equal(revoked.status,409,"open Deal still requires an eligible human");
   assert.equal((await fixtureRpc(dispatch,task.completeTask,admin,{taskId:lastTask,
     precondition:{expectedVersion:2n,idempotencyKey:"complete-"+suffix}})).status,200);
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,empty)).status,409,"open Deal alone protects the roster");
   assert.equal((await fixtureRpc(dispatch,sales.closeDeal,admin,{dealId:lastDeal,outcome:3,reasonCode:"qa.closed",
     precondition:{expectedVersion:1n,idempotencyKey:"close-"+suffix}})).status,200);
   const archiveLast={teamId:lastTeam,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+suffix}};
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,archiveLast)).status,200);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,deactivate)).status,200);
   assert.equal((await fixtureRpcError(dispatch,member.getMyMembership,lastJwt,{})).status,401);
   const reopen={dealId:lastDeal,reasonCode:"qa.revisit",precondition:{expectedVersion:2n,idempotencyKey:"reopen-"+suffix}};
   const before=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastDeal))[0]?.n;
   assert.ok(typeof before === "number");
   assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,admin,reopen)).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,admin,{...reopen,owner:lastOwner})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,admin,{...reopen,owner:{owner:{case:"assigneeIdentity",value:lastEmail}}})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,viewer,{...reopen,owner:actorOwner})).status,403);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastDeal))[0]?.n,before);
   const reopened=await fixtureRpc(dispatch,sales.reopenDeal,admin,{...reopen,owner:actorOwner});
   assert.equal(reopened.status,200,formatRpcFailure(reopened));
   assert.equal(reopened.body.deal?.version,3n);
   assert.equal(reopened.body.deal?.owner?.owner.value,actorOwner.owner.value);
   assert.equal((await fixtureRpc(dispatch,sales.reopenDeal,admin,{...reopen,owner:actorOwner})).body.receipt?.replayed,true);
   assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,admin,{...reopen,owner:lastOwner})).status,409,"changed idempotent intent");
   const events=(await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId:lastDeal,page:{pageSize:10}})).body.events;
   assert.deepEqual(events.map(e=>e.dealVersion),[3n,2n,1n]);
   assert.equal(events[0]?.kind,5);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastDeal))[0]?.n,before+1);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastTeam))[0]?.n,2);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastEmail))[0]?.n,2);

   // A closed team-owned deal and team archive race on the same D1 invariant.
   const raceId="sk07-reopen-race-"+randomUUID(),raceDeal="deal-"+raceId;
   const raceOwner={owner:{case:"teamId",value:raceId}} as const;
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:raceId,name:"Reopen race "+raceId,
     roster:{memberEmails:["native-a@example.test"]},precondition:{idempotencyKey:"team-"+raceId}})).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.createDeal,admin,input(raceDeal,raceOwner,"EUR",100n))).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.closeDeal,admin,{dealId:raceDeal,outcome:3,reasonCode:"qa.close",
     precondition:{expectedVersion:1n,idempotencyKey:"close-"+raceId}})).status,200);
   const reopenRace={dealId:raceDeal,reasonCode:"qa.reopen",precondition:{expectedVersion:2n,idempotencyKey:"reopen-"+raceId}};
   const archiveRace={teamId:raceId,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+raceId}};
   const [rr,ar]=await Promise.all([
     dispatchFixtureRpc(dispatch,sales.reopenDeal,admin,reopenRace),
     dispatchFixtureRpc(dispatch,member.updateTeam,admin,archiveRace)]);
   assert.equal(Number(rr.status===200)+Number(ar.status===200),1,"reopen and archive cannot both commit");
   assert.equal(rr.status===200?ar.status:rr.status,409);
   if(rr.status===200){
     assert.equal((await fixtureRpc(dispatch,sales.closeDeal,admin,{dealId:raceDeal,outcome:3,reasonCode:"qa.cleanup",
       precondition:{expectedVersion:3n,idempotencyKey:"cleanup-"+raceId}})).status,200);
     assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,archiveRace)).status,200);
   }
   console.log(JSON.stringify({event:"crm.native-sk07c4-owner-lifecycle",result:"PASS",
     last_member_guard:true,claim_deactivation_race:true,closed_owner_retained:true,typed_reopen_reassign:true,
     reopen_archive_race:true,replay:true,one_event_receipt_audit:true}));
}

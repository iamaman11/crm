import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { DealService, OpportunityTeamRole } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { MemberService } from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { fixtureRpc, fixtureRpcError, fixtureRpcRace } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type LifecycleFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 jwt: (email: string) => string;
 accountId: string; admin: string; viewer: string; tenantB: string;
};
export async function verifyLifecycle({dispatch,sql,jwt,accountId,admin,viewer,tenantB}: LifecycleFixture) {
 const sales=DealService.method;
 const member=MemberService.method;
 const task=TaskService.method;
 // Independent signed viewer and original lifecycle CAS; test at the end
 // so disposable fixtures never change earlier deterministic analytics.
 {
   const suffix=randomUUID(),email="opportunity-"+suffix+"@example.test";
   const token=jwt(email),pid="opportunity-pipeline-"+suffix,id="opportunity-deal-"+suffix;
   const pre=(key: string,version?: bigint)=>({idempotencyKey:"opportunity-"+key+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{email,role:1,precondition:pre("member")})).status,200);
   const pipeline=await fixtureRpc(dispatch,sales.createPipeline,admin,{pipelineId:pid,name:"Opportunity QA",
     stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:1000},{stageId:"qualified",name:"Qualified",probabilityBasisPoints:5000}],precondition:pre("pipeline")});
   assert.equal(pipeline.status,200,formatRpcFailure(pipeline));
   const deal=await fixtureRpc(dispatch,sales.createDeal,admin,{dealId:id,name:"Opportunity QA",accountId,
     stage:{pipelineId:pid,stageId:"lead"},precondition:pre("deal")});
   assert.equal(deal.status,200,formatRpcFailure(deal));
   const grant=(role: OpportunityTeamRole | undefined,version: bigint,key: string)=>({dealId:id,email,...(role===undefined?{}:{role}),precondition:pre(key,version)});
   const update=(version: bigint,key: string)=>({dealId:id,name:{operation:{case:"set" as const,value:"Opportunity "+key}},precondition:pre(key,version)});
   const lead=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(OpportunityTeamRole.LEAD,1n,"lead"));
   assert.equal(lead.status,200,formatRpcFailure(lead));
   assert.equal(lead.body.deal?.opportunityTeam[0]?.memberVersion,1n);
   const advance={dealId:id,targetStage:{pipelineId:pid,stageId:"qualified"},targetStatus:1,precondition:pre("move",2n)};
   const moved=await fixtureRpc(dispatch,sales.advanceStage,token,advance);
   assert.equal(moved.status,200,formatRpcFailure(moved));
   assert.equal(moved.body.deal?.stage?.stageId,"qualified");
   assert.equal(moved.body.deal?.version,3n);
   const moveReplay=await fixtureRpc(dispatch,sales.advanceStage,token,advance);
   assert.equal(moveReplay.status,200,formatRpcFailure(moveReplay));
   assert.equal(moveReplay.body.receipt?.replayed,true);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?","tenant-a",id))[0]?.n,1);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{email,active:false,precondition:pre("deactivate",1n)})).status,200);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,token,update(3n,"inactive"))).status,401);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{email,active:true,precondition:pre("reactivate",2n)})).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.getDeal,token,{dealId:id})).body.capabilities?.edit,false);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,token,update(3n,"stale-grant"))).status,403);
   const renewed=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(OpportunityTeamRole.LEAD,3n,"renew"));
   assert.equal(renewed.status,200,formatRpcFailure(renewed));
   assert.equal(renewed.body.deal?.opportunityTeam[0]?.memberVersion,3n);
   const [revoke,write]=await Promise.all([
     fixtureRpcRace(dispatch,sales.setOpportunityParticipant,admin,grant(undefined,4n,"revoke")),
     fixtureRpcRace(dispatch,sales.updateDeal,token,update(4n,"race"))
   ]);
   assert.equal([revoke,write].filter(r=>r.status===200).length,1);
   assert.ok([403,409].includes((revoke.status===200?write:revoke).status));
   if(write.status===200){
     const removed=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(undefined,5n,"revoke-after-race"));
     assert.equal(removed.status,200,formatRpcFailure(removed));
   }
   const final=(await fixtureRpc(dispatch,sales.getDeal,token,{dealId:id})).body;
   assert.ok(final.deal && final.capabilities);
   assert.equal(final.deal.opportunityTeam.length,0);
   assert.equal(final.capabilities.edit,false);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,token,update(final.deal.version,"after-revoke"))).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.advanceStage,token,advance)).status,403);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?","tenant-a",id))[0]?.n,1);
   const events=(await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId:id,page:{pageSize:25}})).body.events;
   assert.equal(events.find(e=>e.kind===3)?.actor,email);
   console.log(JSON.stringify({event:"crm.native-opportunity-delegation",result:"PASS",signed_viewer:true,stage_link_once:true,member_version_pin:true,reactivation_no_restore:true,revocation_cas:true,race:[revoke.status,write.status]}));
 }
 // Cross-owner fixed template with separate fixtures after earlier analytics.
 {
   const suffix=randomUUID(),leadEmail="coord-"+suffix+"@example.test",targetEmail="target-"+suffix+"@example.test",otherEmail="outside-"+suffix+"@example.test";
   const lead=jwt(leadEmail),target=jwt(targetEmail),outside=jwt(otherEmail);
   const tid="coord-team-"+suffix,otherTeam="other-team-"+suffix;
   const pre=(key: string,version?: bigint)=>({idempotencyKey:"coord-"+key+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   for(const [key,email] of [["lead",leadEmail],["target",targetEmail],["outside",otherEmail]] as const)assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{email,role:1,precondition:pre(key)})).status,200);
   const team=await fixtureRpc(dispatch,member.createTeam,admin,{teamId:tid,name:"Coordinator "+suffix,roster:{memberEmails:[leadEmail,targetEmail]},precondition:pre("team")});
   assert.equal(team.status,200,formatRpcFailure(team));
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:otherTeam,name:"Outside "+suffix,roster:{memberEmails:[otherEmail]},precondition:pre("otherteam")})).status,200);
   const grant={teamId:tid,queueCoordinator:{email:leadEmail,enabled:true},precondition:pre("grant",1n)};
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,lead,grant)).status,403);
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,{...grant,queueCoordinator:{email:otherEmail,enabled:true},precondition:pre("foreign",1n)})).status,400);
   const granted=await fixtureRpc(dispatch,member.updateTeam,admin,grant);
   assert.equal(granted.status,200,formatRpcFailure(granted));
   assert.equal(granted.body.team?.queueCoordinators[0]?.memberVersion,1n);
   const taskId="coord-task-"+suffix,dealId="coord-deal-"+suffix,taskRace="coord-race-"+suffix;
   const owner={owner:{case:"teamId" as const,value:tid}},foreignOwner={owner:{case:"teamId" as const,value:otherTeam}};
   for(const [id,own] of [[taskId,owner],[taskRace,owner],["foreign-task-"+suffix,foreignOwner]] as const)assert.equal((await fixtureRpc(dispatch,task.createTask,admin,{taskId:id,accountId,subject:"Coordinator "+id,priority:2,owner:own,precondition:pre("task-"+id)})).status,200);
   assert.equal((await fixtureRpc(dispatch,sales.createDeal,admin,{dealId,name:"Coordinator "+suffix,accountId,stage:{pipelineId:"pipeline.enterprise",stageId:"lead"},owner,precondition:pre("deal")})).status,200);
   const taskRead=(await fixtureRpc(dispatch,task.getTask,lead,{taskId})).body;
   const dealRead=(await fixtureRpc(dispatch,sales.getDeal,lead,{dealId})).body;
   assert.deepEqual(taskRead.teamDistribution,dealRead.teamDistribution);
   assert.ok(taskRead.teamDistribution?.scope);
   assert.deepEqual(taskRead.teamDistribution.eligibleMemberEmails,[leadEmail,targetEmail].sort());
   assert.equal((await fixtureRpc(dispatch,task.getTask,target,{taskId})).body.teamDistribution,undefined);
   assert.equal((await fixtureRpc(dispatch,task.getTask,outside,{taskId})).body.teamDistribution,undefined);
   let scope=taskRead.teamDistribution.scope;
   const currentGrant=async()=>(await fixtureRpc(dispatch,member.getTeam,viewer,{teamId:tid})).body.team?.queueCoordinators[0];
   assert.equal((await currentGrant())?.effective,true);
   assert.equal((await fixtureRpc(dispatch,member.listTeams,viewer,{page:{pageSize:100}})).body.teams.find(t=>t.teamId===tid)?.queueCoordinators[0]?.effective,true);
   const effects=async()=>(await sql("SELECT (SELECT COUNT(*) FROM crm_audit) AS audit,(SELECT COUNT(*) FROM crm_activity_task_receipts) AS tasks,(SELECT COUNT(*) FROM crm_sales_deal_receipts) AS deals,(SELECT COUNT(*) FROM crm_sales_deal_events) AS events"))[0];
   const beforeDenied=await effects();
   const targetOwner={owner:{case:"assigneeIdentity" as const,value:targetEmail}};
   const taskMove=(id: string,key: string,owner=targetOwner,version=1n)=>({taskId:id,owner,teamDistribution:scope,precondition:pre(key,version)});
   const dealMove={dealId,owner:targetOwner,teamDistribution:scope,precondition:pre("deal-move",1n)};
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,target,taskMove(taskId,"nonlead"))).status,403);
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,{...taskMove(taskId,"field"),subject:{operation:{case:"set",value:"Forbidden"}}})).status,403);
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,taskMove(taskId,"outside",{owner:{case:"assigneeIdentity" as const,value:otherEmail}}))).status,403);
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,taskMove("foreign-task-"+suffix,"cross-team"))).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,lead,{...dealMove,probabilityBasisPoints:5000,precondition:pre("money",1n)})).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,lead,{...dealMove,owner:{owner:{case:"assigneeIdentity" as const,value:otherEmail}},precondition:pre("outside-deal",1n)})).status,403);
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,tenantB,taskMove(taskId,"cross-tenant-task"))).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,tenantB,{...dealMove,precondition:pre("cross-tenant-deal",1n)})).status,403);
   assert.deepEqual(await effects(),beforeDenied);
   const command=taskMove(taskId,"move");
   const moved=await fixtureRpc(dispatch,task.updateTask,lead,command);
   assert.equal(moved.status,200,formatRpcFailure(moved));
   assert.equal(moved.body.task?.owner?.owner.value,targetEmail);
   assert.equal((await fixtureRpc(dispatch,task.updateTask,lead,command)).body.receipt?.replayed,true);
   const movedDeal=await fixtureRpc(dispatch,sales.updateDeal,lead,dealMove);
   assert.equal(movedDeal.status,200,formatRpcFailure(movedDeal));
   assert.equal(movedDeal.body.deal?.owner?.owner.value,targetEmail);
   assert.equal((await fixtureRpc(dispatch,sales.updateDeal,lead,dealMove)).body.receipt?.replayed,true);
   assert.equal((await fixtureRpc(dispatch,sales.getDeal,lead,{dealId})).body.teamDistribution,undefined);
   const afterMoves=await effects();
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,taskMove(taskId,"already-personal",targetOwner,2n))).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,lead,{...dealMove,precondition:pre("already-personal-deal",2n)})).status,403);
   assert.deepEqual(await effects(),afterMoves);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{email:leadEmail,active:false,precondition:pre("deactivate",1n)})).status,200);
   assert.equal((await currentGrant())?.effective,false);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{email:leadEmail,active:true,precondition:pre("reactivate",2n)})).status,200);
   assert.equal((await currentGrant())?.effective,false);
   assert.equal((await fixtureRpc(dispatch,task.getTask,lead,{taskId:taskRace})).body.teamDistribution,undefined);
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,taskMove(taskRace,"stale-enrollment"))).status,403);
   const renewed=await fixtureRpc(dispatch,member.updateTeam,admin,{...grant,precondition:pre("renew",2n)});
   assert.equal(renewed.status,200,formatRpcFailure(renewed));
   assert.equal((await currentGrant())?.memberVersion,3n);
   assert.equal((await currentGrant())?.effective,true);
   const roster=async(emails: string[],key: string,version: bigint)=>fixtureRpc(dispatch,member.updateTeam,admin,{teamId:tid,roster:{memberEmails:emails},precondition:pre(key,version)});
   assert.equal((await roster([leadEmail],"remove-target",3n)).status,200);
   {const distribution=(await fixtureRpc(dispatch,task.getTask,lead,{taskId:taskRace})).body.teamDistribution;
    assert.ok(distribution?.scope); scope=distribution.scope;}
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,taskMove(taskRace,"removed-target"))).status,403);
   assert.equal((await roster([leadEmail,targetEmail],"restore-target",4n)).status,200);
   assert.equal((await roster([targetEmail],"remove-coordinator",5n)).status,200);
   assert.equal(await currentGrant(),undefined);
   assert.equal((await roster([leadEmail,targetEmail],"restore-coordinator",6n)).status,200);
   assert.equal((await fixtureRpc(dispatch,task.getTask,lead,{taskId:taskRace})).body.teamDistribution,undefined);
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{...grant,precondition:pre("regrant",7n)})).status,200);
   {const distribution=(await fixtureRpc(dispatch,task.getTask,lead,{taskId:taskRace})).body.teamDistribution;
    assert.ok(distribution?.scope); scope=distribution.scope;}
   const revoke={teamId:tid,queueCoordinator:{email:leadEmail,enabled:false},precondition:pre("revoke",8n)};
   const [removed,race]=await Promise.all([
     fixtureRpcRace(dispatch,member.updateTeam,admin,revoke),
     fixtureRpcRace(dispatch,task.updateTask,lead,taskMove(taskRace,"race"))
   ]);
   assert.equal(removed.status,200,formatRpcFailure(removed));
   assert.ok([200,403,409].includes(race.status),formatRpcFailure(race));
   assert.equal((await fixtureRpcError(dispatch,task.updateTask,lead,command)).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,lead,dealMove)).status,403);
   assert.equal((await fixtureRpc(dispatch,member.getTeam,viewer,{teamId:tid})).body.team?.queueCoordinators.length,0);
   console.log(JSON.stringify({event:"crm.native-team-distribution",result:"PASS",shared_access_template:true,task_deal:true,scoped_targets:true,owner_only:true,replay:true,revocation:true,effective_projection:true,enrollment_pin:true,roster_removal:true,tenant_isolation:true,rejected_side_effects:false,race:race.status}));
 }

}

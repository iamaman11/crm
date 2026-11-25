import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { MemberService } from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import { TaskPriority, TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail, dispatchFixtureRpc, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { decodeMemberV2, decodeTaskV2 } from "../../../../packages/client/test/cloudflare-binary-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type WorkforceFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 jwt: (email: string) => string;
 accountId: string; admin: string; viewer: string; tenantB: string; knownService: string;
};
export async function verifyWorkforce({dispatch,sql,jwt,accountId,admin,viewer,tenantB,knownService}: WorkforceFixture) {
 const member=MemberService.method;
 const task=TaskService.method;
 const race=async<T,>(send: ()=>ReturnType<typeof dispatchFixtureRpc>, decode:(bytes:Uint8Array)=>T)=>{
   const response=await send();
   const bytes=new Uint8Array(await response.arrayBuffer());
   if(response.status>=400){
     const error: unknown=JSON.parse(new TextDecoder().decode(bytes));
     assert.ok(typeof record(error).code==="string");
     return {status:response.status,headers:response.headers,body:undefined};
   }
   return {status:response.status,headers:response.headers,body:decode(bytes)};
 };
 // SK-07.B: signed human membership, CAS role/deactivation and original
 // TaskOwner reassignment. No identity headers or service->human impersonation.
 const beforeMemberRead=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 const myMember=await fixtureRpc(dispatch,member.getMyMembership,admin,{});
 assert.equal(myMember.status,200,formatRpcFailure(myMember));
 assert.equal(myMember.body.member?.email,"native-a@example.test");
 assert.equal(myMember.body.member?.role,2);
 assert.equal((await fixtureRpc(dispatch,member.getMyMembership,viewer,{})).body.member?.role,1);
 assert.equal((await fixtureRpc(dispatch,member.getMyMembership,knownService,{})).body.member?.email,
   "service:ci-fixture.access","service actor remains a SERVICE");
 assert.equal((await fixtureRpc(dispatch,member.getMyMembership,tenantB,{})).body.member?.email,"native-b@example.test");
 const initialRoster=await fixtureRpc(dispatch,member.listMembers,viewer,{page:{pageSize:1}});
 assert.equal(initialRoster.status,200);
 assert.equal(initialRoster.body.members.length,1);
 assert.ok(initialRoster.body.pageInfo?.nextPageToken);
 assert.equal((await fixtureRpcError(dispatch,member.getMember,tenantB,{email:"native-a@example.test"})).status,404);
 assert.equal((await fixtureRpc(dispatch,member.getMember,viewer,{email:"native-a@example.test"})).status,200);
 assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   beforeMemberRead,"Signed membership reads must not append audit");

 const newStaff="native-staff@example.test";
 const staffCreate={email:newStaff,role:1,
   precondition:{idempotencyKey:"sk07-member-create-staff"}};
 assert.equal((await fixtureRpcError(dispatch,member.provisionMember,viewer,staffCreate)).status,403);
 assert.equal((await fixtureRpc(dispatch,member.provisionMember,tenantB,{
   ...staffCreate,precondition:{idempotencyKey:"sk07-member-tenant-b"}
 })).status,200,"tenant B may independently invite an email ONLY if globally unused");
 const staffWrongTenant=await fixtureRpcError(dispatch,member.provisionMember,admin,staffCreate);
 assert.equal(staffWrongTenant.status,409,"an email enrolled by tenant B cannot be reparented");
 assert.equal(staffWrongTenant.body.code,"aborted");
 const staffConflict=decodeFixtureDetail(DomainErrorDetailSchema,records(record(staffWrongTenant.body).details));
 assert.equal(staffConflict.domainCode,"ACCESS_MEMBER_ALREADY_EXISTS");
 assert.equal(staffConflict.commitOutcome,MutationCommitOutcome.REJECTED);
 assert.equal(staffConflict.requestId,staffWrongTenant.headers.get("x-request-id"));
 // A second independent human is provisioned inside tenant A, not minted as JWT.
 const assignee="native-assignee@example.test";
 const staffInput={email:assignee,role:1,
   precondition:{idempotencyKey:"sk07-member-create-assignee"}};
 const staffCreated=await fixtureRpc(dispatch,member.provisionMember,admin,staffInput);
 assert.equal(staffCreated.status,200,formatRpcFailure(staffCreated));
 assert.equal(staffCreated.body.member?.email,assignee);
 assert.equal(staffCreated.body.member?.active,true);
 assert.equal(staffCreated.body.member?.version,1n);
 assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,staffInput)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,member.provisionMember,admin,{...staffInput,role:2})).status,409);
 const memberProof=await fixtureRpc(dispatch,member.getMember,viewer,{email:assignee,
   receiptIdempotencyKey:staffInput.precondition.idempotencyKey});
 assert.equal(memberProof.body.confirmedMutation?.originalRequestId,
   staffCreated.body.receipt?.requestId);
 assert.equal(memberProof.body.confirmedMutation?.resourceVersion,1n);
 assert.equal((await fixtureRpcError(dispatch,member.getMember,tenantB,{email:assignee})).status,404);
 assert.equal((await fixtureRpcError(dispatch,member.provisionMember,admin,{
   email:"service:forged.access",role:2,
   precondition:{idempotencyKey:"sk07-member-forged-service"}
 })).status,400);

 // Signed invited HUMAN login really joins the same tenant through Access.
 const invitedJwt=jwt(assignee);
 const invitedMe=await fixtureRpc(dispatch,member.getMyMembership,invitedJwt,{});
 assert.equal(invitedMe.status,200);
 assert.equal(invitedMe.body.member?.email,assignee);
 assert.equal((await fixtureRpcError(dispatch,member.provisionMember,invitedJwt,staffCreate)).status,403);
 const changeMember={email:assignee,role:2,
   precondition:{idempotencyKey:"sk07-member-promote",expectedVersion:1n}};
 const promoted=await fixtureRpc(dispatch,member.updateMember,admin,changeMember);
 assert.equal(promoted.status,200,formatRpcFailure(promoted));
 assert.equal(promoted.body.member?.version,2n);
 assert.equal(promoted.body.member?.role,2);
 assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,changeMember)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,member.updateMember,admin,{
   ...changeMember,precondition:{idempotencyKey:"sk07-member-stale",expectedVersion:1n}
 })).status,409);
 assert.equal((await fixtureRpcError(dispatch,member.updateMember,invitedJwt,{
   email:assignee,active:false,
   precondition:{idempotencyKey:"sk07-member-self",expectedVersion:2n}
 })).status,403,"no self lockout");
 assert.equal((await fixtureRpcError(dispatch,member.updateMember,admin,{
   email:"native-b@example.test",active:false,
   precondition:{idempotencyKey:"sk07-member-foreign",expectedVersion:1n}
 })).status,409,"cross tenant membership mutation is forbidden");

 // One original Task owner: assign to a signed human, query BEFORE LIMIT,
 // reassign by original Rust domain Patch<TaskOwner>, CAS and receipts.
 const delegatedTask="sk07-task-"+randomUUID();
 const delegatedCreate={taskId:delegatedTask,accountId,subject:"SK07 assigned Task",
   priority:2,assigneeEmail:assignee,
   precondition:{idempotencyKey:"sk07-task-create-"+delegatedTask}};
 const delegated=await fixtureRpc(dispatch,task.createTask,admin,delegatedCreate);
 assert.equal(delegated.status,200,formatRpcFailure(delegated));
 assert.equal(delegated.body.task?.assigneeIdentity,assignee);
 const delegatedList=await fixtureRpc(dispatch,task.listTasks,viewer,{
   assigneeEmail:assignee,page:{pageSize:100}
 });
 assert.equal(delegatedList.status,200);
 assert.ok(delegatedList.body.tasks.some(t=>t.taskId===delegatedTask));
 assert.equal((await fixtureRpc(dispatch,task.listTasks,invitedJwt,{assignedToMe:true,
   page:{pageSize:100}})).body.tasks.some(t=>t.taskId===delegatedTask),true);
 assert.equal((await fixtureRpcError(dispatch,task.listTasks,viewer,{assignedToMe:true,
   assigneeEmail:assignee})).status,400);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,{assigneeEmail:assignee})).body.tasks.length,0);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{
   ...delegatedCreate,taskId:"sk07-task-foreign",
   assigneeEmail:"native-b@example.test",
   precondition:{idempotencyKey:"sk07-task-foreign"}
 })).status,400);
 const workload=await fixtureRpc(dispatch,task.listTaskWorkload,viewer,{});
 assert.equal(workload.status,200);
 assert.ok(workload.body.buckets.some(b=>b.assigneeIdentity===assignee&&b.openCount>0n));
 assert.ok(workload.body.asOfUnixSeconds>0n);
 assert.deepEqual((await fixtureRpc(dispatch,task.listTaskWorkload,admin,{})).body.buckets,workload.body.buckets);
 assert.equal((await fixtureRpc(dispatch,task.listTaskWorkload,tenantB,{})).body.buckets
   .some(b=>b.assigneeIdentity===assignee),false);
 const personalBefore={taskId:delegatedTask,
   precondition:{expectedVersion:1n,idempotencyKey:"sk07-not-my-task"}};
 assert.equal((await fixtureRpcError(dispatch,task.completeTask,viewer,personalBefore)).status,403);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,viewer,{
   ...personalBefore,reminderAt:{seconds:1860000000n,nanos:0}
 })).status,403);
 const delegationChange={taskId:delegatedTask,assigneeEmail:"native-viewer@example.test",
   precondition:{idempotencyKey:"sk07-task-reassign",expectedVersion:1n}};
 const reassign=await fixtureRpc(dispatch,task.updateTask,admin,delegationChange);
 assert.equal(reassign.status,200,formatRpcFailure(reassign));
 assert.equal(reassign.body.task?.assigneeIdentity,"native-viewer@example.test");
 assert.equal(reassign.body.task?.version,2n);
 assert.equal((await fixtureRpc(dispatch,task.updateTask,admin,delegationChange)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,admin,{
   ...delegationChange,precondition:{idempotencyKey:"sk07-task-stale",expectedVersion:1n}
 })).status,409);
 const delegatedProof=await fixtureRpc(dispatch,task.getTask,viewer,{
   taskId:delegatedTask,receiptIdempotencyKey:"sk07-task-reassign"
 });
 assert.equal(delegatedProof.body.confirmedMutation?.originalRequestId,
   reassign.body.receipt?.requestId);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,viewer,{assignedToMe:true,
   page:{pageSize:100}})).body.tasks.some(t=>t.taskId===delegatedTask),true);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,invitedJwt,{assignedToMe:true,
   page:{pageSize:100}})).body.tasks.some(t=>t.taskId===delegatedTask),false);

 // Personal writes use the signed viewer, not administrator impersonation.
 const personalReminder={taskId:delegatedTask,reminderAt:{seconds:1860000000n,nanos:0},
   precondition:{expectedVersion:2n,idempotencyKey:"sk07-own-reminder"}};
 const reminderOwn=await fixtureRpc(dispatch,task.scheduleReminder,viewer,personalReminder);
 assert.equal(reminderOwn.status,200,formatRpcFailure(reminderOwn));
 assert.equal(reminderOwn.body.task?.version,3n);
 assert.equal((await fixtureRpc(dispatch,task.scheduleReminder,viewer,personalReminder)).body.receipt?.replayed,true);
 const personalComplete={taskId:delegatedTask,
   precondition:{expectedVersion:3n,idempotencyKey:"sk07-own-complete"}};
 assert.equal((await fixtureRpcError(dispatch,task.completeTask,tenantB,personalComplete)).status,404);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,viewer,{
   ...delegationChange,precondition:{expectedVersion:3n,idempotencyKey:"sk07-no-self-delegate"}
 })).status,403);
 const ownDone=await fixtureRpc(dispatch,task.completeTask,viewer,personalComplete);
 assert.equal(ownDone.status,200,formatRpcFailure(ownDone));
 assert.equal(ownDone.body.task?.status,2);
 assert.equal(ownDone.body.task?.version,4n);
 assert.equal((await fixtureRpc(dispatch,task.completeTask,viewer,personalComplete)).body.receipt?.replayed,true);
 assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND actor=?",
   "tenant-a",delegatedTask,"native-viewer@example.test"))[0]?.n,2);

 // Assignment and revocation contend in the same D1. A successful assignment
 // must appear before revocation; a rejected command must not write/audit.
 const raceCreateId="sk07-race-create-"+randomUUID();
 const raceUpdateId="sk07-race-update-"+randomUUID();
 const raceBase=await fixtureRpc(dispatch,task.createTask,admin,{
   taskId:raceUpdateId,accountId,subject:"SK07 race reassignment base",priority:TaskPriority.NORMAL,
   precondition:{idempotencyKey:"sk07-race-base-"+raceUpdateId}
 });
 assert.equal(raceBase.status,200,formatRpcFailure(raceBase));
 const raceCreate={...delegatedCreate,taskId:raceCreateId,
   precondition:{idempotencyKey:"sk07-race-assign-"+raceCreateId}};
 const raceUpdate={taskId:raceUpdateId,assigneeEmail:assignee,
   precondition:{expectedVersion:1n,idempotencyKey:"sk07-race-assign-"+raceUpdateId}};
 const [createRace,updateRace,deactivate]=await Promise.all([
   race(()=>dispatchFixtureRpc(dispatch,task.createTask,admin,raceCreate),bytes=>decodeTaskV2("CreateTask",bytes)),
   race(()=>dispatchFixtureRpc(dispatch,task.updateTask,admin,raceUpdate),bytes=>decodeTaskV2("UpdateTask",bytes)),
   race(()=>dispatchFixtureRpc(dispatch,member.updateMember,admin,{email:assignee,active:false, precondition:{idempotencyKey:"sk07-member-deactivate",expectedVersion:2n}}),bytes=>decodeMemberV2("UpdateMember",bytes))
 ]);
 assert.equal(deactivate.status,200,formatRpcFailure(deactivate));
 assert.ok(deactivate.body?.receipt);
 const revokedAt=(await sql("SELECT rowid AS seq FROM crm_audit WHERE tenant_id=? AND request_id=?",
   "tenant-a",deactivate.body.receipt.requestId))[0]?.seq;
 assert.ok(typeof revokedAt==="number");
 for(const [response,taskId,action,method] of [
   [createRace,raceCreateId,"activity_tasks.create","CreateTask"],
   [updateRace,raceUpdateId,"activity_tasks.update","UpdateTask"]
 ] as const){
   const audit=await sql("SELECT rowid AS seq FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action=?",
     "tenant-a",taskId,action);
   if(response.status===200){
     assert.equal(audit.length,1);
     const assignedAt=audit[0]?.seq;
     assert.ok(typeof assignedAt==="number");
     assert.ok(assignedAt<revokedAt,"assignment committed after member was revoked");
     const replay=method==="CreateTask"
       ? await fixtureRpc(dispatch,task.createTask,admin,raceCreate)
       : await fixtureRpc(dispatch,task.updateTask,admin,raceUpdate);
     assert.equal(replay.body.receipt?.replayed,true);
   }else{
     assert.ok([400,409].includes(response.status),formatRpcFailure(response));
     assert.equal(audit.length,0,"rejected concurrent assignment wrote audit");
   }
 }
 assert.equal(deactivate.body.member?.active,false);
 assert.equal((await fixtureRpcError(dispatch,member.getMyMembership,invitedJwt,{})).status,401);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{
   ...delegatedCreate,taskId:"sk07-task-inactive",assigneeEmail:assignee,
   precondition:{idempotencyKey:"sk07-task-inactive"}
 })).status,400);
 assert.equal((await fixtureRpc(dispatch,member.listMembers,viewer,{includeInactive:true,
   page:{pageSize:100}})).body.members.some(m=>m.email===assignee&&!m.active),true);
 const workflowAudit=await sql("SELECT action FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND (subject_id=? OR subject_id=?) ORDER BY rowid",assignee,delegatedTask);
 assert.deepEqual(workflowAudit.map(r=>r.action),[
   "access.member.provision","access.member.update","activity_tasks.create",
   "activity_tasks.update","activity_tasks.reminder_scheduled","activity_tasks.complete","access.member.update"
 ]);
 console.log(JSON.stringify({event:"crm.native-sk07b-workforce",result:"PASS",
   signed_human:true,service_distinct:true,tenant_ab:true,viewer_read_all:true,
   admin_delegation_only:true,personal_task_write:true,atomic_assignment_revocation:true,
   member_receipts:true,cas:true,replay:true,revocation:true,
   task_assignee_original_domain:true,mine_signed:true,workload_before_paging:true}));

 // SK-07.C shared Task pool and original Access team aggregate.
 const teamId="sk07-team-"+randomUUID();
 const teamMember="native-viewer@example.test";
 const teamInput={teamId,name:"Native shared team "+teamId,description:"QA",
   roster:{memberEmails:[teamMember,"native-a@example.test"]},
   precondition:{idempotencyKey:"sk07-team-create-"+teamId}};
 assert.equal((await fixtureRpcError(dispatch,member.createTeam,viewer,teamInput)).status,403);
 const teamCreated=await fixtureRpc(dispatch,member.createTeam,admin,teamInput);
 assert.equal(teamCreated.status,200,formatRpcFailure(teamCreated));
 assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,teamInput)).body.receipt?.replayed,true);
 const teamRead=await fixtureRpc(dispatch,member.getTeam,viewer,{teamId,receiptIdempotencyKey:teamInput.precondition.idempotencyKey});
 assert.equal(teamRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal((await fixtureRpcError(dispatch,member.getTeam,tenantB,{teamId})).status,404);
 assert.ok((await fixtureRpc(dispatch,member.listTeams,viewer,{page:{pageSize:100}})).body.teams.some(t=>t.teamId===teamId));
 const poolId="sk07-pool-"+randomUUID();
 const poolOwner={owner:{case:"teamId" as const,value:teamId}};
 const poolCreate={taskId:poolId,accountId,subject:"Native team pool",priority:2,owner:poolOwner,
   precondition:{idempotencyKey:"sk07-pool-create-"+poolId}};
 const pool=await fixtureRpc(dispatch,task.createTask,admin,poolCreate);
 assert.equal(pool.status,200,formatRpcFailure(pool));
 assert.equal(pool.body.task?.ownerTeamId,teamId);
 assert.deepEqual(pool.body.task?.owner?.owner,poolOwner.owner);
 // A pre-existing v2 request maps to precisely the same original digest.
 const {owner:canonicalPoolOwner,...legacyPoolCreate}=poolCreate;
 assert.equal(canonicalPoolOwner.owner.value,teamId);
 assert.equal((await fixtureRpc(dispatch,task.createTask,admin,{...legacyPoolCreate,ownerTeamId:teamId})).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{...poolCreate,ownerTeamId:teamId})).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{...poolCreate,owner:{}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.listTasks,viewer,{ownerFilter:poolOwner,assignedToMe:true})).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.listTasks,viewer,{ownerFilter:poolOwner,ownerTeamId:teamId})).status,400);
 const poolBucket=(await fixtureRpc(dispatch,task.listTaskWorkload,viewer,{})).body.buckets.find(b=>b.owner?.owner.case==="teamId"&&b.owner.owner.value===teamId);
 assert.equal(poolBucket?.openCount,1n);
 const teamArchive={teamId,active:false,precondition:{expectedVersion:1n,idempotencyKey:"sk07-team-archive-"+teamId}};
 assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,teamArchive)).status,409);
 const teamList=await fixtureRpc(dispatch,task.listTasks,viewer,{ownerFilter:poolOwner,page:{pageSize:1}});
 assert.equal(teamList.body.tasks[0]?.taskId,poolId);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,{ownerFilter:poolOwner})).body.tasks.length,0);
 const claim={taskId:poolId,precondition:{expectedVersion:1n,idempotencyKey:"sk07-pool-claim-"+poolId}};
 assert.equal((await fixtureRpcError(dispatch,task.claimTask,knownService,claim)).status,403);
 assert.equal((await fixtureRpcError(dispatch,task.claimTask,tenantB,claim)).status,404);
 const claimOther={...claim,precondition:{...claim.precondition,idempotencyKey:"sk07-other-claim-"+poolId}};
 const claimResults=await Promise.all([
   race(()=>dispatchFixtureRpc(dispatch,task.claimTask,viewer,claim),bytes=>decodeTaskV2("ClaimTask",bytes)),
   race(()=>dispatchFixtureRpc(dispatch,task.claimTask,admin,claimOther),bytes=>decodeTaskV2("ClaimTask",bytes))
 ]);
 assert.deepEqual(claimResults.map(r=>r.status).sort(),[200,409]);
 const claimWinner=claimResults[0].status===200?viewer:admin;
 const wonInput=claimResults[0].status===200?claim:claimOther;
 const claimed=claimResults.find(r=>r.status===200);
 assert.ok(claimed?.body);
 assert.equal(claimed.body.task?.version,2n);
 assert.equal(claimed.body.task?.owner?.owner.case,"assigneeIdentity");
 assert.equal(claimed.body.task?.owner?.owner.value,claimWinner===viewer?teamMember:"native-a@example.test");
 assert.equal((await fixtureRpc(dispatch,task.claimTask,claimWinner,wonInput)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,viewer,{ownerTeamId:teamId})).body.tasks.length,0);
 assert.equal((await fixtureRpc(dispatch,task.completeTask,claimWinner,{taskId:poolId,
   precondition:{expectedVersion:2n,idempotencyKey:"sk07-pool-complete-"+poolId}})).status,200);
 assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,teamArchive)).status,200);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{...poolCreate,taskId:"sk07-inactive-pool-"+poolId,
   precondition:{idempotencyKey:"sk07-inactive-pool-"+poolId}})).status,400);
 assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",poolId))[0]?.n,3);
 console.log(JSON.stringify({event:"crm.native-sk07c-team-pool",result:"PASS",atomic_claim:true,tenant_ab:true,archive_guard:true,receipts:true}));

 // SK-07.C3: roster revocation and assignment/archival are actual atomic
 // owner races, not browser timing assumptions. All identities remain signed.
 {
   const id="sk07-claim-race-"+randomUUID();
   const create={teamId:id,name:"Claim revocation "+id,roster:{memberEmails:["native-a@example.test"]},
     precondition:{idempotencyKey:"create-"+id}};
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,create)).status,200);
   const owner={owner:{case:"teamId" as const,value:id}};
   const taskId="sk07-race-task-"+randomUUID();
   const input={taskId,accountId,subject:"Claim revocation QA",priority:2,owner,
     precondition:{idempotencyKey:"create-"+taskId}};
   assert.equal((await fixtureRpc(dispatch,task.createTask,admin,input)).status,200);
   const attempt={taskId,precondition:{expectedVersion:1n,idempotencyKey:"claim-"+taskId}};
   assert.equal((await fixtureRpcError(dispatch,task.claimTask,viewer,attempt)).status,403);
   const add={teamId:id,roster:{memberEmails:["native-a@example.test",teamMember]},
     precondition:{expectedVersion:1n,idempotencyKey:"add-"+id}};
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,add)).status,200);
   const remove={teamId:id,roster:{memberEmails:["native-a@example.test"]},
     precondition:{expectedVersion:2n,idempotencyKey:"remove-"+id}};
   const [claim,removed]=await Promise.all([
     race(()=>dispatchFixtureRpc(dispatch,task.claimTask,viewer,attempt),bytes=>decodeTaskV2("ClaimTask",bytes)),
     race(()=>dispatchFixtureRpc(dispatch,member.updateTeam,admin,remove),bytes=>decodeMemberV2("UpdateTeam",bytes))
   ]);
   assert.equal(removed.status,200,formatRpcFailure(removed));
   assert.ok([200,403,409].includes(claim.status),formatRpcFailure(claim));
   const current=await fixtureRpc(dispatch,task.getTask,admin,{taskId});
   assert.equal(current.status,200);
   if(claim.status===200){
     assert.equal(current.body.task?.owner?.owner.value,teamMember);
     assert.equal(current.body.task?.version,2n);
   }else{
     assert.equal(current.body.task?.owner?.owner.value,id);
     assert.equal(current.body.task?.version,1n);
     assert.equal((await fixtureRpcError(dispatch,task.claimTask,viewer,{...attempt,precondition:{...attempt.precondition,idempotencyKey:"denied-"+taskId}})).status,403);
     assert.equal((await fixtureRpc(dispatch,task.claimTask,admin,{...attempt,precondition:{...attempt.precondition,idempotencyKey:"admin-"+taskId}})).status,200);
   }
   assert.equal((await fixtureRpc(dispatch,task.completeTask,claim.status===200?viewer:admin,{taskId,
     precondition:{expectedVersion:2n,idempotencyKey:"complete-"+taskId}})).status,200);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",taskId))[0]?.n,3);
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{teamId:id,active:false,
     precondition:{expectedVersion:3n,idempotencyKey:"archive-"+id}})).status,200);

   const raceTeam="sk07-assignment-race-"+randomUUID();
   const raceCreate={teamId:raceTeam,name:"Assignment archive "+raceTeam,roster:{memberEmails:["native-a@example.test"]},
     precondition:{idempotencyKey:"create-"+raceTeam}};
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,raceCreate)).status,200);
   const raceTask="sk07-assignment-task-"+randomUUID();
   const raceInput={taskId:raceTask,accountId,subject:"Assignment archive QA",priority:TaskPriority.NORMAL,
     owner:{owner:{case:"teamId" as const,value:raceTeam}},precondition:{idempotencyKey:"create-"+raceTask}};
   const archive={teamId:raceTeam,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+raceTeam}};
   const [assigned,archived]=await Promise.all([
     race(()=>dispatchFixtureRpc(dispatch,task.createTask,admin,raceInput),bytes=>decodeTaskV2("CreateTask",bytes)),
     race(()=>dispatchFixtureRpc(dispatch,member.updateTeam,admin,archive),bytes=>decodeMemberV2("UpdateTeam",bytes))
   ]);
   assert.equal(Number(assigned.status===200)+Number(archived.status===200),1,
     "assignment and archive cannot both commit");
   if(assigned.status===200){
     assert.equal(archived.status,409);
     assert.equal((await fixtureRpc(dispatch,task.claimTask,admin,{taskId:raceTask,precondition:{expectedVersion:1n,idempotencyKey:"claim-"+raceTask}})).status,200);
     assert.equal((await fixtureRpc(dispatch,task.completeTask,admin,{taskId:raceTask,precondition:{expectedVersion:2n,idempotencyKey:"complete-"+raceTask}})).status,200);
     assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,archive)).status,200);
   }else{
     assert.ok([400,409].includes(assigned.status),formatRpcFailure(assigned));
     assert.equal((await fixtureRpcError(dispatch,task.getTask,admin,{taskId:raceTask})).status,404);
   }
   const team=await fixtureRpc(dispatch,member.getTeam,viewer,{teamId:raceTeam});
   assert.equal(team.body.team?.active,false);
   console.log(JSON.stringify({event:"crm.native-sk07c3-owner-oneof",result:"PASS",shared_owner:true,
     legacy_digest:true,mixed_rejected:true,nonmember_denied:true,claim_remove_race:true,assignment_archive_race:true}));
 }

}

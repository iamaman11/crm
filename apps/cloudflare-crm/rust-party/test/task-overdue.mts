import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import type { ListTasksRequest } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { collectFixturePages, fixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type OverdueFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string;
 accountNextId: string;
 admin: string;
 viewer: string;
 tenantB: string;
};
export async function verifyOverdueTasks({dispatch,sql,accountId,accountNextId,admin,viewer,tenantB}: OverdueFixture) {
 const task=TaskService.method;
 // The same original Task owner filters overdue instants BEFORE the keyset
 // LIMIT, preserving the typed status/priority/account/search/Deal filters.
 const dueSeeds=[
   ["task-overdue-a-"+randomUUID(),"Native overdue A",2,1600000000n],
   ["task-overdue-z-"+randomUUID(),"Native overdue Z",4,1640995200n],
   ["task-due-future-"+randomUUID(),"Native due future",4,1900000000n],
   ["task-no-due-"+randomUUID(),"Native unscheduled",2,null],
   ["task-overdue-completed-"+randomUUID(),"Native already completed",2,1600000000n]
 ] as const;
 for(const [taskId,subject,priority,due] of dueSeeds){
   const created=await fixtureRpc(dispatch,task.createTask,admin,{taskId,accountId:accountNextId,subject,priority,
     ...(due===null?{}:{dueAt:{seconds:due,nanos:0}}),
     precondition:{idempotencyKey:"candidate-overdue-create-"+taskId}});
   assert.equal(created.status,200,formatRpcFailure(created));
 }
 const [dueA,dueZ,dueFuture,dueNone,dueComplete]=dueSeeds.map(x=>x[0]);
 assert.ok(dueA && dueZ && dueFuture && dueNone && dueComplete);
 const completeSeed=await fixtureRpc(dispatch,task.completeTask,admin,{taskId:dueComplete,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-overdue-complete"}});
 assert.equal(completeSeed.status,200,formatRpcFailure(completeSeed));
 const overdueAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 const collectTasks=(input: Partial<Pick<ListTasksRequest, "overdueOnly" | "accountId" | "priority" | "status" | "search" | "dealId">>)=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,task.listTasks,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:result.body.tasks,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const overdueList=async(input: Partial<Pick<ListTasksRequest, "overdueOnly" | "accountId" | "priority" | "status" | "search" | "dealId">>={})=>(await collectTasks(
   {overdueOnly:true,...input})).map(x=>x.taskId);
 assert.deepEqual((await overdueList()).sort(),[dueA,dueZ].sort());
 assert.deepEqual(await overdueList({accountId:accountNextId}),[dueA,dueZ].sort());
 assert.deepEqual(await overdueList({accountId}),[]);
 assert.deepEqual(await overdueList({priority:4}),[dueZ]);
 assert.deepEqual(await overdueList({status:2}),[]);
 assert.deepEqual(await overdueList({search:"Native overdue A"}),[dueA]);
 assert.deepEqual(await overdueList({dealId:"missing-deal"}),[]);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,{overdueOnly:true})).body.tasks.length,0);
 assert.deepEqual((await collectTasks(
   {overdueOnly:true,accountId:accountNextId})).map(x=>x.taskId),
   (await collectTasks(
   {overdueOnly:true,accountId:accountNextId})).map(x=>x.taskId));
 const viewerOverdue=await fixtureRpc(dispatch,task.listTasks,viewer,{overdueOnly:true,accountId:accountNextId});
 const adminOverdue=await fixtureRpc(dispatch,task.listTasks,admin,{overdueOnly:true,accountId:accountNextId});
 assert.deepEqual(viewerOverdue.body,adminOverdue.body);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   overdueAuditBefore,"Overdue owner reads never mutate audit");
 assert.equal((await fixtureRpc(dispatch,task.getTask,viewer,{taskId:dueFuture})).status,200);
 assert.equal((await fixtureRpc(dispatch,task.getTask,viewer,{taskId:dueNone})).status,200);
 const resolvedOverdue=await fixtureRpc(dispatch,task.completeTask,admin,{taskId:dueA,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-overdue-resolved"}});
 assert.equal(resolvedOverdue.status,200);
 assert.deepEqual(await overdueList(),[dueZ],"Completion removes overdue Task from server results");
 console.log(JSON.stringify({event:"crm.native-task-overdue-original-owner",result:"PASS",
   owner:"crm.activities",server_time:true,original_cursor:true,tenant:true,viewer:true,
   status:true,priority:true,account:true,search:true,deal:true,no_read_audit:true}));

}

import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import type { ListTasksRequest } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { collectFixturePages, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type DueFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountNextId: string;
 admin: string;
 viewer: string;
 tenantB: string;
};
type DueFilter = Partial<Pick<ListTasksRequest, "status" | "sortByDue" | "search" | "withoutDueOnly">> & {
 dueFrom?: Pick<NonNullable<ListTasksRequest["dueFrom"]>, "seconds" | "nanos">;
 dueBefore?: Pick<NonNullable<ListTasksRequest["dueBefore"]>, "seconds" | "nanos">;
 page?: {pageSize: number};
};
export async function verifyTaskDueOrdering({dispatch,sql,accountNextId,admin,viewer,tenantB}: DueFixture) {
 const task=TaskService.method;
 const collectTasks=({status,...input}: DueFilter)=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,task.listTasks,viewer,{...input,...(status===undefined?{}:{status}),page:{pageSize:1,pageToken}});
   return {items:result.body.tasks,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 // SK-06: one original Task owner filters due instants before LIMIT, using a
 // different (due-nanos, id) keyset. Existing ID-sorted Task views are unchanged.
 const sk06Seeds=[
   ["sk06-task-z-"+randomUUID(),"SK06 Workday earliest",1800100000n],
   ["sk06-task-a-"+randomUUID(),"SK06 Workday same A",1800200000n],
   ["sk06-task-c-"+randomUUID(),"SK06 Workday same C",1800200000n],
   ["sk06-task-undated-"+randomUUID(),"SK06 Workday undated",null],
   ["sk06-task-done-"+randomUUID(),"SK06 Workday completed",1800150000n]
 ] as const;
 for(const [taskId,subject,due] of sk06Seeds){
   const made=await fixtureRpc(dispatch,task.createTask,admin,{
     taskId,subject,accountId:accountNextId,priority:2,
     ...(due===null?{}:{dueAt:{seconds:due,nanos:0}}),
     precondition:{idempotencyKey:"sk06-task-create-"+taskId}
   });
   assert.equal(made.status,200,formatRpcFailure(made));
 }
 const [sk06Early,sk06SameA,sk06SameC,sk06Undated,sk06Done]=sk06Seeds.map(x=>x[0]);
 assert.ok(sk06Early && sk06SameA && sk06SameC && sk06Undated && sk06Done);
 const sk06Completed=await fixtureRpc(dispatch,task.completeTask,admin,{taskId:sk06Done,
   precondition:{expectedVersion:1n,idempotencyKey:"sk06-task-complete-"+sk06Done}});
 assert.equal(sk06Completed.status,200,formatRpcFailure(sk06Completed));
 const sk06ReadAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 const sk06DueFilter={status:1,sortByDue:true,search:"SK06 Workday ",
   dueFrom:{seconds:1800100000n,nanos:0},
   dueBefore:{seconds:1800300000n,nanos:0},page:{pageSize:1}};
 const sk06DueTasks=await collectTasks(sk06DueFilter);
 assert.deepEqual(sk06DueTasks.map(task=>task.taskId),
   [sk06Early,sk06SameA,sk06SameC],
   "Task due keyset must order by exact nanoseconds then ID before each LIMIT");
 assert.deepEqual((await collectTasks({
   status:1,withoutDueOnly:true,search:"SK06 Workday ",page:{pageSize:1}
 })).map(task=>task.taskId),[sk06Undated]);
 assert.deepEqual((await collectTasks({
   status:1,sortByDue:true,
   dueFrom:{seconds:1800200000n,nanos:0},
   dueBefore:{seconds:1800200001n,nanos:0},search:"SK06 Workday ",page:{pageSize:1}
 })).map(task=>task.taskId),[sk06SameA,sk06SameC]);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,sk06DueFilter)).body.tasks.length,0);
 const sk06Viewer=await fixtureRpc(dispatch,task.listTasks,viewer,{...sk06DueFilter,page:{pageSize:2}});
 const sk06Admin=await fixtureRpc(dispatch,task.listTasks,admin,{...sk06DueFilter,page:{pageSize:2}});
 assert.equal(sk06Viewer.status,200);
 assert.deepEqual(sk06Viewer.body,sk06Admin.body,"viewer must see same authorized Task fields");
 for(const bad of [
   {...sk06DueFilter,page:{pageSize:2,pageToken:"invalid"}},
   {...sk06DueFilter,dueFrom:{seconds:1800300000n,nanos:0}},
   {...sk06DueFilter,sortByDue:false,withoutDueOnly:true}
 ]){
   assert.equal((await fixtureRpcError(dispatch,task.listTasks,viewer,bad)).status,400,
     "SK-06 invalid cursor/range/undated mix must fail closed");
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   sk06ReadAuditBefore,"SK-06 Task read filtering must never append audit");

}

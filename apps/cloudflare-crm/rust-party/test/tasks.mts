import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import type { ListTasksRequest } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, fixtureRpcError, fixtureRpcPath } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { binaryTaskV2 } from "../../../../packages/client/test/cloudflare-binary-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type TaskFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string;
 accountNextId: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
};
export async function verifyTasks({dispatch,sql,accountId,accountNextId,admin,viewer,tenantB,knownService}: TaskFixture) {
 const task=TaskService.method;
 const collectTasks=(input: Partial<Pick<ListTasksRequest, "accountId" | "priority" | "status" | "search">> & {page?: {pageSize: number}}={})=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,task.listTasks,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:result.body.tasks,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const taskOther="task-aaa-other-"+randomUUID();
 const taskMid="task-mid-ours-"+randomUUID();
 const taskLast="task-zzz-ours-"+randomUUID();
 const taskInput=(taskId: string,accountId: string,sequence: string)=>({
   taskId,accountId,subject:"Check original Account "+sequence,
   description:"Signed actor task "+sequence,priority:2,
   dueAt:{seconds:1860000000n,nanos:987654321},
   reminderAt:{seconds:1850000000n,nanos:123456789},
   precondition:{idempotencyKey:"candidate-task-create-"+sequence}
 });
 const {description: _description,...taskOtherBase}=taskInput(taskOther,accountNextId,"other");
 const taskOtherInput={...taskOtherBase,priority:4};
 const taskMidInput=taskInput(taskMid,accountId,"mid");
 const taskLastInput=taskInput(taskLast,accountId,"last");
 assert.equal((await fixtureRpcError(dispatch,task.createTask,null,taskMidInput)).status,401);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,viewer,taskMidInput)).status,403);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{...taskMidInput,
   priority:0,precondition:{idempotencyKey:"candidate-task-invalid-priority"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{...taskMidInput,
   accountId:"missing-account",
   precondition:{idempotencyKey:"candidate-task-missing-account"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,tenantB,{...taskMidInput,
   precondition:{idempotencyKey:"candidate-task-foreign-account"}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{...taskMidInput,
   reminderAt:{seconds:1700000000n,nanos:0},
   precondition:{idempotencyKey:"candidate-task-past-reminder"}})).status,400);
 const taskCreated=await fixtureRpc(dispatch,task.createTask,admin,taskMidInput);
 assert.equal(taskCreated.status,200,formatRpcFailure(taskCreated));
 assert.equal(taskCreated.body.task?.taskId,taskMid);
 assert.equal(taskCreated.body.task?.accountId,accountId);
 assert.equal(taskCreated.body.task?.status,1);
 assert.equal(taskCreated.body.task?.priority,2);
 assert.equal(taskCreated.body.task?.description,"Signed actor task mid");
 assert.equal(taskCreated.body.task?.dueAt?.nanos,987654321);
 assert.equal(taskCreated.body.task?.reminderAt?.nanos,123456789);
 assert.equal(taskCreated.body.receipt?.resourceVersion,1n);
 assert.equal(taskCreated.body.receipt?.requestId,taskCreated.headers.get("x-request-id"));
 const taskReplay=await fixtureRpc(dispatch,task.createTask,admin,taskMidInput);
 assert.equal(taskReplay.status,200);
 assert.equal(taskReplay.body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch,task.createTask,admin,{
   ...taskMidInput,subject:"Different same key"})).status,409);
 assert.equal((await fixtureRpcError(dispatch,task.getTask,tenantB,{taskId:taskMid})).status,404);
 assert.equal((await fixtureRpc(dispatch,task.getTask,viewer,{taskId:taskMid}))
   .body.task?.accountId,accountId);
 const taskOtherCreated=await fixtureRpc(dispatch,task.createTask,admin,taskOtherInput);
 assert.equal(taskOtherCreated.status,200,formatRpcFailure(taskOtherCreated));
 assert.equal(taskOtherCreated.body.task?.description,"",
   "optional CreateTask.description absent is canonical empty Task.description");
 assert.equal((await fixtureRpc(dispatch,task.createTask,admin,taskLastInput)).status,200);
 const taskAuditBeforeFilters=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 // Original Task subject search: signed, before paging, composed with existing filters.
 assert.deepEqual((await collectTasks({search:"Check original Account mid"})).map(t=>t.taskId),[taskMid]);
 assert.deepEqual((await collectTasks({search:"Check original Account",page:{pageSize:1}})).map(t=>t.taskId),[taskOther,taskMid,taskLast].sort());
 assert.deepEqual((await collectTasks({search:"Check original Account",priority:2,accountId})).map(t=>t.taskId),[taskMid,taskLast]);
 assert.deepEqual((await collectTasks({search:"task-mid-ours"})).map(t=>t.taskId),[taskMid]);
 assert.equal((await collectTasks({search:"Signed actor task mid"})).length,0,"description must not broaden Task subject search");
 assert.equal((await collectTasks({search:"check original account"})).length,0,"search follows case-sensitive UI contract");
 assert.equal((await collectTasks({search:"' OR 1=1 --"})).length,0,"search never interpolates SQL");
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,{search:"Check original Account"})).body.tasks.length,0);
 const namedTasks=(await fixtureRpc(dispatch,task.listTasks,viewer,{search:"Check original Account mid"})).body;
 assert.equal(namedTasks.tasks[0]?.taskId,taskMid);
 const namedAccount=(await sql("SELECT search_name FROM crm_account_records WHERE tenant=? AND id=?","tenant-a",accountId))[0];
 assert.deepEqual(Object.entries(namedTasks.accountNames),[[accountId,namedAccount?.search_name]]);
 assert.equal((await fixtureRpcError(dispatch,task.listTasks,viewer,{search:"x".repeat(201)})).status,400);
 assert.deepEqual((await collectTasks({priority:2})).map(t=>t.taskId),[taskMid,taskLast]);
 assert.deepEqual((await collectTasks({priority:4})).map(t=>t.taskId),[taskOther]);
 assert.equal((await collectTasks({priority:4,accountId})).length,0);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,{priority:4})).body.tasks.length,0);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,viewer,{priority:4})).body.tasks[0]?.taskId,taskOther);
 for(const priority of [0,99]){
   const invalid=await fixtureRpcError(dispatch,task.listTasks,viewer,{priority});
   assert.equal(invalid.status,400);assert.equal(invalid.body.code,"invalid_argument");
   assert.equal(record(invalid.body).message,"invalid TaskPriority filter");
   assert.ok(invalid.headers.get("x-request-id"));
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,taskAuditBeforeFilters,
   "Priority filters and invalid read requests must not create audit writes");
 const taskFilter=await collectTasks(
   {accountId,status:1});
 assert.deepEqual(taskFilter.map(t=>t.taskId),[taskMid,taskLast]);
 assert.equal((await collectTasks(
   {accountId:"missing-account"})).length,0);
 assert.equal((await fixtureRpc(dispatch,task.listTasks,tenantB,{
   accountId,page:{pageSize:1}})).body.tasks.length,0);
 assert.equal((await fixtureRpcError(dispatch,task.listTasks,viewer,{
   page:{pageSize:101}})).status,400);
 // UpdateTask: original Rust invariants and atomic state/receipt/audit.
 const updateKey="candidate-task-update-other";
 const baseUpdate={taskId:taskOther,precondition:{expectedVersion:1n,idempotencyKey:updateKey}};
 const taskRow=async()=>(await sql("SELECT state,version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"'"))[0];
 const taskAudits=async()=>(await sql("SELECT action,request_id FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+taskOther+"' ORDER BY rowid"));
 const beforeTask=(await taskRow()),beforeTaskAudit=(await taskAudits());
 const patchSubject=(value: string)=>({operation:{case:"set" as const,value}});
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,null,{...baseUpdate,subject:patchSubject("No auth")})).status,401);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,viewer,{...baseUpdate,subject:patchSubject("No permission")})).status,403);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,tenantB,{...baseUpdate,subject:patchSubject("Wrong tenant")})).status,404);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,admin,baseUpdate)).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,admin,{...baseUpdate,subject:{operation:{case:"clear",value:true}}})).status,400);
 // Field 4 is deliberately encoded as an unknown TaskPriority enum value.
 const unknownPriority=await dispatch(fixtureRpcPath(task.updateTask),{method:"POST",headers:{
   "content-type":"application/proto","connect-protocol-version":"1","cf-access-jwt-assertion":admin},
   body:new Uint8Array([...binaryTaskV2("UpdateTask",baseUpdate),0x20,99])});
 assert.equal(unknownPriority.status,400);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,admin,{...baseUpdate,priority:0})).status,400);
 const rejected=await fixtureRpcError(dispatch,task.updateTask,admin,{...baseUpdate,
   subject:patchSubject("Must not persist"),description:{operation:{case:"clear",value:true}},
   priority:1,
   dueAt:{operation:{case:"set",value:{seconds:1840000000n,nanos:0}}}});
 assert.equal(rejected.status,400,formatRpcFailure(rejected));
 assert.deepEqual((await taskRow()),beforeTask,"late rejection must not persist an earlier field change");
 assert.deepEqual((await taskAudits()),beforeTaskAudit,"rejection must not create audit");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a' AND key='"+updateKey+"'"))[0]?.n,0);
 const updateTask={...baseUpdate,subject:patchSubject("Edited original Task"),
   description:{operation:{case:"set" as const,value:"Edited through Connect"}},
   priority:3,dueAt:{operation:{case:"clear" as const,value:true}}};
 const updatedTask=await fixtureRpc(dispatch,task.updateTask,knownService,updateTask);
 assert.equal(updatedTask.status,200,formatRpcFailure(updatedTask));
 assert.ok(updatedTask.body.task);
 assert.ok(updatedTask.body.receipt);
 assert.equal(updatedTask.body.task.subject,"Edited original Task");
 assert.equal(updatedTask.body.task.description,"Edited through Connect");
 assert.equal(updatedTask.body.task.priority,3);
 assert.equal(updatedTask.body.task.dueAt,undefined);
 assert.equal(updatedTask.body.task.version,2n);
 assert.equal(updatedTask.body.receipt.resourceVersion,2n);
 assert.equal(updatedTask.body.receipt.requestId,updatedTask.headers.get("x-request-id"));
 assert.deepEqual((await taskAudits()).map(x=>x.action),["activity_tasks.create","activity_tasks.update"]);
 assert.equal((await taskAudits())[1]?.request_id,updatedTask.headers.get("x-request-id"));
 assert.deepEqual((await collectTasks({priority:3,accountId:accountNextId})).map(x=>x.taskId),[taskOther]);
 assert.equal((await collectTasks({priority:4,accountId:accountNextId})).length,0);
 const replayTask=await fixtureRpc(dispatch,task.updateTask,admin,updateTask);
 assert.equal(replayTask.status,200);
 assert.ok(replayTask.body.receipt);
 assert.equal(replayTask.body.receipt.replayed,true);
 assert.equal((await taskAudits()).length,2,"replay cannot duplicate audit");
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,admin,{...updateTask,subject:patchSubject("Same key, different payload")})).status,409);
 assert.equal((await fixtureRpcError(dispatch,task.updateTask,admin,{...updateTask,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-update-stale"}})).status,409);
 assert.equal((await taskAudits()).length,2,"stale version must not create audit");
 const secondTaskEdit=await fixtureRpc(dispatch,task.updateTask,admin,{taskId:taskOther,
   description:{operation:{case:"clear",value:true}},priority:2,
   dueAt:{operation:{case:"set",value:{seconds:1870000000n,nanos:987654321}}},
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-task-update-second"}});
 assert.equal(secondTaskEdit.status,200,formatRpcFailure(secondTaskEdit));
 assert.ok(secondTaskEdit.body.task?.dueAt);
 assert.equal(secondTaskEdit.body.task.description,"");
 assert.equal(secondTaskEdit.body.task.dueAt.nanos,987654321);
 assert.equal(secondTaskEdit.body.task.version,3n);
 assert.deepEqual((await taskAudits()).map(x=>x.action),["activity_tasks.create","activity_tasks.update","activity_tasks.update"]);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a' AND key IN ('candidate-task-create-other','candidate-task-update-other','candidate-task-update-second')"))[0]?.n,3);
 const taskComplete={taskId:taskMid,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-complete-mid"}};
 assert.equal((await fixtureRpcError(dispatch,task.completeTask,viewer,taskComplete)).status,403);
 assert.equal((await fixtureRpcError(dispatch,task.completeTask,admin,{
   ...taskComplete,precondition:{expectedVersion:0n,
     idempotencyKey:"candidate-task-invalid-version"}})).status,400);
 const completed=await fixtureRpc(dispatch,task.completeTask,knownService,taskComplete);
 assert.equal(completed.status,200,formatRpcFailure(completed));
 assert.equal(completed.body.changed,true);
 assert.equal(completed.body.task?.status,2);
 assert.equal(completed.body.task?.version,2n);
 assert.ok(completed.body.task?.completedAt);
 assert.ok(completed.body.task.completedAt.seconds>0n);
 assert.equal(completed.body.task?.reminderAt,undefined);
 assert.equal(completed.body.receipt?.resourceVersion,2n);
 const completedReplay=await fixtureRpc(dispatch,task.completeTask,admin,taskComplete);
 assert.equal(completedReplay.status,200,formatRpcFailure(completedReplay));
 assert.equal(completedReplay.body.receipt?.replayed,true);
 assert.equal(completedReplay.body.task?.version,2n);
 const beforeNoop=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+taskMid+"'"))[0]?.n;
 const unchangedTask=await fixtureRpc(dispatch,task.completeTask,admin,{
   taskId:taskMid,
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-task-unchanged"}});
 assert.equal(unchangedTask.status,200,formatRpcFailure(unchangedTask));
 assert.equal(unchangedTask.body.changed,false);
 assert.equal(unchangedTask.body.receipt,undefined,
   "unchanged completion must not claim persisted D1 receipt");
 assert.equal((await fixtureRpcError(dispatch,task.completeTask,admin,{
   ...taskComplete,precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-task-stale"}})).status,409);
 const lastComplete=await fixtureRpc(dispatch,task.completeTask,admin,{
   taskId:taskLast,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-complete-last"}});
 assert.equal(lastComplete.status,200,formatRpcFailure(lastComplete));
 assert.equal(lastComplete.body.changed,true);
 const completedTasks=await collectTasks(
   {accountId,status:2});
 assert.deepEqual(completedTasks.map(t=>t.taskId),[taskMid,taskLast]);
 assert.deepEqual((await collectTasks({accountId,status:2,priority:2})).map(t=>t.taskId),[taskMid,taskLast]);
 assert.equal((await collectTasks(
   {accountId,status:1})).length,0);
 assert.equal((await collectTasks(
   {accountId:accountNextId,status:1})).length,1);
 assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='tenant-a' "+
   "AND id='"+taskMid+"'"))[0]?.version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+taskMid+"'"))[0]?.n,beforeNoop);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts "+
   "WHERE tenant='tenant-a' AND key IN ('candidate-task-create-mid',"+
   "'candidate-task-complete-mid')"))[0]?.n,2);

  assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+taskOther+"' ORDER BY rowid")),[
    {action:"activity_tasks.create",actor:"native-a@example.test",
      request_id:taskOtherCreated.headers.get("x-request-id")},
    {action:"activity_tasks.update",actor:"service:ci-fixture.access",
      request_id:updatedTask.headers.get("x-request-id")},
    {action:"activity_tasks.update",actor:"native-a@example.test",
      request_id:secondTaskEdit.headers.get("x-request-id")}
  ],"Task update replay keeps first signed author and trace");
  assert.deepEqual((await sql("SELECT action,actor FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+taskMid+"' ORDER BY rowid")),[
    {action:"activity_tasks.create",actor:"native-a@example.test"},
    {action:"activity_tasks.complete",actor:"service:ci-fixture.access"}
  ],"Task complete and update stay distinct; no-op produces no extra audit");
  const taskActorSnapshot=async()=>(await sql("SELECT "+
    "(SELECT state FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"') AS state,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"));
  const beforeTaskFault=(await taskActorSnapshot());
  (await sql("CREATE TRIGGER fixture_fail_task_actor BEFORE INSERT ON crm_audit "+
    "WHEN NEW.subject_id='"+taskOther+"' BEGIN SELECT RAISE(ABORT,'fixture task audit'); END;"));
  try{
    const failed=await fixtureRpcError(dispatch,task.updateTask,admin,{taskId:taskOther,
      subject:patchSubject("Must not persist"),
      precondition:{expectedVersion:3n,idempotencyKey:"candidate-task-actor-failed"}});
    assert.equal(failed.status,503);
    assert.equal(failed.body.code,"unavailable");
    const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
    assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
    assert.equal(detail.domainCode,"ACTIVITIES_TASK_WRITE_UNCONFIRMED");
    assert.equal(detail.requestId,failed.headers.get("x-request-id"));
    assert.deepEqual((await taskActorSnapshot()),beforeTaskFault);
  }finally{await sql("DROP TRIGGER fixture_fail_task_actor");}
  console.log(JSON.stringify({event:"crm.native-r2a-task-actor-audit",result:"PASS",
    update_complete_actor:true,no_op:true,failed_audit_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-activities-tasks-v2",
   result:"PASS",rpc_methods:["CreateTask","UpdateTask","ScheduleReminder","CompleteTask","GetTask","ListTasks"],
   generated_binary:true,original_owner:"crm.activities",signed_access:true,
   tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,account_fk:true,d1_cas:true,
   no_op_without_write:true,replay:true,d1_receipts:true,d1_audit:true,
   published_active:true}));


 return {taskOther,taskMid,taskLast,taskMidInput,taskCreated,taskActorSnapshot,taskAudits};
}

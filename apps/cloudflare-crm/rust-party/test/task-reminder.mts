import assert from "node:assert/strict";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type ReminderFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 taskActorSnapshot: () => Promise<ReadonlyArray<Record<string, unknown>>>;
 taskAudits: () => Promise<ReadonlyArray<Record<string, unknown>>>;
 taskOther: string;
 taskMid: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
};
export async function verifyTaskReminder({dispatch,sql,taskActorSnapshot,taskAudits,taskOther,taskMid,admin,viewer,tenantB,knownService}: ReminderFixture) {
 const task=TaskService.method;
 // M1-08: restore the ORIGINAL crm-activities Task::schedule_reminder through
 // the existing generated TaskService, one Task table and atomic D1 trigger.
 const scheduleTask={taskId:taskOther,reminderAt:{seconds:1860000000n,nanos:123456789},
  precondition:{expectedVersion:3n,idempotencyKey:"candidate-task-reminder-first"}};
 const beforeReminder=await taskActorSnapshot();
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,null,scheduleTask)).status,401);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,viewer,scheduleTask)).status,403);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,tenantB,scheduleTask)).status,404);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
  taskId:taskOther,precondition:{expectedVersion:3n,idempotencyKey:"candidate-reminder-missing"}
 })).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
  ...scheduleTask,reminderAt:{seconds:1700000000n,nanos:0},
  precondition:{expectedVersion:3n,idempotencyKey:"candidate-reminder-past"}
 })).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
  ...scheduleTask,reminderAt:{seconds:1880000000n,nanos:0},
  precondition:{expectedVersion:3n,idempotencyKey:"candidate-reminder-after-due"}
 })).status,400);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
  ...scheduleTask,precondition:{expectedVersion:2n,idempotencyKey:"candidate-reminder-stale"}
 })).status,409);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
  taskId:taskMid,reminderAt:scheduleTask.reminderAt,
  precondition:{expectedVersion:2n,idempotencyKey:"candidate-reminder-completed"}
 })).status,409);
 assert.deepEqual(await taskActorSnapshot(),beforeReminder,
  "Invalid/denied/stale/completed reminder must not modify D1, audit or receipts");
 const scheduled=await fixtureRpc(dispatch,task.scheduleReminder,admin,scheduleTask);
 assert.equal(scheduled.status,200,formatRpcFailure(scheduled));
 assert.equal(scheduled.body.changed,true);
 assert.equal(scheduled.body.task?.version,4n);
 assert.equal(scheduled.body.task?.reminderAt?.seconds,1860000000n);
 assert.equal(scheduled.body.task?.reminderAt?.nanos,123456789);
 assert.equal(scheduled.body.receipt?.resourceVersion,4n);
 assert.equal(scheduled.body.receipt?.requestId,scheduled.headers.get("x-request-id"));
 assert.equal((await taskAudits()).at(-1)?.action,"activity_tasks.reminder_scheduled");
 assert.equal((await taskAudits()).at(-1)?.request_id,scheduled.headers.get("x-request-id"));
 const reminderReplayed=await fixtureRpc(dispatch,task.scheduleReminder,knownService,scheduleTask);
 assert.equal(reminderReplayed.status,200,formatRpcFailure(reminderReplayed));
 assert.equal(reminderReplayed.body.receipt?.replayed,true);
 assert.equal(reminderReplayed.body.receipt?.resourceVersion,4n);
 assert.equal((await taskAudits()).filter(x=>x.action==="activity_tasks.reminder_scheduled").length,1);
 assert.equal((await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
  ...scheduleTask,reminderAt:{seconds:1860000001n,nanos:0}
 })).status,409,"same key with different reminder intent must reject");
 const beforeNoChange=await taskActorSnapshot();
 const unchangedReminder=await fixtureRpc(dispatch,task.scheduleReminder,admin,{
  ...scheduleTask,precondition:{expectedVersion:4n,idempotencyKey:"candidate-reminder-same"}
 });
 assert.equal(unchangedReminder.status,200,formatRpcFailure(unchangedReminder));
 assert.equal(unchangedReminder.body.changed,false);
 assert.equal(unchangedReminder.body.receipt,undefined);
 assert.equal(unchangedReminder.body.task?.version,4n);
 assert.deepEqual(await taskActorSnapshot(),beforeNoChange,
  "same reminder time must not create a version, receipt or audit");
 const movedReminder=await fixtureRpc(dispatch,task.scheduleReminder,knownService,{
  ...scheduleTask,reminderAt:{seconds:1861000000n,nanos:222333444},
  precondition:{expectedVersion:4n,idempotencyKey:"candidate-reminder-reschedule"}
 });
 assert.equal(movedReminder.status,200,formatRpcFailure(movedReminder));
 assert.equal(movedReminder.body.task?.version,5n);
 assert.equal(movedReminder.body.task?.reminderAt?.seconds,1861000000n);
 assert.equal((await taskAudits()).filter(x=>x.action==="activity_tasks.reminder_scheduled").length,2);
 assert.equal((await fixtureRpc(dispatch,task.getTask,viewer,{taskId:taskOther})).body.task?.version,5n);
 assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"'"))[0]?.version,5);
 const beforeReminderFault=await taskActorSnapshot();
 await sql("CREATE TRIGGER fixture_fail_reminder_audit BEFORE INSERT ON crm_audit "+
  "WHEN NEW.subject_id='"+taskOther+"' AND NEW.action='activity_tasks.reminder_scheduled' "+
  "BEGIN SELECT RAISE(ABORT,'fixture reminder audit'); END;");
 try{
  const denied=await fixtureRpcError(dispatch,task.scheduleReminder,admin,{
    ...scheduleTask,reminderAt:{seconds:1862000000n,nanos:0},
    precondition:{expectedVersion:5n,idempotencyKey:"candidate-reminder-audit-rollback"}
  });
  assert.equal(denied.status,503,formatRpcFailure(denied));
  assert.equal(record(denied.body).message,"ACTIVITIES_TASK_WRITE_UNCONFIRMED");
  assert.deepEqual(await taskActorSnapshot(),beforeReminderFault,
    "Late reminder audit failure must roll back Task state and original receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_reminder_audit");}
 console.log(JSON.stringify({event:"crm.native-m1-08-reminder",result:"PASS",
  domain:"crm.activities.Task::schedule_reminder",post_create:true,reschedule:true,
  no_op_without_write:true,completed_denied:true,after_due_denied:true,
  stale_cas:true,replay:true,tenant_ab:true,receipt_audit_atomic:true}));

}

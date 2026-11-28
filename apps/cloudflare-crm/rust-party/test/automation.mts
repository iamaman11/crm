import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { AutomationService } from "../../../../packages/client/gen/crm/activities/v2/automation_pb.js";
import { InteractionService } from "../../../../packages/client/gen/crm/activities/v2/interaction_pb.js";
import { TaskService } from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { dealCreationInput } from "../../../../packages/client/test/cloudflare-deal-fixture.ts";
import { MemberService } from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import { fixtureRpc, fixtureRpcError, fixtureRpcRace } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type AutomationFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: (string | number)[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string; contactId: string; dealId: string; admin: string; viewer: string; tenantB: string; knownService: string;
 runScheduled: (input:{cron:string;scheduledTime:Date})=>Promise<{outcome:string;noRetry:boolean}>;
};
export async function verifyAutomation({dispatch,sql,accountId,contactId,dealId,admin,viewer,tenantB,knownService,runScheduled}: AutomationFixture) {
 const automation=AutomationService.method;
 const interaction=InteractionService.method;
 const tasks=TaskService.method;
 const member=MemberService.method;
 // SK-08A: preserve the signed manual Interaction workflow while scheduled
 // Deal and Task sources use the same durable run ledger and original Task owner.
 {
   const suffix=randomUUID(),ruleId="automation-rule-"+suffix;
   const sourceId="automation-source-"+suffix;
   const person="automation-person-"+suffix+"@example.test";
   const rkey=(label: string,version?: bigint)=>({idempotencyKey:"automation-"+label+"-"+suffix,
     ...(version===undefined?{}:{expectedVersion:version})});
   assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{
     email:person,role:1,precondition:rkey("provision")
   })).status,200);
   const put={ruleId,trigger:1,enabled:true,
     taskSubject:"SK08A customer follow-up",assigneeEmail:person,
     dueAfterSeconds:3600n,precondition:rkey("rule")};
   assert.equal((await fixtureRpcError(dispatch,automation.putAutomationRule,viewer,put)).status,403);
   assert.equal((await fixtureRpcError(dispatch,automation.putAutomationRule,admin,{
     ...put,trigger:0,precondition:rkey("unsupported")
   })).status,400);
   const created=await fixtureRpc(dispatch,automation.putAutomationRule,admin,put);
   assert.equal(created.status,200,formatRpcFailure(created));
   assert.equal(created.body.rule?.version,1n);
   assert.equal((await fixtureRpc(dispatch,automation.putAutomationRule,admin,put)).body.receipt?.replayed,true);
   assert.equal((await fixtureRpcError(dispatch,automation.putAutomationRule,admin,{
     ...put,taskSubject:"Another intent, same key"
   })).status,409);
   assert.equal((await fixtureRpcError(dispatch,automation.getAutomationRule,tenantB,{ruleId})).status,404);
   const read=await fixtureRpc(dispatch,automation.getAutomationRule,viewer,{
     ruleId,receiptIdempotencyKey:put.precondition.idempotencyKey
   });
   assert.equal(read.status,200);
   assert.deepEqual(read.body.rule,created.body.rule);
   assert.equal(read.body.confirmedMutation?.resourceVersion,1n);
   const all=await fixtureRpc(dispatch,automation.listAutomationRules,viewer,{page:{pageSize:100}});
   assert.equal(all.status,200);
   assert.ok(all.body.rules.some(value=>value.ruleId===ruleId));
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_rule_versions WHERE tenant=? AND rule_id=?",
     "tenant-a",ruleId))[0]?.n,1);
   const interactionInput=(id: string,label: string)=>({
     interactionId:id,accountId,kind:1,text:"Automated interaction "+label,
     occurredAt:{seconds:1780000000n,nanos:0},
     precondition:rkey("interaction-"+label)
   });
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,
     interactionInput(sourceId,"one"))).status,200);
   const runInput=(id: string,keyLabel: string)=>({ruleId,interactionId:id,
     expectedRuleVersion:1n,precondition:rkey(keyLabel)});
   const runRace=async(input: ReturnType<typeof runInput>)=>{
     const response=await fixtureRpcRace(dispatch,automation.runInteractionAutomation,admin,input);
     return {status:response.status};
   };
   const firstRun=runInput(sourceId,"run");
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,viewer,firstRun)).status,403);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,tenantB,firstRun)).status,404);
   const startCount=(await sql("SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0]?.n;
   assert.ok(typeof startCount==="number");
   const executed=await fixtureRpc(dispatch,automation.runInteractionAutomation,admin,firstRun);
   assert.equal(executed.status,200,formatRpcFailure(executed));
   assert.equal(executed.body.receipt?.resourceVersion,1n);
   assert.equal(executed.body.run?.status,2);
   assert.equal(executed.body.run?.disposition,1);
   assert.equal(executed.body.run?.attemptCount,1n);
   assert.equal(executed.body.run?.canCancel,false);
   assert.ok(executed.body.run);
   const taskId=executed.body.run.taskId;
   const task=(await fixtureRpc(dispatch,tasks.getTask,viewer,{taskId})).body.task;
   assert.ok(task?.owner?.owner.value && task.dueAt);
   assert.equal(task.accountId,accountId);
   assert.equal(task.subject,put.taskSubject);
   assert.equal(task.owner.owner.value,person);
   assert.equal(task.status,1);
   assert.ok(task.dueAt.seconds>0n);
   const replay=await fixtureRpc(dispatch,automation.runInteractionAutomation,admin,firstRun);
   assert.equal(replay.status,200,formatRpcFailure(replay));
   assert.equal(replay.body.receipt?.replayed,true);
   assert.equal(replay.body.run?.taskId,taskId);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0]?.n,startCount+1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",taskId))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     "tenant-a",taskId))[0]?.n,1);
   const proof=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:sourceId,receiptIdempotencyKey:firstRun.precondition.idempotencyKey
   });
   assert.equal(proof.status,200);
   assert.equal(proof.body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:sourceId,receiptIdempotencyKey:"wrong-request-key"
   })).body.confirmedMutation,undefined);
   assert.ok((await fixtureRpc(dispatch,automation.listAutomationRuns,viewer,{ruleId,page:{pageSize:25}}))
     .body.runs.some(x=>x.taskId===taskId&&x.status===2));
   assert.equal((await fixtureRpc(dispatch,automation.listAutomationRuns,tenantB,{ruleId,page:{pageSize:25}}))
     .body.runs.length,0);
   // A pending run preserves the exact snapshot after human revocation. It
   // cannot create a Task until that human is reactivated by signed Access.
   const second="automation-second-"+suffix;
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,
     interactionInput(second,"two"))).status,200);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{
     email:person,active:false,precondition:rkey("revoke",1n)
   })).status,200);
   const alias=runInput(sourceId,"run-alias");
   const aliasReplay=await fixtureRpc(dispatch,automation.runInteractionAutomation,admin,alias);
   assert.equal(aliasReplay.status,200,formatRpcFailure(aliasReplay));
   assert.equal(aliasReplay.body.receipt?.replayed,true);
   assert.equal(aliasReplay.body.run?.taskId,taskId);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:sourceId,receiptIdempotencyKey:alias.precondition.idempotencyKey
   })).body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,{
     ...alias,expectedRuleVersion:2n
   })).status,409);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,{
     ...firstRun,interactionId:second
   })).status,409);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,{
     ...alias,interactionId:second
   })).status,409);
   assert.equal((await fixtureRpcError(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:second
   })).status,404);
   const secondInput=runInput(second,"run-two");
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,secondInput)).status,400);
   const pending=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:second});
   assert.equal(pending.status,200);
   assert.equal(pending.body.run?.status,1);
   assert.equal(pending.body.run?.disposition,3);
   assert.equal(pending.body.run?.attemptCount,0n);
   assert.equal(pending.body.run?.canCancel,true);
   assert.equal(pending.body.confirmedMutation,undefined);
   const waitingList=await fixtureRpc(dispatch,automation.listAutomationRuns,viewer,{ruleId,page:{pageSize:25}});
   assert.equal(waitingList.body.runs.find(x=>x.interactionId===second)?.disposition,3);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{
     email:person,active:true,precondition:rkey("reactivate",2n)
   })).status,200);
   const ready=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:second});
   assert.equal(ready.body.run?.disposition,2);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,{
     ...runInput(second,"wrong-pinned-version"),expectedRuleVersion:2n
   })).status,400);
   const recovered=await fixtureRpc(dispatch,automation.runInteractionAutomation,admin,secondInput);
   assert.equal(recovered.status,200,formatRpcFailure(recovered));
   assert.equal(recovered.body.run?.status,2);
   assert.equal((await fixtureRpc(dispatch,automation.runInteractionAutomation,admin,secondInput))
     .body.receipt?.replayed,true);
   // Concurrent distinct sources with a single request key: at most one
   // claim and one original Task. The losing source must have no run row.
   const raceA="automation-key-race-a-"+suffix,raceB="automation-key-race-b-"+suffix;
   for(const [id,label] of [[raceA,"key-a"],[raceB,"key-b"]] as const)
     assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,interactionInput(id,label))).status,200);
   const keyRace=await Promise.all([raceA,raceB].map(id=>
     runRace(runInput(id,"shared-key"))));
   assert.deepEqual(keyRace.map(x=>x.status).sort((a,b)=>a-b),[200,409]);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND interaction_id IN (?,?)",
     "tenant-a",ruleId,raceA,raceB))[0]?.n,1);

   // Native D1 task-audit failure: actual write rolls back, attempt is
   // durably bounded and the original intent remains available for recovery.
   const exhausted="automation-exhausted-"+suffix;
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,interactionInput(exhausted,"exhausted"))).status,200);
   const exhaustedInput=runInput(exhausted,"exhausted-run");
   await sql("CREATE TRIGGER fixture_fail_automation_task BEFORE INSERT ON crm_audit "+
     "WHEN NEW.action='activity_tasks.create' BEGIN SELECT RAISE(ABORT,'fixture task rollback'); END;");
   let failed: Awaited<ReturnType<typeof getRun>> | undefined;
   const getRun=async()=>{
     const result=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:exhausted});
     assert.ok(result.body.run);
     return result.body.run;
   };
   try {
     for(let i=1;i<=5;i++){
       assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,exhaustedInput)).status,503);
       failed=await getRun();
       assert.equal(failed.attemptCount,BigInt(i));
       assert.equal(failed.disposition,i===5?8:7);
       assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,exhaustedInput)).status,400);
       // Seed elapsed backoff in the same native D1; no wall-clock sleeps.
       if(i<5)await sql("UPDATE crm_activity_automation_runs SET next_attempt_at_unix_millis=0 WHERE tenant=? AND task_id=?","tenant-a",failed.taskId);
     }
   }finally{await sql("DROP TRIGGER fixture_fail_automation_task");}
   assert.ok(failed);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?","tenant-a",failed.taskId))[0]?.n,0);
   const cancelInput={ruleId,interactionId:exhausted,expectedRuleVersion:1n,precondition:rkey("cancel-exhausted")};
   assert.equal((await fixtureRpcError(dispatch,automation.cancelAutomationRun,viewer,cancelInput)).status,403);
   assert.equal((await fixtureRpcError(dispatch,automation.cancelAutomationRun,tenantB,cancelInput)).status,404);
   assert.equal((await fixtureRpcError(dispatch,automation.cancelAutomationRun,admin,{
     ...cancelInput,precondition:exhaustedInput.precondition
   })).status,409);
   const cancelled=await fixtureRpc(dispatch,automation.cancelAutomationRun,admin,cancelInput);
   assert.equal(cancelled.status,200,formatRpcFailure(cancelled));
   assert.equal(cancelled.body.run?.status,3);
   assert.equal(cancelled.body.run?.disposition,9);
   assert.equal(cancelled.body.run?.canCancel,false);
   assert.equal((await fixtureRpc(dispatch,automation.cancelAutomationRun,admin,cancelInput)).body.receipt?.replayed,true);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:exhausted,receiptIdempotencyKey:cancelInput.precondition.idempotencyKey
   })).body.confirmedMutation?.originalRequestId,cancelled.body.receipt?.originalRequestId);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:exhausted,receiptIdempotencyKey:exhaustedInput.precondition.idempotencyKey
   })).body.confirmedMutation,undefined);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,exhaustedInput)).status,400);
   await assert.rejects(()=>sql(
     "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) "+
     "SELECT tenant,?,account_id,state,version,?,idem_digest,request_id,actor FROM crm_activity_task_records WHERE tenant=? AND id=?",
     failed.taskId,failed.taskId,"tenant-a",taskId),/AUTOMATION_TASK_LEASE_REQUIRED/);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activities.automation.run.cancel'","tenant-a",failed.taskId))[0]?.n,1);
   assert.equal((await fixtureRpcError(dispatch,automation.cancelAutomationRun,admin,{
     ruleId,interactionId:sourceId,expectedRuleVersion:1n,precondition:rkey("cancel-committed")
   })).status,400);
   // Two real signed calls to one run may return a replay or contention.
   // Neither can create a second Task or consume another successful attempt.
   const leaseRace="automation-lease-race-"+suffix;
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,interactionInput(leaseRace,"lease-race"))).status,200);
   const leaseResults=await Promise.all(["lease-a","lease-b"].map(key=>
     runRace(runInput(leaseRace,key))));
   assert.ok(leaseResults.some(x=>x.status===200));
   assert.ok(leaseResults.every(x=>[200,409].includes(x.status)));
   const leaseRun=(await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:leaseRace})).body.run;
   assert.ok(leaseRun);
   assert.equal(leaseRun.attemptCount,1n);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'","tenant-a",leaseRun.taskId))[0]?.n,1);
   // Keep one durable claim without a Task; test the commit-time D1 guard
   // against the exact pending row after rule revocation, not a mock.
   const blocked="automation-blocked-"+suffix;
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,interactionInput(blocked,"blocked"))).status,200);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{
     email:person,active:false,precondition:rkey("revoke-blocked",3n)
   })).status,200);
   const blockedInput=runInput(blocked,"blocked-run");
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,blockedInput)).status,400);
   assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{
     email:person,active:true,precondition:rkey("reactivate-blocked",4n)
   })).status,200);
   const blockedRun=(await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:blocked
   })).body.run;
   assert.ok(blockedRun);
   assert.equal(blockedRun.status,1);
   // Native durable lease recovery fixture: seed a previously acquired lease
   // as if its worker stopped; verify the live API and physical write fence.
   await sql("UPDATE crm_activity_automation_runs SET attempt_count=1,lease_request_id=?,lease_actor=?,lease_until_unix_millis=unixepoch('now')*1000+60000,next_attempt_at_unix_millis=unixepoch('now')*1000+65000 WHERE tenant=? AND task_id=?",
     "fixture-lease-"+suffix,person,"tenant-a",blockedRun.taskId);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:blocked})).body.run?.disposition,6);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,blockedInput)).status,409);
   await assert.rejects(()=>sql(
     "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) "+
     "SELECT tenant,?,account_id,state,version,?,idem_digest,request_id,actor FROM crm_activity_task_records WHERE tenant=? AND id=?",
     blockedRun.taskId,blockedRun.taskId,"tenant-a",taskId),/AUTOMATION_TASK_LEASE_REQUIRED/);
   await sql("UPDATE crm_activity_automation_runs SET lease_until_unix_millis=0,next_attempt_at_unix_millis=0 WHERE tenant=? AND task_id=?",
     "tenant-a",blockedRun.taskId);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:blocked})).body.run?.disposition,2);
   // Native D1 aborts only the run finalization audit; original Task and
   // receipt were committed in their own atomic owner statement.
   const reconcileSource="automation-reconcile-"+suffix;
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,
     interactionInput(reconcileSource,"reconcile"))).status,200);
   const reconcileInput=runInput(reconcileSource,"reconcile-run");
   await sql("CREATE TRIGGER fixture_fail_automation_commit BEFORE INSERT ON crm_audit "+
     "WHEN NEW.action='activities.automation.run.committed' "+
     "BEGIN SELECT RAISE(ABORT,'fixture run finalization rollback'); END;");
   try {
     assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,reconcileInput)).status,503);
   } finally {await sql("DROP TRIGGER fixture_fail_automation_commit");}
   const unreconciled=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:reconcileSource,receiptIdempotencyKey:reconcileInput.precondition.idempotencyKey
   });
   assert.equal(unreconciled.body.run?.status,1);
   assert.equal(unreconciled.body.run?.disposition,5);
   assert.equal(unreconciled.body.run?.canCancel,false);
   assert.equal((await fixtureRpcError(dispatch,automation.cancelAutomationRun,admin,{
     ruleId,interactionId:reconcileSource,expectedRuleVersion:1n,precondition:rkey("cancel-receipt")
   })).status,400);
   assert.equal(unreconciled.body.confirmedMutation,undefined);
   assert.ok(unreconciled.body.run);
   const reconcileTask=unreconciled.body.run.taskId;
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",reconcileTask))[0]?.n,1);
   const disabled=await fixtureRpc(dispatch,automation.putAutomationRule,admin,{
     ...put,enabled:false,precondition:rkey("disable",1n)
   });
   assert.equal(disabled.status,200,formatRpcFailure(disabled));
   assert.equal(disabled.body.rule?.version,2n);
   const terminal=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{ruleId,interactionId:blocked});
   assert.equal(terminal.body.run?.disposition,4);
   const reconcileAfterDisable=await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:reconcileSource
   });
   assert.equal(reconcileAfterDisable.body.run?.disposition,5);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,viewer,reconcileInput)).status,403);
   const reconciled=await fixtureRpc(dispatch,automation.runInteractionAutomation,admin,reconcileInput);
   assert.equal(reconciled.status,200,formatRpcFailure(reconciled));
   assert.equal(reconciled.body.run?.disposition,1);
   assert.equal(reconciled.body.receipt?.replayed,true);
   assert.equal((await fixtureRpc(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:reconcileSource,receiptIdempotencyKey:reconcileInput.precondition.idempotencyKey
   })).body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     "tenant-a",reconcileTask))[0]?.n,1);
   // Attempt the same physical Task INSERT boundary after disabling the rule.
   // A real D1 trigger, not the earlier Rust check, must reject it.
   await assert.rejects(()=>sql(
     "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) "+
     "SELECT tenant,?,account_id,state,version,?,idem_digest,request_id,actor "+
     "FROM crm_activity_task_records WHERE tenant=? AND id=?",
     blockedRun.taskId,blockedRun.taskId,"tenant-a",taskId),/AUTOMATION_RULE_NOT_ENABLED_OR_STALE/);
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,blockedInput)).status,400);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?",
     "tenant-a",blockedRun.taskId))[0]?.n,0);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",blockedRun.taskId))[0]?.n,0);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_rule_versions WHERE tenant=? AND rule_id=?",
     "tenant-a",ruleId))[0]?.n,2);
   const third="automation-third-"+suffix;
   assert.equal((await fixtureRpc(dispatch,interaction.createInteraction,admin,
     interactionInput(third,"three"))).status,200);
   const countBeforeDenied=(await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0]?.n;
   assert.equal((await fixtureRpcError(dispatch,automation.runInteractionAutomation,admin,{
     ...runInput(third,"run-three"),expectedRuleVersion:2n
   })).status,400);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0]?.n,countBeforeDenied);
   assert.equal((await fixtureRpcError(dispatch,automation.getAutomationRun,viewer,{
     ruleId,interactionId:third
   })).status,404);
   console.log(JSON.stringify({event:"crm.native-sk08a-rule-run",result:"PASS",
     generated_connect:true,signed_access:true,original_task_owner:true,
     exact_source_receipt:true,tenant_ab:true,viewer_write_denied:true,
     rule_version_cas:true,single_task_audit:true,exact_replay:true,
     durable_pending:true,revocation_recovery:true,disabled_fail_closed:true,
     request_key_conflict:true,concurrent_key_conflict:true,alias_receipt:true,
     atomic_rule_guard:true,typed_disposition:true,pinned_retry:true,
     original_receipt_reconciliation_after_disable:true,bounded_attempts:true,backoff:true,
     cancellation_receipt:true,cancelled_task_fence:true,concurrent_lease:true,
     expired_lease_readiness:true,stale_writer_fence:true,production_cron_disabled:true}));
 }
 // SK-08A.3: the same native Worker Cron dispatches immutable Deal-created
 // events and overdue original Tasks. No browser, HTTP route or second Task
 // writer participates. Repeated ticks and recovery preserve one Task receipt.
 {
   const suffix=randomUUID();
   const key=(label:string)=>({idempotencyKey:"automation-scheduled-"+label+"-"+suffix});
   const person="automation-scheduled-"+suffix+"@example.test";
   assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{
     email:person,role:1,precondition:key("member")
   })).status,200);
   const tick=async(scheduledTime=new Date())=>{
     const result=await runScheduled({cron:"*/5 * * * *",scheduledTime});
     assert.equal(result.outcome,"ok","native Cron callback must return without throwing");
     assert.equal(result.noRetry,false);
   };

   // A deal created before publication is not backfilled. A new deal is
   // discovered from the immutable Sales lifecycle event after publication.
   const oldDealRunCount=(await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND source_type='deal_created' AND source_id=?",
     "tenant-a",dealId)).at(0)?.n;
   const oldDealEvent=(await sql(
     "SELECT rowid AS event_rowid FROM crm_sales_deal_events "+
     "WHERE tenant=? AND deal_id=? AND action='created'","tenant-a",dealId))[0];
   assert.ok(oldDealEvent);
   const dealRuleId="automation-deal-rule-"+suffix;
   const dealRule=await fixtureRpc(dispatch,automation.putAutomationRule,admin,{
     ruleId:dealRuleId,trigger:3,enabled:true,taskSubject:"Deal-created follow-up "+suffix,
     assigneeEmail:person,dueAfterSeconds:3600n,precondition:key("deal-rule")
   });
   assert.equal(dealRule.status,200,formatRpcFailure(dealRule));
   const publishedDealRule=(await sql(
     "SELECT published_after_deal_event_rowid FROM crm_activity_automation_rules WHERE tenant=? AND id=?",
     "tenant-a",dealRuleId))[0];
   assert.ok(publishedDealRule);
   assert.ok(Number(publishedDealRule.published_after_deal_event_rowid)>=Number(oldDealEvent.event_rowid));
   // Simulate equal millisecond timestamps: the immutable event rowid, not a
   // rounded wall-clock comparison, must keep this older Deal out of the rule.
   await sql(
     "UPDATE crm_activity_automation_rules SET version=version+1,published_at_unix_nanos=("+
     "SELECT occurred_at_unix_nanos FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND action='created'),"+
     "idem_key=?,idem_digest=?,request_id=?,actor=? WHERE tenant=? AND id=? AND version=1",
     "tenant-a",dealId,"automation-deal-rule-v2-"+suffix,"f".repeat(64),
     "automation-deal-rule-v2-request-"+suffix,"native-a@example.test","tenant-a",dealRuleId);
   const revisedDealRule=(await sql(
     "SELECT version FROM crm_activity_automation_rules WHERE tenant=? AND id=?",
     "tenant-a",dealRuleId))[0];
   assert.equal(revisedDealRule?.version,2);
   const newDealId="automation-deal-source-"+suffix;
   const dealInput={...dealCreationInput(newDealId,accountId,contactId),
     name:"Automation source deal "+suffix,precondition:key("deal-create")};
   const createdDeal=await fixtureRpc(dispatch,DealService.method.createDeal,admin,dealInput);
   assert.equal(createdDeal.status,200,formatRpcFailure(createdDeal));
   const dealEvent=(await sql(
     "SELECT rowid AS event_rowid,deal_version,action FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND action='created'",
     "tenant-a",newDealId))[0];
   assert.ok(dealEvent);
   assert.ok(Number(dealEvent.event_rowid)>Number(publishedDealRule.published_after_deal_event_rowid));
   await Promise.all([tick(),tick()]);
   await tick();
   const dealRuns=await sql(
     "SELECT source_event_id,rule_version,source_version,task_id,status,attempt_count FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='deal_created' AND source_id=?",
     "tenant-a",dealRuleId,newDealId);
   assert.equal(dealRuns.length,1,"repeated Cron ticks must claim one Deal source");
   const dealRun=dealRuns[0]!;
   assert.equal(dealRun.status,"committed");
   assert.equal(dealRun.rule_version,2);
   assert.equal(dealRun.source_version,dealEvent.deal_version);
   assert.equal(dealRun.attempt_count,1);
   const dealTaskId=String(dealRun.task_id);
   const dealTask=(await fixtureRpc(dispatch,tasks.getTask,viewer,{taskId:dealTaskId})).body.task;
   assert.ok(dealTask);
   assert.equal(dealTask.accountId,accountId);
   assert.equal(dealTask.assigneeIdentity,person);
   assert.equal(dealTask.subject,"Deal-created follow-up "+suffix);
   assert.equal(dealTask.relatedDealId,newDealId);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",dealTaskId))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     "tenant-a",dealTaskId))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create' AND actor=?",
     "tenant-a",dealTaskId,"service:crm-automation"))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND source_type='deal_created' AND source_id=?",
     "tenant-b",newDealId))[0]?.n,0,"tenant B must not observe or execute tenant A's source");
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND source_type='deal_created' AND source_id=?",
     "tenant-a",dealId))[0]?.n,oldDealRunCount,
     "publishing a Deal rule must not replay old lifecycle events");

   // Task-overdue is a timer over the current open Task version. A completed
   // or rescheduled source before a tick is never dispatched.
   const overdueRuleId="automation-overdue-rule-"+suffix;
   const overdueRule=await fixtureRpc(dispatch,automation.putAutomationRule,admin,{
     ruleId:overdueRuleId,trigger:2,enabled:true,taskSubject:"Overdue follow-up "+suffix,
     assigneeEmail:person,dueAfterSeconds:0n,precondition:key("overdue-rule")
   });
   assert.equal(overdueRule.status,200,formatRpcFailure(overdueRule));
   const overdueSourceId="automation-overdue-source-"+suffix;
   const overdueSource=await fixtureRpc(dispatch,tasks.createTask,admin,{
     taskId:overdueSourceId,accountId,subject:"Original overdue task "+suffix,priority:2,
     dueAt:{seconds:1700000000n,nanos:0},precondition:key("overdue-source")
   });
   assert.equal(overdueSource.status,200,formatRpcFailure(overdueSource));
   await Promise.all([tick(),tick()]);
   await tick();
   const overdueRuns=await sql(
     "SELECT source_version,task_id,status,attempt_count FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,overdueSourceId);
   assert.equal(overdueRuns.length,1,"a Task source is consumed once across Cron ticks");
   const overdueRun=overdueRuns[0]!;
   assert.equal(overdueRun.status,"committed");
   assert.equal(overdueRun.source_version,1);
   assert.equal(overdueRun.attempt_count,1);
   const overdueTaskId=String(overdueRun.task_id);
   assert.notEqual(overdueTaskId,overdueSourceId);
   assert.equal((await fixtureRpc(dispatch,tasks.getTask,viewer,{taskId:overdueTaskId}))
     .body.task?.subject,"Overdue follow-up "+suffix);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",overdueTaskId))[0]?.n,1);

   // A delayed Cron invocation must use actual Worker time when it is later
   // than scheduledTime; otherwise this Task would not yet be overdue.
   const delayedSourceId="automation-overdue-delayed-" + suffix;
   const delayedDueMillis=Date.now()-60_000;
   const delayedDueAt={
     seconds:BigInt(Math.floor(delayedDueMillis/1000)),
     nanos:(delayedDueMillis%1000)*1_000_000
   };
   assert.equal((await fixtureRpc(dispatch,tasks.createTask,admin,{
     taskId:delayedSourceId,accountId,subject:"Delayed Cron catch-up source",priority:2,
     dueAt:delayedDueAt,precondition:key("delayed-source")
   })).status,200);
   await tick(new Date(Date.now()-120_000));
   const delayedRun=(await sql(
     "SELECT task_id,status,source_version FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,delayedSourceId))[0];
   assert.ok(delayedRun);
   assert.equal(delayedRun.status,"committed");
   assert.equal(delayedRun.source_version,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",String(delayedRun.task_id)))[0]?.n,1);

   // A new published rule version is a distinct source identity. The same
   // still-overdue Task may be consumed once by that new version, but repeated
   // ticks of either version remain idempotent.
   const revisedRule=await fixtureRpc(dispatch,automation.putAutomationRule,admin,{
     ruleId:overdueRuleId,trigger:2,enabled:true,
     taskSubject:"Overdue follow-up revision "+suffix,assigneeEmail:person,
     dueAfterSeconds:0n,
     precondition:{expectedVersion:1n,idempotencyKey:"automation-overdue-rule-v2-"+suffix}
   });
   assert.equal(revisedRule.status,200,formatRpcFailure(revisedRule));
   await tick();
   await tick();
   const versionedRuns=await sql(
     "SELECT rule_version,task_id,status FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=? ORDER BY rule_version",
     "tenant-a",overdueRuleId,overdueSourceId);
   assert.equal(versionedRuns.length,2,"each published rule version may consume a Task source once");
   assert.deepEqual(versionedRuns.map(row=>row.rule_version),[1,2]);
   assert.equal(versionedRuns[1]?.status,"committed");
   assert.notEqual(versionedRuns[0]?.task_id,versionedRuns[1]?.task_id);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",String(versionedRuns[1]?.task_id)))[0]?.n,1);

   // Lost acknowledgement after the original Task receipt is durable:
   // recovery replays that exact receipt, never creates a second Task.
   const reconcileSourceId="automation-overdue-reconcile-"+suffix;
   assert.equal((await fixtureRpc(dispatch,tasks.createTask,admin,{
     taskId:reconcileSourceId,accountId,subject:"Overdue reconcile source",priority:2,
     dueAt:{seconds:1700000000n,nanos:0},precondition:key("reconcile-source")
   })).status,200);
   await sql("CREATE TRIGGER fixture_fail_scheduled_automation_commit BEFORE INSERT ON crm_audit "+
     "WHEN NEW.action='activities.automation.run.committed' "+
     "BEGIN SELECT RAISE(ABORT,'fixture scheduled run finalization rollback'); END;");
   try { await tick(); } finally { await sql("DROP TRIGGER fixture_fail_scheduled_automation_commit"); }
   const pendingReconcile=(await sql(
     "SELECT task_id,status,attempt_count FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,reconcileSourceId))[0];
   assert.ok(pendingReconcile);
   assert.equal(pendingReconcile.status,"pending");
   const reconciledTaskId=String(pendingReconcile.task_id);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",reconciledTaskId))[0]?.n,1,"Task receipt survives a failed run acknowledgement");
   await tick();
   const recovered=(await sql(
     "SELECT status,attempt_count FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,reconcileSourceId))[0];
   assert.equal(recovered?.status,"committed");
   assert.equal(recovered?.attempt_count,1,"receipt reconciliation does not create another attempt");
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?",
     "tenant-a",reconciledTaskId))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     "tenant-a",reconciledTaskId))[0]?.n,1);

   // Operator cancellation fences a pending run before Task commit.
   const cancelSourceId="automation-overdue-cancel-"+suffix;
   assert.equal((await fixtureRpc(dispatch,tasks.createTask,admin,{
     taskId:cancelSourceId,accountId,subject:"Overdue cancellation source",priority:2,
     dueAt:{seconds:1700000000n,nanos:0},precondition:key("cancel-source")
   })).status,200);
   await sql("CREATE TRIGGER fixture_fail_scheduled_automation_task BEFORE INSERT ON crm_activity_task_records "+
     "WHEN NEW.id LIKE 'automation-%' BEGIN SELECT RAISE(ABORT,'fixture scheduled Task blocked'); END;");
   try { await tick(); } finally { await sql("DROP TRIGGER fixture_fail_scheduled_automation_task"); }
   const cancelRun=(await sql(
     "SELECT task_id,last_error_code FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,cancelSourceId))[0];
   assert.ok(cancelRun);
   assert.ok(cancelRun.last_error_code,"the run journal retains the typed failure reason");
   const cancelled=await fixtureRpc(dispatch,automation.cancelAutomationRun,admin,{
     ruleId:overdueRuleId,sourceType:2,sourceId:cancelSourceId,expectedRuleVersion:2n,
     precondition:key("cancel-scheduled")
   });
   assert.equal(cancelled.status,200,formatRpcFailure(cancelled));
   assert.equal(cancelled.body.run?.status,3);
   assert.equal(cancelled.body.run?.disposition,9);
   await tick();
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?",
     "tenant-a",String(cancelRun.task_id)))[0]?.n,0,"cancelled scheduled intent cannot create a Task");
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=? AND cancel_request_id IS NOT NULL",
     "tenant-a",overdueRuleId,cancelSourceId))[0]?.n,1);

   // If the source is completed while a failed run is waiting for backoff,
   // the exact source version is marked stale and cannot create a follow-up.
   const staleSourceId="automation-overdue-stale-"+suffix;
   assert.equal((await fixtureRpc(dispatch,tasks.createTask,admin,{
     taskId:staleSourceId,accountId,subject:"Overdue stale source",priority:2,
     dueAt:{seconds:1700000000n,nanos:0},precondition:key("stale-source")
   })).status,200);
   await sql("CREATE TRIGGER fixture_fail_scheduled_stale_task BEFORE INSERT ON crm_activity_task_records "+
     "WHEN NEW.id LIKE 'automation-%' BEGIN SELECT RAISE(ABORT,'fixture stale Task blocked'); END;");
   try { await tick(); } finally { await sql("DROP TRIGGER fixture_fail_scheduled_stale_task"); }
   const staleRun=(await sql(
     "SELECT task_id FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,staleSourceId))[0];
   assert.ok(staleRun);
   assert.equal((await fixtureRpc(dispatch,tasks.completeTask,knownService,{
     taskId:staleSourceId,precondition:{expectedVersion:1n,idempotencyKey:"automation-complete-stale-"+suffix}
   })).status,200);
   const staleLeaseRequest="fixture-stale-live-lease-"+suffix;
   await sql("UPDATE crm_activity_automation_runs SET lease_request_id=?,lease_actor=?, "+
     "lease_until_unix_millis=unixepoch('now')*1000+60000,next_attempt_at_unix_millis=0 "+
     "WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     staleLeaseRequest,"service:crm-automation","tenant-a",overdueRuleId,staleSourceId);
   await tick();
   const liveLeaseRun=(await sql(
     "SELECT source_stale FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,staleSourceId))[0];
   assert.equal(liveLeaseRun?.source_stale,0,"a live lease must fence the stale-source transition");
   await sql("UPDATE crm_activity_automation_runs SET lease_until_unix_millis=0,next_attempt_at_unix_millis=0 "+
     "WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=? AND lease_request_id=?",
     "tenant-a",overdueRuleId,staleSourceId,staleLeaseRequest);
   await tick();
   const stale=(await sql(
     "SELECT source_stale,last_error_code FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,staleSourceId))[0];
   assert.equal(stale?.source_stale,1);
   assert.equal(stale?.last_error_code,"AUTOMATION_TASK_SOURCE_STALE");
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activities.automation.run.source_stale'",
     "tenant-a",String(staleRun.task_id)))[0]?.n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?",
     "tenant-a",String(staleRun.task_id)))[0]?.n,0);

   // A task rescheduled before its first due scan is not an overdue source.
   const rescheduledSourceId="automation-overdue-rescheduled-"+suffix;
   assert.equal((await fixtureRpc(dispatch,tasks.createTask,admin,{
     taskId:rescheduledSourceId,accountId,subject:"Overdue rescheduled source",priority:2,
     dueAt:{seconds:1700000000n,nanos:0},precondition:key("rescheduled-source")
   })).status,200);
   assert.equal((await fixtureRpc(dispatch,tasks.updateTask,knownService,{
     taskId:rescheduledSourceId,dueAt:{operation:{case:"set",value:{
       seconds:1900000000n,nanos:0
     }}},precondition:{expectedVersion:1n,idempotencyKey:"automation-reschedule-"+suffix}
   })).status,200);
   await tick();
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type='task_overdue' AND source_id=?",
     "tenant-a",overdueRuleId,rescheduledSourceId))[0]?.n,0);
   console.log(JSON.stringify({event:"crm.native-sk08a-autonomous-dispatch",result:"PASS",
     real_worker_scheduled:true,deal_created:true,task_overdue:true,closed_browser_independent:true,
     bounded_catch_up:true,tenant_isolation:true,duplicate_source_fenced:true,
     original_task_owner:true,system_actor_audited:true,receipt_recovery:true,
     concurrent_cron:true,late_scheduled_time_catch_up:true,
     operator_cancellation:true,stale_source_fenced:true,rescheduled_source_ignored:true}));
 }

}

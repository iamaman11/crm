import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {TaskService} from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import {collectFixturePages,fixtureRpc,fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {string} from "../../scripts/json-boundary.mts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type LinkedFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  linked: ReadonlyArray<Record<string,unknown>>;
  admin: string; viewer: string; tenantB: string; knownService: string;
  dealId: string; dealLast: string; accountId: string; accountNextId: string; taskMid: string;
};
function number(value: unknown): number { assert.ok(typeof value === "number"); return value; }
export async function verifySalesLinkedTasks({dispatch,sql,linked,admin,viewer,tenantB,knownService,
  dealId,dealLast,accountId,accountNextId,taskMid}: LinkedFixture) {
  const tasks=TaskService.method;
  const pages=async(filter: {dealId: string; accountId?: string; status?: 2; page?: {pageSize: number}})=>
    collectFixturePages(async pageToken=>{
      const {body}=await fixtureRpc(dispatch,tasks.listTasks,viewer,{...filter,page:{pageSize:1,pageToken}});
      return {items:body.tasks,nextPageToken:body.pageInfo?.nextPageToken};
    });
  // M1-08 product check: not a legacy generic typed-resource filter.
  // Customers can have multiple Deals and manual Account tasks. The concrete
  // Deal page should show ONLY Tasks whose original domain state refers to
  // this specific sales.deal, with exact tenant+Account/page filtering.
  const appliedTask=string(linked.find(row=>row.disposition==="applied")?.task_id);
  assert.ok(appliedTask,"linked Sales→Task must retain the Task ID");
  const linkedTaskRead=await fixtureRpc(dispatch,tasks.getTask,viewer,{taskId:appliedTask});
  assert.equal(linkedTaskRead.status,200,formatRpcFailure(linkedTaskRead));
  assert.equal(linkedTaskRead.body.task?.relatedDealId,dealId);
  assert.equal(linkedTaskRead.body.task?.accountId,accountId);
  assert.equal((await fixtureRpc(dispatch,tasks.getTask,viewer,{taskId:taskMid})).body.task?.relatedDealId,"",
    "ordinary Account task must not claim a Deal relation");
  const scopedDeals=await pages({dealId,accountId,page:{pageSize:1}});
  assert.deepEqual(scopedDeals.map(task=>task.taskId),[appliedTask],
    "Deal task view must not mix in manual Account tasks or another Deal");
  assert.equal(scopedDeals[0]?.relatedDealId,dealId);
  assert.deepEqual((await pages({dealId,accountId,status:2}))
    .map(task=>task.taskId),[],"status filter applies before pagination");
  assert.deepEqual((await pages({dealId,accountId:accountNextId}))
    .map(task=>task.taskId),[],"Account/Deal cross-scope is an empty intersection");
  assert.deepEqual((await pages({dealId:dealLast}))
    .map(task=>task.taskId),[],"unlinked Deal cannot see another Deal's Tasks");
  assert.deepEqual((await pages({dealId:"nonexistent-valid-deal"}))
    .map(task=>task.taskId),[],"unknown Deal does not broaden the result");
  assert.equal((await fixtureRpc(dispatch,tasks.listTasks,tenantB,{dealId,page:{pageSize:1}})).body.tasks.length,0);
  assert.equal((await fixtureRpcError(dispatch,tasks.listTasks,admin,{dealId:"invalid/deal"})).status,400);
  assert.equal((await fixtureRpcError(dispatch,tasks.listTasks,null,{dealId})).status,401);
  const scopedAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
  assert.ok(typeof scopedAudit==="number");
  await fixtureRpc(dispatch,tasks.listTasks,viewer,{dealId});
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,scopedAudit,
    "Deal task navigation must be read-only");
  console.log(JSON.stringify({event:"crm.native-m1-deal-task-navigation",result:"PASS",
    deal_link_from_original_task:true,scope_before_cursor:true,
    manual_account_task_excluded:true,other_deal_excluded:true,
    signed_tenant_filter:true,no_read_audit:true}));
  // M1-08 B: a salesperson can add an ordinary manual Task to a specific
  // existing Deal. The original Task owner validates the Sales owner and
  // commits the normal receipt/audit atomically, never using a second writer.
  const manualDealTaskId="task-manual-deal-"+randomUUID();
  const manualDealInput={taskId:manualDealTaskId,accountId,dealId,
    subject:"Manual deal follow up",priority:2 as const,
    precondition:{idempotencyKey:"task-manual-deal-create-001"}};
  const manualBefore=await sql("SELECT "+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits");
  assert.equal((await fixtureRpcError(dispatch,tasks.createTask,viewer,manualDealInput)).status,403);
  assert.equal((await fixtureRpcError(dispatch,tasks.createTask,tenantB,manualDealInput)).status,400);
  assert.equal((await fixtureRpcError(dispatch,tasks.createTask,admin,{...manualDealInput,dealId:"invalid/deal",
    precondition:{idempotencyKey:"manual-deal-invalid-id"}})).status,400);
  assert.equal((await fixtureRpcError(dispatch,tasks.createTask,admin,{...manualDealInput,dealId:"missing-deal",
    precondition:{idempotencyKey:"manual-deal-missing"}})).status,400);
  assert.equal((await fixtureRpcError(dispatch,tasks.createTask,admin,{...manualDealInput,accountId:accountNextId,
    precondition:{idempotencyKey:"manual-deal-wrong-account"}})).status,400);
  assert.deepEqual(await sql("SELECT "+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"),manualBefore,
    "Rejected Deal links must never write task, receipt or audit");
  const manualCreated=await fixtureRpc(dispatch,tasks.createTask,admin,manualDealInput);
  assert.equal(manualCreated.status,200,formatRpcFailure(manualCreated));
  assert.equal(manualCreated.body.task?.relatedDealId,dealId);
  assert.equal(manualCreated.body.task?.accountId,accountId);
  assert.equal(manualCreated.body.receipt?.replayed,false);
  const manualReplay=await fixtureRpc(dispatch,tasks.createTask,knownService,manualDealInput);
  assert.equal(manualReplay.status,200,formatRpcFailure(manualReplay));
  assert.equal(manualReplay.body.receipt?.replayed,true);
  assert.equal((await fixtureRpcError(dispatch,tasks.createTask,admin,{...manualDealInput,dealId:dealLast})).status,409);
  assert.equal((await fixtureRpc(dispatch,tasks.getTask,viewer,{taskId:manualDealTaskId})).body.task?.relatedDealId,dealId);
  const manualScoped=await pages({dealId,accountId,page:{pageSize:1}});
  assert.deepEqual(manualScoped.map(task=>task.taskId).sort(),[appliedTask,manualDealTaskId].sort());
  assert.ok(!manualScoped.some(task=>task.taskId===taskMid),
    "A general Account Task is not a Task of the Deal");
  const manualAfter=await sql("SELECT "+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits");
  assert.ok(manualBefore[0] && manualAfter[0]);
  for(const key of ["tasks","receipts","audits"] as const) {
    assert.ok(typeof manualBefore[0][key] === "number");
    assert.ok(typeof manualAfter[0][key] === "number");
  }
  assert.equal(number(manualAfter[0].tasks),number(manualBefore[0].tasks)+1);
  assert.equal(number(manualAfter[0].receipts),number(manualBefore[0].receipts)+1);
  assert.equal(number(manualAfter[0].audits),number(manualBefore[0].audits)+1);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id='"+manualDealTaskId+"' AND action='activity_tasks.create'"))[0]?.n,1);
  console.log(JSON.stringify({event:"crm.native-m1-manual-deal-task",result:"PASS",
    original_task_owner:true,deal_and_account_crosscheck:true,negative_no_write:true,
    idempotent_replay:true,exact_scoped_list:true}));
}

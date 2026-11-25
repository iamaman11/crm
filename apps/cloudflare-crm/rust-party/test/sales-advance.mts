import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {TaskService} from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import {fixtureRpc, fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type AdvanceFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string; viewer: string; accountId: string; dealId: string;
};
export async function verifySalesAdvance({dispatch,sql,admin,viewer,accountId,dealId}: AdvanceFixture) {
const sales=DealService.method, tasks=TaskService.method;
const stageLead={pipelineId:"pipeline.enterprise",stageId:"lead",ordinal:1};
const stageQualified={pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:2};
const advanceSale={dealId,targetStage:stageQualified,targetStatus:1,
  precondition:{expectedVersion:2n,idempotencyKey:"connect-sales-advance-main"}};
assert.equal((await fixtureRpcError(dispatch,sales.advanceStage,viewer,advanceSale)).status,403);
assert.equal((await fixtureRpcError(dispatch,sales.advanceStage,admin,{
  ...advanceSale,targetStage:stageLead,
  precondition:{expectedVersion:2n,idempotencyKey:"connect-sales-regress"}})).status,400);
const taskCountBefore=(await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0]?.n;
const advancedSale=await fixtureRpc(dispatch,sales.advanceStage,admin,advanceSale);
assert.equal(advancedSale.status,200,formatRpcFailure(advancedSale));
assert.equal(advancedSale.body.deal?.stage?.stageId,"qualified");
assert.equal(advancedSale.body.deal?.version,3n);
assert.equal(advancedSale.body.receipt?.resourceVersion,3n);
assert.equal(advancedSale.body.receipt?.replayed,false);
const taskCountAfter=(await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0]?.n;
assert.ok(typeof taskCountBefore==="number");
assert.ok(typeof taskCountAfter==="number");
assert.equal(taskCountAfter,taskCountBefore+1,
  "original Sales stage event must create one Activities Task atomically");
const advanceReplayed=await fixtureRpc(dispatch,sales.advanceStage,admin,advanceSale);
assert.equal(advanceReplayed.status,200);
assert.equal(advanceReplayed.body.receipt?.replayed,true);
assert.equal(advanceReplayed.body.receipt?.originalRequestId,advancedSale.headers.get("x-request-id"));
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0]?.n,taskCountAfter);
// Signed workerd cross-owner operational check: a real stage move delivered
// one original Task. Only its OPEN status and authoritative dueAt may mark
// the Deal as having an overdue linked action.
const movedEvent=(await fixtureRpc(dispatch,sales.listDealEvents,viewer,{
  dealId,page:{pageSize:5}})).body.events[0];
assert.ok(movedEvent?.linkedTaskId);
const overdueLinkedTask=await fixtureRpc(dispatch,tasks.updateTask,admin,{
  taskId:movedEvent.linkedTaskId,
  dueAt:{operation:{case:"set",value:{seconds:1577836800n,nanos:0}}},
  precondition:{expectedVersion:1n,idempotencyKey:"r73b-linked-task-overdue"}
});
assert.equal(overdueLinkedTask.status,200,formatRpcFailure(overdueLinkedTask));
const operationalRead=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{
  pipelineId:"pipeline.enterprise",status:1,accountId});
assert.equal(operationalRead.status,200,formatRpcFailure(operationalRead));
const qualifiedOp=operationalRead.body.operational.find(x=>x.stageId==="qualified");
assert.ok(qualifiedOp,"the moved OPEN Deal belongs to qualified");
assert.equal(qualifiedOp.overdueLinkedTaskCount,1n);
assert.equal(qualifiedOp.knownStageAgeCount,1n);
assert.equal((await fixtureRpcError(dispatch,sales.advanceStage,admin,{
  dealId,targetStage:{pipelineId:"pipeline.enterprise",stageId:"fictional-won",ordinal:3},
  targetStatus:2,closeReasonCode:"contract-signed",
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-fake-stage-close"}
})).status,400,"closing cannot fabricate a terminal stage");
assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,admin,{
  dealId,outcome:2,
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-missing-close-reason"}
})).status,400,"closing requires an explicit reason");
assert.equal((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId})).body.deal?.version,3n,
  "rejected synthetic stage and missing reason cannot change the open deal");
const closeSale={dealId,outcome:2,reasonCode:"contract-signed",
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-close-main"}};
const closedSale=await fixtureRpc(dispatch,sales.closeDeal,admin,closeSale);
assert.equal(closedSale.status,200,formatRpcFailure(closedSale));
assert.equal(closedSale.body.deal?.status,2);
assert.equal(closedSale.body.deal?.stage?.stageId,"qualified",
  "closing must preserve the real current stage");
assert.equal(closedSale.body.deal?.version,4n);
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,admin,{
  dealId,name:{operation:{case:"set",value:"Forbidden post-close edit"}},
  precondition:{expectedVersion:4n,idempotencyKey:"connect-sales-update-closed"}
})).status,409,"closed Deal must be immutable");
assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,admin,{
  dealId,outcome:2,reasonCode:"contract-signed",
  precondition:{expectedVersion:4n,idempotencyKey:"connect-sales-double-close"}
})).status,409,"double-close must be rejected");
assert.ok(closedSale.body.deal?.closedAt);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0]?.n,taskCountAfter,
  "Won closure must not create a second follow-up Task");
assert.equal((await fixtureRpc(dispatch,sales.closeDeal,admin,closeSale))
  .body.receipt?.replayed,true);
assert.equal((await fixtureRpc(dispatch,sales.getPipeline,viewer,{pipelineId:"pipeline.enterprise"}))
  .body.pipeline?.version,2n);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records "+
  "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0]?.n,2,
  "two original one-batch Sales->Activities delivery receipts");
const dealHistory=await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId,page:{pageSize:25}});
assert.equal(dealHistory.status,200,formatRpcFailure(dealHistory));
assert.deepEqual(dealHistory.body.events.map(e=>e.dealVersion),[4n,3n,2n,1n]);
assert.deepEqual(dealHistory.body.events.map(e=>e.kind),[4,3,2,1],
  "Append-only lifecycle history must be ordered and typed");
assert.ok(dealHistory.body.events[0]);
assert.ok(dealHistory.body.events[1]);
assert.equal(dealHistory.body.events[0].reasonCode,"contract-signed");
assert.equal(dealHistory.body.events[1].linkedTaskId?.length>0,true);
assert.equal(dealHistory.body.events[0].linkedTaskId,"",
  "Close does not invent a follow-up Task");
assert.equal(dealHistory.body.events[0].actor,"native-a@example.test");
assert.equal(dealHistory.body.events[0].requestId,closedSale.headers.get("x-request-id"));
assert.equal((await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId,
  page:{pageSize:2}})).body.pageInfo?.nextPageToken,"3");
assert.deepEqual((await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId,
  page:{pageSize:2,pageToken:"3"}})).body.events.map(e=>e.dealVersion),[2n,1n]);
assert.equal((await fixtureRpcError(dispatch,sales.listDealEvents,viewer,{dealId,
  page:{pageSize:2,pageToken:"not-an-integer"}})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.listDealEvents,viewer,{
  dealId:"missing-"+randomUUID(),page:{pageSize:1}})).status,404);

return {advancedSale,closedSale,closeSale};
}

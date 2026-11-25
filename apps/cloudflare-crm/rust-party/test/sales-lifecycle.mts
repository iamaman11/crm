import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {DomainErrorDetailSchema} from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import {decodeFixtureDetail,fixtureRpc,fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {record,records,string} from "../../scripts/json-boundary.mts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";
import type {verifyPipeline} from "./pipeline.mts";
import type {verifySalesAdvance} from "./sales-advance.mts";
type BindingValue = string | number | null | ArrayBuffer;
type FixtureResult = {success: boolean; results: Record<string,unknown>[]};
type Statement = {bind(...values: BindingValue[]): Statement; all(): Promise<FixtureResult>};
type LifecycleFixture = {
  dispatch: RpcDispatch; accountId: string; dealId: string; dealLast: string; admin: string; viewer: string;
  sql: (statement: string, ...bindings: BindingValue[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  statement: (query: string, ...bindings: BindingValue[]) => Statement;
  batch: (statements: Statement[]) => Promise<FixtureResult[]>;
  pipelineSeed: Awaited<ReturnType<typeof verifyPipeline>>;
  closeSale: Awaited<ReturnType<typeof verifySalesAdvance>>["closeSale"];
};
export async function verifySalesLifecycle({dispatch,sql,statement,batch,accountId,dealId,dealLast,admin,viewer,pipelineSeed,closeSale}: LifecycleFixture) {
const sales=DealService.method;
const stageQualified={pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:2};
const salesActorSnapshot=async()=>(await sql("SELECT "+
    "(SELECT state FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id='"+dealLast+"') AS deal,"+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_sales_link_delivery_records) AS deliveries,"+
    "(SELECT COUNT(*) FROM crm_sales_deal_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"));
  const beforeSalesFault=(await salesActorSnapshot());
  (await sql("CREATE TRIGGER fixture_fail_sales_link_actor BEFORE INSERT ON crm_audit "+
    "WHEN NEW.action='sales_activities_link.applied' "+
    "BEGIN SELECT RAISE(ABORT,'fixture linked audit'); END;"));
  try{
    const failed=await fixtureRpcError(dispatch,sales.advanceStage,admin,{dealId:dealLast,
      targetStage:stageQualified,targetStatus:1,
      precondition:{expectedVersion:1n,idempotencyKey:"connect-sales-linked-actor-fail"}});
    assert.equal(failed.status,503);
    assert.equal(record(failed.body).code,"unavailable");
    assert.equal(record(failed.body).message,"SALES_COMMIT_UNCONFIRMED");
    const outcomeDetail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
    assert.equal(outcomeDetail.commitOutcome,2);
    assert.equal(outcomeDetail.requestId,failed.headers.get("x-request-id"));
    assert.deepEqual((await salesActorSnapshot()),beforeSalesFault,
      "Late link audit failure rolls back Deal, new Task, receipt, delivery and audit");
  }finally{await sql("DROP TRIGGER fixture_fail_sales_link_actor");}
  // Reconfigure a populated pipeline: stable stage IDs, atomic rank rebase.
  const oldRank=(await sql("SELECT id,version,state FROM crm_sales_deal_records WHERE tenant='tenant-a' "+
    "AND id IN ('"+dealId+"','"+dealLast+"') ORDER BY id"));
  const oldDealAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id IN ('"+dealId+"','"+dealLast+"')"))[0]?.n;
  const oldDealReceipts=(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0]?.n;
  assert.ok(typeof oldDealAudit === "number" && typeof oldDealReceipts === "number");
  assert.ok(pipelineSeed.stages[0] && pipelineSeed.stages[1] && pipelineSeed.stages[2]);
  const reorder={pipelineId:"pipeline.enterprise",name:"Occupied stage reorder",
    stages:[pipelineSeed.stages[1],pipelineSeed.stages[0],pipelineSeed.stages[2]],
    archived:false,
    precondition:{expectedVersion:2n,idempotencyKey:"connect-r7-reorder-used-stage"}};
  // A late D1 failure after the catalog CAS must roll back BOTH the
  // Pipeline audit/receipt and every Deal rank projection.
  const failedRebaseSnapshot=async()=>(await sql("SELECT (SELECT state FROM crm_sales_pipeline_records "+
    "WHERE tenant='tenant-a' AND id='pipeline.enterprise') AS catalog, "+
    "(SELECT state FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id='"+dealId+"') AS closed, "+
    "(SELECT state FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id='"+dealLast+"') AS opened, "+
    "(SELECT COUNT(*) FROM crm_sales_pipeline_receipts WHERE tenant='tenant-a') AS receipts, "+
    "(SELECT COUNT(*) FROM crm_audit WHERE tenant_id='tenant-a') AS audits"));
  const beforeFailedRebase=(await failedRebaseSnapshot());
  (await sql("CREATE TRIGGER fixture_deny_rank_rebase BEFORE UPDATE ON crm_sales_deal_records "+
    "WHEN NEW.id='"+dealLast+"' AND NEW.version=OLD.version "+
    "BEGIN SELECT RAISE(ABORT,'fixture rank rollback'); END;"));
  try {
    const denied=await fixtureRpcError(dispatch,sales.updatePipeline,admin,{
      ...reorder,precondition:{expectedVersion:2n,idempotencyKey:"r7-rebase-rollback"}
    });
    assert.equal(denied.status,503);
    assert.equal(record(denied.body).code,"unavailable");
    const deniedDetail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(denied.body).details));
    assert.equal(deniedDetail.commitOutcome,2);
    assert.equal(deniedDetail.domainCode,"SALES_PIPELINE_WRITE_UNCONFIRMED");
    assert.equal(deniedDetail.requestId,denied.headers.get("x-request-id"));
    assert.deepEqual((await failedRebaseSnapshot()),beforeFailedRebase,
      "Late D1 failure must roll back catalog, both Deals, receipts and audit");
  }finally{await sql("DROP TRIGGER fixture_deny_rank_rebase");}
  const reordered=await fixtureRpc(dispatch,sales.updatePipeline,admin,reorder);
  assert.equal(reordered.status,200,formatRpcFailure(reordered));
  assert.equal(reordered.body.pipeline?.version,3n);
  assert.deepEqual(reordered.body.pipeline?.stages.map(s=>s.stageId),["qualified","lead","won"]);
  assert.deepEqual(reordered.body.pipeline?.stages.map(s=>s.ordinal),[1,2,3]);
  assert.equal((await fixtureRpc(dispatch,sales.updatePipeline,admin,reorder)).body.receipt?.replayed,true);
  const rankAfter=(await sql("SELECT id,version,state FROM crm_sales_deal_records WHERE tenant='tenant-a' "+
    "AND id IN ('"+dealId+"','"+dealLast+"') ORDER BY id"));
  assert.deepEqual(rankAfter.map(x=>x.version),oldRank.map(x=>x.version),
    "Rank-only metadata must not change Deal versions");
  assert.equal(record(record(JSON.parse(string(rankAfter.find(x=>x.id===dealId)?.state))).stage).ordinal,1);
  assert.equal(record(record(JSON.parse(string(rankAfter.find(x=>x.id===dealLast)?.state))).stage).ordinal,2);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id IN ('"+dealId+"','"+dealLast+"')"))[0]?.n,oldDealAudit,
    "No fictional Deal audit for catalog reorder");
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0]?.n,oldDealReceipts,
    "No fictional Deal receipt for catalog reorder");
  assert.equal((await fixtureRpcError(dispatch,sales.updatePipeline,admin,{
    ...reorder,stages:[pipelineSeed.stages[1],pipelineSeed.stages[2]],
    precondition:{expectedVersion:3n,idempotencyKey:"r7-deny-delete-occupied"}
  })).status,409,"Referenced stage deletion must fail");
  assert.equal((await fixtureRpcError(dispatch,sales.updatePipeline,admin,{
    ...reorder,archived:true,
    precondition:{expectedVersion:3n,idempotencyKey:"r7-deny-archive-open"}
  })).status,409,"Open Deals must forbid pipeline archive");
  assert.equal((await fixtureRpc(dispatch,sales.getPipeline,viewer,{pipelineId:"pipeline.enterprise"}))
    .body.pipeline?.version,3n,"Failed catalog writes must roll back");
  const freshInput={
    dealId:"r7-catalog-rank-"+randomUUID(),name:"Owner ordered",accountId,
    stage:{pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:999},
    probabilityBasisPoints:1000,
    precondition:{idempotencyKey:"r471-ci-receipt-read-fail"}
  };
  const unreadableCommit=await fixtureRpcError(dispatch,sales.createDeal,admin,freshInput);
  assert.equal(unreadableCommit.status,503);
  const unreadableDetail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(unreadableCommit.body).details));
  assert.equal(unreadableDetail.commitOutcome,2);
  assert.equal(unreadableDetail.requestId,unreadableCommit.headers.get("x-request-id"));
  assert.equal((await sql("SELECT version FROM crm_sales_deal_records WHERE tenant=? AND id=?",
    "tenant-a",freshInput.dealId))[0]?.version,1);
  const fresh=await fixtureRpc(dispatch,sales.createDeal,admin,freshInput);
  assert.equal(fresh.status,200,formatRpcFailure(fresh));
  assert.equal(fresh.body.receipt?.replayed,true);
  assert.equal(fresh.body.receipt?.originalRequestId,unreadableCommit.headers.get("x-request-id"));
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?",
    "tenant-a",freshInput.dealId))[0]?.n,1);
  console.log(JSON.stringify({event:"crm.native.s1-receipt-read-failure",result:"PASS",
    committed_before_failed_read:true,unconfirmed:true,same_intent_replay:true,single_audit:true}));
  assert.equal(fresh.body.deal?.stage?.ordinal,1);
  // An ordinary UpdateDeal wins version 2 without a stage link.
  // Simulate the losing CAS's remaining batch against the actual D1 binding.
  assert.ok(fresh.body.deal);
  const winnerId=fresh.body.deal.dealId;
  const winner=await fixtureRpc(dispatch,sales.updateDeal,admin,{
    dealId:winnerId,name:{operation:{case:"set",value:"CAS winner"}},
    precondition:{expectedVersion:1n,idempotencyKey:"r471-cas-winner"}
  });
  assert.equal(winner.status,200,formatRpcFailure(winner));
  assert.equal(winner.body.deal?.version,2n);
  const casWinnerReceiptsBefore=await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts");
  const casWinnerAuditsBefore=await sql("SELECT COUNT(*) AS n FROM crm_audit");
  await assert.rejects(()=>batch([
    statement("UPDATE crm_sales_deal_records SET request_id=? WHERE tenant=? AND id=? AND version=?",
      "r471-losing-request","tenant-a",winnerId,1),
    statement("INSERT INTO crm_sales_link_delivery_records(tenant,delivery_id,event_id,deal_id,deal_version,disposition,state_json,request_id,actor) "+
      "VALUES(?,?,?,?,?,'ignored','{}',?,?)","tenant-a","r471-losing-delivery","r471-losing-event",
      winnerId,2,"r471-losing-request","native-a@example.test")
  ]),/SALES_LINK_SOURCE_COMMIT_MISMATCH/);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?",
    "tenant-a",winnerId))[0]?.n,0);
  assert.deepEqual(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts"),casWinnerReceiptsBefore);
  assert.deepEqual(await sql("SELECT COUNT(*) AS n FROM crm_audit"),casWinnerAuditsBefore);
  assert.deepEqual(await sql("SELECT original_request_id FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
    "tenant-a","r471-cas-winner"),[{original_request_id:winner.headers.get("x-request-id")}]);
  console.log(JSON.stringify({event:"crm.native-r7-occupied-stage-reorder",result:"PASS",
    stable_ids:true,open_closed_rebased:true,versions_preserved:true,
    audit_receipt_single_owner:true,cas:true,invalid_deletion_denied:true,
    catalog_assigned_ordinal:true}));
  // A finished Deal is reopened only through the explicit original owner.
  const reopenSale={dealId,reasonCode:"customer-revisited",
    precondition:{expectedVersion:4n,idempotencyKey:"connect-sales-reopen-main"}};
  assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,viewer,reopenSale)).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,admin,{
    ...reopenSale,precondition:{expectedVersion:3n,idempotencyKey:"r72-reopen-stale"}
  })).status,409);
  const reopenedSale=await fixtureRpc(dispatch,sales.reopenDeal,admin,reopenSale);
  assert.equal(reopenedSale.status,200,formatRpcFailure(reopenedSale));
  assert.equal(reopenedSale.body.deal?.status,1);
  assert.equal(reopenedSale.body.deal?.version,5n);
  assert.equal(reopenedSale.body.deal?.closedAt,undefined);
  assert.equal(reopenedSale.body.deal?.stage?.stageId,"qualified");
  assert.equal((await fixtureRpc(dispatch,sales.reopenDeal,admin,reopenSale)).body.receipt?.replayed,true);
  assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,admin,{
    dealId,reasonCode:"attempt-again",
    precondition:{expectedVersion:5n,idempotencyKey:"r72-reopen-already-open"}
  })).status,409);
  const afterReopen=await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId,page:{pageSize:25}});
  assert.deepEqual(afterReopen.body.events.map(e=>e.dealVersion),[5n,4n,3n,2n,1n]);
  assert.equal(afterReopen.body.events[0]?.kind,5);
  assert.equal(afterReopen.body.events[0]?.reasonCode,"customer-revisited");
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0]?.n,2,
    "reopen must not invent a stage-change delivery");
  const reclosed=await fixtureRpc(dispatch,sales.closeDeal,admin,{
    dealId,outcome:3,reasonCode:"customer-cancelled",
    precondition:{expectedVersion:5n,idempotencyKey:"r72-close-again"}
  });
  assert.equal(reclosed.status,200);
  assert.equal(reclosed.body.deal?.status,3);
  assert.equal(reclosed.body.deal?.version,6n);
  assert.equal((await fixtureRpc(dispatch,sales.closeDeal,admin,closeSale)).body.receipt?.replayed,true,
    "first CloseDeal must replay unchanged after stage reorder and reopen");
  const reopenAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id='"+dealId+"'"))[0]?.n;
  const reopenReceiptBefore=(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0]?.n;
  const reopenEventsBefore=(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_events "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0]?.n;
  assert.ok(typeof reopenAuditBefore === "number" && typeof reopenReceiptBefore === "number" &&
    typeof reopenEventsBefore === "number");
  (await sql("CREATE TRIGGER fixture_fail_reopen_audit BEFORE INSERT ON crm_audit "+
    "WHEN NEW.action='sales_deals.reopen' "+
    "BEGIN SELECT RAISE(ABORT,'fixture reopen audit'); END;"));
  try{
    const failed=await fixtureRpcError(dispatch,sales.reopenDeal,admin,{dealId,reasonCode:"revisit",
      precondition:{expectedVersion:6n,idempotencyKey:"r72-failed-reopen"}});
    assert.equal(failed.status,503);
    assert.equal(record(failed.body).message,"SALES_COMMIT_UNCONFIRMED");
  }finally{await sql("DROP TRIGGER fixture_fail_reopen_audit");}
  assert.equal((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId})).body.deal?.version,6n);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id='"+dealId+"'"))[0]?.n,reopenAuditBefore);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0]?.n,reopenReceiptBefore);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_events "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0]?.n,reopenEventsBefore);
  console.log(JSON.stringify({event:"crm.native-r72-deal-lifecycle",result:"PASS",
    separate_close_reopen:true,closed_readonly:true,actor_audit:true,
    append_only_history:true,cursor:true,linked_task:true,
    replay_after_reclose:true,stale_cas:true,rollback:true}));
  console.log(JSON.stringify({event:"crm.native-r2a-sales-linked-actor-audit",result:"PASS",
    all_sales_actions:true,linked_task_delivery:true,late_batch_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-sales-v2",result:"PASS",
  rpc_methods:["CreatePipeline","UpdatePipeline","GetPipeline","ListPipelines","ListStageDeals",
    "CreateDeal","UpdateDeal","AdvanceStage","CloseDeal","ReopenDeal",
    "ListDealEvents","GetDeal","ListDeals"],
  generated_binary:true,original_owner:"crm.sales",signed_access:true,tenant_ab:true,
  viewer_read:true,viewer_write_denied:true,exact_money:true,
  before_limit_filters:true,cas:true,idempotent_receipts:true,
  atomic_sales_activities:true,no_extra_audit:true,published_active:true}));

}

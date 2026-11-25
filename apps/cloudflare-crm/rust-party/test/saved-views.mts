import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type SavedViewsFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string;
 admin: string;
 viewer: string;
 tenantB: string;
};

export async function verifySavedViews({dispatch,sql,accountId,admin,viewer,tenantB}: SavedViewsFixture) {
 const sales=DealService.method;
// R7.3 named views: one signed Rust owner, tenant D1, replay, CAS, archived
// visibility, negative inputs and atomic audit rollback.
const viewAId="sales-view-"+randomUUID();
const viewBId="sales-view-"+randomUUID();
const viewAInput={
  viewId:viewAId,name:"Open enterprise",pipelineId:"pipeline.enterprise",
  status:1,accountId,
  precondition:{idempotencyKey:"r73-view-create-1"}
};
assert.equal((await fixtureRpcError(dispatch,sales.createSalesView,viewer,viewAInput)).status,403);
assert.equal((await fixtureRpcError(dispatch,sales.createSalesView,admin,{
  ...viewAInput,status:0,precondition:{idempotencyKey:"r73-view-invalid-status"}
})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createSalesView,admin,{
  ...viewAInput,pipelineId:"pipeline.unknown",
  precondition:{idempotencyKey:"r73-view-unknown-pipeline"}
})).status,404);
assert.equal((await fixtureRpcError(dispatch,sales.createSalesView,admin,{
  ...viewAInput,accountId:"bad/id",
  precondition:{idempotencyKey:"r73-view-invalid-account"}
})).status,400);
const viewCreated=await fixtureRpc(dispatch,sales.createSalesView,admin,viewAInput);
assert.equal(viewCreated.status,200,formatRpcFailure(viewCreated));
assert.equal(viewCreated.body.view?.version,1n);
const viewCreateProof=await fixtureRpc(dispatch,sales.getSalesView,viewer,{
  viewId:viewAId,receiptIdempotencyKey:"r73-view-create-1"
});
assert.equal(viewCreateProof.status,200);
assert.equal(viewCreateProof.body.confirmedMutation?.originalRequestId,
  viewCreated.body.receipt?.requestId);
assert.equal(viewCreateProof.body.confirmedMutation?.resourceVersion,1n);
assert.equal((await sql("SELECT original_request_id FROM crm_sales_saved_view_receipts "+
  "WHERE tenant='tenant-a' AND key='r73-view-create-1'"))[0]?.original_request_id,
  viewCreated.body.receipt?.requestId);
assert.equal(viewCreated.body.view?.status,1);
assert.equal(viewCreated.body.view?.accountId,accountId);
assert.equal((await fixtureRpc(dispatch,sales.createSalesView,admin,viewAInput)).body.receipt?.replayed,true);
assert.equal((await fixtureRpcError(dispatch,sales.createSalesView,admin,{
  ...viewAInput,name:"different"
})).status,409);
assert.equal((await fixtureRpcError(dispatch,sales.getSalesView,tenantB,{viewId:viewAId})).status,404);
assert.equal((await fixtureRpc(dispatch,sales.listSalesViews,tenantB,{page:{pageSize:10}})).body.views.length,0);
assert.equal((await fixtureRpc(dispatch,sales.getSalesView,viewer,{viewId:viewAId})).body.view?.viewId,viewAId);
const viewBCreated=await fixtureRpc(dispatch,sales.createSalesView,admin,{
  viewId:viewBId,name:"Won across pipelines",status:2,
  precondition:{idempotencyKey:"r73-view-create-2"}
});
assert.equal(viewBCreated.status,200,formatRpcFailure(viewBCreated));
const wrongViewProof=await fixtureRpc(dispatch,sales.getSalesView,viewer,{
  viewId:viewBId,receiptIdempotencyKey:"r73-view-create-1"
});
assert.equal(wrongViewProof.status,200);
assert.equal(wrongViewProof.body.confirmedMutation,undefined,
  "A receipt for another saved view must never be disclosed");
const firstViewsPage=await fixtureRpc(dispatch,sales.listSalesViews,viewer,{page:{pageSize:1}});
assert.equal(firstViewsPage.status,200,formatRpcFailure(firstViewsPage));
assert.equal(firstViewsPage.body.views.length,1);
assert.ok(firstViewsPage.body.pageInfo?.nextPageToken);
const secondViewsPage=await fixtureRpc(dispatch,sales.listSalesViews,viewer,{
  page:{pageSize:1,pageToken:firstViewsPage.body.pageInfo.nextPageToken}
});
assert.equal(secondViewsPage.body.views.length,1);
assert.equal(secondViewsPage.body.pageInfo?.nextPageToken,"");
assert.deepEqual(new Set([firstViewsPage.body.views[0]?.viewId,secondViewsPage.body.views[0]?.viewId]),
  new Set([viewAId,viewBId]));
const viewUpdate={
  viewId:viewAId,name:"Won in enterprise",pipelineId:"pipeline.enterprise",
  status:2,precondition:{expectedVersion:1n,idempotencyKey:"r73-view-update-1"}
};
const updatedView=await fixtureRpc(dispatch,sales.updateSalesView,admin,viewUpdate);
assert.equal(updatedView.status,200,formatRpcFailure(updatedView));
assert.equal(updatedView.body.view?.version,2n);
const viewUpdateProof=await fixtureRpc(dispatch,sales.getSalesView,viewer,{
  viewId:viewAId,receiptIdempotencyKey:"r73-view-update-1"
});
assert.equal(viewUpdateProof.status,200);
assert.equal(viewUpdateProof.body.confirmedMutation?.originalRequestId,
  updatedView.body.receipt?.requestId);
assert.equal(viewUpdateProof.body.confirmedMutation?.resourceVersion,2n);
const oldViewCreateProof=await fixtureRpc(dispatch,sales.getSalesView,viewer,{
  viewId:viewAId,receiptIdempotencyKey:"r73-view-create-1"
});
assert.equal(oldViewCreateProof.body.confirmedMutation?.originalRequestId,
  viewCreated.body.receipt?.requestId);
assert.equal(oldViewCreateProof.body.confirmedMutation?.resourceVersion,1n);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id='"+
  viewUpdateProof.body.confirmedMutation?.requestId+"'"))[0]?.n,0,
  "Saved-view receipt lookup is read-only");
assert.equal(updatedView.body.view?.accountId,undefined);
assert.equal((await fixtureRpc(dispatch,sales.updateSalesView,admin,viewUpdate)).body.receipt?.replayed,true);
assert.equal((await fixtureRpcError(dispatch,sales.updateSalesView,admin,{
  ...viewUpdate,name:"stale",precondition:{
    expectedVersion:1n,idempotencyKey:"r73-view-update-stale"}
})).status,409);
const beforeFailedView=(await sql("SELECT state,version FROM crm_sales_saved_view_records WHERE tenant='tenant-a' AND id='"+viewAId+"'"));
const beforeFailedReceipts=(await sql("SELECT COUNT(*) AS n FROM crm_sales_saved_view_receipts WHERE tenant='tenant-a'"))[0]?.n;
(await sql("CREATE TRIGGER fixture_fail_saved_view_audit BEFORE INSERT ON crm_audit "+
  "WHEN NEW.action='sales_view.update' AND NEW.subject_id='"+viewAId+"' "+
  "BEGIN SELECT RAISE(ABORT,'fixture view audit rollback'); END;"));
try {
  const failed=await fixtureRpcError(dispatch,sales.updateSalesView,admin,{
    ...viewUpdate,name:"Failed write",precondition:{
      expectedVersion:2n,idempotencyKey:"r73-view-audit-rollback"}
  });
  assert.equal(failed.status,503);
  assert.equal(failed.body.code,"unavailable");
  const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
  assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
  assert.equal(detail.domainCode,"SALES_VIEW_WRITE_UNCONFIRMED");
  assert.equal(detail.requestId,failed.headers.get("x-request-id"));
  assert.deepEqual((await sql("SELECT state,version FROM crm_sales_saved_view_records "+
    "WHERE tenant='tenant-a' AND id='"+viewAId+"'")),beforeFailedView);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_saved_view_receipts "+
    "WHERE tenant='tenant-a'"))[0]?.n,beforeFailedReceipts);
} finally {await sql("DROP TRIGGER fixture_fail_saved_view_audit");}
const archivedView=await fixtureRpc(dispatch,sales.updateSalesView,admin,{
  ...viewUpdate,archived:true,precondition:{
    expectedVersion:2n,idempotencyKey:"r73-view-archive"}
});
assert.equal(archivedView.status,200,formatRpcFailure(archivedView));
assert.equal(archivedView.body.view?.archived,true);
assert.deepEqual((await fixtureRpc(dispatch,sales.listSalesViews,viewer,{})).body.views.map(x=>x.viewId),[viewBId]);
assert.equal((await fixtureRpc(dispatch,sales.listSalesViews,viewer,{includeArchived:true})).body.views.length,2);
assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+viewAId+"' ORDER BY rowid")).map(x=>x.action),
  ["sales_view.create","sales_view.update","sales_view.update"]);
console.log(JSON.stringify({event:"crm.native-r73-saved-views",result:"PASS",
  typed_filters:true,tenant_isolation:true,keyset:true,cas:true,
  replay:true,signed_audit:true,archive:true,atomic_rollback:true}));
}

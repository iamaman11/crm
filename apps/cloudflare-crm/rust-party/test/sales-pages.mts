import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {collectFixturePages, fixtureRpc, fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type PagesFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string; viewer: string; tenantB: string; accountId: string; dealId: string;
};
export async function verifySalesPages({dispatch,sql,admin,viewer,tenantB,accountId,dealId}: PagesFixture) {
  const sales=DealService.method;
  const dealOther="sales-aaa-other-"+randomUUID();
  const dealLast="sales-zzz-ours-"+randomUUID();
  for(const [index,nextId,pipe] of [
    [0,dealOther,"pipeline.other"],[1,dealLast,"pipeline.enterprise"]
  ] as const){
    const created=await fixtureRpc(dispatch,sales.createDeal,admin,{
      dealId:nextId,name:"Sales page "+index,accountId,
      stage:{pipelineId:pipe,stageId:"lead",ordinal:1},probabilityBasisPoints:1000,
      precondition:{idempotencyKey:index===0?"r471-ci-ack-lost":"connect-sales-paging-"+index}});
    assert.equal(created.status,200,formatRpcFailure(created));
    if(index===0){
      assert.ok(created.body.receipt);
      assert.equal(created.body.receipt.replayed,true);
      assert.equal(created.body.receipt.originalRequestId,created.headers.get("x-request-id"));
      assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?",
        "tenant-a",nextId))[0]?.n,1);
      console.log(JSON.stringify({event:"crm.native.s1-ack-reconciled",result:"PASS",receipt:true,single_audit:true}));
    }
  }
  assert.equal((await fixtureRpcError(dispatch,sales.listDeals,viewer,{page:{pageSize:101}})).status,400);
  assert.equal((await fixtureRpcError(dispatch,sales.listDeals,viewer,{status:0})).status,400);
  assert.equal((await fixtureRpc(dispatch,sales.listDeals,tenantB,{page:{pageSize:1}})).body.deals.length,0);
  const pages=async(filter: {status?: 2; pipelineId?: string})=>collectFixturePages(async pageToken=>{
    const {body}=await fixtureRpc(dispatch,sales.listDeals,viewer,{...filter,page:{pageSize:1,pageToken}});
    return {items:body.deals,nextPageToken:body.pageInfo?.nextPageToken};
  });
  assert.deepEqual((await pages({status:2})).map(x=>x.dealId),[dealId]);
  assert.deepEqual((await pages({pipelineId:"pipeline.enterprise"})).map(x=>x.dealId),[dealId,dealLast]);
  assert.equal((await pages({pipelineId:"pipeline.other"})).length,1);
  return {dealOther,dealLast};
}

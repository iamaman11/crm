import assert from "node:assert/strict";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {collectFixturePages, fixtureRpc, fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";

type SearchFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  viewer: string; tenantB: string; accountId: string; dealId: string; dealOther: string; dealLast: string;
};

export async function verifySalesSearch({dispatch,sql,viewer,tenantB,accountId,dealId,dealOther,dealLast}: SearchFixture) {
  const list=DealService.method.listDeals;
  const beforeDealLookupAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
  assert.equal(typeof beforeDealLookupAudit,"number");
  const searchSales=await collectFixturePages(async pageToken=>{
    const {body}=await fixtureRpc(dispatch,list,viewer,{search:"Sales page",page:{pageSize:1,pageToken}});
    return {items:body.deals,nextPageToken:body.pageInfo?.nextPageToken};
  });
  assert.deepEqual(searchSales.map(x=>x.dealId),[dealOther,dealLast],
    "both matching Deals across pipelines must survive pageSize=1 and keyset cursor");
  const pages=async(filter: {search: string; pipelineId?: string; status?: 2; accountId?: string})=>
    collectFixturePages(async pageToken=>{
      const {body}=await fixtureRpc(dispatch,list,viewer,{...filter,page:{pageSize:1,pageToken}});
      return {items:body.deals,nextPageToken:body.pageInfo?.nextPageToken};
    });
  assert.deepEqual((await pages({search:"  Sales page 1  ",pipelineId:"pipeline.enterprise"})).map(x=>x.dealId),[dealLast],
    "server trims search and composes pipeline before LIMIT");
  assert.deepEqual((await pages({search:"Updated Connect Deal",status:2,accountId})).map(x=>x.dealId),[dealId],
    "search intersects exact status and Account filters");
  assert.deepEqual((await pages({search:dealOther})).map(x=>x.dealId),[dealOther],
    "exact ID fragments also find the owner-authorized Deal");
  for(const literal of ["%","_","'","Несуществующая сделка"]){
    assert.equal((await fixtureRpc(dispatch,list,viewer,{search:literal,page:{pageSize:1}})).body.deals.length,0,
      "special search characters must be literal, not SQL wildcards");
  }
  assert.equal((await fixtureRpcError(dispatch,list,viewer,{search:"X".repeat(201)})).status,400);
  assert.equal((await fixtureRpc(dispatch,list,viewer,{search:"X".repeat(200)})).status,200);
  assert.equal((await fixtureRpc(dispatch,list,tenantB,{search:"Sales page"})).body.deals.length,0);
  assert.equal((await fixtureRpcError(dispatch,list,null,{search:"Sales page"})).status,401);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
    beforeDealLookupAudit,"Deal search and its negative cases are read-only");
  console.log(JSON.stringify({event:"crm.native-m2-deal-name-search",result:"PASS",
    typed:true,server_before_limit:true,literal:true,tenant:true,readonly:true}));
}

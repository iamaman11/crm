import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {AccountService} from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import {fixtureRpc, fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type BoardFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string; viewer: string; tenantB: string; accountId: string; dealLast: string;
};
export async function verifySalesBoard({dispatch,sql,admin,viewer,tenantB,accountId,dealLast}: BoardFixture) {
const sales=DealService.method;
// A non-UI test pipeline has more than one board page. The server count must
// be 27, not the 25 records rendered in a stage column.
for(let index=0;index<26;index++){
  const created=await fixtureRpc(dispatch,sales.createDeal,admin,{
    dealId:"r7-overflow-"+String(index).padStart(2,"0"),name:"Pagination Deal "+index,
    accountId,stage:{pipelineId:"pipeline.other",stageId:"lead",ordinal:1},
    probabilityBasisPoints:1000,
    precondition:{idempotencyKey:"r7-stage-pagination-"+index}
  });
  assert.equal(created.status,200,formatRpcFailure(created));
}
const overflow=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:25}});
assert.equal(overflow.status,200,formatRpcFailure(overflow));
assert.equal(overflow.body.totalCount,27n,"board count must include all 27, not the page");
assert.equal(overflow.body.deals.length,25);
const account=(await fixtureRpc(dispatch,AccountService.method.getAccount,viewer,{accountId})).body.account;
assert.ok(account);
const ownerAccountName=account.name;
assert.deepEqual(Object.entries(overflow.body.accountNames),[[accountId,ownerAccountName]],
  "25 cards sharing an Account carry one canonical name");
const salesNamedPage=await fixtureRpc(dispatch,sales.listDeals,viewer,{accountId,page:{pageSize:25}});
assert.equal(salesNamedPage.status,200);
assert.deepEqual(Object.entries(salesNamedPage.body.accountNames),[[accountId,ownerAccountName]]);
assert.deepEqual((await fixtureRpc(dispatch,sales.listDeals,admin,{accountId,page:{pageSize:25}})).body.accountNames,
  salesNamedPage.body.accountNames);
assert.deepEqual(Object.keys((await fixtureRpc(dispatch,sales.listDeals,tenantB,{accountId})).body.accountNames),[]);
assert.deepEqual(overflow.body.currencyTotals,[],
  "missing optional Money is not zero currency or an invented total");
assert.ok(overflow.body.pageInfo?.nextPageToken);
const overflowLast=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",
  page:{pageSize:25,pageToken:overflow.body.pageInfo.nextPageToken}});
assert.equal(overflowLast.status,200);
assert.equal(overflowLast.body.deals.length,2);
assert.equal(overflowLast.body.totalCount,27n);
assert.deepEqual(overflowLast.body.accountNames,overflow.body.accountNames);
assert.deepEqual(overflowLast.body.currencyTotals,overflow.body.currencyTotals);
const stageCount=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.enterprise",stageId:"lead",page:{pageSize:1}});
assert.equal(stageCount.status,200,formatRpcFailure(stageCount));
assert.equal(stageCount.body.totalCount,1n);
assert.equal(stageCount.body.deals.length,1);
assert.equal(stageCount.body.deals[0]?.dealId,dealLast);
assert.deepEqual(stageCount.body.currencyTotals,[]);
const qualifiedTotals=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.enterprise",stageId:"qualified",page:{pageSize:1}});
assert.equal(qualifiedTotals.status,200,formatRpcFailure(qualifiedTotals));
assert.deepEqual(qualifiedTotals.body.currencyTotals.map(t=>[
  t.currencyCode,t.totalMinorUnits,t.amountDealCount
]),[["PLN","98765",1n]],
  "closed money is included unless the caller selects a status filter");
assert.deepEqual((await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.enterprise",stageId:"qualified",status:1
})).body.currencyTotals,[],
  "status filter is applied to BOTH stage counts and summaries");

// Five monetary Deals are added AFTER the 27-row pagination fixture:
// 2 USD amounts exceed int64 when aggregated, while individually valid.
// USD, PLN, JPY and KWD must never be added across currencies.
const currencyCases=[
  ["USD",9223372036854775807n],
  ["USD",1n],
  ["PLN",2345n],
  ["JPY",345n],
  ["KWD",4567n],
] as const;
for(const [index,[currencyCode,minorUnits]] of currencyCases.entries()){
  const made=await fixtureRpc(dispatch,sales.createDeal,admin,{
    dealId:"r7-totals-"+String(index).padStart(2,"0"),
    name:"Currency Deal "+index,accountId,
    stage:{pipelineId:"pipeline.other",stageId:"lead",ordinal:999},
    amount:{minorUnits,currencyCode},probabilityBasisPoints:2000,
    ...(index===0?{expectedCloseDate:{year:2020,month:1,day:1}}:
      index===1?{expectedCloseDate:{year:2030,month:1,day:1}}:{}),
    precondition:{idempotencyKey:"r7-stage-totals-"+index}
  });
  assert.equal(made.status,200,formatRpcFailure(made));
}
const totalView=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:1}});
assert.equal(totalView.status,200,formatRpcFailure(totalView));
assert.equal(totalView.body.totalCount,32n);
assert.equal(totalView.body.deals.length,1);
assert.ok(totalView.body.pageInfo?.nextPageToken);
assert.deepEqual(totalView.body.currencyTotals.map(t=>[
  t.currencyCode,t.totalMinorUnits,t.amountDealCount
]),[
  ["JPY","345",1n],["KWD","4567",1n],["PLN","2345",1n],
  ["USD","9223372036854775808",2n]
], "full-stage exact totals must not overflow int64 or mix currencies");
const initialMetric=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{
  pipelineId:"pipeline.other"});
assert.equal(initialMetric.status,200,formatRpcFailure(initialMetric));
assert.equal(initialMetric.body.totalDealCount,32n);
assert.deepEqual(initialMetric.body.statuses.map(x=>[x.stageId,x.status,x.dealCount]),
  [["lead",1,32n]],"metrics must count all Deals beyond one 25-row page");
assert.deepEqual(initialMetric.body.currencies.map(x=>[
  x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
]),[
  ["JPY","345","690000",1n],
  ["KWD","4567","9134000",1n],
  ["PLN","2345","4690000",1n],
  ["USD","9223372036854775808","18446744073709551616000",2n]
], "exact 128-bit sums, exact weighted numerator and currency separation");
// Dates are evaluated in UTC against one server snapshot, not by current
// browser page or client timezone. The original 27 Deals have no expected date.
const operational=initialMetric.body.operational;
assert.equal(operational.length,1);
assert.ok(operational[0]);
assert.equal(operational[0].stageId,"lead");
assert.equal(operational[0].status,1);
assert.equal(operational[0].overdueExpectedCloseCount,1n);
assert.equal(operational[0].dueNext30DaysCount,0n);
assert.equal(operational[0].dueLaterCount,1n);
assert.equal(operational[0].noExpectedCloseCount,30n);
assert.equal(operational[0].knownStageAgeCount,32n);
assert.ok(BigInt(operational[0].totalStageAgeSeconds)>=0n);
assert.equal(operational[0].overdueLinkedTaskCount,0n);
assert.ok(initialMetric.body.asOfUnixSeconds>0n);
const metricsTenantDenied=await fixtureRpcError(dispatch,sales.getPipelineMetrics,tenantB,{
  pipelineId:"pipeline.other"});
assert.equal(metricsTenantDenied.status,404,"other tenant cannot read foreign Pipeline totals");
const portfolioAudit=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n;
assert.equal(typeof portfolioAudit,"number");
const portfolioRequest={pipelineId:"pipeline.other"};
const portfolio=(await fixtureRpc(dispatch,sales.getSalesPortfolio,viewer,portfolioRequest));
assert.equal(portfolio.status,200,formatRpcFailure(portfolio));
assert.ok(portfolio.body.counts);
assert.ok(portfolio.body.pipelines[0]);
assert.ok(portfolio.body.pipelines[0].stages[0]);
assert.equal(portfolio.body.counts.pipelineCount,1n);
assert.equal(portfolio.body.counts.totalDealCount,32n);
assert.equal(portfolio.body.counts.openCount,32n);
assert.equal(portfolio.body.counts.noExpectedCloseCount,30n);
assert.equal(portfolio.body.counts.overdueExpectedCloseCount,1n);
assert.deepEqual(portfolio.body.pipelines[0].counts,portfolio.body.counts);
assert.equal(portfolio.body.pipelines[0].stages[0].dealCount,32n);
assert.equal(portfolio.body.pipelines[0].stages[0].stageId,"lead");
assert.ok(portfolio.body.pipelines[0].stages[0].averageStageAgeDays !== undefined);
assert.ok(portfolio.body.pipelines[0].stages[0].averageStageAgeDays>=0n);
assert.deepEqual(portfolio.body.forecastCurrencies.map(x=>[
  x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
]),initialMetric.body.currencies.map(x=>[
  x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
]));
const portfolioAdmin=(await fixtureRpc(dispatch,sales.getSalesPortfolio,admin,portfolioRequest)).body;
assert.deepEqual(portfolioAdmin.counts,portfolio.body.counts);
assert.deepEqual(portfolioAdmin.forecastCurrencies,portfolio.body.forecastCurrencies);
assert.equal((await fixtureRpcError(dispatch,sales.getSalesPortfolio,tenantB,portfolioRequest)).status,404);
assert.equal((await fixtureRpcError(dispatch,sales.getSalesPortfolio,viewer,{...portfolioRequest,comparePrevious:true})).status,400);
const comparison=await fixtureRpc(dispatch,sales.getSalesPortfolio,viewer,{...portfolioRequest,comparePrevious:true,
  analysisPeriod:{startDate:{year:2026,month:3,day:1},endDate:{year:2026,month:3,day:2}}});
assert.equal(comparison.status,200,formatRpcFailure(comparison));
assert.ok(comparison.body.previousPeriod);
assert.ok(comparison.body.previousClosedOutcomes);
assert.deepEqual(comparison.body.previousPeriod.startDate,{...comparison.body.previousPeriod.startDate,year:2026,month:2,day:27});
assert.deepEqual(comparison.body.previousPeriod.endDate,{...comparison.body.previousPeriod.endDate,year:2026,month:2,day:28});
assert.equal(comparison.body.previousClosedOutcomes.denominator,0n);
assert.equal(comparison.body.previousClosedOutcomes.winRateBasisPoints,undefined);
assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n,portfolioAudit);
console.log(JSON.stringify({event:"crm.native-sales-portfolio",result:"PASS",exact_i128:true,server_period:true,viewer_parity:true,tenant_ab:true,no_read_audit:true}));
assert.equal((await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{
  pipelineId:"pipeline.other",status:2})).body.totalDealCount,0n);
assert.equal((await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{
  pipelineId:"pipeline.other",accountId:randomUUID()})).body.totalDealCount,0n);
assert.equal((await fixtureRpcError(dispatch,sales.getPipelineMetrics,viewer,{
  pipelineId:"pipeline.other",status:0})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.getPipelineMetrics,viewer,{
  pipelineId:"pipeline.missing"})).status,404);
const totalNext=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",
  page:{pageSize:1,pageToken:totalView.body.pageInfo.nextPageToken}});
assert.equal(totalNext.status,200);
assert.equal(totalNext.body.deals.length,1);
assert.equal(totalNext.body.totalCount,32n);
assert.deepEqual(totalNext.body.currencyTotals,totalView.body.currencyTotals,
  "summary must not depend on independently paginated Deal cards");
assert.equal((await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",status:2
})).body.totalCount,0n);
assert.deepEqual((await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",status:2
})).body.currencyTotals,[]);
const missingAccount=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",accountId:randomUUID()});
assert.equal(missingAccount.status,200);
assert.equal(missingAccount.body.totalCount,0n);
assert.deepEqual(Object.keys(missingAccount.body.accountNames),[]);
assert.deepEqual(missingAccount.body.currencyTotals,[]);
const updatedCurrency=await fixtureRpc(dispatch,sales.updateDeal,admin,{
  dealId:"r7-totals-01",
  amount:{operation:{case:"set",value:{minorUnits:2n,currencyCode:"USD"}}},
  precondition:{expectedVersion:1n,idempotencyKey:"r7-stage-totals-update"}
});
assert.equal(updatedCurrency.status,200,formatRpcFailure(updatedCurrency));
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,admin,{
  dealId:"r7-totals-01",
  amount:{operation:{case:"set",value:{minorUnits:3n,currencyCode:"USD"}}},
  precondition:{expectedVersion:1n,idempotencyKey:"r7-stage-totals-stale"}
})).status,409);
const afterCurrencyUpdate=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
  pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:1}});
assert.equal(afterCurrencyUpdate.body.totalCount,32n);
assert.equal(afterCurrencyUpdate.body.currencyTotals
  .find(t=>t.currencyCode==="USD")?.totalMinorUnits,"9223372036854775809",
  "failed CAS cannot affect exact full-stage sum");
assert.equal((await fixtureRpcError(dispatch,sales.listStageDeals,tenantB,{
  pipelineId:"pipeline.other",stageId:"lead"
})).status,404, "no cross-tenant summaries");
console.log(JSON.stringify({event:"crm.native-r71-board-currency-totals",result:"PASS",
  full_stage_count:32,visible_page:1,distinct_currencies:4,
  amount_exceeds_int64:true,missing_amount:true,
  status_account_filters:true,tenant_isolation:true,stale_cas:true}));

}

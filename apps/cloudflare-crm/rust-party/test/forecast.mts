import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import type { SalesAnalysisPeriod } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type CalendarDate = Pick<NonNullable<SalesAnalysisPeriod["startDate"]>, "year" | "month" | "day">;
type AnalysisPeriod = {startDate?: CalendarDate; endDate?: CalendarDate};
type ForecastFixture = {dispatch: RpcDispatch; sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>; accountId: string; admin: string; viewer: string; tenantB: string};
export async function verifyForecast({dispatch,sql,accountId,admin,viewer,tenantB}: ForecastFixture) {
 const sales=DealService.method;
 // R7.3 forecast uses the same original owner, CAS, D1 binding and
 // read snapshot; this isolated tenant-a pipeline is created after the
 // existing regression scenarios and is disposed with the native database.
 {
   const pipelineId="pipeline.forecast-native";
   const stages=[
     {stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
     {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500},
     {stageId:"proposal",name:"Proposal",probabilityBasisPoints:8000}
   ];
   const pre=(suffix: string,version?: bigint)=>({
     idempotencyKey:"r73-forecast-"+suffix,...(version!==undefined?{expectedVersion:version}:{})
   });
   const pipeline=await fixtureRpc(dispatch,sales.createPipeline,admin,{
     pipelineId,name:"Forecast native",stages,precondition:pre("pipeline")
   });
   assert.equal(pipeline.status,200,formatRpcFailure(pipeline));
   const ids=[randomUUID(),randomUUID(),randomUUID()] as const;
   const create=(id: string,category: number,units: bigint,currency: string,probability?: number)=>({
     dealId:id,name:"Forecast "+id,accountId,stage:{pipelineId,stageId:"lead",ordinal:1},
     forecastCategory:category,amount:{minorUnits:units,currencyCode:currency},
     ...(probability===undefined?{}:{probabilityBasisPoints:probability}),
     precondition:pre("create-"+id)
   });
   const firstInput={...create(ids[0],3,10001n,"EUR"),expectedCloseDate:{year:2027,month:2,day:28}};
   const first=await fixtureRpc(dispatch,sales.createDeal,admin,firstInput);
   const second=await fixtureRpc(dispatch,sales.createDeal,admin,{
     ...create(ids[1],2,2500n,"USD",0),expectedCloseDate:{year:2027,month:3,day:1}
   });
   const third=await fixtureRpc(dispatch,sales.createDeal,admin,create(ids[2],4,985n,"EUR",5000));
   for(const result of [first,second,third])assert.equal(result.status,200,formatRpcFailure(result));
   assert.ok(first.body.deal && second.body.deal && third.body.deal);
   assert.equal(first.body.deal.probabilityBasisPoints,1000);
   assert.equal(first.body.deal.probabilitySource,2);
   assert.equal(first.body.deal.forecastCategory,3);
   assert.equal(second.body.deal.probabilityBasisPoints,0);
   assert.equal(second.body.deal.probabilitySource,1);
   assert.equal(third.body.deal.forecastCategory,4);
   const replay=await fixtureRpc(dispatch,sales.createDeal,admin,firstInput);
   assert.equal(replay.status,200);
   assert.ok(replay.body.receipt && replay.body.deal);
   assert.equal(replay.body.receipt.replayed,true);
   assert.equal(replay.body.deal.probabilitySource,2);
   const badCategory=await fixtureRpcError(dispatch,sales.updateDeal,admin,{
     dealId:ids[0],forecastCategory:0,precondition:pre("invalid-category",1n)
   });
   assert.equal(badCategory.status,400);
   const badSource=await fixtureRpcError(dispatch,sales.updateDeal,admin,{
     dealId:ids[0],useStageProbability:true,probabilityBasisPoints:5,precondition:pre("invalid-source",1n)
   });
   assert.ok([400,422].includes(badSource.status),formatRpcFailure(badSource));
   const move=async(id: string,targetStage: string,version: bigint,suffix: string)=>fixtureRpc(dispatch,sales.advanceStage,admin,{
     dealId:id,targetStage:{pipelineId,stageId:targetStage,ordinal:2},targetStatus:1,precondition:pre(suffix,version)
   });
   const moved=await move(ids[0],"qualified",1n,"stage-move");
   assert.equal(moved.status,200,formatRpcFailure(moved));
   assert.ok(moved.body.deal);
   assert.equal(moved.body.deal.probabilityBasisPoints,4500);
   assert.equal(moved.body.deal.probabilitySource,2);
   const movedManual=await move(ids[1],"qualified",1n,"manual-move");
   assert.equal(movedManual.status,200,formatRpcFailure(movedManual));
   assert.ok(movedManual.body.deal);
   assert.equal(movedManual.body.deal.probabilityBasisPoints,0);
   assert.equal(movedManual.body.deal.probabilitySource,1);
   const reset=await fixtureRpc(dispatch,sales.updateDeal,admin,{
     dealId:ids[1],useStageProbability:true,forecastCategory:3,
     precondition:pre("reset",movedManual.body.deal.version)
   });
   assert.equal(reset.status,200,formatRpcFailure(reset));
   assert.ok(reset.body.deal);
   assert.equal(reset.body.deal.probabilityBasisPoints,4500);
   assert.equal(reset.body.deal.probabilitySource,2);
   assert.equal(reset.body.deal.forecastCategory,3);
   const stale=await fixtureRpcError(dispatch,sales.updateDeal,admin,{
     dealId:ids[1],forecastCategory:1,precondition:pre("stale",1n)
   });
   assert.equal(stale.status,409);
   const metric=async(status?: number)=>fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{
     pipelineId,accountId,...(status===undefined?{}:{status})
   });
   const summary=await metric();
   assert.equal(summary.status,200,formatRpcFailure(summary));
   assert.deepEqual(summary.body.forecastCategories.map(x=>[x.category,x.dealCount,x.stageDefaultCount,x.manualCount]),
     [[3,2n,2n,0n],[4,1n,0n,1n]]);
   assert.deepEqual(summary.body.forecastCurrencies.map(x=>[x.category,x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount]),
     [[3,"EUR","10001","45004500",1n],[3,"USD","2500","11250000",1n],[4,"EUR","985","4925000",1n]]);
   const windowMetric=async(analysisPeriod: AnalysisPeriod)=>fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,{
     pipelineId,accountId,analysisPeriod
   });
   const windowMetricError=async(analysisPeriod: AnalysisPeriod)=>fixtureRpcError(dispatch,sales.getPipelineMetrics,viewer,{pipelineId,accountId,analysisPeriod});
   const day={year:2027,month:2,day:28};
   const window=await windowMetric({startDate:day,endDate:day});
   assert.equal(window.status,200,formatRpcFailure(window));
   assert.equal(window.body.totalDealCount,3n,"window must not filter board inventory");
   assert.deepEqual(window.body.forecastCurrencies.map(x=>[x.category,x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints]),
     [[3,"EUR","10001","45004500"]]);
   assert.deepEqual(window.body.undatedForecastCurrencies.map(x=>[x.category,x.currencyCode,x.totalMinorUnits]),
     [[4,"EUR","985"]]);
   assert.ok(window.body.closedOutcomes);
   assert.equal(window.body.closedOutcomes?.denominator,0n);
   assert.equal(window.body.closedOutcomes.winRateBasisPoints,undefined);
   assert.equal((await windowMetricError({startDate:{year:2027,month:3,day:1},endDate:day})).status,400);
   assert.equal((await windowMetricError({startDate:{year:2027,month:2,day:29},endDate:day})).status,400);
   assert.equal((await windowMetricError({startDate:day})).status,400);
   const closed=await fixtureRpc(dispatch,sales.closeDeal,admin,{
     dealId:ids[0],outcome:2,reasonCode:"forecast-accepted",precondition:pre("close",moved.body.deal.version)
   });
   assert.equal(closed.status,200,formatRpcFailure(closed));
   assert.equal((await metric(2)).body.forecastCategories.length,0);
   const openSummary=await metric();
   assert.ok(openSummary.body.closedOutcomes);
   assert.equal(openSummary.body.closedOutcomes?.wonCount,1n);
   assert.equal(openSummary.body.closedOutcomes.lostCount,0n);
   assert.equal(openSummary.body.closedOutcomes?.denominator,1n);
   assert.equal(openSummary.body.closedOutcomes.winRateBasisPoints,10000);
   assert.equal((await metric(2)).body.closedOutcomes,undefined);
   assert.ok(closed.body.deal?.closedAt);
   const closeDay=new Date(Number(closed.body.deal.closedAt.seconds)*1000);
   const utcDay={year:closeDay.getUTCFullYear(),month:closeDay.getUTCMonth()+1,day:closeDay.getUTCDate()};
   assert.equal((await windowMetric({startDate:utcDay,endDate:utcDay})).body.closedOutcomes?.wonCount,1n);
   assert.equal((await windowMetric({startDate:{year:2000,month:1,day:1},endDate:{year:2000,month:1,day:1}})).body.closedOutcomes?.denominator,0n);
   assert.equal(openSummary.body.totalDealCount,3n);
   assert.equal(openSummary.body.forecastCategories.find(x=>x.category===3)?.dealCount,1n);
   assert.equal(openSummary.body.forecastCurrencies.some(x=>x.category===3&&x.currencyCode==="EUR"),false);
   const lost=await fixtureRpc(dispatch,sales.closeDeal,admin,{
     dealId:ids[2],outcome:3,reasonCode:"forecast-lost",precondition:pre("close-lost",third.body.deal.version)
   });
   assert.equal(lost.status,200,formatRpcFailure(lost));
   const bothClosed=(await metric()).body.closedOutcomes;
   assert.ok(bothClosed);
   assert.deepEqual([bothClosed.wonCount,bothClosed.lostCount,bothClosed.denominator,bothClosed.winRateBasisPoints],
     [1n,1n,2n,5000]);
   assert.ok(lost.body.deal);
   const lostReopened=await fixtureRpc(dispatch,sales.reopenDeal,admin,{
     dealId:ids[2],reasonCode:"forecast-lost-reopened",precondition:pre("reopen-lost",lost.body.deal.version)
   });
   assert.equal(lostReopened.status,200,formatRpcFailure(lostReopened));
   const reopened=await fixtureRpc(dispatch,sales.reopenDeal,admin,{
     dealId:ids[0],reasonCode:"forecast-reopened",precondition:pre("reopen",closed.body.deal.version)
   });
   assert.equal(reopened.status,200,formatRpcFailure(reopened));
   assert.ok(reopened.body.deal);
   assert.equal(reopened.body.deal.forecastCategory,3);
   assert.equal(reopened.body.deal.probabilityBasisPoints,4500);
   assert.equal((await metric()).body.closedOutcomes?.denominator,0n);
   const persisted=await sql("SELECT json_extract(state,'$.probability_source') AS source,json_extract(state,'$.forecast_category') AS category FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id=?",ids[0]);
   assert.deepEqual(persisted,[{source:"stage_default",category:"commit"}]);
   assert.deepEqual((await metric()).body.forecastCurrencies,summary.body.forecastCurrencies);
   // Catalog configuration changes must not silently edit existing Deals.
   assert.ok(pipeline.body.pipeline);
   const updatedCatalog=await fixtureRpc(dispatch,sales.updatePipeline,admin,{
     pipelineId,name:"Forecast native",stages:stages.map(stage=>stage.stageId==="qualified"?
       {...stage,probabilityBasisPoints:6000}:stage),archived:false,
     precondition:pre("catalog-edit",pipeline.body.pipeline.version)
   });
   assert.equal(updatedCatalog.status,200,formatRpcFailure(updatedCatalog));
   const retained=await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:ids[0]});
   assert.equal(retained.status,200);
   assert.ok(retained.body.deal);
   assert.equal(retained.body.deal.probabilityBasisPoints,4500);
   const refresh=await fixtureRpc(dispatch,sales.updateDeal,admin,{
     dealId:ids[0],useStageProbability:true,
     precondition:pre("refresh-default",retained.body.deal.version)
   });
   assert.equal(refresh.status,200,formatRpcFailure(refresh));
   assert.ok(refresh.body.deal);
   assert.equal(refresh.body.deal.probabilityBasisPoints,6000);
   assert.equal(refresh.body.deal.probabilitySource,2);
   assert.equal((await metric()).body.forecastCurrencies.find(x=>x.category===3&&x.currencyCode==="EUR")?.weightedMinorUnitsTimesBasisPoints,"60006000");
   assert.equal((await fixtureRpcError(dispatch,sales.getPipelineMetrics,tenantB,{pipelineId})).status,404);
   console.log(JSON.stringify({event:"crm.native-r73-forecast",result:"PASS",stage_default:true,
     manual_zero:true,move:true,reset:true,category:true,currency:true,close_reopen:true,cas:true,tenant:true}));
 }

}

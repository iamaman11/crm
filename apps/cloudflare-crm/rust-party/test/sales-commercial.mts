import assert from "node:assert/strict";
import { randomUUID, createHash } from "node:crypto";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { MemberService } from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import { decodeFixtureDetail, fixtureRpc, fixtureRpcError, fixtureRpcRace } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { record, records, string } from "../../scripts/json-boundary.mts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";

type Statement = {bind(...values: (string | number | null | ArrayBuffer)[]): Statement; all(): Promise<{success: boolean; results: Record<string,unknown>[]}>};
type CommercialFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: (string | number | null | ArrayBuffer)[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 statement: (query: string, ...params: (string | number | null | ArrayBuffer)[]) => Statement;
 batch: (statements: Statement[]) => Promise<unknown>;

 accountId: string; admin: string; viewer: string; tenantB: string;
};
export async function verifySalesCommercial({dispatch,sql,statement,batch,accountId,admin,viewer,tenantB}: CommercialFixture) {
 const sales=DealService.method,member=MemberService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>, code: string) => {
   assert.equal(failed.status,503); assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,code); assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 const required=<T,>(value: T | undefined | null): T => {
   assert.ok(value !== undefined && value !== null, "Expected generated response field");
   return value;
 };
   // D2: typed rules, exact money, preview, deterministic dispatch, capacity,
   // replay and receipt/event/audit are tested on the same signed D1 path.
   const suffix=randomUUID(),team="routing-team-"+suffix,pipeline="routing-pipeline-"+suffix;
   const emailA="routing-a-"+suffix+"@example.test",emailB="routing-b-"+suffix+"@example.test";
   const ruleId="routing-rule-"+suffix,secondId="routing-second-"+suffix;
   const territoryId="routing-territory-"+suffix;
   const actor=(email: string)=>({owner:{case:"assigneeIdentity",value:email}} as const);
   const target={owner:{case:"teamId",value:team}} as const;
   const pre=(label: string,version?: bigint)=>({idempotencyKey:"routing-"+label+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   const revision=async (tenant: string)=>BigInt(string(required((await sql("SELECT CAST(COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?),0) AS TEXT) AS revision",tenant))[0]).revision));
   const initialFence=await revision("tenant-a"),initialOtherFence=await revision("tenant-b");
   for(const [label,email] of [["a",emailA],["b",emailB]] as const)assert.equal((await fixtureRpc(dispatch,member.provisionMember,admin,{email,role:1,precondition:pre("member-"+label)})).status,200);
   assert.equal(await revision("tenant-a"),initialFence+2n);
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:team,name:"Routing QA "+suffix,roster:{memberEmails:[emailA,emailB]},precondition:pre("team")})).status,200);
   assert.equal(await revision("tenant-a"),initialFence+3n);
   (await fixtureRpc(dispatch,sales.createPipeline,admin,{pipelineId:pipeline,name:"Routing "+suffix,stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:5000}],precondition:pre("pipeline")})).body;
   (await fixtureRpc(dispatch,sales.createTerritory,admin,{territoryId,configuration:{name:"Routing territory "+suffix,defaultOwner:actor(emailB)},precondition:pre("territory")})).body;
   // The shared read fence now covers catalog membership as well.
   assert.equal(await revision("tenant-a"),initialFence+5n);
   assert.equal(await revision("tenant-b"),initialOtherFence);
   const stage={pipelineId:pipeline,stageId:"lead",ordinal:1};
   const facts={accountId,stage,territoryId,amount:{currencyCode:"EUR",minorUnits:100n}};
   const cfg={name:"Routing workload "+suffix,priority:10,criteria:{territoryId,pipelineId:pipeline,stageId:"lead",currencyCode:"EUR",minimumMinorUnits:100n,maximumMinorUnits:100n},target,dispatch:2,maximumOpenDeals:1,overflow:1};
   const {maximumOpenDeals: _limit, ...baseFallback}=cfg;
   assert.equal(_limit,1);
   const fallback={...baseFallback,name:"Routing fallback "+suffix,priority:20,criteria:{pipelineId:pipeline},target:actor(emailB),dispatch:1,overflow:1};
   const req={ruleId,configuration:cfg,precondition:pre("rule")};
   const created=(await fixtureRpc(dispatch,sales.createSalesRoutingRule,admin,req)).body;
   assert.equal(required(created.rule).version,1n);
   assert.equal(required(((await fixtureRpc(dispatch,sales.createSalesRoutingRule,admin,req)).body).receipt).replayed,true);
   const get=(await fixtureRpc(dispatch,sales.getSalesRoutingRule,viewer,{ruleId,receiptIdempotencyKey:req.precondition.idempotencyKey})).body;
   assert.deepEqual(get.rule,created.rule);assert.equal(required(get.confirmedMutation).resourceVersion,1n);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesRoutingRule,tenantB,{ruleId})).status,404);
   assert.equal(((await fixtureRpc(dispatch,sales.listSalesRoutingRules,tenantB,{})).body).rules.length,0);
   assert.equal((await fixtureRpcError(dispatch,sales.createSalesRoutingRule,viewer,{...req,ruleId:"denied-"+suffix,precondition:pre("denied")})).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.createSalesRoutingRule,admin,{ruleId:secondId,configuration:{...fallback,priority:10},precondition:pre("duplicate")})).status,409);
   (await fixtureRpc(dispatch,sales.createSalesRoutingRule,admin,{ruleId:secondId,configuration:fallback,precondition:pre("second")})).body;
   assert.equal(((await fixtureRpc(dispatch,sales.listSalesRoutingRules,viewer,{})).body).rules.length,2);
   assert.equal((await fixtureRpcError(dispatch,sales.updateTerritory,admin,{territoryId,configuration:{name:"Routing territory "+suffix,defaultOwner:actor(emailB),archived:true},precondition:pre("blocked-territory",1n)})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.updatePipeline,admin,{pipelineId:pipeline,name:"Routing "+suffix,archived:true,stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:5000}],precondition:pre("blocked-pipeline",1n)})).status,409);
   assert.equal((await fixtureRpcError(dispatch,member.updateTeam,admin,{teamId:team,active:false,precondition:pre("blocked-team",1n)})).status,409);
   const before=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n;
   const preview=(await fixtureRpc(dispatch,sales.previewSalesRouting,viewer,facts)).body;
   assert.equal(required(preview.decision).ruleId,ruleId);assert.equal(required(required(preview.decision).selectedOwner).owner.value,emailA);
   assert.equal(required(preview.decision).selectedOpenDeals,0n);
   assert.deepEqual((await fixtureRpc(dispatch,sales.previewSalesRouting,admin,facts)).body,preview);
   assert.equal(required(((await fixtureRpc(dispatch,sales.previewSalesRouting,viewer,{...facts,amount:{currencyCode:"USD",minorUnits:100n}})).body).decision).ruleId,secondId);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n,before);
   const deal=(label: string,expected?: string)=>({dealId:"routing-"+label+"-"+suffix,name:"Routing "+label+" "+suffix,accountId,stage,amount:facts.amount,automaticRouting:{territoryId,...(expected?{expectedDecision:expected}:{})},precondition:pre("deal-"+label)});
   const one=deal("one",preview.expectedDecision);
   const first=(await fixtureRpc(dispatch,sales.createDeal,admin,one)).body;
   assert.deepEqual(required(first.deal).initialRouting,preview.decision);
   assert.equal(required(required(first.deal).owner).owner.value,emailA);
   const p2=(await fixtureRpc(dispatch,sales.previewSalesRouting,viewer,facts)).body;
   assert.equal(required(required(p2.decision).selectedOwner).owner.value,emailB);
   const race=[deal("race-a",p2.expectedDecision),deal("race-b",p2.expectedDecision)];
   const results=await Promise.all(race.map(input=>fixtureRpcRace(dispatch,sales.createDeal,admin,input)));
   assert.equal(results.filter(r=>r.status===200).length,1);
   assert.equal(results.filter(r=>r.status===409).length,1);
   const winner=race[results.findIndex(r=>r.status===200)];
   assert.equal(required(required(((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:required(winner).dealId})).body).deal).owner).owner.value,emailB);
   assert.equal((await fixtureRpcError(dispatch,sales.previewSalesRouting,viewer,facts)).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,deal("full"))).status,409);
   const update={ruleId,configuration:{...cfg,overflow:2},precondition:pre("overflow",1n)};
   const ruleBefore=await sql("SELECT state,version FROM crm_sales_routing_rule_records WHERE tenant=? AND id=?","tenant-a",ruleId);
   await sql("CREATE TRIGGER fixture_fail_rule_audit BEFORE INSERT ON crm_audit WHEN NEW.action='sales.routing_rule.update' BEGIN SELECT RAISE(ABORT,'fixture routing rule audit rollback'); END;");
   try{
     assertUnconfirmed(await fixtureRpcError(dispatch,sales.updateSalesRoutingRule,admin,{...update,precondition:pre("rule-fault",1n)}),"SALES_ROUTING_RULE_WRITE_UNCONFIRMED");
     assert.deepEqual(await sql("SELECT state,version FROM crm_sales_routing_rule_records WHERE tenant=? AND id=?","tenant-a",ruleId),ruleBefore);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_routing_rule_receipts WHERE tenant=? AND key=?","tenant-a",pre("rule-fault",1n).idempotencyKey))[0]?.n,0);
   }finally{await sql("DROP TRIGGER fixture_fail_rule_audit");}
   assert.equal(required(((await fixtureRpc(dispatch,sales.updateSalesRoutingRule,admin,update)).body).rule).version,2n);
   assert.equal(required(((await fixtureRpc(dispatch,sales.updateSalesRoutingRule,admin,update)).body).receipt).replayed,true);
   assert.equal(required(((await fixtureRpc(dispatch,sales.createDeal,admin,one)).body).receipt).replayed,true);
   assert.deepEqual(required(((await fixtureRpc(dispatch,sales.createDeal,admin,one)).body).deal).initialRouting,required(first.deal).initialRouting);
   assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,deal("stale",preview.expectedDecision))).status,409);
   const p3=(await fixtureRpc(dispatch,sales.previewSalesRouting,viewer,facts)).body;
   assert.equal(required(p3.decision).capacityOverflow,true);
   assert.equal(required(required(p3.decision).selectedOwner).owner.value,team);
   const overflow=deal("overflow",p3.expectedDecision);
   const pooled=(await fixtureRpc(dispatch,sales.createDeal,admin,overflow)).body;
   assert.equal(required(required(pooled.deal).owner).owner.value,team);
   const {automaticRouting: _routing, ...manualBase}=deal("manual");
   assert.ok(_routing);
   const manual={...manualBase,owner:actor(emailB)};
   const manualSaved=(await fixtureRpc(dispatch,sales.createDeal,admin,manual)).body;
   assert.equal(required(required(manualSaved.deal).initialRouting).source,4);
   const events=(await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId:one.dealId})).body;
   assert.deepEqual(required(events.events[0]).initialRouting,required(first.deal).initialRouting);
   const stored=await sql("SELECT json_extract(state,'$.initial_routing.rule_id') rule_id FROM crm_sales_deal_records WHERE tenant=? AND id=?","tenant-a",one.dealId);
   assert.equal(required(stored[0]).rule_id,ruleId);
   await sql("CREATE TRIGGER fixture_fail_routing_audit BEFORE INSERT ON crm_audit WHEN NEW.subject_id='routing-fault-"+suffix+"' BEGIN SELECT RAISE(ABORT,'fixture routing audit rollback'); END;");
   try{
     assertUnconfirmed(await fixtureRpcError(dispatch,sales.createDeal,admin,deal("fault")),"SALES_COMMIT_UNCONFIRMED");
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_records WHERE tenant=? AND id=?","tenant-a","routing-fault-"+suffix))[0]?.n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",pre("deal-fault").idempotencyKey))[0]?.n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_events WHERE tenant=? AND deal_id=?","tenant-a","routing-fault-"+suffix))[0]?.n,0);
   }finally{await sql("DROP TRIGGER fixture_fail_routing_audit");}
   (await fixtureRpc(dispatch,sales.updateSalesRoutingRule,admin,{ruleId,configuration:{...cfg,overflow:2,archived:true},precondition:pre("archive-rule",2n)})).body;
   (await fixtureRpc(dispatch,sales.updateSalesRoutingRule,admin,{ruleId:secondId,configuration:{...fallback,archived:true},precondition:pre("archive-second",1n)})).body;
   assert.equal(required(((await fixtureRpc(dispatch,sales.previewSalesRouting,viewer,facts)).body).decision).source,3);
   const noTerritory={accountId,stage,amount:facts.amount};
   assert.equal((await fixtureRpcError(dispatch,sales.previewSalesRouting,viewer,noTerritory)).status,409);
   const previousAssignment=((await fixtureRpc(dispatch,sales.getSalesAccountAssignment,viewer,{accountId})).body).assignment;
   const beforeAssignmentFence=await revision("tenant-a");
   (await fixtureRpc(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,configuration:{territoryId,primaryOwner:actor(emailB),overlays:[],archived:false},precondition:pre("assignment",required(previousAssignment).version)})).body;
   assert.equal(await revision("tenant-a"),beforeAssignmentFence+1n);
   assert.equal(required(((await fixtureRpc(dispatch,sales.previewSalesRouting,viewer,noTerritory)).body).decision).source,2);
   assert.equal((await fixtureRpcError(dispatch,sales.previewSalesRouting,viewer,{...facts,territoryId:"conflicting-"+suffix})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.previewSalesRouting,tenantB,facts)).status,409);
   const activeAssignment=((await fixtureRpc(dispatch,sales.getSalesAccountAssignment,viewer,{accountId})).body).assignment;
   assert.equal((await fixtureRpcError(dispatch,sales.previewDealCloseCredit,viewer,{dealId:one.dealId,expectedVersion:1n,
     selection:{territoryId:"conflict-"+suffix}})).status,409);
   (await fixtureRpc(dispatch,sales.updateSalesAccountAssignment,admin,{accountId,configuration:{...required(required(activeAssignment).configuration),archived:true},precondition:pre("assignment-archive",required(activeAssignment).version)})).body;
   const reassigned=(await fixtureRpc(dispatch,sales.updateDeal,admin,{dealId:one.dealId,owner:actor(emailB),precondition:pre("manual-reassign",1n)})).body;
   assert.equal(required(required(reassigned.deal).owner).owner.value,emailB);
   assert.deepEqual(required(reassigned.deal).initialRouting,required(first.deal).initialRouting);
   // Explicit close credit shares the actual owner resolver and atomic guards.
   const secondTeam="credit-team-"+suffix;
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:secondTeam,name:"Credit choice "+suffix,
     roster:{memberEmails:[emailB]},precondition:pre("credit-team")})).status,200);
   const previewInput={dealId:one.dealId,expectedVersion:2n,selection:{teamId:team,territoryId}};
   const creditPreview=(await fixtureRpc(dispatch,sales.previewDealCloseCredit,viewer,previewInput)).body;
   assert.equal(creditPreview.eligibleTeams.length,2);
   assert.equal(required(creditPreview.creditedTeam).id,team);
   assert.equal(required(creditPreview.creditedTerritory).id,territoryId);
   assert.equal(creditPreview.territoryAssigned,false);
   assert.deepEqual((await fixtureRpc(dispatch,sales.previewDealCloseCredit,admin,previewInput)).body,creditPreview);
   assert.equal(((await fixtureRpc(dispatch,sales.previewDealCloseCredit,viewer,{...previewInput,selection:{}})).body).creditedTeam,undefined);
   assert.equal(((await fixtureRpc(dispatch,sales.previewDealCloseCredit,viewer,{...previewInput,selection:{leaveTeamUnallocated:true}})).body).creditedTeam,undefined);
   assert.equal((await fixtureRpcError(dispatch,sales.previewDealCloseCredit,tenantB,previewInput)).status,404);
   assert.equal((await fixtureRpcError(dispatch,sales.previewDealCloseCredit,viewer,{...previewInput,selection:{teamId:"missing-"+suffix}})).status,400);
   const closeIntent={dealId:one.dealId,outcome:2,reasonCode:"qa.cleanup",precondition:pre("close-stale",2n),
     creditSelection:previewInput.selection,expectedCreditDecision:creditPreview.expectedDecision};
   assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeIntent)).status,403);
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{teamId:secondTeam,name:"Credit choice revised "+suffix,
     roster:{memberEmails:[emailB]},precondition:pre("credit-team-rename",1n)})).status,200);
   assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,admin,closeIntent)).status,409);
   assert.equal(required(((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:one.dealId})).body).deal).version,2n);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",closeIntent.precondition.idempotencyKey))[0]?.n,0);
   let replayCreditIntent;
   for(const [index,input] of [one,winner,overflow,manual].entries()){
     const version=index===0?2n:1n;
     const selection=index===0?{teamId:team,territoryId}:{teamId:team};
     const credit=(await fixtureRpc(dispatch,sales.previewDealCloseCredit,viewer,{dealId:required(input).dealId,expectedVersion:version,selection})).body;
     const request={dealId:required(input).dealId,outcome:index===0?2:3,reasonCode:"qa.cleanup",precondition:pre("close-"+index,version),
       creditSelection:selection,expectedCreditDecision:credit.expectedDecision};
     if(index===0)replayCreditIntent=request;
     const closed=(await fixtureRpc(dispatch,sales.closeDeal,admin,request)).body;
     const fact=required(closed.deal).commercialCloseFact;
     assert.equal(required(fact).closeVersion,version+1n);
     assert.equal(required(fact).outcome,request.outcome);
     assert.equal(required(required(fact).amount).currencyCode,facts.amount.currencyCode);
     assert.equal(required(required(fact).amount).minorUnits,facts.amount.minorUnits);
     assert.deepEqual(required(fact).owner,required(closed.deal).owner);
     assert.equal(required(required(fact).creditedTeam).id,team);
     if(index===0)assert.equal(required(required(fact).creditedTerritory).id,territoryId);
     assert.deepEqual(required(((await fixtureRpc(dispatch,sales.closeDeal,admin,request)).body).deal).commercialCloseFact,fact);
     assert.deepEqual(required(((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:required(input).dealId})).body).deal).commercialCloseFact,fact);
     assert.deepEqual(required(((await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId:required(input).dealId})).body).events[0]).commercialCloseFact,fact);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_events WHERE tenant=? AND deal_id=?","tenant-a",required(input).dealId))[0]?.n,Number(version)+1);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",required(input).dealId))[0]?.n,Number(version)+1);
   }
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{teamId:secondTeam,active:false,
     precondition:pre("credit-team-archive",2n)})).status,200);
   assert.equal(required(((await fixtureRpc(dispatch,sales.closeDeal,admin,required(replayCreditIntent))).body).receipt).replayed,true);
   const originalClose=required(((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:one.dealId})).body).deal).commercialCloseFact;
   const reopened=(await fixtureRpc(dispatch,sales.reopenDeal,admin,{dealId:one.dealId,reasonCode:"qa.revisit",precondition:pre("credit-reopen",3n)})).body;
   assert.equal(required(reopened.deal).commercialCloseFact,undefined);
   const reclosed=(await fixtureRpc(dispatch,sales.closeDeal,admin,{dealId:one.dealId,outcome:3,reasonCode:"qa.cleanup",precondition:pre("credit-reclose",4n)})).body;
   assert.equal(required(required(reclosed.deal).commercialCloseFact).closeVersion,5n);
   const creditEvents=((await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId:one.dealId})).body).events;
   assert.deepEqual(required(creditEvents[0]).commercialCloseFact,required(reclosed.deal).commercialCloseFact);
   assert.deepEqual(required(creditEvents[1]).commercialCloseFact,originalClose);
   assert.deepEqual(required(creditEvents[2]).commercialCloseFact,originalClose);
   assert.equal(required(creditEvents[1]).kind,5);
   const storedCredit=await sql("SELECT commercial_close_fact FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND deal_version IN (3,4) ORDER BY deal_version","tenant-a",one.dealId);
   assert.equal(storedCredit.length,2);
   assert.deepEqual(JSON.parse(string(required(storedCredit[0]).commercial_close_fact)),JSON.parse(string(required(storedCredit[1]).commercial_close_fact)));
   // D3 quota publication, overlap, exact event net and historical revision.
   const quotaId="quota-"+suffix;
   const year=new Date().getUTCFullYear();
   const quotaCfg={name:"QA quota "+suffix,scope:{scope:{case:"owner" as const,value:target}},pipelineId:pipeline,
     period:{startDate:{year,month:1,day:1},endDate:{year,month:12,day:31},timeZone:"UTC",cadence:3},
     target:{currencyCode:"EUR",minorUnits:1000n},published:false,revisionReason:"qa.draft"};
   const draft={quotaId,configuration:quotaCfg,precondition:pre("quota-draft")};
   assert.equal((await fixtureRpcError(dispatch,sales.createSalesQuota,viewer,draft)).status,403);
   const quota=(await fixtureRpc(dispatch,sales.createSalesQuota,admin,draft)).body;
   assert.equal(required(quota.quota).version,1n);
   assert.equal(required(((await fixtureRpc(dispatch,sales.createSalesQuota,admin,draft)).body).receipt).replayed,true);
   const publish={quotaId,configuration:{...quotaCfg,published:true,revisionReason:"qa.publish"},precondition:pre("quota-publish",1n)};
   const publishedQuota=(await fixtureRpc(dispatch,sales.updateSalesQuota,admin,publish)).body;
   assert.equal(required(publishedQuota.quota).version,2n);
   assert.equal((await fixtureRpcError(dispatch,sales.createSalesQuota,admin,{...draft,quotaId:"overlap-"+suffix,configuration:publish.configuration,precondition:pre("quota-overlap")})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesQuota,tenantB,{quotaId})).status,404);
   assert.equal(((await fixtureRpc(dispatch,sales.listSalesQuotas,tenantB,{})).body).quotas.length,0);
   assert.deepEqual((await fixtureRpc(dispatch,sales.getSalesQuota,viewer,{quotaId})).body,(await fixtureRpc(dispatch,sales.getSalesQuota,admin,{quotaId})).body);
   assert.equal(((await fixtureRpc(dispatch,sales.listSalesQuotas,viewer,{})).body).quotas.some(q=>q.quotaId===quotaId),true);
   const firstQuota=(await fixtureRpc(dispatch,sales.getSalesQuota,viewer,{quotaId,version:1n,receiptIdempotencyKey:draft.precondition.idempotencyKey})).body;
   assert.equal(required(required(firstQuota.quota).configuration).published,false);
   assert.equal(required(firstQuota.confirmedMutation).resourceVersion,1n);
   const perf=(await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId,pageSize:1})).body;
   assert.equal(perf.grossMinorUnits,"100");assert.equal(perf.reversalMinorUnits,"100");
   assert.equal(perf.netMinorUnits,"0");assert.equal(perf.remainingMinorUnits,"1000");
   assert.equal(perf.facts.length,1);assert.ok(perf.next);
   const page2=(await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos,eventSequence:perf.eventSequence,after:perf.next,pageSize:1})).body;
   assert.equal(page2.netMinorUnits,perf.netMinorUnits);assert.notDeepEqual(page2.facts[0],perf.facts[0]);
   // Scorecard shares the immutable ledger and exact quota semantics. A
   // monthly goal does not borrow the custom annual target.
   const month=new Date().getUTCMonth()+1;
   const monthlyId="quota-month-"+suffix;
   const monthlyConfig={...publish.configuration,name:"Monthly "+suffix,
     period:{startDate:{year,month,day:1},endDate:{year,month,day:new Date(Date.UTC(year,month,0)).getUTCDate()},timeZone:"UTC",cadence:1}};
   const monthlyQuota=((await fixtureRpc(dispatch,sales.createSalesQuota,admin,{quotaId:monthlyId,configuration:monthlyConfig,precondition:pre("monthly-goal")})).body).quota;
   const scoreInput={axis:3,currencyCode:"EUR",pipelineId:pipeline,firstDate:{year,month,day:1},timeZone:"UTC",cadence:1,periodCount:2};
   const auditBeforeScore=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n;
   const historyInput={pipelineId:pipeline,timeZone:"UTC",comparePrevious:true,
     period:{startDate:{year,month:1,day:1},endDate:{year,month:12,day:31}}};
   const history=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,historyInput)).body;
   assert.ok(history.current);
   const historyDeal=history.current.deals.find(d=>d.dealId===one.dealId);
   assert.equal(required(historyDeal).status,3);
   assert.equal(required(historyDeal).finalCloseVersion,5n);
   assert.deepEqual(required(historyDeal).closeVersions,[3n,5n]);
   assert.deepEqual(required(historyDeal).reopenVersions,[4n]);
   assert.equal(required(historyDeal).visits.length,2);
   assert.equal(required(historyDeal).reasonCode,"qa.cleanup");
   assert.equal(required(history.current.finalCloses).wonCount,0n);
   assert.equal(required(history.current.finalCloses).lostCount,4n);
   const historyPaged=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,{...historyInput,pageSize:1})).body;
   assert.equal(required(historyPaged.current).totalDealCount,history.current.totalDealCount);
   assert.deepEqual(required(historyPaged.current).finalCloses,history.current.finalCloses);
   const historyIds=required(historyPaged.current).deals.map(d=>d.dealId);
   let historyAfter=required(historyPaged.current).nextDealId;
   while(historyAfter){
     const page=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,{...historyInput,pageSize:1,
       snapshot:historyPaged.snapshot,detail:{afterDealId:historyAfter}})).body;
     assert.equal(page.current,undefined);
     assert.equal(required(page.detail).deals.length,1);
     historyIds.push(required(required(page.detail).deals[0]).dealId);
     historyAfter=required(page.detail).nextDealId;
   }
   assert.deepEqual(historyIds,history.current.deals.map(d=>d.dealId));
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesHistory,viewer,{...historyInput,timeZone:"Europe/Warsaw",
     snapshot:historyPaged.snapshot,detail:{afterDealId:""}})).status,400);
   const historyExportScope={...historyInput,snapshot:historyPaged.snapshot};
   const historyCsv=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:7,scope:{case:"history",value:historyExportScope}})).body;
   assert.equal(historyCsv.csvSha256Hex,createHash("sha256").update(historyCsv.csv).digest("hex"));
   assert.deepEqual(required(required(historyCsv.history).current).finalCloses,history.current.finalCloses);
   assert.ok(Buffer.from(historyCsv.csv).toString("utf8").includes("history_window,current,total_deal_count,"));
   const historyDetailCsv=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:8,scope:{case:"history",value:{...historyExportScope,detail:{}}}})).body;
   assert.ok(Buffer.from(historyDetailCsv.csv).toString("utf8").includes("historical_deal,"+one.dealId+","));
   assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,viewer,{kind:7,scope:{case:"history",value:historyExportScope}})).status,403);
   const historyAdmin=(await fixtureRpc(dispatch,sales.getSalesHistory,admin,historyInput)).body;
   assert.deepEqual(required(historyAdmin.current).finalCloses,history.current.finalCloses);
   assert.deepEqual(required(historyAdmin.current).deals,history.current.deals);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesHistory,tenantB,historyInput)).status,404);
   const tenantHistory=(await fixtureRpc(dispatch,sales.getSalesHistory,tenantB,{...historyInput,pipelineId:undefined})).body;
   assert.equal(required(tenantHistory.current).deals.some(d=>d.dealId===one.dealId),false);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesHistory,viewer,{...historyInput,timeZone:"Invalid/Zone"})).status,400);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n,auditBeforeScore);
   const score=(await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scoreInput)).body;
   assert.equal(score.buckets.length,2);
   assert.equal(required(score.buckets[0]).endExclusiveUnixNanos,required(score.buckets[1]).startUnixNanos);
   const scoreTeam=required(required(score.buckets[0]).dimensions.find(d=>d.dimensionId===team)).cell;
   assert.equal(required(required(scoreTeam).quota).quotaId,monthlyId);
   assert.equal(required(required(scoreTeam).quota).version,required(monthlyQuota).version);
   for(const field of ["grossMinorUnits","reversalMinorUnits","netMinorUnits","wonCount","lostCount","reversalCount","remainingMinorUnits","attainmentBasisPoints"] as const)
     assert.equal(required(scoreTeam)[field],perf[field],field);
   assert.equal(required(required(required(score.buckets[1]).dimensions.find(d=>d.dimensionId===team)).cell).netMinorUnits,"0");
   assert.equal(required(required(required(score.buckets[1]).dimensions.find(d=>d.dimensionId===team)).cell).quota,undefined);
   assert.equal(required(required(score.buckets[0]).organization).quota,undefined);
   assert.deepEqual(((await fixtureRpc(dispatch,sales.getSalesScorecard,admin,scoreInput)).body).buckets,score.buckets);
   for(const axis of [1,2,4]){
     const other=(await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,{...scoreInput,axis})).body;
     for(const field of ["grossMinorUnits","reversalMinorUnits","netMinorUnits","wonCount","lostCount","sourceEventCount"] as const)
       assert.equal(required(required(other.buckets[0]).organization)[field],required(required(score.buckets[0]).organization)[field],field);
     if(required(axis)==1)assert.equal(required(other.buckets[0]).dimensions.reduce((n,d)=>n+BigInt(required(d.cell).netMinorUnits),0n),BigInt(required(required(other.buckets[0]).organization).netMinorUnits));
   }
   const otherTenant=(await fixtureRpc(dispatch,sales.getSalesScorecard,tenantB,scoreInput)).body;
   assert.equal(required(required(otherTenant.buckets[0]).organization).sourceEventCount,0n);
   assert.equal(required(required(otherTenant.buckets[0]).organization).netMinorUnits,"0");
   assert.ok(!required(otherTenant.buckets[0]).dimensions.some(d=>d.dimensionId===team));
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesScorecard,viewer,{...scoreInput,periodCount:25})).status,400);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesScorecard,viewer,{...scoreInput,firstDate:{year,month,day:2}})).status,400);
   const scoreDetail={...scoreInput,detail:{bucketIndex:0,organization:false,dimensionId:team,
     asOfUnixNanos:score.asOfUnixNanos,eventSequence:score.eventSequence,expectedEventCount:required(scoreTeam).sourceEventCount,pageSize:1}};
   const scoreExportRequest={kind:5,scope:{case:"scorecard" as const,value:scoreInput}};
   const scoreExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,scoreExportRequest)).body;
   const scoreCsv=Buffer.from(scoreExport.csv).toString("utf8");
   assert.equal(scoreExport.csvSha256Hex,createHash("sha256").update(scoreExport.csv).digest("hex"));
   assert.equal(scoreExport.filename,"crm-sales-scorecard.csv");
   assert.equal(scoreExport.specificationVersion,"sales-analytics-csv/v1");
   assert.deepEqual(required(scoreExport.scorecard).buckets,score.buckets);
   assert.ok(scoreCsv.startsWith("section,record_id,field,value\n"));
   assert.ok(scoreCsv.includes("gross_minor_units,100\n"));
   assert.ok(scoreCsv.includes("quota_id,"+monthlyId+"\n"));
   assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,viewer,scoreExportRequest)).status,403);
   assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,admin,{kind:5})).status,400);
   const detailExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:6,scope:{case:"scorecard",value:scoreDetail}})).body;
   assert.ok(Buffer.from(detailExport.csv).toString("utf8").includes("signed_minor_units,-100\n"));
   assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,admin,{kind:6,scope:{case:"scorecard",value:{...scoreDetail,detail:{...scoreDetail.detail,expectedEventCount:required(scoreTeam).sourceEventCount+1n}}}})).status,409);
   const firstScorePage=((await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scoreDetail)).body).detail;
   assert.equal(required(firstScorePage).facts.length,1);assert.ok(required(firstScorePage).next);
   assert.equal(required(firstScorePage).sourceEventCount,required(scoreTeam).sourceEventCount);
   assert.deepEqual(((await fixtureRpc(dispatch,sales.getSalesScorecard,admin,scoreDetail)).body).detail,firstScorePage);
   const nextScorePage=((await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,{...scoreDetail,detail:{...scoreDetail.detail,after:required(firstScorePage).next}})).body).detail;
   assert.notDeepEqual(required(nextScorePage).facts[0],required(firstScorePage).facts[0]);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesScorecard,tenantB,scoreDetail)).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesScorecard,viewer,{...scoreDetail,detail:{...scoreDetail.detail,expectedEventCount:required(scoreTeam).sourceEventCount+1n}})).status,409);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n,auditBeforeScore);
   // Disposable fixture: more than the former 2000-event limit, exact values beyond JS safe integers.
   const scaleCount=2501,scaleAmount=9007199254740993n;
   const scaleInsert=`WITH RECURSIVE nums(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM nums WHERE n<${scaleCount})
     INSERT INTO crm_sales_deal_events(tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,stage_id,
       previous_stage_ordinal,stage_ordinal,previous_status,status,reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version,commercial_close_fact)
     SELECT e.tenant,e.deal_id,10000+n,e.action,e.pipeline_id,e.previous_stage_id,e.stage_id,e.previous_stage_ordinal,e.stage_ordinal,
       e.previous_status,e.status,e.reason_code,e.actor,e.request_id,e.occurred_at_unix_nanos,e.catalog_version,
       json_set(e.commercial_close_fact,'$.close_version',10000+n,'$.amount.minor_units','${scaleAmount}')
       FROM nums CROSS JOIN crm_sales_deal_events e WHERE e.tenant=? AND e.deal_id=? AND e.deal_version=3`;
   let scaleReport,scaledScoreDetail;
   const scaleReportId="report-scale-"+suffix;
   try{
     const beforeEventOnly=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,historyInput)).body;
     const eventOnlyFence=await revision("tenant-a");
     await sql(scaleInsert.replace("n<"+scaleCount,"n<120"),"tenant-a",one.dealId);
     assert.equal(await revision("tenant-a"),eventOnlyFence+120n);
     // A deliberately incomplete stream belongs to the current year only;
     // it must not invalidate a historical window before immutable creation.
     assert.equal((await fixtureRpcError(dispatch,sales.getSalesHistory,viewer,{...historyInput,snapshot:beforeEventOnly.snapshot,
       detail:{afterDealId:""}})).status,409);
     assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,admin,{kind:7,scope:{case:"history",value:{...historyInput,
       snapshot:beforeEventOnly.snapshot}}})).status,409);
     const gappedHistory=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,historyInput)).body;
     assert.equal(required(gappedHistory.current).unknownHistoryCount,1n);
     assert.equal(required(required(gappedHistory.current).deals.find(d=>d.dealId===one.dealId)).status,undefined);
     assert.equal(required(required(gappedHistory.current).deals.find(d=>d.dealId===one.dealId)).createdAtUnixNanos,required(historyDeal).createdAtUnixNanos);
     assert.equal(required(gappedHistory.previous).unknownHistoryCount,0n);
     assert.equal(required(gappedHistory.previous).deals.length,0);
     console.log(JSON.stringify({event:"crm.native-history-cutoff-membership",result:"PASS",gap_current:true,gap_future_excluded:true,immutable_creation:true}));
     const pagedScore=(await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scoreInput)).body;
     const pagedTeam=required(required(pagedScore.buckets[0]).dimensions.find(d=>d.dimensionId===team)).cell;
     const fullDetail=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:6,scope:{case:"scorecard",value:{...scoreDetail,detail:{
       ...scoreDetail.detail,asOfUnixNanos:pagedScore.asOfUnixNanos,eventSequence:pagedScore.eventSequence,expectedEventCount:required(pagedTeam).sourceEventCount
     }}}})).body;
     const detailCsv=Buffer.from(fullDetail.csv).toString("utf8");
     const versions=detailCsv.split("\n").filter(line=>line.startsWith("actual_event,")&&line.includes(",deal_version,"));
     assert.equal(versions.length,Number(required(pagedTeam).sourceEventCount));
     assert.equal(new Set(versions).size,versions.length);
     assert.ok(versions.length>100);
     assert.equal(fullDetail.csvSha256Hex,createHash("sha256").update(fullDetail.csv).digest("hex"));
     await sql(scaleInsert.replace("SELECT 1 UNION ALL","SELECT 121 UNION ALL"),"tenant-a",one.dealId);
     const started=Date.now();
     const scaled=(await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId,pageSize:1})).body;
     const elapsed=Date.now()-started;
     assert.equal(scaled.grossMinorUnits,String(100n+BigInt(scaleCount)*scaleAmount));
     assert.equal(scaled.reversalMinorUnits,"100");
     assert.equal(scaled.netMinorUnits,String(BigInt(scaleCount)*scaleAmount));
     assert.equal(scaled.wonCount,perf.wonCount+BigInt(scaleCount));
     assert.ok(scaled.eventSequence>perf.eventSequence);
     assert.equal(scaled.facts.length,1);
     assert.ok(scaled.next);
     const scoreStarted=Date.now();
     const scaledScore=(await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scoreInput)).body;
     const scoreElapsed=Date.now()-scoreStarted;
     const scaledTeam=required(required(scaledScore.buckets[0]).dimensions.find(d=>d.dimensionId===team)).cell;
     assert.equal(required(scaledTeam).netMinorUnits,scaled.netMinorUnits);
     assert.equal(required(scaledTeam).sourceEventCount,required(scoreTeam).sourceEventCount+BigInt(scaleCount));
     assert.deepEqual(((await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scoreDetail)).body).detail,firstScorePage);
     scaledScoreDetail={...scoreDetail,detail:{...scoreDetail.detail,asOfUnixNanos:scaledScore.asOfUnixNanos,
       eventSequence:scaledScore.eventSequence,expectedEventCount:required(scaledTeam).sourceEventCount}};
     const exactExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,scoreExportRequest)).body;
     assert.ok(Buffer.from(exactExport.csv).toString("utf8").includes("net_minor_units,"+scaled.netMinorUnits+"\n"));
     assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,admin,{kind:6,scope:{case:"scorecard",value:scaledScoreDetail}})).status,429);
     assert.equal(required(((await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scaledScoreDetail)).body).detail).sourceEventCount,required(scaledTeam).sourceEventCount);
     console.log(JSON.stringify({event:"crm.native-d3-scorecard-scale",result:"PASS",added_events:scaleCount,periods:2,elapsed_ms:scoreElapsed,exact_i128:true,frozen_membership:true}));
     const frozenRequest={quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos,eventSequence:perf.eventSequence,pageSize:1};
     assert.deepEqual((await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,frozenRequest)).body,perf);
     assert.deepEqual((await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,admin,frozenRequest)).body,perf);
     const scaledNext=(await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId,quotaVersion:2n,
       asOfUnixNanos:scaled.asOfUnixNanos,eventSequence:scaled.eventSequence,after:scaled.next,pageSize:1})).body;
     assert.equal(scaledNext.netMinorUnits,scaled.netMinorUnits);
     assert.notDeepEqual(scaledNext.facts[0],scaled.facts[0]);
     assert.equal((await fixtureRpcError(dispatch,sales.getSalesQuotaPerformance,tenantB,frozenRequest)).status,404);
     assert.equal((await fixtureRpcError(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos,after:perf.next})).status,400);
     scaleReport=((await fixtureRpc(dispatch,sales.saveSalesPerformanceReport,admin,{reportId:scaleReportId,name:"Scale "+suffix,quotaId,quotaVersion:2n,precondition:pre("report-scale")})).body).report;
     assert.equal(required(required(scaleReport).actuals).netMinorUnits,scaled.netMinorUnits);
     assert.equal(((await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{reportId:scaleReportId,pageSize:1})).body).actualHistoryAvailable,true);
     console.log(JSON.stringify({event:"crm.native-d3-quota-scale",result:"PASS",added_events:scaleCount,
       elapsed_ms:elapsed,exact_i128:true,frozen_membership:true,tenant_ab:true,viewer_parity:true}));
   }finally{
     await sql("DELETE FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND deal_version BETWEEN 10001 AND 12501","tenant-a",one.dealId);
   }
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesScorecard,viewer,scaledScoreDetail)).status,409);
   (await fixtureRpc(dispatch,sales.updateSalesQuota,admin,{quotaId:monthlyId,configuration:{...monthlyConfig,archived:true,revisionReason:"qa.monthly-archive"},precondition:pre("monthly-archive",required(monthlyQuota).version)})).body;
   const archivedScore=required(required(((await fixtureRpc(dispatch,sales.getSalesScorecard,viewer,scoreInput)).body).buckets[0]).dimensions.find(d=>d.dimensionId===team)).cell;
   assert.equal(required(required(required(archivedScore).quota).configuration).archived,true);
   assert.equal(required(required(archivedScore).quota).version,required(monthlyQuota).version+1n);
   assert.equal(required(archivedScore).netMinorUnits,required(scoreTeam).netMinorUnits);
   console.log(JSON.stringify({event:"crm.native-d3-scorecards",result:"PASS",calendar:true,dimensions:true,quota_versions:true,exact_totals:true,detail_paging:true,retention_visible:true,tenant_ab:true,viewer_parity:true,no_read_audit:true}));
   // D3 durable report and forecast: exact estimates, frozen inputs, replay,
   // independent read rights, exclusions, mutation rollback and no new audit on reads.
   const retainedReport=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{reportId:scaleReportId,pageSize:1})).body;
   assert.deepEqual(retainedReport.report,scaleReport);
   assert.equal(retainedReport.actualHistoryAvailable,false);
   assert.equal(retainedReport.actualFacts.length,0);
   const reportTeam="report-team-"+suffix;
   assert.equal((await fixtureRpc(dispatch,member.createTeam,admin,{teamId:reportTeam,name:"Report ambiguous "+suffix,
     roster:{memberEmails:[emailB]},precondition:pre("report-team")})).status,200);
   const forecastDate={year,month:6,day:15};
   const forecastInputs=[
     {label:"included-a",amount:{currencyCode:"EUR",minorUnits:1001n},probabilityBasisPoints:5000,forecastCategory:3,expectedCloseDate:forecastDate},
     {label:"included-b",amount:{currencyCode:"EUR",minorUnits:2000n},probabilityBasisPoints:2500,forecastCategory:2,expectedCloseDate:forecastDate},
     {label:"omitted",amount:{currencyCode:"EUR",minorUnits:999n},forecastCategory:4,expectedCloseDate:forecastDate},
     {label:"undated",amount:{currencyCode:"EUR",minorUnits:999n}},
     {label:"other-currency",amount:{currencyCode:"USD",minorUnits:999n},expectedCloseDate:forecastDate},
     {label:"missing",expectedCloseDate:forecastDate},
     {label:"unallocated",owner:actor(emailB),amount:{currencyCode:"EUR",minorUnits:999n},expectedCloseDate:forecastDate},
     {label:"outside",amount:{currencyCode:"EUR",minorUnits:999n},expectedCloseDate:{year:year+1,month:1,day:1}}
   ].map(({label,...fields})=>({dealId:"report-"+label+"-"+suffix,name:"Report "+label+" "+suffix,
     accountId,stage,owner:target,...fields,precondition:pre("forecast-"+label)}));
   for(const input of forecastInputs)(await fixtureRpc(dispatch,sales.createDeal,admin,input)).body;
   const previewReport=(await fixtureRpc(dispatch,sales.previewSalesPerformanceReport,viewer,{quotaId,quotaVersion:2n,pageSize:1})).body;
   assert.equal(required(required(previewReport.report).forecast).openMinorUnits,"3001");
   assert.equal(required(required(previewReport.report).forecast).weightedMinorUnitsTimesBasisPoints,"10005000");
   assert.equal(required(required(previewReport.report).forecast).weightedCoverageBasisPoints,"10005");
   assert.equal(required(required(previewReport.report).forecast).openCoverageBasisPoints,"30010");
   assert.equal(required(required(previewReport.report).forecast).includedCount,2n);
   for(const field of ["omittedCount","undatedCount","otherCurrencyCount","missingAmountCount","unallocatedCount"] as const)
     assert.equal(required(required(previewReport.report).forecast)[field],1n,field);
   assert.equal(previewReport.forecastSources.length,1);
   assert.equal(previewReport.moreForecastSources,true);
   const reportId="report-"+suffix;
   const reportIntent={reportId,name:"Frozen commercial "+suffix,quotaId,quotaVersion:2n,precondition:pre("save-report")};
   assert.equal((await fixtureRpcError(dispatch,sales.saveSalesPerformanceReport,viewer,reportIntent)).status,403);
   const savedReport=(await fixtureRpc(dispatch,sales.saveSalesPerformanceReport,admin,reportIntent)).body;
   assert.equal(required(required(savedReport.report).forecast).openMinorUnits,"3001");
   assert.equal(required(required(savedReport.report).actuals).netMinorUnits,"0");
   assert.equal(required(required(savedReport.report).actuals).remainingMinorUnits,"1000");
   assert.equal(required(required(savedReport.report).quota).version,2n);
   assert.equal(required(savedReport.report).formulaVersion,1);
   assert.equal(required(((await fixtureRpc(dispatch,sales.saveSalesPerformanceReport,admin,reportIntent)).body).receipt).replayed,true);
   assert.equal((await fixtureRpcError(dispatch,sales.saveSalesPerformanceReport,admin,{...reportIntent,name:"Different"})).status,409);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesPerformanceReport,tenantB,{reportId})).status,404);
   assert.equal(((await fixtureRpc(dispatch,sales.listSalesPerformanceReports,tenantB,{})).body).reports.length,0);
   const readBefore=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n;
   const getReport={reportId,pageSize:1};
   const readReport=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,getReport)).body;
   assert.deepEqual(readReport,(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,admin,getReport)).body);
   assert.deepEqual(readReport.report,savedReport.report);
   assert.equal(readReport.actualHistoryAvailable,true);
   assert.ok(readReport.actualNext);assert.ok(readReport.forecastNext);
   const compareRequest={...getReport,baselineReportId:scaleReportId};
   const compared=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,compareRequest)).body;
   const reportExportRequest={kind:1,scope:{case:"report" as const,value:{reportId}}};
   const reportExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,reportExportRequest)).body;
   assert.equal(reportExport.csvSha256Hex,createHash("sha256").update(reportExport.csv).digest("hex"));
   assert.deepEqual((await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,reportExportRequest)).body,reportExport);
   assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,tenantB,reportExportRequest)).status,404);
   const comparisonExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:2,scope:{case:"report",value:{reportId,baselineReportId:scaleReportId}}})).body;
   assert.ok(Buffer.from(comparisonExport.csv).toString("utf8").includes("comparison,"+reportId+",status,comparable"));
   const actualExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:3,scope:{case:"report" as const,value:{reportId}}})).body;
   assert.ok(Buffer.from(actualExport.csv).toString("utf8").includes("signed_minor_units,-100\n"));
   const forecastExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:4,scope:{case:"report" as const,value:{reportId}}})).body;
   assert.ok(Buffer.from(forecastExport.csv).toString("utf8").includes("forecast_source,"));
   assert.equal((await fixtureRpcError(dispatch,sales.exportSalesAnalytics,admin,{kind:3,scope:{case:"report",value:{reportId:scaleReportId}}})).status,409);
   const retainedExport=(await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,{kind:1,scope:{case:"report",value:{reportId:scaleReportId}}})).body;
   assert.ok(Buffer.from(retainedExport.csv).toString("utf8").includes("actual_history_available,false\n"));
   console.log(JSON.stringify({event:"crm.native-sales-analytics-export",result:"PASS",sha256:reportExport.csvSha256Hex}));
   assert.deepEqual(compared,(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,admin,compareRequest)).body);
   assert.deepEqual(required(compared.comparison).baseline,scaleReport);
   assert.equal(required(compared.comparison).status,1);
   assert.equal(required(required(compared.comparison).delta).netMinorUnits,String(-BigInt(scaleCount)*scaleAmount));
   assert.equal(required(required(compared.comparison).delta).targetMinorUnits,"0");
   assert.equal(required(required(compared.comparison).delta).weightedCoverageBasisPoints,undefined);
   const reversedComparison=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{reportId:scaleReportId,baselineReportId:reportId,pageSize:1})).body;
   assert.equal(required(required(reversedComparison.comparison).delta).netMinorUnits,String(BigInt(scaleCount)*scaleAmount));
   assert.equal(reversedComparison.actualHistoryAvailable,false);
   assert.equal(required(reversedComparison.comparison).status,1);
   const identicalComparison=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{...getReport,baselineReportId:reportId})).body;
   assert.equal(required(required(identicalComparison.comparison).delta).netMinorUnits,"0");
   assert.equal(required(required(identicalComparison.comparison).delta).weightedCoverageBasisPoints,"0");
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesPerformanceReport,tenantB,compareRequest)).status,404);
   assert.equal((await fixtureRpcError(dispatch,sales.getSalesPerformanceReport,viewer,{...getReport,baselineReportId:"missing-"+suffix})).status,404);
   const nextReport=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{...getReport,actualAfter:readReport.actualNext,forecastAfter:readReport.forecastNext})).body;
   assert.deepEqual(nextReport.report,readReport.report);
   assert.notDeepEqual(nextReport.actualFacts,readReport.actualFacts);
   assert.notDeepEqual(nextReport.forecastSources,readReport.forecastSources);
   const receiptReport=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{...getReport,receiptIdempotencyKey:reportIntent.precondition.idempotencyKey})).body;
   assert.equal(required(receiptReport.confirmedMutation).resourceVersion,1n);
   assert.equal(required(receiptReport.confirmedMutation).originalRequestId,required(savedReport.receipt).requestId);
   assert.equal(((await fixtureRpc(dispatch,sales.listSalesPerformanceReports,viewer,{quotaId})).body).reports.some(r=>r.reportId===reportId),true);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0]?.n,readBefore);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",reportId))[0]?.n,7);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",reportId))[0]?.n,1);
   // Industrial synchronous capture: >2500 open rows, independent of the
   // closed-event scale proof above. Clone only disposable valid fixture state.
   const prefix="report-scale-open-"+suffix+"-";
   const openScale=2501,limitScale=8193;
   // Fixed-width fixture IDs: indexed binary range, not D1-limited LIKE patterns.
   const fixtureId=(n: number)=>prefix+String(n).padStart(6,"0");
   const fixtureFirst=fixtureId(1),fixtureLast=fixtureId(limitScale);
   const scaledReportId="report-open-scale-"+suffix;
   const capacityReportId="report-capacity-"+suffix;
   const cloneOpen=async(first: number,last: number)=>sql(
     `WITH RECURSIVE nums(n) AS (SELECT ?1 UNION ALL SELECT n+1 FROM nums WHERE n<?2)
      INSERT INTO crm_sales_deal_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor)
      SELECT d.tenant,?3||printf('%06d',n),d.account_id,json_set(d.state,'$.deal_id',?3||printf('%06d',n)),
        d.version,?3||printf('%06d',n),d.idem_digest,?3||printf('%06d',n),d.actor
      FROM nums CROSS JOIN crm_sales_deal_records d WHERE d.tenant=?4 AND d.id=?5`,
     first,last,prefix,"tenant-a",required(forecastInputs[0]).dealId);
   const beforeFence=await revision("tenant-a"),otherFence=await revision("tenant-b");
   try{
     await cloneOpen(1,openScale);
     // Each inserted Deal and its immutable created event invalidate the shared read fence.
     assert.equal(await revision("tenant-a"),beforeFence+2n*BigInt(openScale));
     assert.equal(await revision("tenant-b"),otherFence);
     const openAmount=3001n+BigInt(openScale)*1001n;
     const weightedAmount=10005000n+BigInt(openScale)*1001n*5000n;
     const captureInput={quotaId,quotaVersion:2n,pageSize:1};
     const started=Date.now();
     const largePreview=(await fixtureRpc(dispatch,sales.previewSalesPerformanceReport,viewer,captureInput)).body;
     const previewMs=Date.now()-started;
     assert.equal(required(required(largePreview.report).forecast).openMinorUnits,String(openAmount));
     assert.equal(required(required(largePreview.report).forecast).weightedMinorUnitsTimesBasisPoints,String(weightedAmount));
     assert.equal(required(required(largePreview.report).forecast).includedCount,BigInt(openScale+2));
     assert.equal(largePreview.forecastSources.length,1);
     assert.equal(largePreview.moreForecastSources,true);
     const saveStart=Date.now();
     const largeReport=((await fixtureRpc(dispatch,sales.saveSalesPerformanceReport,admin,{...reportIntent,reportId:scaledReportId,precondition:pre("open-scale-report")})).body).report;
     const saveMs=Date.now()-saveStart;
     assert.deepEqual(required(largeReport).forecast,required(largePreview.report).forecast);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",scaledReportId))[0]?.n,openScale+7);
     const tail=(await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{reportId:scaledReportId,forecastAfter:prefix+String(openScale-1).padStart(6,"0"),pageSize:1})).body;
     assert.equal(required(tail.forecastSources[0]).dealId,prefix+String(openScale).padStart(6,"0"));
     assert.equal(required(required(tail.forecastSources[0]).amount).minorUnits,1001n);
     assert.equal(required(tail.forecastSources[0]).probabilityBasisPoints,5000);
     assert.equal((await fixtureRpcError(dispatch,sales.getSalesPerformanceReport,tenantB,{reportId:scaledReportId})).status,404);
     const fence=await revision("tenant-a");
     await assert.rejects(()=>batch([
       statement("UPDATE crm_sales_deal_records SET state=state WHERE tenant=? AND id=?","tenant-a",required(forecastInputs[0]).dealId),
       statement("INSERT INTO crm_ci_missing_forecast_table VALUES (1)")
     ]));
     assert.equal(await revision("tenant-a"),fence,"failed batch must roll back the fence");
     // Race a real typed owner command against the capture. The fixture
     // loader cannot bypass the catalogue rank-only guard on existing Deals.
     // Changing a team's eligibility would affect many rows but would also
     // alter unrelated fixture semantics. Use a single real Deal mutation.
     const raceFence=await revision("tenant-a");
     const [race,mutation]=await Promise.all([
       fixtureRpcRace(dispatch,sales.previewSalesPerformanceReport,viewer,captureInput),
       fixtureRpc(dispatch,sales.updateDeal,admin,{dealId:fixtureId(openScale),probabilityBasisPoints:1000,
         precondition:pre("open-scale-race",1n)})
     ]);
     assert.equal(mutation.status,200);
     assert.equal(required(mutation.body.deal).version,2n);
     assert.equal(required(mutation.body.deal).probabilityBasisPoints,1000);
     assert.equal(await revision("tenant-a"),raceFence+2n);
     const newWeight=weightedAmount-1001n*4000n;
     assert.ok([200,409].includes(race.status));
     if(race.status===200) {
       const body=required(race.body);
       assert.ok([String(weightedAmount),String(newWeight)].includes(required(required(body.report).forecast).weightedMinorUnitsTimesBasisPoints),"mixed capture");
     } else assert.match(string(record(race.errorBody).message),/SALES_FORECAST_SNAPSHOT_CHANGED/);
     assert.equal(required(required(((await fixtureRpc(dispatch,sales.previewSalesPerformanceReport,viewer,captureInput)).body).report).forecast).weightedMinorUnitsTimesBasisPoints,String(newWeight));
     assert.deepEqual(((await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,{reportId:scaledReportId,pageSize:1})).body).report,largeReport);
     await cloneOpen(openScale+1,limitScale);
     // The same retained fixture exceeds both obsolete historical limits.
     // Exact summary membership is checked against original physical D1 records.
     const expectedHistory=BigInt(String((await sql("SELECT COUNT(*) n FROM crm_sales_deal_records WHERE tenant=? AND json_extract(state,'$.stage.pipeline_id')=?",
       "tenant-a",pipeline))[0]?.n));
     const historyLarge=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,{...historyInput,pageSize:5})).body;
     assert.ok(expectedHistory>8192n);
     assert.equal(required(historyLarge.current).totalDealCount,expectedHistory);
     assert.equal(required(required(historyLarge.current).stateAtCutoff).totalCount+required(historyLarge.current).unknownHistoryCount,expectedHistory);
     assert.equal(required(historyLarge.current).deals.length,5);
     const historyLargeNext=(await fixtureRpc(dispatch,sales.getSalesHistory,viewer,{...historyInput,pageSize:5,
       snapshot:historyLarge.snapshot,detail:{afterDealId:required(historyLarge.current).nextDealId}})).body;
     assert.equal(required(historyLargeNext.detail).deals.length,5);
     assert.ok(required(required(historyLargeNext.detail).deals[0]).dealId>required(historyLarge.current).nextDealId);
     console.log(JSON.stringify({event:"crm.native-history-industrial-scale",result:"PASS",
       deals:String(expectedHistory),page_size:5,exact_membership:true,fenced_detail:true}));
     const oversized=await fixtureRpcError(dispatch,sales.previewSalesPerformanceReport,viewer,captureInput);
     assert.equal(oversized.status,429);
     assert.match(string(record(oversized.body).message),/SALES_FORECAST_CAPTURE_CAPACITY/);
     const refused=await fixtureRpcError(dispatch,sales.saveSalesPerformanceReport,admin,{...reportIntent,reportId:capacityReportId,precondition:pre("capacity-report")});
     assert.equal(refused.status,429);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_reports WHERE tenant=? AND id=?","tenant-a",capacityReportId))[0]?.n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",capacityReportId))[0]?.n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",capacityReportId))[0]?.n,0);
     assert.equal(await revision("tenant-b"),otherFence);
     console.log(JSON.stringify({event:"crm.native-d3-open-forecast-scale",result:"PASS",added_open_deals:openScale,preview_ms:previewMs,save_ms:saveMs,capture_capacity_rejected:true,concurrent_outcome:race.status,exact_weight:true,tenant_fence:true,rollback_fence:true}));
   }catch(error){
     console.error("D3 open forecast scale primary failure:",error);
     throw error;
   }finally{
     await sql("DELETE FROM crm_sales_deal_events WHERE tenant=? AND deal_id>=? AND deal_id<=?","tenant-a",fixtureFirst,fixtureLast);
     await sql("DELETE FROM crm_sales_deal_receipts WHERE tenant=? AND key>=? AND key<=?","tenant-a",fixtureFirst,fixtureLast);
     await sql("DELETE FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",pre("open-scale-race",1n).idempotencyKey);
     await sql("DELETE FROM crm_audit WHERE tenant_id=? AND subject_id>=? AND subject_id<=?","tenant-a",fixtureFirst,fixtureLast);
     await sql("DELETE FROM crm_sales_deal_records WHERE tenant=? AND id>=? AND id<=?","tenant-a",fixtureFirst,fixtureLast);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_records WHERE tenant=? AND id>=? AND id<=?","tenant-a",fixtureFirst,fixtureLast))[0]?.n,0);
   }
   const faultId="report-fault-"+suffix;
   await sql("CREATE TRIGGER fixture_report_audit_fail BEFORE INSERT ON crm_audit WHEN NEW.action='sales.performance_report.save' BEGIN SELECT RAISE(ABORT,'fixture report audit rollback'); END;");
   try{
     assertUnconfirmed(await fixtureRpcError(dispatch,sales.saveSalesPerformanceReport,admin,{...reportIntent,reportId:faultId,precondition:pre("report-fault")}),"SALES_PERFORMANCE_REPORT_WRITE_UNCONFIRMED");
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_reports WHERE tenant=? AND id=?","tenant-a",faultId))[0]?.n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",faultId))[0]?.n,0);
   }finally{await sql("DROP TRIGGER fixture_report_audit_fail");}
   const beforeOwnerFence=await revision("tenant-a");
   const changedForecast=(await fixtureRpc(dispatch,sales.updateDeal,admin,{dealId:required(forecastInputs[0]).dealId,amount:{operation:{case:"set",value:{currencyCode:"EUR",minorUnits:9000n}}},precondition:pre("forecast-update",1n)})).body;
   assert.equal(required(changedForecast.deal).version,2n);
   assert.equal(await revision("tenant-a"),beforeOwnerFence+2n);
   assert.equal(await revision("tenant-b"),otherFence);
   assert.equal(required(required(((await fixtureRpc(dispatch,sales.previewSalesPerformanceReport,viewer,{quotaId,quotaVersion:2n})).body).report).forecast).openMinorUnits,"11000");
   assert.deepEqual((await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,getReport)).body,readReport);
   for(const [index,input] of forecastInputs.entries())(await fixtureRpc(dispatch,sales.closeDeal,admin,{dealId:input.dealId,outcome:3,reasonCode:"qa.report.cleanup",precondition:pre("report-close-"+index,index===0?2n:1n)})).body;
   assert.deepEqual((await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,getReport)).body,readReport);
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{teamId:reportTeam,active:false,precondition:pre("archive-report-team",1n)})).status,200);
   const beforeQuota=await sql("SELECT state,version FROM crm_sales_quota_records WHERE tenant=? AND id=?","tenant-a",quotaId);
   await sql("CREATE TRIGGER fixture_fail_quota_audit BEFORE INSERT ON crm_audit WHEN NEW.action='sales.quota.update' BEGIN SELECT RAISE(ABORT,'fixture quota audit rollback'); END;");
   try{
     assertUnconfirmed(await fixtureRpcError(dispatch,sales.updateSalesQuota,admin,{quotaId,configuration:{...publish.configuration,revisionReason:"qa.fault"},precondition:pre("quota-fault",2n)}),"SALES_QUOTA_WRITE_UNCONFIRMED");
     assert.deepEqual(await sql("SELECT state,version FROM crm_sales_quota_records WHERE tenant=? AND id=?","tenant-a",quotaId),beforeQuota);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_quota_revisions WHERE tenant=? AND id=?","tenant-a",quotaId))[0]?.n,2);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_quota_receipts WHERE tenant=? AND key=?","tenant-a",pre("quota-fault",2n).idempotencyKey))[0]?.n,0);
   }finally{await sql("DROP TRIGGER fixture_fail_quota_audit");}
   const revised=(await fixtureRpc(dispatch,sales.updateSalesQuota,admin,{quotaId,configuration:{...publish.configuration,target:{currencyCode:"EUR",minorUnits:2000n},revisionReason:"qa.revise"},precondition:pre("quota-revise",2n)})).body;
   assert.equal(required(revised.quota).version,3n);
   const historical=(await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos})).body;
   assert.equal(historical.remainingMinorUnits,"1000");
   assert.equal(((await fixtureRpc(dispatch,sales.getSalesQuotaPerformance,viewer,{quotaId})).body).remainingMinorUnits,"2000");
   (await fixtureRpc(dispatch,sales.updateSalesQuota,admin,{quotaId,configuration:{...required(required(revised.quota).configuration),archived:true,revisionReason:"qa.archive"},precondition:pre("quota-archive",3n)})).body;
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_quota_revisions WHERE tenant=? AND id=?","tenant-a",quotaId))[0]?.n,4);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",quotaId))[0]?.n,4);
   assert.deepEqual((await fixtureRpc(dispatch,sales.getSalesPerformanceReport,viewer,getReport)).body,readReport);
   assert.deepEqual(((await fixtureRpc(dispatch,sales.saveSalesPerformanceReport,admin,reportIntent)).body).report,savedReport.report);
   assert.deepEqual((await fixtureRpc(dispatch,sales.exportSalesAnalytics,admin,reportExportRequest)).body,reportExport);
   console.log(JSON.stringify({event:"crm.native-d3-performance-report",result:"PASS",exact_forecast:true,immutable_report:true,source_paging:true,tenant_ab:true,receipt_replay:true,atomic_audit_rollback:true,no_read_audit:true,retention_visible:true}));
   (await fixtureRpc(dispatch,sales.updateTerritory,admin,{territoryId,configuration:{name:"Routing territory "+suffix,defaultOwner:actor(emailB),archived:true},precondition:pre("archive-territory",1n)})).body;
   assert.equal((await fixtureRpc(dispatch,member.updateTeam,admin,{teamId:team,active:false,precondition:pre("archive-team",1n)})).status,200);
   for(const [label,email] of [["a",emailA],["b",emailB]] as const)assert.equal((await fixtureRpc(dispatch,member.updateMember,admin,{email,active:false,precondition:pre("deactivate-"+label,1n)})).status,200);
   console.log(JSON.stringify({event:"crm.native-sk07d2-routing",result:"PASS",priority:true,exact_money:true,capacity_race:true,overflow:true,stale_preview:true,immutable_attribution:true,tenant_ab:true,receipt_event_audit:true}));
}

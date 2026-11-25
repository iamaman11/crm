import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {readFileSync} from "node:fs";
import {join} from "node:path";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {fixtureRpc,fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {record,string} from "../../scripts/json-boundary.mts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type BindingValue = string | number | null | ArrayBuffer;
type FixtureResult = {success: boolean; results: Record<string,unknown>[]};
type Statement = {bind(...values: BindingValue[]): Statement; all(): Promise<FixtureResult>};
type ScaleFixture = {
  dispatch: RpcDispatch; cwd: string; accountId: string; viewer: string; admin: string;
  sql: (statement: string, ...bindings: BindingValue[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  statement: (query: string, ...bindings: BindingValue[]) => Statement;
  batch: (statements: Statement[]) => Promise<FixtureResult[]>;
  describeQueryPlan: (value: ReadonlyArray<Record<string,unknown>>) => string;
};
export async function verifySalesScale({dispatch,cwd,accountId,viewer,admin,sql,statement,batch,describeQueryPlan}: ScaleFixture) {
const sales=DealService.method;
const metricRead=(filter: {pipelineId: string; status: 1; accountId: string})=>
  fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
const number=(value: unknown): number=>{assert.ok(typeof value==="number");return value;};
const integer=(value: unknown): bigint=>{
  assert.ok(typeof value==="string"||typeof value==="number"||typeof value==="bigint");
  return BigInt(value);
};
  const sourceId="r7-totals-02";
  const [source]=(await sql("SELECT state,account_id,version,actor FROM crm_sales_deal_records "+
    "WHERE tenant='tenant-a' AND id='"+sourceId+"'"));
  assert.ok(source);
  const sourceState=string(source.state), sourceAccount=string(source.account_id), sourceVersion=number(source.version), sourceActor=string(source.actor);
  assert.ok(sourceState.includes('"'+sourceId+'"'),
    "load fixture must clone a valid domain-serialized Deal");
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records "+
    "WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%'"))[0]?.n,0);
  // The two native D1 scale checkpoints (100 and 1000 Deals) used to bind
  // and execute 1000 separate INSERT statements. SQLite can generate the exact
  // same disposable records in one native INSERT per checkpoint.
  const insertSql=`WITH RECURSIVE scale_ids(i) AS (
    SELECT ?1 UNION ALL SELECT i+1 FROM scale_ids WHERE i<?2
  )
  INSERT INTO crm_sales_deal_records
    (tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor,last_action)
  SELECT 'tenant-a','r7-scale-'||printf('%04d',i),?3,
    replace(?4,?5,'"'||'r7-scale-'||printf('%04d',i)||'"'),
    ?6,'r7-scale-key-'||i,'load-fixture',lower(hex(randomblob(16))),?7,'sales_deals.create'
  FROM scale_ids RETURNING id`;
  let inserted=0;
  const seedTo=async (n: number)=>{
    if(inserted>=n)return;
    const began=performance.now(),from=inserted;
    const rows=await sql(insertSql,from,n-1,sourceAccount,sourceState,
      '"'+sourceId+'"',sourceVersion,sourceActor);
    const expectedIds=Array.from({length:n-from},(_,offset)=>
      "r7-scale-"+String(from+offset).padStart(4,"0"));
    assert.deepEqual(rows.map(row=>row.id).sort(),expectedIds,
      "native D1 must create precisely the required Deal fixture IDs");
    const mismatches=await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records "+
      "WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%' "+
      "AND json_extract(state,'$.deal_id')<>id");
    assert.equal(mismatches[0]?.n,0,"every cloned domain state must retain its own Deal ID");
    inserted=n;
    console.log(JSON.stringify({event:"crm.native-r7-scale-fixture",result:"PASS",
      total_deals:n,created:n-from,sql_inserts:1,elapsed_ms:Math.round(performance.now()-began)}));
  };
  const benchmark=async (n: number)=>{
    await seedTo(n);
    const began=performance.now();
    const first=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
      pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:25}});
    const latencyMs=Math.round(performance.now()-began);
    assert.equal(first.status,200,formatRpcFailure(first));
    assert.equal(first.body.totalCount,BigInt(n+32));
    assert.equal(first.body.deals.length,25);
    assert.ok(first.body.pageInfo?.nextPageToken);
    assert.deepEqual(first.body.currencyTotals.map(t=>[
      t.currencyCode,t.totalMinorUnits,t.amountDealCount
    ]),[
      ["JPY","345",1n],["KWD","4567",1n],
      ["PLN",String(BigInt(n+1)*2345n),BigInt(n+1)],
      ["USD","9223372036854775809",2n]
    ],"summary must cover all "+n+" added Deals, not the 25-card page");
    const second=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{
      pipelineId:"pipeline.other",stageId:"lead",
      page:{pageSize:25,pageToken:first.body.pageInfo.nextPageToken}});
    assert.equal(second.status,200);
    assert.equal(second.body.deals.length,25);
    assert.equal(second.body.totalCount,BigInt(n+32));
    assert.deepEqual(second.body.currencyTotals,first.body.currencyTotals);
    assert.ok(latencyMs<15000,
      "bounded indexed native D1 summary exceeded 15 s at "+n+" Deals: "+latencyMs+" ms");
    // SQL readback keeps money as text, including sums beyond signed int64.
    const filter={pipelineId:"pipeline.other",status:1 as const,accountId};
    const metricStart=performance.now();
    const metrics=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
    const metricMs=Math.round(performance.now()-metricStart);
    assert.equal(metrics.status,200,formatRpcFailure(metrics));
    const facts=await sql("SELECT json_extract(state,'$.amount.currency') AS currency,"+
      "json_extract(state,'$.amount.minor_units') AS units,"+
      "json_extract(state,'$.probability_basis_points') AS bps FROM crm_sales_deal_records "+
      "WHERE tenant=? AND json_extract(state,'$.stage.pipeline_id')=? "+
      "AND json_extract(state,'$.status')='open' AND account_id=?",
      "tenant-a","pipeline.other",accountId);
    assert.equal(facts.length,n+32);
    assert.equal(metrics.body.totalDealCount,BigInt(facts.length));
    const expected=new Map<string,[bigint,bigint,bigint]>();
    for(const row of facts){
      if(row.currency===null)continue;
      const currency=string(row.currency);
      const v=expected.get(currency)??[0n,0n,0n];
      v[0]+=integer(row.units);v[1]+=integer(row.units)*integer(row.bps);v[2]++;
      expected.set(currency,v);
    }
    assert.deepEqual(metrics.body.currencies.map(x=>[
      x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
    ]),[...expected].sort(([a],[b])=>a.localeCompare(b)).map(([c,v])=>[c,String(v[0]),String(v[1]),v[2]]));
    assert.deepEqual(metrics.body.statuses.map(x=>[x.stageId,x.status,x.dealCount]),
      [["lead",1,BigInt(n+32)]]);
    assert.equal(metrics.body.operational[0]?.knownStageAgeCount,BigInt(n+32));
    assert.equal(metrics.body.operational[0]?.noExpectedCloseCount,BigInt(n+30));
    assert.ok(metricMs<15000,"native pipeline metrics exceeded 15 s at "+n+" Deals: "+metricMs+" ms");
    console.log(JSON.stringify({event:"crm.native-r73d-metrics-scale",result:"PASS",
      added_deals:n,matched_deals:facts.length,latency_ms:metricMs,exact_sql_comparison:true,
      nested_filters:true,exact_i128:true}));
    if(n===1000){
      const portfolio=await fixtureRpc(dispatch,sales.getSalesPortfolio,viewer,{pipelineId:"pipeline.other"});
      assert.equal(portfolio.status,200,formatRpcFailure(portfolio));
      const historyYear=new Date().getUTCFullYear();
      const historyScope={pipelineId:"pipeline.other",timeZone:"UTC",pageSize:17,
        period:{startDate:{year:historyYear-1,month:1,day:1},endDate:{year:historyYear,month:12,day:31}}};
      const history=await fixtureRpc(dispatch,sales.getSalesHistory,viewer,historyScope);
      assert.equal(history.status,200,formatRpcFailure(history));
      assert.ok(history.body.current?.stateAtCutoff);
      assert.equal(history.body.current.totalDealCount,1032n);
      assert.equal(history.body.current.deals.length,17);
      assert.equal(history.body.current.stateAtCutoff.totalCount+history.body.current.unknownHistoryCount,1032n);
      const historyNext=await fixtureRpc(dispatch,sales.getSalesHistory,viewer,{...historyScope,
        snapshot:history.body.snapshot,detail:{afterDealId:history.body.current.nextDealId}});
      assert.equal(historyNext.status,200,formatRpcFailure(historyNext));
      assert.ok(historyNext.body.detail?.deals[0]);
      assert.equal(historyNext.body.detail.deals.length,17);
      assert.ok(historyNext.body.detail.deals[0].dealId>history.body.current.nextDealId);
      console.log(JSON.stringify({event:"crm.native-history-scale",result:"PASS",
        deals:1032,page_size:17,full_summary:true,fenced_detail:true}));
      assert.ok(portfolio.body.counts);
      assert.equal(portfolio.body.counts.totalDealCount,1032n);
      assert.deepEqual(portfolio.body.forecastCurrencies.map(x=>[
        x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
      ]),metrics.body.forecastCurrencies.map(x=>[
        x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
      ]));
      // Both ends of the old paged scan change in a single fixture transaction.
      // Every committed state has the same total and weighted amount.
      const ids=["r7-scale-0000","r7-scale-0999"] as const;
      // Atomic D1 batch competes with signed RPCs via the same binding.
      const updateSql="UPDATE crm_sales_deal_records SET "+
        "state=json_set(state,'$.amount.minor_units',?,'$.version',version+1),version=version+1,"+
        "idem_key=?,idem_digest='load-fixture',request_id=?,last_action='sales_deals.update' "+
        "WHERE tenant='tenant-a' AND id=? RETURNING id";
      const readBalances=async()=>(await sql("SELECT json_type(state,'$.amount.minor_units') AS kind,"+
        "json_extract(state,'$.amount.minor_units') AS units "+
        "FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id IN (?,?) ORDER BY id",
        ...ids)).map(row=>[row.kind,row.units]);
      const baseline=metrics.body.currencies;
      const forecastBaseline=metrics.body.forecastCurrencies;
      assert.deepEqual(await readBalances(),[["text","2345"],["text","2345"]]);
      for(let attempt=0;attempt<12;attempt++){
        const amounts=attempt%2===0?["2346","2344"] as const:["2345","2345"] as const;
        const statements=ids.map((id,i)=>statement(updateSql,string(amounts[i]),
          "r7-scale-key-swap-"+attempt+"-"+i,randomUUID(),id));
        let read,write;
        if(attempt%2===0){
          read=metricRead(filter);
          write=batch(statements);
        }else{
          write=batch(statements);
          read=metricRead(filter);
        }
        const [snapshot,changed]=await Promise.all([read,write]);
        assert.deepEqual(changed.map(result=>result.results[0]?.id),ids);
        assert.deepEqual(await readBalances(),amounts.map(value=>["text",value]));
        if(snapshot.status!==200)
          console.error(JSON.stringify({event:"crm.native-r73d-concurrent-failure",
            attempt,status:snapshot.status,request_id:snapshot.headers.get("x-request-id")}));
        assert.equal(snapshot.status,200,formatRpcFailure(snapshot));
        const snapshotBody=snapshot.body;
        assert.equal(snapshotBody.totalDealCount,1032n);
        assert.deepEqual(snapshotBody.currencies,baseline,
          "a concurrent atomic balance swap must not create a mixed money snapshot");
        assert.deepEqual(snapshotBody.forecastCurrencies,forecastBaseline,
          "forecast must share the same consistent snapshot");
        const whole=await fixtureRpc(dispatch,sales.getSalesPortfolio,viewer,{pipelineId:"pipeline.other"});
        assert.equal(whole.status,200,formatRpcFailure(whole));
        assert.deepEqual(whole.body.forecastCurrencies.map(x=>[
          x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
        ]),forecastBaseline.map(x=>[
          x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
        ]));
      }
      const after=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
      assert.equal(after.status,200,formatRpcFailure(after));
      assert.deepEqual(after.body.currencies,baseline);
      console.log(JSON.stringify({event:"crm.native-r73d-concurrent-metrics",result:"PASS",
        committed_atomic_swaps:12,concurrent_reads:12,final_read:true,exact_totals:true,d1_binding:true}));
      // Corrupt a disposable record outside the visible page. Both analytics
      // and Kanban must reject it through the original Rust persistence owner.
      // D1 permits only a catalog rank rebase without a version increment.
      // Use a disposable versioned fixture mutation, with unique receipt and
      // request identity, to exercise malformed persistence without bypassing
      // the real D1 guards or audit/history triggers.
      const invalidId=ids[1];
      await assert.rejects(sql("UPDATE crm_sales_deal_records SET "+
        "state=json_set(state,'$.amount.minor_units',2345) WHERE tenant='tenant-a' AND id=?",
        invalidId),/SALES_CATALOG_RANK_ONLY_REQUIRED/);
      let faultSequence=0;
      const invalid=async(field: string,value: string,id: string)=>{
        const [before]=await sql("SELECT version FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id=?",id);
        assert.ok(before);
        const beforeVersion=number(before.version);
        const key="r7-scale-key-fault-"+(++faultSequence);
        const rows=await sql("UPDATE crm_sales_deal_records SET "+
          "state=json_set(state,?,json(?),'$.version',version+1),version=version+1,"+
          "idem_key=?,idem_digest='load-fixture',request_id=?,last_action='sales_deals.update' "+
          "WHERE tenant='tenant-a' AND id=? AND version=? RETURNING id,version",
          field,value,key,randomUUID(),id,beforeVersion);
        assert.deepEqual(rows,[{id,version:beforeVersion+1}]);
        const [receipt]=await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts WHERE tenant='tenant-a' AND key=?",key);
        assert.equal(receipt?.n,1);
      };
      for(const [field,bad,good] of [
        ["$.amount.minor_units","2345",'"2345"'],
        ["$.amount.minor_units","2345.0",'"2345"'],
        ["$.probability_basis_points",'"2000"',"2000"],
        ["$.probability_basis_points","2000.5","2000"],
        ["$.probability_source",'"unknown"','"manual"'],
        ["$.forecast_category",'"unknown"','"pipeline"']
      ] as const){
        try{
          await invalid(field,bad,invalidId);
          const brokenMetrics=await fixtureRpcError(dispatch,sales.getPipelineMetrics,viewer,filter);
          assert.equal(brokenMetrics.status,500);
          assert.equal(record(brokenMetrics.body).message,"Sales D1 metrics read failed");
          const brokenBoard=await fixtureRpcError(dispatch,sales.listStageDeals,viewer,{...filter,stageId:"lead",
            page:{pageSize:1}});
          assert.equal(brokenBoard.status,500);
          assert.equal(record(brokenBoard.body).message,"Sales D1 stage list failed");
        }finally{
          await invalid(field,good,invalidId);
        }
      }
      const restored=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
      assert.equal(restored.status,200);
      assert.deepEqual(restored.body.currencies,baseline);
      // The original persisted Money domain spans i128, while this version
      // of the individual public Deal wire amount is i64. Test off-page facts.
      const largeUnits=9223372036854775808n;
      try{
        await invalid("$.amount.minor_units",JSON.stringify(String(largeUnits)),invalidId);
        const large=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
        const largeBoard=await fixtureRpc(dispatch,sales.listStageDeals,viewer,{...filter,stageId:"lead",page:{pageSize:1}});
        assert.equal(large.status,200,formatRpcFailure(large));
        assert.equal(largeBoard.status,200,formatRpcFailure(largeBoard));
        const oldPln=baseline.find(x=>x.currencyCode==="PLN");
        const newPln=large.body.currencies.find(x=>x.currencyCode==="PLN");
        assert.ok(oldPln && newPln);
        const delta=largeUnits-2345n;
        assert.equal(newPln.totalMinorUnits,String(BigInt(oldPln.totalMinorUnits)+delta));
        assert.equal(newPln.weightedMinorUnitsTimesBasisPoints,
          String(BigInt(oldPln.weightedMinorUnitsTimesBasisPoints)+delta*2000n));
        assert.equal(largeBoard.body.currencyTotals.find(x=>x.currencyCode==="PLN")?.totalMinorUnits,
          newPln.totalMinorUnits);
      }finally{
        await invalid("$.amount.minor_units",'"2345"',invalidId);
      }
      // Exercise ordinary signed Connect mutations concurrently with reads.
      // Each capture must match one entire committed state, not a hybrid.
      const current=await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:invalidId});
      assert.equal(current.status,200);
      assert.ok(current.body.deal);
      const payload={dealId:invalidId,amount:{operation:{case:"set" as const,value:{
        minorUnits:2346n,currencyCode:"PLN"}}},precondition:{
        expectedVersion:current.body.deal.version,idempotencyKey:"r7-scale-key-rpc-update"}};
      const [capture,boardCapture,changed]=await Promise.all([
        fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter),
        fixtureRpc(dispatch,sales.listStageDeals,viewer,{...filter,stageId:"lead",page:{pageSize:1}}),
        fixtureRpc(dispatch,sales.updateDeal,admin,payload)]);
      assert.equal(capture.status,200,formatRpcFailure(capture));
      assert.equal(boardCapture.status,200,formatRpcFailure(boardCapture));
      assert.equal(changed.status,200,formatRpcFailure(changed));
      const committed=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
      assert.equal(committed.status,200);
      const beforePln=baseline.find(x=>x.currencyCode==="PLN");
      const afterPln=committed.body.currencies.find(x=>x.currencyCode==="PLN");
      assert.ok(beforePln && afterPln);
      assert.equal(afterPln.totalMinorUnits,String(BigInt(beforePln.totalMinorUnits)+1n));
      assert.equal(afterPln.weightedMinorUnitsTimesBasisPoints,
        String(BigInt(beforePln.weightedMinorUnitsTimesBasisPoints)+2000n));
      const numericSignature=(x: typeof baseline)=>JSON.stringify(x.map(m=>[m.currencyCode,m.totalMinorUnits,
        m.weightedMinorUnitsTimesBasisPoints,String(m.amountDealCount)]));
      assert.ok([numericSignature(baseline),numericSignature(committed.body.currencies)]
        .includes(numericSignature(capture.body.currencies)));
      const boardPln=boardCapture.body.currencyTotals.find(x=>x.currencyCode==="PLN");
      assert.ok(boardPln);
      assert.ok([beforePln.totalMinorUnits,afterPln.totalMinorUnits].includes(boardPln.totalMinorUnits));
      assert.ok(changed.body.deal);
      const revert=await fixtureRpc(dispatch,sales.updateDeal,admin,{...payload,
        amount:{operation:{case:"set",value:{minorUnits:2345n,currencyCode:"PLN"}}},
        precondition:{expectedVersion:changed.body.deal.version,idempotencyKey:"r7-scale-key-rpc-revert"}});
      assert.equal(revert.status,200,formatRpcFailure(revert));
      const final=await fixtureRpc(dispatch,sales.getPipelineMetrics,viewer,filter);
      assert.equal(final.status,200);
      assert.deepEqual(final.body.currencies,baseline);
      console.log(JSON.stringify({event:"crm.native-r73d-strict-persistence",result:"PASS",
        invalid_money_types:2,invalid_probability_types:2,invalid_forecast_metadata_types:2,board_and_metrics:true,
        individual_persisted_i128:true,signed_concurrent_update:true,restored:true}));
    }
    console.log(JSON.stringify({event:"crm.native-r71-kanban-scale",result:"PASS",
      added_deals:n,stage_total_count:n+32,rendered_per_page:25,
      full_stage_currencies:4,latency_ms:latencyMs,cursor_stable:true,
      original_rust_owner:true,exact_i128:true}));
  };
  try {
    // EXPLAIN both actual native Rust statements, not approximations.
    const rust=readFileSync(join(cwd,"src","sales_deals.rs"),"utf8");
    const stageQuery=rust.match(/"SELECT id,state FROM crm_sales_deal_records WHERE \{condition\} ORDER BY id"/)?.[0];
    assert.ok(stageQuery,"canonical stage snapshot SELECT must be present");
    const stageSql=stageQuery.slice(1,-1).replace("{condition}",
      "tenant=? AND json_extract(state,'$.stage.pipeline_id')=? "+
      "AND json_extract(state,'$.stage.stage_id')=? AND json_extract(state,'$.status')=? AND account_id=?");
    const plan=await sql("EXPLAIN QUERY PLAN "+stageSql,
      "tenant-a","pipeline.other","lead","open",accountId);
    assert.ok(plan.some(row=>string(row.detail).includes("idx_crm_sales_board_stage")),
      "the 1000+ row snapshot must use the existing stage/ID index");
    // Compose the actual Rust format string from its shared raw SQL constants.
    // Preserve the same indexed plan checks after extracting the common fields.
    const clock=rust.match(/const METRIC_CLOCK: &str = r#"([\s\S]*?)"#;/)?.[1];
    const fields=rust.match(/const METRIC_FIELDS: &str = r#"([\s\S]*?)"#;/)?.[1];
    const query=rust.match(/"WITH \{METRIC_CLOCK\} SELECT \{METRIC_FIELDS\} FROM snapshot_clock c CROSS JOIN crm_sales_deal_records d WHERE \{condition\}"/)?.[0];
    assert.ok(clock&&fields&&query,"canonical metrics SELECT must be present");
    const metricsSql=query.slice(1,-1).replace("{METRIC_CLOCK}",clock).replace("{METRIC_FIELDS}",fields).replace("{condition}",
      "d.tenant=? AND json_extract(d.state,'$.stage.pipeline_id')=? "+
      "AND json_extract(d.state,'$.status')=? AND d.account_id=?");
    const metricsPlan=await sql("EXPLAIN QUERY PLAN "+metricsSql,
      "tenant-a","pipeline.other","open",accountId);
    assert.ok(metricsPlan.some(row=>string(row.detail).includes("idx_crm_sales_board_stage")));
    assert.ok(metricsPlan.some(row=>string(row.detail).includes("crm_sales_deal_events")));
    assert.ok(metricsPlan.some(row=>string(row.detail).includes("idx_crm_sales_link_deal")));
    const portfolioQuery=rust.match(/r#"(WITH \{METRIC_CLOCK\},[\s\S]*?FROM snapshot_clock c CROSS JOIN selected_deals d)"#/)?.[1];
    const portfolioCondition=rust.match(/"(d\.tenant=\? AND json_extract\(d\.state,'\$\.stage\.pipeline_id'\) IN \(SELECT id FROM eligible WHERE \? OR json_extract\(state,'\$\.archived'\)=0\))"/)?.[1];
    assert.ok(portfolioQuery&&portfolioCondition,"canonical portfolio snapshot SELECT must be present");
    const portfolioSql=portfolioQuery.replace("{METRIC_CLOCK}",clock).replace("{METRIC_FIELDS}",fields).replace("{condition}",portfolioCondition);
    const portfolioPlan=await sql("EXPLAIN QUERY PLAN "+portfolioSql,
      "tenant-a","pipeline.other","pipeline.other","tenant-a",0);
    assert.ok(portfolioPlan.some(row=>string(row.detail).includes("crm_sales_pipeline_records")),describeQueryPlan(portfolioPlan));
    assert.ok(portfolioPlan.some(row=>string(row.detail).includes("crm_sales_deal_events")),describeQueryPlan(portfolioPlan));
    assert.ok(portfolioPlan.some(row=>string(row.detail).includes("idx_crm_sales_link_deal")),describeQueryPlan(portfolioPlan));
    // The new forecast capture has a different keyset access pattern. Verify
    // both exact query variants without weakening the original plan assertions.
    const captureRust=readFileSync(join(cwd,"src","sales_performance.rs"),"utf8");
    const capturePage=captureRust.match(/SELECT d\.tenant,d\.id,d\.state,d\.account_id FROM crm_sales_deal_records d WHERE d\.tenant=\?1[\s\S]*?ORDER BY d\.id LIMIT \?4/)?.[0];
    assert.ok(capturePage,"canonical forecast keyset SELECT must be present");
    for(const [filter,index,params] of [
      ["","idx_crm_forecast_open_deal",["tenant-a","", "0",256]],
      [" AND json_extract(d.state,'$.stage.pipeline_id')=?5","idx_crm_forecast_open_pipeline",["tenant-a","","0",256,"pipeline.other"]]
    ] as const){
      const captureSql="WITH fence AS MATERIALIZED (SELECT COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?1),0) revision) "+
        capturePage.replace("{pipeline_filter}",filter);
      const capturePlan=await sql("EXPLAIN QUERY PLAN "+captureSql,...params);
      assert.ok(capturePlan.some(row=>string(row.detail).includes("SEARCH d USING INDEX "+index)&&string(row.detail).includes("id>?")),
        describeQueryPlan(capturePlan));
      assert.ok(!capturePlan.some(row=>string(row.detail).includes("TEMP B-TREE FOR ORDER BY")),describeQueryPlan(capturePlan));
    }
    await benchmark(100);
    await benchmark(1000);
  }finally{
    // Only disposable scale fixtures; preserve the same atomic cleanup.
    await batch([
      statement("DELETE FROM crm_sales_deal_events WHERE tenant='tenant-a' AND deal_id LIKE 'r7-scale-%'"),
      statement("DELETE FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%'"),
      statement("DELETE FROM crm_sales_deal_receipts WHERE tenant='tenant-a' AND key LIKE 'r7-scale-key-%'"),
      statement("DELETE FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id LIKE 'r7-scale-%'"),
    ]);
    assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records "+
      "WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%'"))[0]?.n,0);
  }
}

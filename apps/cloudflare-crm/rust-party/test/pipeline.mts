import assert from "node:assert/strict";
import { DealService } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type PipelineFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 admin: string;
 viewer: string;
 tenantB: string;
};

export async function verifyPipeline({dispatch,sql,admin,viewer,tenantB}: PipelineFixture) {
 const sales=DealService.method;
// The authoritative pipeline catalog is created before any new Deal write.
// Stage order is owner-assigned, never supplied as an untrusted rank.
const pipelineSeed={pipelineId:"pipeline.enterprise",name:"Enterprise",
  stages:[
    {stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
    {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500},
    {stageId:"won",name:"Historical terminal",probabilityBasisPoints:10000}
  ],precondition:{idempotencyKey:"connect-r7-pipeline-create-1"}};
assert.ok(pipelineSeed.stages[0]);
assert.equal((await fixtureRpcError(dispatch,sales.createPipeline,viewer,pipelineSeed)).status,403);
assert.equal((await fixtureRpcError(dispatch,sales.createPipeline,admin,{
  ...pipelineSeed,stages:[pipelineSeed.stages[0],pipelineSeed.stages[0]],
  precondition:{idempotencyKey:"connect-r7-pipeline-duplicate-stage"}
})).status,400);
const pipelineCreated=await fixtureRpc(dispatch,sales.createPipeline,admin,pipelineSeed);
assert.equal(pipelineCreated.status,200,formatRpcFailure(pipelineCreated));
assert.equal(pipelineCreated.body.pipeline?.version,1n);
assert.deepEqual(pipelineCreated.body.pipeline?.stages.map(s=>s.ordinal),[1,2,3]);
const pipelineCreateProof=await fixtureRpc(dispatch,sales.getPipeline,viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"connect-r7-pipeline-create-1"
});
assert.equal(pipelineCreateProof.status,200,formatRpcFailure(pipelineCreateProof));
assert.equal(pipelineCreateProof.body.confirmedMutation?.originalRequestId,
  pipelineCreated.body.receipt?.requestId);
assert.equal(pipelineCreateProof.body.confirmedMutation?.resourceVersion,1n);
assert.notEqual(pipelineCreateProof.body.confirmedMutation?.requestId,
  pipelineCreated.body.receipt?.requestId);
assert.equal((await sql("SELECT original_request_id FROM crm_sales_pipeline_receipts "+
  "WHERE tenant='tenant-a' AND key='connect-r7-pipeline-create-1'"))[0]?.original_request_id,
  pipelineCreated.body.receipt?.requestId);
assert.equal((await fixtureRpcError(dispatch,sales.getPipeline,viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"short"
})).status,400);
assert.equal((await fixtureRpc(dispatch,sales.createPipeline,admin,pipelineSeed)).body.receipt?.replayed,true);
assert.equal((await fixtureRpcError(dispatch,sales.createPipeline,admin,{...pipelineSeed,name:"modified"})).status,409);
assert.equal((await fixtureRpcError(dispatch,sales.getPipeline,tenantB,{pipelineId:"pipeline.enterprise"})).status,404);
assert.equal((await fixtureRpc(dispatch,sales.getPipeline,viewer,{pipelineId:"pipeline.enterprise"})).status,200);
const pipelineOther=await fixtureRpc(dispatch,sales.createPipeline,admin,{
  pipelineId:"pipeline.other",name:"Other",stages:[pipelineSeed.stages[0]],
  precondition:{idempotencyKey:"connect-r7-pipeline-create-2"}});
assert.equal(pipelineOther.status,200,formatRpcFailure(pipelineOther));
const wrongPipelineProof=await fixtureRpc(dispatch,sales.getPipeline,viewer,{
  pipelineId:"pipeline.other",receiptIdempotencyKey:"connect-r7-pipeline-create-1"
});
assert.equal(wrongPipelineProof.status,200);
assert.equal(wrongPipelineProof.body.confirmedMutation,undefined,
  "A receipt for another pipeline must never be disclosed as commit evidence");
const pipelineList=await fixtureRpc(dispatch,sales.listPipelines,viewer,{page:{pageSize:1}});
assert.equal(pipelineList.status,200,formatRpcFailure(pipelineList));
assert.equal(pipelineList.body.pipelines.length,1);
assert.ok(pipelineList.body.pageInfo?.nextPageToken);
const currencyRows=pipelineList.body.currencies;
assert.equal(currencyRows.length,165);
const currencyByCode=new Map(currencyRows.map(row=>[row.currencyCode,row.minorUnitExponent]));
assert.equal(currencyByCode.size,currencyRows.length);
for(const [code,exponent] of [["JPY",0],["USD",2],["EUR",2],["KWD",3],["CLF",4]] as const)
  assert.equal(currencyByCode.get(code),exponent);
for(const code of ["ZZZ","XXX","XAU","BGN"])assert.equal(currencyByCode.has(code),false);
const pipelineNext=await fixtureRpc(dispatch,sales.listPipelines,viewer,{page:{pageSize:1,pageToken:pipelineList.body.pageInfo.nextPageToken}});
assert.equal(pipelineNext.status,200,formatRpcFailure(pipelineNext));
assert.equal(pipelineNext.body.pipelines.length,1);
assert.equal(pipelineNext.body.currencies.length,0);
const pipelineUpdated=await fixtureRpc(dispatch,sales.updatePipeline,admin,{
  pipelineId:"pipeline.enterprise",name:"Enterprise managed",
  stages:pipelineSeed.stages,archived:false,
  precondition:{expectedVersion:1n,idempotencyKey:"connect-r7-pipeline-rename-1"}});
assert.equal(pipelineUpdated.status,200,formatRpcFailure(pipelineUpdated));
assert.equal(pipelineUpdated.body.pipeline?.version,2n);
const pipelineUpdateProof=await fixtureRpc(dispatch,sales.getPipeline,viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"connect-r7-pipeline-rename-1"
});
assert.equal(pipelineUpdateProof.status,200);
assert.equal(pipelineUpdateProof.body.confirmedMutation?.originalRequestId,
  pipelineUpdated.body.receipt?.requestId);
assert.equal(pipelineUpdateProof.body.confirmedMutation?.resourceVersion,2n);
const oldPipelineCreateProof=await fixtureRpc(dispatch,sales.getPipeline,viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"connect-r7-pipeline-create-1"
});
assert.equal(oldPipelineCreateProof.body.confirmedMutation?.originalRequestId,
  pipelineCreated.body.receipt?.requestId);
assert.equal(oldPipelineCreateProof.body.confirmedMutation?.resourceVersion,1n,
  "A later update must not overwrite immutable create evidence");
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id='"+
  pipelineUpdateProof.body.confirmedMutation?.requestId+"'"))[0]?.n,0,
  "Receipt lookup is read-only and must not append audit");
assert.equal((await fixtureRpcError(dispatch,sales.updatePipeline,admin,{
  pipelineId:"pipeline.enterprise",name:"stale",
  stages:pipelineSeed.stages,archived:false,
  precondition:{expectedVersion:1n,idempotencyKey:"connect-r7-pipeline-stale"}
})).status,409);
return pipelineSeed;
}

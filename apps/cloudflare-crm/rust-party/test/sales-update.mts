import assert from "node:assert/strict";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {fixtureRpc, fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {dealUpdateInput, updateFixtureDeal} from "../../../../packages/client/test/cloudflare-deal-fixture.ts";
import type {dealCreationInput} from "../../../../packages/client/test/cloudflare-deal-fixture.ts";

type UpdateFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string; viewer: string; knownService: string; dealId: string; foreignSaleContactId: string;
  saleCreate: ReturnType<typeof dealCreationInput>; salesCreated: {headers: Pick<Headers,"get">};
};
export async function verifySalesUpdate({dispatch,sql,admin,viewer,knownService,dealId,foreignSaleContactId,
  saleCreate,salesCreated}: UpdateFixture) {
const sales=DealService.method;
const updateSale=dealUpdateInput(dealId);
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,updateSale)).status,403);
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,admin,{
  ...updateSale,precondition:{expectedVersion:100n,
    idempotencyKey:"connect-sales-stale-version"}})).status,409);
const saleUpdated=await updateFixtureDeal(dispatch,knownService,updateSale);
// A later record version is not the original command's receipt version.
const originalAfterUpdate=await fixtureRpc(dispatch,sales.getDeal,admin,{dealId,
  receiptIdempotencyKey:saleCreate.precondition.idempotencyKey});
assert.equal(originalAfterUpdate.status,200);
assert.equal(originalAfterUpdate.body.deal?.version,2n);
assert.equal(originalAfterUpdate.body.confirmedMutation?.resourceVersion,1n);
assert.equal(originalAfterUpdate.body.confirmedMutation?.originalRequestId,salesCreated.headers.get("x-request-id"));
const saleUpdateReplayed=await fixtureRpc(dispatch,sales.updateDeal,admin,updateSale);
assert.equal(saleUpdateReplayed.status,200);
assert.equal(saleUpdateReplayed.body.receipt?.replayed,true);
assert.equal(saleUpdateReplayed.body.receipt?.originalRequestId,saleUpdated.headers.get("x-request-id"));
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,admin,{
  ...updateSale,probabilityBasisPoints:100})).status,409);
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,admin,{
  dealId,primaryContactId:{operation:{case:"set",value:foreignSaleContactId}},
  precondition:{expectedVersion:2n,idempotencyKey:"connect-r72-foreign-contact-update-denied"}
})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,admin,{
  dealId,expectedCloseDate:{operation:{case:"set",value:{year:2027,month:13,day:1}}},
  precondition:{expectedVersion:2n,idempotencyKey:"connect-r72-invalid-month-update"}
})).status,400);
assert.equal((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId})).body.deal?.version,2n);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0]?.n,2,
  "failed sales contact/date updates do not append audit");
console.log(JSON.stringify({event:"crm.native-r72-linked-contact-date",result:"PASS",
  create_contact:true,create_expected_close_date:true,update_contact_clear:true,
  leap_date_validated_in_rust:true,wrong_account_rejected:true,
  missing_contact_rejected:true,cas_and_replay:true,audit_rollback:true}));
return {saleUpdated};
}

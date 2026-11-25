import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {ContactPointService} from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import {DomainErrorDetailSchema} from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import {decodeFixtureDetail,fixtureRpc,fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {createFixtureDeal,dealCreationInput} from "../../../../packages/client/test/cloudflare-deal-fixture.ts";
import {record,records} from "../../scripts/json-boundary.mts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type CreateFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string; viewer: string; tenantB: string; accountId: string; contactId: string; pagingPersonId: string;
};
export async function verifySalesCreate({dispatch,sql,admin,viewer,tenantB,accountId,contactId,pagingPersonId}: CreateFixture) {
const sales=DealService.method;
const dealId="sales-mid-"+randomUUID();
const stageLead={pipelineId:"pipeline.enterprise",stageId:"lead",ordinal:1};
const stageQualified={pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:2};
const saleCreate=dealCreationInput(dealId,accountId,contactId);
const foreignSaleContactId="sales-contact-foreign-"+randomUUID();
const foreignSaleContact=await fixtureRpc(dispatch,ContactPointService.method.createContactPoint,admin,{
  contactPointId:foreignSaleContactId,partyId:pagingPersonId,kind:1,
  value:"sales-foreign@example.test",preferred:false,
  precondition:{idempotencyKey:"connect-r72-foreign-contact-create"}
});
assert.equal(foreignSaleContact.status,200,formatRpcFailure(foreignSaleContact));
const denialAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0]?.n;
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,null,saleCreate)).status,401);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,viewer,saleCreate)).status,403);
for(const currencyCode of ["ZZZ","XXX","XAU","BGN"]){
  const rejected=await fixtureRpcError(dispatch,sales.createDeal,admin,{
    ...saleCreate,amount:{minorUnits:1n,currencyCode},
    precondition:{idempotencyKey:"connect-sales-unsupported-currency-"+currencyCode}
  });
  assert.equal(rejected.status,400,formatRpcFailure(rejected));
}
const {stage: omittedStage, ...withoutStage}=saleCreate;
assert.ok(omittedStage);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...withoutStage,
  precondition:{idempotencyKey:"connect-sales-no-stage"}})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,accountId:"missing-account",
  precondition:{idempotencyKey:"connect-sales-no-account"}})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,probabilityBasisPoints:10001,
  precondition:{idempotencyKey:"connect-sales-invalid-probability"}})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,stage:{pipelineId:"pipeline.enterprise",stageId:"unknown",ordinal:1},
  precondition:{idempotencyKey:"connect-sales-invalid-catalog-stage"}})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,precondition:{idempotencyKey:"connect-sales-create-invalid-version",
    expectedVersion:1n}})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,primaryContactId:foreignSaleContactId,
  precondition:{idempotencyKey:"connect-r72-foreign-contact-create-denied"}
})).status,400,"a ContactPoint owned by another Party cannot be attached to this Account");
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,primaryContactId:"foreign-tenant-contact-id",
  precondition:{idempotencyKey:"connect-r72-missing-contact-denied"}
})).status,400);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,expectedCloseDate:{year:2027,month:2,day:29},
  precondition:{idempotencyKey:"connect-r72-bad-leapdate-denied"}
})).status,400,"calendar validation must be in original Rust, not a JS Date");
assert.ok(typeof denialAuditBefore==="number");
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0]?.n,denialAuditBefore,
  "invalid Contact/CalendarDate must not create a Deal or append audit");
const invalidDealName=await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,name:"",precondition:{idempotencyKey:"r471-invalid-deal-name"}});
assert.equal(invalidDealName.status,400);
const invalidNameDetail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(invalidDealName.body).details));
assert.equal(invalidNameDetail.domainCode,"SALES_DEAL_NAME_INVALID");
assert.equal(invalidNameDetail.commitOutcome,1);
assert.equal(invalidNameDetail.requestId,invalidDealName.headers.get("x-request-id"));
assert.equal(invalidNameDetail.fieldViolations[0]?.fieldPath,"deal.name");
const salesCreated=await createFixtureDeal(dispatch,admin,saleCreate);
const saleReplayed=await fixtureRpc(dispatch,sales.createDeal,admin,saleCreate);
assert.equal(saleReplayed.status,200);
assert.equal(saleReplayed.body.receipt?.replayed,true);
assert.equal(salesCreated.body.receipt?.originalRequestId,salesCreated.headers.get("x-request-id"));
assert.equal(saleReplayed.body.receipt?.originalRequestId,salesCreated.headers.get("x-request-id"));
assert.equal(saleReplayed.body.receipt?.requestId,saleReplayed.headers.get("x-request-id"));
assert.notEqual(saleReplayed.body.receipt?.requestId,saleReplayed.body.receipt?.originalRequestId);
assert.equal((await fixtureRpcError(dispatch,sales.createDeal,admin,{
  ...saleCreate,name:"changed idempotency payload"})).status,409);
assert.equal((await fixtureRpcError(dispatch,sales.getDeal,tenantB,{dealId})).status,404);
assert.equal((await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId})).body.deal?.dealId,dealId);
assert.equal((await fixtureRpcError(dispatch,sales.getDeal,viewer,{dealId:"bad/invalid"})).status,400);
// Recovery reads the original receipt without executing a write.
const receiptRead=await fixtureRpc(dispatch,sales.getDeal,admin,{dealId,
  receiptIdempotencyKey:saleCreate.precondition.idempotencyKey});
assert.equal(receiptRead.status,200);
assert.equal(receiptRead.body.confirmedMutation?.originalRequestId,salesCreated.headers.get("x-request-id"));
assert.equal(receiptRead.body.confirmedMutation?.resourceVersion,1n);
assert.ok(receiptRead.body.confirmedMutation);
assert.equal(Object.hasOwn(receiptRead.body.confirmedMutation,"replayed"),false);
assert.equal(receiptRead.body.confirmedMutation?.requestId,receiptRead.headers.get("x-request-id"));
assert.equal((await fixtureRpc(dispatch,sales.getDeal,admin,{dealId,receiptIdempotencyKey:"r471-unknown-key"})).body.confirmedMutation,undefined);
assert.equal((await fixtureRpcError(dispatch,sales.getDeal,tenantB,{dealId,receiptIdempotencyKey:saleCreate.precondition.idempotencyKey})).status,404);
assert.equal((await fixtureRpcError(dispatch,sales.getDeal,null,{dealId,receiptIdempotencyKey:saleCreate.precondition.idempotencyKey})).status,401);
const receiptRequestId=receiptRead.headers.get("x-request-id");
assert.ok(receiptRequestId);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id=?",receiptRequestId))[0]?.n,0);
return {dealId,stageLead,stageQualified,saleCreate,foreignSaleContactId,salesCreated};
}

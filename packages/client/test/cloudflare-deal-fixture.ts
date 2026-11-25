import assert from "node:assert/strict";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { DealService, DealStatus } from "../gen/crm/sales/v2/deal_pb.js";
import { fixtureRpc } from "./cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "./cloudflare-rpc-fixture.ts";

type CreateDealMethod = typeof DealService.method.createDeal;

export function dealCreationInput(dealId: string, accountId: string, contactId: string) {
  return {
    dealId,
    name: "Connect Deal",
    accountId,
    stage: { pipelineId: "pipeline.enterprise", stageId: "lead", ordinal: 999 },
    amount: { minorUnits: 12500n, currencyCode: "USD" },
    probabilityBasisPoints: 2500,
    primaryContactId: contactId,
    expectedCloseDate: { year: 2027, month: 2, day: 28 },
    precondition: { idempotencyKey: "connect-sales-create-main" },
  } satisfies MessageInitShape<CreateDealMethod["input"]>;
}

export async function createFixtureDeal(
  dispatch: RpcDispatch,
  jwt: string,
  payload: ReturnType<typeof dealCreationInput>,
) {
  const { headers, body } = await fixtureRpc(dispatch, DealService.method.createDeal, jwt, payload);
  const requestId = headers.get("x-request-id");
  assert.equal(body.deal?.dealId, payload.dealId);
  assert.equal(body.deal?.name, payload.name);
  assert.equal(body.deal?.stage?.ordinal, 1, "The catalog assigns the stage rank");
  assert.equal(body.deal?.amount?.minorUnits, payload.amount.minorUnits);
  assert.equal(body.deal?.amount?.currencyCode, payload.amount.currencyCode);
  assert.equal(body.deal?.status, DealStatus.OPEN);
  assert.equal(body.deal?.primaryContactId, payload.primaryContactId);
  assert.equal(body.deal?.expectedCloseDate?.year, payload.expectedCloseDate.year);
  assert.equal(body.deal?.expectedCloseDate?.month, payload.expectedCloseDate.month);
  assert.equal(body.deal?.expectedCloseDate?.day, payload.expectedCloseDate.day);
  assert.ok(body.deal?.createdAt);
  assert.ok(body.deal?.updatedAt);
  assert.equal(body.receipt?.resourceVersion, 1n);
  assert.equal(body.receipt?.replayed, false);
  assert.equal(body.receipt?.requestId, requestId);
  return { headers, body };
}

export function dealUpdateInput(dealId: string) {
  return {
    dealId,
    name: {operation: {case: "set", value: "Updated Connect Deal"}},
    amount: {operation: {case: "set", value: {minorUnits: 98765n, currencyCode: "PLN"}}},
    probabilityBasisPoints: 6750,
    primaryContactId: {operation: {case: "clear", value: true}},
    expectedCloseDate: {operation: {case: "set", value: {year: 2028, month: 2, day: 29}}},
    precondition: {expectedVersion: 1n, idempotencyKey: "connect-sales-update-main"},
  } satisfies MessageInitShape<typeof DealService.method.updateDeal.input>;
}

export async function updateFixtureDeal(
  dispatch: RpcDispatch,
  jwt: string,
  payload: ReturnType<typeof dealUpdateInput>,
) {
  const { headers, body } = await fixtureRpc(dispatch, DealService.method.updateDeal, jwt, payload);
  assert.equal(body.deal?.name, payload.name.operation.value);
  assert.equal(body.deal?.amount?.minorUnits, payload.amount.operation.value.minorUnits);
  assert.equal(body.deal?.amount?.currencyCode, payload.amount.operation.value.currencyCode);
  assert.equal(body.deal?.probabilityBasisPoints, payload.probabilityBasisPoints);
  assert.equal(body.deal?.primaryContactId, "");
  assert.equal(body.deal?.expectedCloseDate?.year, payload.expectedCloseDate.operation.value.year);
  assert.equal(body.deal?.expectedCloseDate?.month, payload.expectedCloseDate.operation.value.month);
  assert.equal(body.deal?.expectedCloseDate?.day, payload.expectedCloseDate.operation.value.day);
  assert.ok(body.deal?.updatedAt);
  assert.equal(body.receipt?.resourceVersion, 2n);
  assert.equal(body.receipt?.replayed, false);
  return { headers, body };
}

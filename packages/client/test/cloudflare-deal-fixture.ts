import assert from "node:assert/strict";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import type { DescMethod, MessageInitShape } from "@bufbuild/protobuf";
import type { DealService } from "../gen/crm/sales/v2/deal_pb";

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
  descriptor: DescMethod,
  dispatch: (path: string, options: RequestInit) => Promise<Response>,
  jwt: string,
  payload: ReturnType<typeof dealCreationInput>,
) {
  // The native runner reads descriptors from the same Buf image as codegen.
  // Verify the dynamic descriptor before attaching its generated TS type.
  const input: CreateDealMethod["input"]["typeName"] = "crm.sales.v2.CreateDealRequest";
  const output: CreateDealMethod["output"]["typeName"] = "crm.sales.v2.CreateDealResponse";
  if (descriptor.parent.typeName !== "crm.sales.v2.DealService" ||
      descriptor.localName !== ("createDeal" satisfies keyof typeof DealService.method) ||
      descriptor.methodKind !== "unary" ||
      descriptor.input.typeName !== input || descriptor.output.typeName !== output) {
    throw new Error("Buf descriptor does not match the generated CreateDeal contract");
  }

  const method = descriptor as CreateDealMethod;
  const path = `/${method.parent.typeName}/${method.name}`;
  const response = await dispatch(path, {
    method: "POST",
    headers: {
      "content-type": "application/proto",
      "connect-protocol-version": "1",
      "cf-access-jwt-assertion": jwt,
    },
    body: new Uint8Array(toBinary(method.input, create(method.input, payload))),
  });
  const requestId = response.headers.get("x-request-id");
  assert.equal(response.status, 200, `CreateDeal request_id=${requestId}`);

  const body = fromBinary(method.output, new Uint8Array(await response.arrayBuffer()));
  assert.equal(body.deal?.dealId, payload.dealId);
  assert.equal(body.deal?.name, payload.name);
  assert.equal(body.deal?.stage?.ordinal, 1, "The catalog assigns the stage rank");
  assert.equal(body.deal?.amount?.minorUnits, payload.amount.minorUnits);
  assert.equal(body.deal?.amount?.currencyCode, payload.amount.currencyCode);
  assert.equal(body.deal?.status, 1);
  assert.equal(body.deal?.primaryContactId, payload.primaryContactId);
  assert.equal(body.deal?.expectedCloseDate?.year, payload.expectedCloseDate.year);
  assert.equal(body.deal?.expectedCloseDate?.month, payload.expectedCloseDate.month);
  assert.equal(body.deal?.expectedCloseDate?.day, payload.expectedCloseDate.day);
  assert.ok(body.deal?.createdAt);
  assert.ok(body.deal?.updatedAt);
  assert.equal(body.receipt?.resourceVersion, 1n);
  assert.equal(body.receipt?.replayed, false);
  assert.equal(body.receipt?.requestId, requestId);
  return { headers: response.headers, body };
}

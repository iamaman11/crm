import assert from "node:assert/strict";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import type { DescMessage, DescMethodUnary, MessageInitShape } from "@bufbuild/protobuf";

type FixtureRequest = {
  method: "POST";
  headers: Record<string, string>;
  body: Uint8Array<ArrayBuffer>;
};

type FixtureResponse = {
  status: number;
  headers: Pick<Headers, "get">;
  arrayBuffer(): Promise<ArrayBuffer>;
  json(): Promise<unknown>;
};

export type RpcDispatch = (path: string, options: FixtureRequest) => Promise<FixtureResponse>;

export async function sendFixtureRpc<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
  expectedStatus: number,
) {
  const path = `/${method.parent.typeName}/${method.name}`;
  const response = await dispatch(path, {
    method: "POST",
    headers: {
      "content-type": "application/proto",
      "connect-protocol-version": "1",
      ...(jwt ? { "cf-access-jwt-assertion": jwt } : {}),
    },
    body: new Uint8Array(toBinary(method.input, create(method.input, input))),
  });
  const requestId = response.headers.get("x-request-id");
  assert.equal(response.status, expectedStatus, `${method.name} request_id=${requestId ?? "missing"}`);
  return response;
}

export async function fixtureRpc<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
) {
  const response = await sendFixtureRpc(dispatch, method, jwt, input, 200);
  const body = fromBinary(method.output, new Uint8Array(await response.arrayBuffer()));
  return { headers: response.headers, body };
}

export function decodeFixtureDetail<M extends DescMessage>(
  schema: M,
  details: ReadonlyArray<Record<string, unknown>>,
) {
  const detail = details.find(item => item.type === schema.typeName);
  assert.ok(typeof detail?.value === "string", "Missing binary Connect error detail");
  return fromBinary(schema, Buffer.from(detail.value, "base64"));
}

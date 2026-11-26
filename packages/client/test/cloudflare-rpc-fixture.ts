import assert from "node:assert/strict";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import type { DescMessage, DescMethod, DescMethodUnary, MessageInitShape, MessageShape } from "@bufbuild/protobuf";

export { codeFromString as parseFixtureConnectCode } from "@connectrpc/connect/protocol-connect";

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

export function fixtureRpcPath(method: DescMethod): string {
  return `/${method.parent.typeName}/${method.name}`;
}

export function encodeFixtureRpcInput<I extends DescMessage>(
  method: DescMethodUnary<I, DescMessage>,
  input: MessageInitShape<NoInfer<I>>,
): Uint8Array<ArrayBuffer> {
  return new Uint8Array(toBinary(method.input, create(method.input, input)));
}

export async function dispatchFixtureRpc<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
  extraHeaders: Readonly<Record<string, string>> = {},
) {
  return dispatch(fixtureRpcPath(method), {
    method: "POST",
    headers: {
      ...extraHeaders,
      "content-type": "application/proto",
      "connect-protocol-version": "1",
      ...(jwt ? { "cf-access-jwt-assertion": jwt } : {}),
    },
    body: encodeFixtureRpcInput(method, input),
  });
}

export async function sendFixtureRpc<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
  expectedStatus: number,
  extraHeaders: Readonly<Record<string, string>> = {},
) {
  const response = await dispatchFixtureRpc(dispatch, method, jwt, input, extraHeaders);
  const requestId = response.headers.get("x-request-id");
  assert.equal(response.status, expectedStatus, `${method.name} request_id=${requestId ?? "missing"}`);
  return response;
}

export async function fixtureRpc<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
  extraHeaders: Readonly<Record<string, string>> = {},
) {
  const response = await sendFixtureRpc(dispatch, method, jwt, input, 200, extraHeaders);
  const body = fromBinary(method.output, new Uint8Array(await response.arrayBuffer()));
  return { status: response.status, headers: response.headers, body };
}

async function decodeFixtureError(response: FixtureResponse, method: DescMethod) {
  const body: unknown = await response.json();
  assert.ok(body && typeof body === "object" && "code" in body && typeof body.code === "string",
    `${method.name} must return a Connect error`);
  return body;
}

export async function fixtureRpcRace<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
) {
  const response = await dispatchFixtureRpc(dispatch, method, jwt, input);
  if (response.status >= 400) {
    const errorBody = await decodeFixtureError(response, method);
    return { status: response.status, headers: response.headers, body: undefined, errorBody };
  }
  const body: MessageShape<O> = fromBinary(method.output, new Uint8Array(await response.arrayBuffer()));
  return { status: response.status, headers: response.headers, body, errorBody: undefined };
}

export async function fixtureRpcError<I extends DescMessage, O extends DescMessage>(
  dispatch: RpcDispatch,
  method: DescMethodUnary<I, O>,
  jwt: string | null,
  input: MessageInitShape<NoInfer<I>>,
) {
  const response = await dispatchFixtureRpc(dispatch, method, jwt, input, {});
  assert.ok(response.status >= 400, `${method.name} must reject the request`);
  const body = await decodeFixtureError(response, method);
  return {status: response.status, headers: response.headers, body};
}

export function decodeFixtureDetail<M extends DescMessage>(
  schema: M,
  details: ReadonlyArray<Record<string, unknown>>,
) {
  const detail = details.find(item => item.type === schema.typeName);
  assert.ok(typeof detail?.value === "string", "Missing binary Connect error detail");
  return fromBinary(schema, Buffer.from(detail.value, "base64"));
}

export async function collectFixturePages<T>(
  read: (pageToken: string) => Promise<{ items: readonly T[]; nextPageToken: string | undefined }>,
  pageSize = 1,
): Promise<T[]> {
  const items: T[] = [];
  const seen = new Set<string>();
  let token = "";
  for (let index = 0; index < 256; index++) {
    const page = await read(token);
    assert.ok(page.items.length <= pageSize, `page_size=${pageSize} must bound the result`);
    items.push(...page.items);
    const next = page.nextPageToken;
    if (!next) return items;
    assert.equal(page.items.length, pageSize, "cursor skipped a matching row");
    assert.ok(!seen.has(next), "cursor cycle in filtered pagination");
    seen.add(next);
    token = next;
  }
  throw new Error("Filtered pagination never reached its last page");
}

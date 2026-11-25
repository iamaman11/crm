import assert from "node:assert/strict";
import { test } from "node:test";
import { formatRpcFailure, readRpcError } from "./rpc-error-response.mts";

test("non-Connect RPC failures report request metadata without response data", async t => {
  const log = t.mock.method(console, "error", () => {});
  const marker = "private-crm-fixture-marker";
  const path = "/crm.sales.v2.DealService/CreateDeal";
  const response = new Response("unexpected response: " + marker, {
    status: 502,
    headers: { "content-type": "text/plain", "x-request-id": "request-123" },
  });

  await assert.rejects(readRpcError(response, path, "POST"), error => {
    assert.match(error.message, /request-123/);
    assert.doesNotMatch(error.stack, new RegExp(marker));
    assert.equal(error.cause, undefined);
    return true;
  });
  assert.equal(log.mock.callCount(), 1);
  const [entry] = log.mock.calls[0].arguments;
  assert.doesNotMatch(entry, new RegExp(marker));
  assert.deepEqual(JSON.parse(entry), {
    event: "crm.native-ci.non-connect-response",
    path,
    method: "POST",
    status: 502,
    content_type: "text/plain",
    request_id: "request-123",
  });
});

test("RPC assertions expose only status and a valid correlation ID", () => {
  const marker = "private-crm-fixture-marker";
  const requestId = "561679ff-b6e2-4c5b-bdf4-428278f2e547";
  const response = {
    status: 403,
    headers: new Headers({"x-request-id": requestId}),
    body: {message: marker},
  };
  assert.equal(formatRpcFailure(response), `status=403, request_id=${requestId}`);
  response.headers.set("x-request-id", marker);
  assert.equal(formatRpcFailure(response), "status=403, request_id=missing");
});

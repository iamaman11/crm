export async function readRpcError(
  response: {status: number; headers: Pick<Headers, "get">; json(): Promise<unknown>},
  path: string,
  method: string,
): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    const evidence = JSON.stringify({
      event: "crm.native-ci.non-connect-response",
      path,
      method,
      status: response.status,
      content_type: response.headers.get("content-type"),
      request_id: response.headers.get("x-request-id"),
    });
    console.error(evidence);
    throw new Error("Non-Connect RPC response: " + evidence);
  }
}

export function formatRpcFailure(
  response: { status: number; headers?: Pick<Headers, "get"> },
): string {
  const requestId = response.headers?.get?.("x-request-id");
  return `status=${response.status}, request_id=${
    requestId && /^[a-f0-9-]{36}$/i.test(requestId) ? requestId : "missing"
  }`;
}

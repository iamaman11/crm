export async function readRpcError(response: Response, path: string, method: string): Promise<unknown> {
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

import assert from "node:assert/strict";
import type { APIResponse, Response } from "playwright";

export type RpcEvidence = {
  path: string;
  status: number;
  request_id?: string | undefined;
  error_code?: string | null;
  source_sha?: string | undefined;
  runtime?: string | undefined;
  header_error?: "COMPLETE_CORRELATION_HEADERS_UNAVAILABLE";
};

export function captureBrowserRpcEvidence() {
  const requests: RpcEvidence[] = [];
  const reads = new Set<Promise<void>>();
  const recordResponse = (response: APIResponse | Response) => {
    const path = new URL(response.url()).pathname;
    if (!path.startsWith("/crm.")) return;
    const entry: RpcEvidence = {path, status: response.status()};
    requests.push(entry);
    const capture = (headers: Record<string, string>) => {
      entry.request_id = headers["x-request-id"];
      entry.error_code = headers["x-crm-error-code"] ?? null;
      entry.source_sha = headers["x-crm-source-sha"];
      entry.runtime = headers["x-crm-runtime"];
    };

    // Ordinary Worker headers are available at the response event. Keep that
    // snapshot if the browser closes while Playwright reads the complete set.
    capture(response.headers());
    if ("allHeaders" in response) {
      const read = response.allHeaders().then(capture).catch(() => {
        if (!entry.source_sha || !entry.runtime || !entry.request_id ||
            (entry.status >= 400 && !entry.error_code)) {
          entry.header_error = "COMPLETE_CORRELATION_HEADERS_UNAVAILABLE";
        }
      });
      reads.add(read);
      void read.then(() => reads.delete(read));
    }
  };
  const flushHeaderReads = async () => {
    while (reads.size) await Promise.all(reads);
  };
  const assertSource = (sourceSha: string) => {
    for (const entry of requests) {
      assert.equal(entry.header_error, undefined, "Complete browser correlation evidence is required");
      assert.ok(entry.source_sha === sourceSha, "Unexpected browser response source for " + entry.path);
      assert.equal(entry.runtime, "native-workers-rs");
      assert.ok(/^[a-f0-9-]{36}$/i.test(entry.request_id ?? ""), "Missing browser request ID for " + entry.path);
    }
  };
  return { requests, recordResponse, flushHeaderReads, assertSource };
}

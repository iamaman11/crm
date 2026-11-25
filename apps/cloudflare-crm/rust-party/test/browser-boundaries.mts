import assert from "node:assert/strict";
import type { APIResponse, Response } from "playwright";
import { string } from "../../scripts/json-boundary.mts";

export type BrowserSql = (query: string, ...params: (string | number)[]) => Promise<ReadonlyArray<Record<string, unknown>>>;

// The query is selected by the test, while D1 results are an external boundary.
export function browserFirstId(sql: BrowserSql, tenant: string) {
  return async (table: string, column: string, value: string): Promise<string> => {
    const rows = await sql("SELECT id FROM " + table + " WHERE tenant=? AND json_extract(state,?)=?",
      tenant, "$." + column, value);
    assert.equal(rows.length, 1);
    return string(rows[0]?.id);
  };
}

// Query strings may contain privacy probes; only compare the RPC pathname.
export function matchesRpcPath(response: APIResponse | Response, path: string): boolean {
  return new URL(response.url()).pathname===path;
}

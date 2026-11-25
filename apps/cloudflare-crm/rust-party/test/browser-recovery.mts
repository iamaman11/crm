import assert from "node:assert/strict";
import {expect} from "playwright/test";
import type {APIResponse, Page, Response, Route} from "playwright";
import {fixtureRpcPath} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {record, string} from "../../scripts/json-boundary.mts";
import {matchesRpcPath} from "./browser-boundaries.mts";
import type {BrowserSql} from "./browser-boundaries.mts";

type RecoveryEvidence = {owner: string; kind: "lookup"; original_request_id: string; subsequent_request_id: string};
type RpcMethod = Parameters<typeof fixtureRpcPath>[0];
type RecoveryOptions = {
  owner: string; method: RpcMethod; readMethod: RpcMethod; storageKey: string;
  auditAction: string; heading: string; trigger: () => Promise<void>;
};
type Dependencies = {
  page: Page; sql: BrowserSql; tenantA: string;
  recordResponse: (response: APIResponse | Response) => void;
  assertMutation: (response: APIResponse, method: string) => Promise<void>;
  reloadDocument: () => Promise<unknown>; checkpoint: (label: string) => void;
  ownerRecovery: RecoveryEvidence[];
};

export function browserRecovery({page,sql,tenantA,recordResponse,assertMutation,reloadDocument,
  checkpoint,ownerRecovery}: Dependencies) {
  // The original commit must be audited exactly once; lookup must not write.
  const assertAuditRecovery=async(originalId: string,readId: string,action: string,owner: string) => {
    for(const id of [originalId,readId])
      assert.match(id,/^[a-f0-9-]{36}$/i,owner+" must have a signed request ID");
    assert.notEqual(originalId,readId,owner+" recovery must be a distinct request");
    assert.match(action,/^[a-z_]+\.[a-z_]+$/,owner+" must use a known audit action");
    const rows=await sql("SELECT request_id,action FROM crm_audit WHERE tenant_id=? AND request_id IN (?,?)",
      tenantA,originalId,readId);
    assert.equal(rows.filter(row=>row.request_id===originalId&&row.action===action).length,
      1,owner+" original commit must have one matching audit action");
    assert.equal(rows.filter(row=>row.request_id===readId).length,
      0,owner+" recovery request must not append audit");
  };
  const reloadAfterPending=async()=>{
    const accept=(dialog: import("playwright").Dialog)=>void dialog.accept();
    page.on("dialog",accept);
    try{await reloadDocument();}finally{page.off("dialog",accept);}
  };
  const recoverOwnerWrite=async({owner,method,readMethod,storageKey,
    auditAction,heading,trigger}: RecoveryOptions)=>{
    const writeRoute="**"+fixtureRpcPath(method);
    let calls=0,originalRequestId: string | undefined;
    const loseAcknowledgement=async (route: Route)=>{
      calls++;
      const committed=await route.fetch();
      recordResponse(committed);await assertMutation(committed,method.name);
      originalRequestId=(await committed.headers())["x-request-id"];
      await route.abort("failed");
    };
    await page.route(writeRoute,loseAcknowledgement);
    await trigger();
    await page.getByRole("heading",{name:/Результат записи не подтверждён/}).last().waitFor();
    const raw: unknown=await page.evaluate(key=>JSON.parse(sessionStorage.getItem(key) ?? "null"),storageKey);
    const marker=record(raw);
    assert.deepEqual(Object.keys(marker).sort(),["expectedVersion","key","resourceId"]);
    assert.equal(marker.expectedVersion,"0");
    assert.match(string(marker.key),/^[a-f0-9-]{36}$/i);
    await reloadAfterPending();
    await page.getByRole("heading",{name:heading,exact:true}).waitFor();
    const check=page.getByRole("button",{name:"Проверить результат записи",exact:true}).last();
    await check.waitFor();
    const pending=page.waitForResponse(response=>
      matchesRpcPath(response,fixtureRpcPath(readMethod))&&response.request().method()==="POST");
    await check.click();
    const readResponse=await pending;
    assert.equal(readResponse.status(),200,owner+" receipt lookup");
    const readRequestId=(await readResponse.headers())["x-request-id"];
    assert.match(readRequestId??"",/^[a-f0-9-]{36}$/i);
    assert.ok(originalRequestId,owner+" original request id");
    assert.ok(readRequestId,owner+" lookup request id");
    assert.equal(calls,1,owner+" recovery must not re-execute the original write");
    await expect.poll(()=>page.evaluate(key=>sessionStorage.getItem(key),storageKey)).toBe(null);
    await assertAuditRecovery(originalRequestId,readRequestId,auditAction,owner);
    ownerRecovery.push({owner,kind:"lookup",original_request_id:originalRequestId,
      subsequent_request_id:readRequestId});
    await page.unroute(writeRoute,loseAcknowledgement);
    checkpoint("S5 "+owner+" lost acknowledgement: opaque marker, immutable receipt lookup, one D1 audit");
    return string(marker.resourceId);
  };
  return {assertAuditRecovery,reloadAfterPending,recoverOwnerWrite};
}

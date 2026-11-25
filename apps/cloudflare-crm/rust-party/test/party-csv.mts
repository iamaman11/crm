import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { PartyCsvService } from "../../../../packages/client/gen/crm/customer_data_operations/v2/party_csv_pb.js";
import { PartyKind, PartyService } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail, fixtureRpc, fixtureRpcError, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type CsvFixture = {
 dispatch: RpcDispatch;
 rpcFetch: (path: string, options: {headers: Record<string, string>}) => Promise<{status: number}>;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 partySnapshot: () => Promise<unknown>;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
 unknownService: string;
};

export async function verifyPartyCsv({dispatch,rpcFetch,sql,partySnapshot,admin,viewer,tenantB,knownService,unknownService}: CsvFixture) {
 const methods=PartyCsvService.method;
 const parties=PartyService.method;
 const count=async (query: string) => {
   const [row]=await sql(query);
   assert.ok(typeof row?.n === "number", "Expected D1 count");
   return row.n;
 };
 const assertUnconfirmed = async (response: Awaited<ReturnType<RpcDispatch>>, code: string) => {
   const body = record(await response.json());
   assert.equal(body.code, "unavailable");
   const detail = decodeFixtureDetail(DomainErrorDetailSchema, records(body.details));
   assert.equal(detail.commitOutcome, MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode, code);
   assert.equal(detail.requestId, response.headers.get("x-request-id"));
 };
 // CSV operations use the original parser/Party owner, explicit admin permissions,
 // atomic D1 writes and a per-operation receipt in that same database.
 const csvBytes=new TextEncoder().encode(
   'party_id,kind,display_name,resource_version\r\nexternal-one,person,CSV One,1\r\nexternal-two,organization,CSV Two,1\r\n');
 const importAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const importPreview=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:csvBytes});
 assert.equal(importPreview.status,200,formatRpcFailure(importPreview));
 assert.equal(importPreview.body.count,2);
 assert.equal(importPreview.body.previouslyImported,0);
 assert.equal(importPreview.body.csvSha256Hex,createHash("sha256").update(csvBytes).digest("hex"));
 assert.equal(new Set(importPreview.body.targetPartyIds).size,2);
 assert.ok(importPreview.body.targetPartyIds.every(id=>!id.startsWith("external-")));
 const otherPreview=await fixtureRpc(dispatch, methods.previewPartyImport, tenantB, {csv:csvBytes});
 assert.equal(otherPreview.status,200,formatRpcFailure(otherPreview));
 assert.ok(otherPreview.body.targetPartyIds.every(id=>!importPreview.body.targetPartyIds.includes(id)));
 const applyInput={csv:csvBytes,expectedSha256Hex:importPreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-import-operation-main"}};
 assert.equal((await fixtureRpcError(dispatch, methods.applyPartyImport, admin, {...applyInput,expectedSha256Hex:"0".repeat(64)})).body.code,"aborted");
 assert.equal((await fixtureRpcError(dispatch, methods.applyPartyImport, admin, {...applyInput,
   precondition:{idempotencyKey:"csv-import-invalid-version",expectedVersion:1n}})).body.code,"invalid_argument");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),importAuditBefore,"previews and invalid input are read-only");
 const applied=await fixtureRpc(dispatch, methods.applyPartyImport, admin, applyInput);
 assert.equal(applied.status,200,formatRpcFailure(applied));
 assert.equal(applied.body.createdCount,2);assert.equal(applied.body.replayedCount,0);
 assert.equal(applied.body.receipt?.replayed,false);
 assert.equal(applied.body.receipt?.requestId,applied.headers.get("x-request-id"));
 assert.equal(applied.body.receipt?.resourceVersion,1n);
 const importReplay=await fixtureRpc(dispatch, methods.applyPartyImport, knownService, applyInput);
 assert.equal(importReplay.body.createdCount,0);assert.equal(importReplay.body.replayedCount,2);
 assert.equal(importReplay.body.receipt?.replayed,true);

 const csvReceiptAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const csvReceiptInput={csv:new Uint8Array(),receiptIdempotencyKey:applyInput.precondition.idempotencyKey};
 const csvReceiptRead=await fixtureRpc(dispatch, methods.previewPartyImport, admin, csvReceiptInput);
 assert.equal(csvReceiptRead.status,200,formatRpcFailure(csvReceiptRead));
 assert.equal(csvReceiptRead.body.csvSha256Hex,applyInput.expectedSha256Hex);
 assert.equal(csvReceiptRead.body.count,2);
 assert.equal(csvReceiptRead.body.previouslyImported,2);
 assert.deepEqual(csvReceiptRead.body.targetPartyIds,[]);
 assert.equal(csvReceiptRead.body.confirmedMutation?.originalRequestId,applied.headers.get("x-request-id"));
 assert.equal(csvReceiptRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal(csvReceiptRead.body.confirmedMutation?.requestId,csvReceiptRead.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyImport, tenantB, csvReceiptInput)).body.code,"not_found");
 assert.equal((await sendFixtureRpc(dispatch, methods.previewPartyImport, viewer, csvReceiptInput, 403)).status,403,
   "CSV receipt lookup must preserve the existing admin-only permission");
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyImport, admin, {csv:new Uint8Array()})).body.code,"invalid_argument");
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyImport, admin, {
   csv:new Uint8Array(),receiptIdempotencyKey:"short"})).body.code,"invalid_argument");
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyImport, admin, {
   csv:new Uint8Array(),receiptIdempotencyKey:"csv-import-missing-key"})).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyImport, admin, {
   csv:csvBytes,receiptIdempotencyKey:applyInput.precondition.idempotencyKey})).body.code,"invalid_argument");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),csvReceiptAuditBefore,
   "CSV receipt lookup must not append audit");
 await sql("ALTER TABLE crm_party_import_receipts RENAME TO fixture_unavailable_party_import_receipts");
 try {
   assert.equal((await sendFixtureRpc(dispatch, methods.previewPartyImport, admin, csvReceiptInput, 503)).status,503,
     "an unavailable batch receipt must remain unconfirmed");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_party_import_receipts RENAME TO crm_party_import_receipts");
 }
 assert.equal((await fixtureRpc(dispatch, methods.applyPartyImport, admin, {...applyInput,
   precondition:{idempotencyKey:"csv-import-another-operation"}})).body.replayedCount,2);
 assert.equal((await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:csvBytes})).body.previouslyImported,2);
 for(const target of importPreview.body.targetPartyIds){
   assert.equal((await fixtureRpc(dispatch, parties.getParty, viewer, {partyId:target})).status,200);
   assert.equal((await fixtureRpcError(dispatch, parties.getParty, tenantB, {partyId:target})).body.code,"not_found");
   assert.deepEqual((await sql("SELECT action,tenant_id,actor,request_id FROM crm_audit WHERE subject_id='"+target+"'")),[
     {action:"parties.create",tenant_id:"tenant-a",actor:"native-a@example.test",request_id:applied.headers.get("x-request-id")},
   ],"Import replay by another actor must retain the original attribution");
 }
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),importAuditBefore+2,"replays never duplicate Party audit");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE tenant='tenant-a'"),2);
 const changedBytes=new TextEncoder().encode(new TextDecoder().decode(csvBytes).replace("CSV One","CSV Changed"));
 assert.equal((await fixtureRpcError(dispatch, methods.applyPartyImport, admin, {...applyInput,csv:changedBytes,
   expectedSha256Hex:createHash("sha256").update(changedBytes).digest("hex")})).body.code,"aborted");
 // Structural file failures remain fail-closed Connect errors.
 for(const invalid of [
   new Uint8Array(),new Uint8Array(16385),new TextEncoder().encode("invalid,csv"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\n"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\n"+
     Array.from({length:21},(_,i)=>"source-"+i+",person,Name "+i+",1\r\n").join(""))
 ]){
   assert.equal((await fixtureRpcError(dispatch, methods.previewPartyImport, admin, {csv:invalid})).body.code,
     "invalid_argument","Malformed header/size/count must never return a commit-ready preview");
 }
 // Row semantic errors become typed, NON-COMMITTABLE previews, not generic 400s.
 const semanticCases=[
   ["x,invalid,Name,1",[[1,"kind","IMPORT_KIND_INVALID"]]],
   ["x,person,Name,0",[[1,"resource_version","IMPORT_SOURCE_VERSION_INVALID"]]],
   ["x,person,One,1\r\nx,person,Two,1",[[2,"party_id","IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID"]]]
 ] as const;
 for(const [rows,expectedIssues] of semanticCases){
   const invalid=new TextEncoder().encode(
     "party_id,kind,display_name,resource_version\r\n"+rows+"\r\n");
   const response=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:invalid});
   assert.equal(response.status,200,formatRpcFailure(response));
   assert.deepEqual(response.body.targetPartyIds,[]);
   assert.deepEqual(response.body.rowIssues.map(x=>[x.rowPosition,x.field,x.code]),
     expectedIssues);
   assert.equal((await sendFixtureRpc(dispatch, methods.applyPartyImport, admin, {
     csv:invalid,expectedSha256Hex:response.body.csvSha256Hex,
     precondition:{idempotencyKey:"csv-reject-semantic-"+createHash("sha256").update(invalid).digest("hex").slice(0,16)}
   }, 400)).status,400,"A typed invalid preview must still be rejected by the owner");
 }
 // The same original CSV parser reports ALL row/field diagnostics, not the
 // first unrelated error code. No target or audit can be created by preview.
 const invalidRowsCsv=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\n"+
   "duplicate-one,invalid,First,0\r\n"+
   "duplicate-one,person,Second,1\r\n"+
   "empty-name,person,,1\r\n"+
   "valid-last,organization,Valid,1\r\n");
 const invalidAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const invalidPartyBefore=await count("SELECT COUNT(*) AS n FROM crm_party_records");
 const invalidReceiptBefore=await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts");
 const invalidPreview=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:invalidRowsCsv});
 assert.equal(invalidPreview.status,200,formatRpcFailure(invalidPreview));
 assert.equal(invalidPreview.body.count,4);
 assert.equal(invalidPreview.body.csvSha256Hex,createHash("sha256").update(invalidRowsCsv).digest("hex"));
 assert.deepEqual(invalidPreview.body.targetPartyIds,[],"No ID from invalid preview is actionable");
 assert.deepEqual(invalidPreview.body.rowIssues.map(issue=>[
   issue.rowPosition,issue.field,issue.code
 ]),[
   [1,"resource_version","IMPORT_SOURCE_VERSION_INVALID"],
   [1,"kind","IMPORT_KIND_INVALID"],
   [2,"party_id","IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID"],
   [3,"display_name","IMPORT_PARTY_INVALID"]
 ]);
 const invalidApply=await sendFixtureRpc(dispatch, methods.applyPartyImport, admin, {
   csv:invalidRowsCsv,expectedSha256Hex:invalidPreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-import-invalid-rows-denied"}
 }, 400);
 assert.equal(invalidApply.status,400,"Apply must revalidate even an advertised digest");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),invalidAuditBefore);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records"),invalidPartyBefore);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts"),invalidReceiptBefore);
 console.log(JSON.stringify({event:"crm.native-n01-csv-row-issues",result:"PASS",
   per_row:true,multiple_errors:true,field_codes:true,sha256:true,apply_denied:true,
   no_partial_write:true}));

 // A conflict on the second target must leave the first target absent.
 const conflictBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nconflict-one,person,First,1\r\nconflict-two,person,Second,1\r\n");
 const conflictPreview=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:conflictBytes});
 assert.equal(conflictPreview.status,200,formatRpcFailure(conflictPreview));
 const [firstTarget,secondTarget]=conflictPreview.body.targetPartyIds;
 assert.ok(firstTarget && secondTarget);
 assert.equal((await fixtureRpc(dispatch, parties.createParty, admin, {partyId:secondTarget,kind:1,displayName:"Existing target",
   precondition:{idempotencyKey:"csv-target-conflict-create"}})).status,200);
 const conflictRepreview=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:conflictBytes});
 assert.equal(conflictRepreview.status,200);
 assert.deepEqual(conflictRepreview.body.targetPartyIds,[]);
 assert.deepEqual(conflictRepreview.body.rowIssues.map(issue=>[
   issue.rowPosition,issue.field,issue.code
 ]),[[2,"party_id","IMPORT_TARGET_PARTY_CONFLICT"]]);
 const failedImport=await fixtureRpcError(dispatch, methods.applyPartyImport, admin, {csv:conflictBytes,
   expectedSha256Hex:conflictPreview.body.csvSha256Hex,precondition:{idempotencyKey:"csv-conflict-operation"}});
 assert.equal(failedImport.body.code,"aborted");
 assert.equal((await fixtureRpcError(dispatch, parties.getParty, viewer, {partyId:firstTarget})).body.code,"not_found");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE key='csv-conflict-operation'"),0);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),importAuditBefore+3);
 // Unlike the preflight target conflict above, this fails inside the second SQL
 // audit INSERT, after the first row has executed in the same D1 batch.
 const rollbackBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nrollback-one,person,First,1\r\nrollback-two,person,Second,1\r\n");
 const rollbackPreview=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:rollbackBytes});
 assert.equal(rollbackPreview.status,200);
 const rollbackInput={csv:rollbackBytes,expectedSha256Hex:rollbackPreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-audit-rollback"}};
 const beforeRollback=(await partySnapshot());
 for(const token of [null,viewer,unknownService])assert.ok((await fixtureRpcError(dispatch, methods.applyPartyImport, token, rollbackInput)).status>=400);
 (await sql("CREATE TRIGGER fixture_fail_import_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+rollbackPreview.body.targetPartyIds[1]+"' "+
   "BEGIN SELECT RAISE(ABORT,'fixture late import audit failure'); END;"));
 try{
   await assertUnconfirmed(await sendFixtureRpc(dispatch, methods.applyPartyImport, knownService, rollbackInput, 503),"IMPORT_WRITE_UNCONFIRMED");
   assert.deepEqual((await partySnapshot()),beforeRollback,"Late batch failure must remove every state, receipt and audit");
 }finally{await sql("DROP TRIGGER fixture_fail_import_audit");}
 const retryImport=await fixtureRpc(dispatch, methods.applyPartyImport, knownService, rollbackInput);
 assert.equal(retryImport.status,200,formatRpcFailure(retryImport));
 assert.equal(retryImport.body.createdCount,2,"Failed batch must not reserve its idempotency key");
 for(const target of rollbackPreview.body.targetPartyIds){
   assert.deepEqual((await sql("SELECT actor,request_id FROM crm_audit WHERE subject_id='"+target+"'")),[
     {actor:"service:ci-fixture.access",request_id:retryImport.headers.get("x-request-id")},
   ]);
 }
 // Product-first simple source: the ORIGINAL CSV ReaderBuilder, optional
 // source identity and position-derived ImportRow, not a second parser/owner.
 const simpleCsv=new TextEncoder().encode(
   'kind,display_name\r\nperson,"Simple, CSV Person"\r\norganization,"Simple ""Quoted"" Org"\r\n');
 const simplePreview=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:simpleCsv});
 assert.equal(simplePreview.status,200,formatRpcFailure(simplePreview));
 assert.equal(simplePreview.body.count,2);
 assert.equal(simplePreview.body.previouslyImported,0);
 assert.equal(simplePreview.body.rowIssues.length,0);
 assert.equal(new Set(simplePreview.body.targetPartyIds).size,2);
 assert.ok(simplePreview.body.targetPartyIds.every(id=>!id.includes("external-one")));
 const simpleTenantB=await fixtureRpc(dispatch, methods.previewPartyImport, tenantB, {csv:simpleCsv});
 assert.equal(simpleTenantB.status,200);
 assert.ok(simpleTenantB.body.targetPartyIds.every(id=>!simplePreview.body.targetPartyIds.includes(id)));
 assert.equal((await sendFixtureRpc(dispatch, methods.previewPartyImport, viewer, {csv:simpleCsv}, 403)).status,403);
 const brokenSimple=new TextEncoder().encode(
   "kind,display_name\r\nperson,Valid Name\r\norganization,\r\n");
 const invalidSimple=await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:brokenSimple});
 assert.equal(invalidSimple.status,200,formatRpcFailure(invalidSimple));
 assert.deepEqual(invalidSimple.body.targetPartyIds,[]);
 assert.deepEqual(invalidSimple.body.rowIssues.map(x=>[x.rowPosition,x.field,x.code]),[
   [2,"display_name","IMPORT_PARTY_INVALID"]]);
 const simpleBeforeAudit=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const simpleBeforeParties=await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'");
 assert.equal((await sendFixtureRpc(dispatch, methods.applyPartyImport, admin, {
   csv:brokenSimple,expectedSha256Hex:invalidSimple.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-simple-invalid-denied"}
 }, 400)).status,400);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),simpleBeforeAudit);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"),simpleBeforeParties);
 const simpleApply={csv:simpleCsv,expectedSha256Hex:simplePreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-simple-import-original-owner"}};
 const simpleSaved=await fixtureRpc(dispatch, methods.applyPartyImport, admin, simpleApply);
 assert.equal(simpleSaved.status,200,formatRpcFailure(simpleSaved));
 assert.equal(simpleSaved.body.createdCount,2);
 assert.equal((await fixtureRpc(dispatch, methods.applyPartyImport, knownService, simpleApply)).body.replayedCount,2);
 assert.equal((await fixtureRpc(dispatch, methods.previewPartyImport, admin, {csv:simpleCsv})).body.previouslyImported,2);
 for(const target of simplePreview.body.targetPartyIds){
   const found=await fixtureRpc(dispatch, parties.getParty, viewer, {partyId:target});
   assert.equal(found.status,200);
   assert.ok(["Simple, CSV Person",'Simple "Quoted" Org'].includes(found.body.party?.displayName ?? ""));
   assert.equal((await sendFixtureRpc(dispatch, parties.getParty, tenantB, {partyId:target}, 404)).status,404);
 }
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"),simpleBeforeParties+2);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),simpleBeforeAudit+2);
 console.log(JSON.stringify({event:"crm.native-simple-csv-original-owner",result:"PASS",
   native_csv:true,headers_two_or_four:true,tenant:true,original_position_id:true,
   row_errors:true,atomic:true,replay:true,quoted_values:true,audit:true}));
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyExport, admin, {kind:0})).body.code,"invalid_argument");
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyExport, admin, {kind:999 as PartyKind})).body.code,"invalid_argument");
 const exportBefore=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const exported=await fixtureRpc(dispatch, methods.previewPartyExport, admin, {});
 assert.equal(exported.status,200,formatRpcFailure(exported));
 assert.equal(exported.body.maximumTenantParties,100);
 assert.deepEqual(exported.body.fields,["party_id","kind","display_name","resource_version"]);
 const downloaded=exported.body.csv;
 assert.ok(downloaded instanceof Uint8Array);
 // Original party-export-csv/v1 uses LF records; do not alter canonical bytes.
 assert.match(new TextDecoder().decode(downloaded),/^party_id,kind,display_name,resource_version\n/);
 assert.equal(createHash("sha256").update(downloaded).digest("hex"),exported.body.csvSha256Hex);
 assert.equal((await sendFixtureRpc(dispatch, methods.previewPartyExport, null, {}, 401)).status,401);
 assert.equal((await sendFixtureRpc(dispatch, methods.previewPartyExport, viewer, {}, 403)).status,403);
 // Retired business HTTP route must not reappear alongside the typed RPC.
 assert.equal((await rpcFetch("/api/customer-data-operations/parties.csv",{
   headers:{"cf-access-jwt-assertion":admin}})).status,404);
 const filteredExport=await fixtureRpc(dispatch, methods.previewPartyExport, admin, {kind:1});
 assert.equal(filteredExport.status,200,formatRpcFailure(filteredExport));
 assert.ok(filteredExport.body.count<exported.body.count);
 assert.equal(createHash("sha256").update(filteredExport.body.csv).digest("hex"),filteredExport.body.csvSha256Hex);
 assert.equal((await fixtureRpc(dispatch, methods.previewPartyExport, tenantB, {})).body.count,0);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),exportBefore);
 // Explicit boundary fixture in tenant B: the 101st row must prevent partial export,
 // even when a filter would reduce the returned data below the limit.
 const boundaryRows=Array.from({length:101},(_,i)=>{
   const state=JSON.stringify({party_id:"csv-boundary-"+i,kind:"person",display_name:"Boundary "+i,
     created_at_unix_nanos:10,updated_at_unix_nanos:10,version:1});
   return "('tenant-b','csv-boundary-"+i+"','"+state+"',1,'csv-boundary-key-"+i+"','fixture','fixture','native-b@example.test','Boundary "+i+"')";
 }).join(",");
 await sql("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) VALUES "+boundaryRows);
 assert.equal((await fixtureRpcError(dispatch, methods.previewPartyExport, tenantB, {kind:2})).body.code,"failed_precondition");
 console.log(JSON.stringify({event:"crm.native-party-csv-v2",result:"PASS",admin_read:true,
   original_owner:"crm.customer-data-operations",party_owner:"crm.parties",
   tenant_ab:true,atomic:true,replay:true,audit:true,verified_actor:true,sql_failure_rollback:true,csv_bytes_sha256:true,bounded:true}));
}

export async function importNfcPartyFixture(dispatch: RpcDispatch, admin: string) {
 const csv = new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nnfc-import,person,Imported Cafe\u0301,1\r\n",
 );
 const {body: preview} = await fixtureRpc(dispatch, PartyCsvService.method.previewPartyImport, admin, {csv});
 await fixtureRpc(dispatch, PartyCsvService.method.applyPartyImport, admin, {
   csv, expectedSha256Hex: preview.csvSha256Hex, precondition: {idempotencyKey: "nfc-import-operation"},
 });
 const [partyId] = preview.targetPartyIds;
 assert.ok(partyId);
 return partyId;
}

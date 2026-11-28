import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { PartyCsvService } from "../../../../packages/client/gen/crm/customer_data_operations/v2/party_csv_pb.js";
import { PartyKind, PartyService } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail, fixtureRpc, fixtureRpcError, fixtureRpcRace, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
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

// Source identity regression coverage: stable reimports, mixed conflicts, tenant isolation,
// stale previews, concurrent Apply, receipt recovery and atomic workerd/D1 rollback.
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

 // Optional named sources bind a stable external-ID digest to the original Party owner.
 const registerSourceInput={displayName:"Pilot CRM",precondition:{idempotencyKey:"csv-source-register"}};
 const registeredSource=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,registerSourceInput);
 assert.equal(registeredSource.status,200,formatRpcFailure(registeredSource));
 assert.equal(registeredSource.body.receipt?.replayed,false);
 const sourceId=registeredSource.body.source?.sourceId;
 assert.ok(typeof sourceId==="string"&&sourceId.startsWith("csv-source-"));
 const listedSources=await fixtureRpc(dispatch,methods.listPartyImportSources,admin,{});
 assert.ok(listedSources.body.sources.some(source=>source.sourceId===sourceId&&source.displayName==="Pilot CRM"));
 assert.equal((await fixtureRpc(dispatch,methods.listPartyImportSources,tenantB,{})).body.sources.length,0);
 assert.equal((await sendFixtureRpc(dispatch,methods.listPartyImportSources,viewer,{},403)).status,403);
 assert.equal((await sendFixtureRpc(dispatch,methods.registerPartyImportSource,viewer,{displayName:"Forbidden"},403)).status,403);
 assert.equal((await sendFixtureRpc(dispatch,methods.listPartyImportSources,null,{},401)).status,401);
 assert.equal((await sendFixtureRpc(dispatch,methods.registerPartyImportSource,null,{displayName:"Unauthenticated"},401)).status,401);
 const sameSourceInput={displayName:"  pilot crm  ",precondition:{idempotencyKey:"csv-source-register-alias"}};
 const sameSource=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,sameSourceInput);
 assert.equal(sameSource.body.source?.sourceId,sourceId,"Source registration is stable and case-insensitive");
 assert.equal(sameSource.body.receipt?.replayed,false);
 const sameSourceReplay=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,sameSourceInput);
 assert.equal(sameSourceReplay.body.source?.sourceId,sourceId);
 assert.equal(sameSourceReplay.body.receipt?.replayed,true);

 const sourceCsv1=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nsource-ext-1,person,Source Person,1\r\nsource-ext-2,organization,Source Org,1\r\n");
 const sourcePreview1=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{csv:sourceCsv1,sourceId});
 assert.equal(sourcePreview1.status,200,formatRpcFailure(sourcePreview1));
 assert.equal(sourcePreview1.body.newCount,2);
 assert.equal(sourcePreview1.body.matchedCount,0);
 assert.equal(sourcePreview1.body.rowIssues.length,0);
 assert.ok(sourcePreview1.body.previewFingerprint.length===64);
 const sourceIds1=[...sourcePreview1.body.targetPartyIds];
 const sourceApply1=await fixtureRpc(dispatch,methods.applyPartyImport,admin,{
   csv:sourceCsv1,sourceId,expectedSha256Hex:sourcePreview1.body.csvSha256Hex,
   expectedPreviewFingerprint:sourcePreview1.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-first-import"}
 });
 assert.equal(sourceApply1.status,200,formatRpcFailure(sourceApply1));
 assert.equal(sourceApply1.body.createdCount,2);
 assert.equal(sourceApply1.body.replayedCount,0);
 assert.equal(sourceApply1.body.receipt?.replayed,false);
 const sourceReplay=await fixtureRpc(dispatch,methods.applyPartyImport,knownService,{
   csv:sourceCsv1,sourceId,expectedSha256Hex:sourcePreview1.body.csvSha256Hex,
   expectedPreviewFingerprint:sourcePreview1.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-first-import"}
 });
 assert.equal(sourceReplay.body.createdCount,0);
 assert.equal(sourceReplay.body.replayedCount,2);
 assert.equal(sourceReplay.body.receipt?.replayed,true);

 // Different file bytes, same source and same external IDs: old Party IDs survive;
 // only the genuinely new external ID creates another Party.
 const sourceCsv2=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nsource-ext-1,person,Source Person,2\r\nsource-ext-2,organization,Source Org,2\r\nsource-ext-3,person,New Source Person,1\r\n");
 const sourcePreview2=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{csv:sourceCsv2,sourceId});
 assert.equal(sourcePreview2.status,200,formatRpcFailure(sourcePreview2));
 assert.equal(sourcePreview2.body.newCount,1);
 assert.equal(sourcePreview2.body.matchedCount,2);
 assert.deepEqual(sourcePreview2.body.targetPartyIds.slice(0,2),sourceIds1);
 const sourceApply2=await fixtureRpc(dispatch,methods.applyPartyImport,admin,{
   csv:sourceCsv2,sourceId,expectedSha256Hex:sourcePreview2.body.csvSha256Hex,
   expectedPreviewFingerprint:sourcePreview2.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-second-import"}
 });
 assert.equal(sourceApply2.status,200,formatRpcFailure(sourceApply2));
 assert.equal(sourceApply2.body.createdCount,1);
 assert.equal(sourceApply2.body.replayedCount,2);
 assert.equal(sourceApply2.body.receipt?.replayed,false);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_source_identities WHERE tenant='tenant-a' AND source_id='"+sourceId+"'"),3);

 // A matched-only reimport still commits an auditable operation receipt; replay adds no second audit.
 const matchedOnlyPreview=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{csv:sourceCsv2,sourceId});
 assert.equal(matchedOnlyPreview.body.newCount,0);
 assert.equal(matchedOnlyPreview.body.matchedCount,3);
 const applyAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='customer_data.csv_import.apply'");
 const matchedOnlyInput={csv:sourceCsv2,sourceId,
   expectedSha256Hex:matchedOnlyPreview.body.csvSha256Hex,
   expectedPreviewFingerprint:matchedOnlyPreview.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-matched-only"}};
 const matchedOnlyApply=await fixtureRpc(dispatch,methods.applyPartyImport,admin,matchedOnlyInput);
 assert.equal(matchedOnlyApply.status,200,formatRpcFailure(matchedOnlyApply));
 assert.equal(matchedOnlyApply.body.createdCount,0);
 assert.equal(matchedOnlyApply.body.replayedCount,3);
 assert.equal(matchedOnlyApply.body.receipt?.replayed,false);
 const matchedOnlyRequestId=matchedOnlyApply.headers.get("x-request-id");
 assert.ok(matchedOnlyRequestId);
 assert.deepEqual(await sql("SELECT action,subject_id,actor,request_id FROM crm_audit WHERE tenant_id=? AND request_id=?",
   "tenant-a",matchedOnlyRequestId),[{
     action:"customer_data.csv_import.apply",subject_id:sourceId,
     actor:"native-a@example.test",request_id:matchedOnlyRequestId
   }]);
 const matchedOnlyReplay=await fixtureRpc(dispatch,methods.applyPartyImport,knownService,matchedOnlyInput);
 assert.equal(matchedOnlyReplay.body.receipt?.replayed,true);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='customer_data.csv_import.apply'"),
   applyAuditBefore+1);

 // A changed known ID is a visible, non-committable preview; it never silently updates Party.
 const sourceConflictBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nsource-ext-1,person,Changed Source Person,3\r\n");
 const conflictPartyCount=await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'");
 const sourceConflict=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{csv:sourceConflictBytes,sourceId});
 assert.equal(sourceConflict.status,200,formatRpcFailure(sourceConflict));
 assert.deepEqual(sourceConflict.body.targetPartyIds,[]);
 assert.deepEqual(sourceConflict.body.rowIssues.map(issue=>[issue.rowPosition,issue.field,issue.code]),[
   [1,"display_name","IMPORT_SOURCE_IDENTITY_CONFLICT"]
 ]);
 assert.equal((await fixtureRpcError(dispatch,methods.applyPartyImport,admin,{
   csv:sourceConflictBytes,sourceId,expectedSha256Hex:sourceConflict.body.csvSha256Hex,
   expectedPreviewFingerprint:sourceConflict.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-conflict-denied"}
 })).body.code,"aborted");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"),conflictPartyCount);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE key='csv-source-conflict-denied'"),0);
 const kindConflict=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{
   csv:new TextEncoder().encode("party_id,kind,display_name,resource_version\r\nsource-ext-2,person,Source Org,2\r\n"),
   sourceId
 });
 assert.deepEqual(kindConflict.body.rowIssues.map(issue=>[issue.rowPosition,issue.field,issue.code]),[
   [1,"kind","IMPORT_SOURCE_IDENTITY_CONFLICT"]
 ]);
 const mixedConflictBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nsource-ext-1,person,Changed Name,3\r\nmixed-new-ext,person,Mixed New Party,1\r\n");
 const mixedConflictBefore=await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'");
 const mixedConflictPreview=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{
   csv:mixedConflictBytes,sourceId
 });
 assert.equal(mixedConflictPreview.body.newCount,1);
 assert.equal(mixedConflictPreview.body.rowIssues.length,1);
 assert.deepEqual(mixedConflictPreview.body.targetPartyIds,[]);
 assert.equal((await fixtureRpcError(dispatch,methods.applyPartyImport,admin,{
   csv:mixedConflictBytes,sourceId,expectedSha256Hex:mixedConflictPreview.body.csvSha256Hex,
   expectedPreviewFingerprint:mixedConflictPreview.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-mixed-conflict-denied"}
 })).body.code,"aborted");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"),mixedConflictBefore);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE key='csv-source-mixed-conflict-denied'"),0);

 // A different source and a different tenant never inherit this source's mapping.
 const secondSource=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,{
   displayName:"Other Pilot",precondition:{idempotencyKey:"csv-source-other-source"}
 });
 assert.equal(secondSource.body.receipt?.replayed,false);
 const secondSourceId=secondSource.body.source?.sourceId;
 assert.ok(typeof secondSourceId==="string"&&secondSourceId!==sourceId);
 const independent=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{
   csv:new TextEncoder().encode("party_id,kind,display_name,resource_version\r\nsource-ext-1,person,Source Person,1\r\n"),
   sourceId:secondSourceId
 });
 assert.equal(independent.body.newCount,1);
 assert.notEqual(independent.body.targetPartyIds[0],sourceIds1[0]);
 const tenantBSource=await fixtureRpc(dispatch,methods.registerPartyImportSource,tenantB,{
   displayName:"Pilot CRM",precondition:{idempotencyKey:"csv-source-tenant-b"}
 });
 assert.equal(tenantBSource.body.receipt?.replayed,false);
 const tenantBSourceId=tenantBSource.body.source?.sourceId;
 assert.ok(typeof tenantBSourceId==="string"&&tenantBSourceId!==sourceId);
 const tenantBPreview=await fixtureRpc(dispatch,methods.previewPartyImport,tenantB,{
   csv:sourceCsv1,sourceId:tenantBSourceId
 });
 assert.equal(tenantBPreview.body.newCount,2);
 assert.notDeepEqual(tenantBPreview.body.targetPartyIds,sourceIds1);
 assert.equal((await fixtureRpcError(dispatch,methods.previewPartyImport,tenantB,{
   csv:sourceCsv1,sourceId
 })).body.code,"not_found");

 // Source registration and its idempotency receipt are one atomic D1 batch.
 const atomicSourceKey="csv-source-register-atomic";
 const sourceRegistrationAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='customer_data.csv_import_source.register'");
 const atomicSourceInput={
   displayName:"Atomic Receipt Source",
   precondition:{idempotencyKey:atomicSourceKey}
 };
 await sql("CREATE TRIGGER fixture_fail_source_registration_receipt BEFORE INSERT ON crm_party_import_source_receipts "+
   "WHEN NEW.key='"+atomicSourceKey+"' BEGIN SELECT RAISE(ABORT,'fixture source receipt failure'); END;");
 try {
   await assertUnconfirmed(await sendFixtureRpc(
     dispatch,methods.registerPartyImportSource,admin,atomicSourceInput,503
   ),"IMPORT_SOURCE_REGISTER_UNCONFIRMED");
 } finally { await sql("DROP TRIGGER fixture_fail_source_registration_receipt"); }
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_sources WHERE tenant='tenant-a' AND normalized_name='atomic receipt source'"),0);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_source_receipts WHERE tenant='tenant-a' AND key='"+atomicSourceKey+"'"),0);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='customer_data.csv_import_source.register'"),sourceRegistrationAuditBefore);
 const atomicSourceRetry=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,atomicSourceInput);
 assert.equal(atomicSourceRetry.status,200,formatRpcFailure(atomicSourceRetry));
 assert.equal(atomicSourceRetry.body.receipt?.replayed,false);
 const atomicSourceReplay=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,atomicSourceInput);
 assert.equal(atomicSourceReplay.body.source?.sourceId,atomicSourceRetry.body.source?.sourceId);
 assert.equal(atomicSourceReplay.body.receipt?.replayed,true);

 // A preview is invalidated when another import binds its previously-new external ID.
 const staleBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nsource-ext-1,person,Source Person,2\r\nstale-ext,person,Stale Preview Person,1\r\n");
 const stalePreview=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{csv:staleBytes,sourceId});
 assert.equal(stalePreview.body.newCount,1);
 assert.equal(stalePreview.body.matchedCount,1);
 const concurrentWinner=await fixtureRpc(dispatch,methods.applyPartyImport,admin,{
   csv:staleBytes,sourceId,expectedSha256Hex:stalePreview.body.csvSha256Hex,
   expectedPreviewFingerprint:stalePreview.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-stale-winner"}
 });
 assert.equal(concurrentWinner.status,200,formatRpcFailure(concurrentWinner));
 assert.equal(concurrentWinner.body.createdCount,1);
 assert.equal(concurrentWinner.body.replayedCount,1);
 const beforeStaleLoser=await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'");
 assert.equal((await fixtureRpcError(dispatch,methods.applyPartyImport,admin,{
   csv:staleBytes,sourceId,expectedSha256Hex:stalePreview.body.csvSha256Hex,
   expectedPreviewFingerprint:stalePreview.body.previewFingerprint,
   precondition:{idempotencyKey:"csv-source-stale-loser"}
 })).body.code,"aborted");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"),beforeStaleLoser);

 // Two concurrent commits for the same source/external ID cannot create duplicates.
 const concurrentBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nconcurrent-ext,person,Concurrent Source Person,1\r\n");
 const concurrentPreview=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{csv:concurrentBytes,sourceId});
 const concurrentInput={
   csv:concurrentBytes,sourceId,expectedSha256Hex:concurrentPreview.body.csvSha256Hex,
   expectedPreviewFingerprint:concurrentPreview.body.previewFingerprint
 };
 const [raceA,raceB]=await Promise.all([
   fixtureRpcRace(dispatch,methods.applyPartyImport,admin,{
     ...concurrentInput,precondition:{idempotencyKey:"csv-source-race-a"}
   }),
   fixtureRpcRace(dispatch,methods.applyPartyImport,knownService,{
     ...concurrentInput,precondition:{idempotencyKey:"csv-source-race-b"}
   })
 ]);
 assert.equal([raceA,raceB].filter(result=>result.status===200).length,1);
 const raceFailure=[raceA,raceB].find(result=>result.status!==200);
 assert.ok(raceFailure&&[409,503].includes(raceFailure.status));
 const concurrentDigest=createHash("sha256").update("concurrent-ext").digest("hex");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_source_identities WHERE tenant='tenant-a' AND source_id='"+
   sourceId+"' AND external_id_sha256='"+concurrentDigest+"'"),1);

 // A late identity-row failure rolls back Party, identity, receipt and audit together.
 const rollbackSource=await fixtureRpc(dispatch,methods.registerPartyImportSource,admin,{
   displayName:"Rollback Source",precondition:{idempotencyKey:"csv-source-rollback"}
 });
 assert.equal(rollbackSource.body.receipt?.replayed,false);
 const rollbackSourceId=rollbackSource.body.source?.sourceId;
 assert.ok(typeof rollbackSourceId==="string");
 const rollbackSourceCsv=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nrollback-ext-1,person,Rollback One,1\r\nrollback-ext-2,person,Rollback Two,1\r\n");
 const rollbackSourcePreview=await fixtureRpc(dispatch,methods.previewPartyImport,admin,{
   csv:rollbackSourceCsv,sourceId:rollbackSourceId
 });
 const rollbackSourceKey="csv-source-atomic-rollback";
 const rollbackSourceInput={
   csv:rollbackSourceCsv,sourceId:rollbackSourceId,
   expectedSha256Hex:rollbackSourcePreview.body.csvSha256Hex,
   expectedPreviewFingerprint:rollbackSourcePreview.body.previewFingerprint,
   precondition:{idempotencyKey:rollbackSourceKey}
 };
 const rollbackSourceDigest=createHash("sha256").update("rollback-ext-2").digest("hex");
 const sourceRollbackAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const sourceRollbackPartyBefore=await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'");
 const sourceRollbackPartyReceiptsBefore=await count("SELECT COUNT(*) AS n FROM crm_party_receipts WHERE tenant='tenant-a'");
 await sql("CREATE TRIGGER fixture_fail_source_identity BEFORE INSERT ON crm_party_source_identities "+
   "WHEN NEW.external_id_sha256='"+rollbackSourceDigest+"' BEGIN SELECT RAISE(ABORT,'fixture source identity failure'); END;");
 try {
   await assertUnconfirmed(await sendFixtureRpc(dispatch,methods.applyPartyImport,knownService,rollbackSourceInput,503),
     "IMPORT_WRITE_UNCONFIRMED");
 } finally { await sql("DROP TRIGGER fixture_fail_source_identity"); }
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"),sourceRollbackPartyBefore);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_receipts WHERE tenant='tenant-a'"),sourceRollbackPartyReceiptsBefore);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_source_identities WHERE tenant='tenant-a' AND source_id='"+rollbackSourceId+"'"),0);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE key='"+rollbackSourceKey+"'"),0);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),sourceRollbackAuditBefore);
 const rollbackSourceRetry=await fixtureRpc(dispatch,methods.applyPartyImport,knownService,rollbackSourceInput);
 assert.equal(rollbackSourceRetry.status,200,formatRpcFailure(rollbackSourceRetry));
 assert.equal(rollbackSourceRetry.body.createdCount,2);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='customer_data.csv_import_identity.bind'"),7);
 // Pilot CRM, Other Pilot, Atomic Receipt Source and Rollback Source are tenant-A sources.
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='customer_data.csv_import_source.register'"),4);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id IN ('source-ext-1','source-ext-2','source-ext-3','stale-ext','concurrent-ext','rollback-ext-1','rollback-ext-2')"),0);
 console.log(JSON.stringify({event:"crm.native-party-csv-source-identity",result:"PASS",
   source_registry:true,stable_external_id:true,changed_bytes:true,mixed_new_and_known:true,
   conflict_preview:true,no_silent_update:true,tenant_and_source_scoped:true,
   stale_preview:true,atomic_identity_binding:true,receipt_recovery:true}));
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

import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {fixtureRpc,fixtureRpcError,decodeFixtureDetail} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {PartyService} from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import {AccountService} from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import {ContactPointService} from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import {PartyRelationshipService} from "../../../../packages/client/gen/crm/party_relationships/v2/relationship_pb.js";
import {TaskService} from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {PrivacyCaseService} from "../../../../packages/client/gen/crm/customer_privacy/v2/case_pb.js";
import {DomainErrorDetailSchema} from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import {number,record,records,string} from "../../scripts/json-boundary.mts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";
import type {verifyAccounts} from "./accounts.mts";
import type {verifyContacts} from "./contacts.mts";
import type {verifySalesCreate} from "./sales-create.mts";
import type {MutationReceipt} from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";

type Origin = {id:string; other:string; key:string; headers:Pick<Headers,"get">};
type ReceiptFixtures = {
  dispatch:RpcDispatch; sql:(query:string,...params:(string|null)[])=>Promise<Record<string,unknown>[]>;
  call:(path:string,identity:string)=>Promise<{status:number;body:Record<string,unknown>}>;
  admin:string; viewer:string; tenantB:string;
  party:Origin; account:Origin; contact:Origin; relationship:Origin; task:Origin;
  saleCreate:Awaited<ReturnType<typeof verifySalesCreate>>["saleCreate"];
  accountRetryInput:Awaited<ReturnType<typeof verifyAccounts>>["accountRetryInput"];
  contactRetryInput:Awaited<ReturnType<typeof verifyContacts>>["contactRetryInput"];
};

export async function verifyReceipts({dispatch,sql,call,admin,viewer,tenantB,
  party,account,contact,relationship,task,saleCreate,accountRetryInput,contactRetryInput}:ReceiptFixtures) {
 const parties=PartyService.method,accounts=AccountService.method,contacts=ContactPointService.method;
 const relationships=PartyRelationshipService.method,tasks=TaskService.method;
 const s5aReceipts=[
   {owner:"party",table:"crm_party_receipts",origin:party,
     read:(id:string,key:string)=>fixtureRpc(dispatch,parties.getParty,viewer,{partyId:id,receiptIdempotencyKey:key}),
     reject:(identity:string|null,key:string)=>fixtureRpcError(dispatch,parties.getParty,identity,{partyId:party.id,receiptIdempotencyKey:key})},
   {owner:"account",table:"crm_account_receipts",origin:account,
     read:(id:string,key:string)=>fixtureRpc(dispatch,accounts.getAccount,viewer,{accountId:id,receiptIdempotencyKey:key}),
     reject:(identity:string|null,key:string)=>fixtureRpcError(dispatch,accounts.getAccount,identity,{accountId:account.id,receiptIdempotencyKey:key})},
   {owner:"contact",table:"crm_contact_point_receipts",origin:contact,
     read:(id:string,key:string)=>fixtureRpc(dispatch,contacts.getContactPoint,viewer,{contactPointId:id,receiptIdempotencyKey:key}),
     reject:(identity:string|null,key:string)=>fixtureRpcError(dispatch,contacts.getContactPoint,identity,{contactPointId:contact.id,receiptIdempotencyKey:key})},
   {owner:"relationship",table:"crm_party_relationship_receipts",origin:relationship,
     read:(id:string,key:string)=>fixtureRpc(dispatch,relationships.getPartyRelationship,viewer,{relationshipId:id,receiptIdempotencyKey:key}),
     reject:(identity:string|null,key:string)=>fixtureRpcError(dispatch,relationships.getPartyRelationship,identity,{relationshipId:relationship.id,receiptIdempotencyKey:key})},
   {owner:"task",table:"crm_activity_task_receipts",origin:task,
     read:(id:string,key:string)=>fixtureRpc(dispatch,tasks.getTask,viewer,{taskId:id,receiptIdempotencyKey:key}),
     reject:(identity:string|null,key:string)=>fixtureRpcError(dispatch,tasks.getTask,identity,{taskId:task.id,receiptIdempotencyKey:key})},
 ];
 for(const {owner,table,origin,read,reject} of s5aReceipts) {
   const original=string(origin.headers.get("x-request-id"));
   const result=await read(origin.id,origin.key);
   const confirmed=result.body.confirmedMutation;
   assert.equal(confirmed?.originalRequestId,original,
     "original signed D1 receipt provenance must survive subsequent owner updates");
   assert.equal(confirmed?.resourceVersion,1n,
     "read-only recovery must use the receipt snapshot version, not mutable latest state");
   const requestId=string(result.headers.get("x-request-id"));
   assert.equal(confirmed.requestId,requestId);
   const saved=await sql("SELECT original_request_id FROM "+table+" WHERE tenant=? AND key=?",
     "tenant-a",origin.key);
   assert.deepEqual(saved,[{original_request_id:original}]);
   assert.equal((await reject(tenantB,origin.key)).status,404,"tenant isolation precedes receipt lookup");
   assert.equal((await reject(null,origin.key)).status,401);
   assert.equal((await reject(viewer,"short")).status,400);
   assert.equal((await read(origin.id,"s5a-unknown-key-"+owner)).body.confirmedMutation,undefined);
   // A same-tenant unrelated resource cannot inherit somebody else's key.
   assert.equal((await read(origin.other,origin.key)).body.confirmedMutation,undefined);
   // Legacy receipts without known origin must not be inferred from row order.
   await sql("UPDATE "+table+" SET original_request_id=NULL WHERE tenant=? AND key=?",
     "tenant-a",origin.key);
   try {
     assert.equal((await read(origin.id,origin.key)).body.confirmedMutation,undefined,
       "historical NULL provenance is not evidence of a committed mutation");
   } finally {
     await sql("UPDATE "+table+" SET original_request_id=? WHERE tenant=? AND key=?",
       original,"tenant-a",origin.key);
   }
   assert.equal((await read(origin.id,origin.key)).body.confirmedMutation?.originalRequestId,original);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id=?",requestId))[0]?.n,
     0,"receipt checks never append audit");
 }
 // D1 read failure keeps the write outcome uncertain; the base Get remains readable.
 const partyProof={partyId:party.id,receiptIdempotencyKey:party.key};
 await sql("ALTER TABLE crm_party_receipts RENAME TO fixture_unavailable_party_receipts");
 try {
   const denied=await fixtureRpcError(dispatch,parties.getParty,viewer,partyProof);
   assert.equal(denied.status,503,formatRpcFailure(denied));
   await fixtureRpc(dispatch,parties.getParty,viewer,{partyId:party.id});
 } finally {
   await sql("ALTER TABLE fixture_unavailable_party_receipts RENAME TO crm_party_receipts");
 }
 assert.equal((await fixtureRpc(dispatch,parties.getParty,viewer,partyProof)).body.confirmedMutation?.originalRequestId,
   string(party.headers.get("x-request-id")));
 console.log(JSON.stringify({event:"crm.native-s5a-crud-commit-provenance",result:"PASS",
   owners:s5aReceipts.map(x=>x.owner),original_request_ids:true,read_only:true,
   latest_version_independent:true,legacy_null_fail_closed:true,
   tenant_isolation:true,wrong_resource_rejected:true}));
 // #471 S2: prove SQL JSON paths and native persisted Rust state agree
 // for all active owner records created through generated binary Connect.
 // These are *SQL usage contracts*, not a new JSON business-schema registry.
 const nativeSqlStateContracts=[
   {table:"crm_party_records",id:"party_id",fields:["kind","display_name"],version:true},
   {table:"crm_account_records",id:"account_id",fields:["name","status"],version:true},
   {table:"crm_contact_point_records",id:"contact_point_id",fields:["kind","normalized_value","status"],version:true},
   {table:"crm_party_relationship_records",id:"party_relationship_id",fields:["status"],version:true},
   {table:"crm_activity_task_records",id:"task_id",fields:["status","priority"],version:true},
   {table:"crm_sales_deal_records",id:"deal_id",
     fields:["status","stage.pipeline_id","stage.stage_id"],version:true},
   {table:"crm_sales_pipeline_records",id:"pipeline_id",fields:["name"],version:false},
   {table:"crm_sales_saved_view_records",id:"view_id",fields:["name"],version:false}
 ];
 let sqlStateRows=0;
 for(const contract of nativeSqlStateContracts){
   const tests=[
     "json_valid(state) IS NOT 1",
     "json_extract(state,'$."+contract.id+"') IS NOT id",
     ...contract.fields.map(path=>"json_type(state,'$."+path+"') IS NOT 'text'"),
     ...(contract.version?["json_extract(state,'$.version') IS NOT version"]:[])
   ];
   if(contract.table==="crm_sales_pipeline_records")
     tests.push("json_type(state,'$.stages') IS NOT 'array'");
   const [result]=await sql("SELECT COUNT(*) AS n,SUM(CASE WHEN "+tests.join(" OR ")+
     " THEN 1 ELSE 0 END) AS broken FROM "+contract.table+" WHERE tenant=?", "tenant-a");
   assert.ok(number(result?.n)>0,contract.table+" must be covered by signed owner fixtures");
   assert.equal(result?.broken,0,contract.table+" SQL paths drifted from the Rust owner codec");
   sqlStateRows+=number(result?.n);
 }
 // The Privacy owner intentionally persists profiled canonical JSON rather
 // than raw Serde, and SQL must never lose or silently bypass the profile.
 const [privacySqlState]=await sql("SELECT COUNT(*) AS n,SUM(CASE WHEN "+
   "json_extract(state,'$.canonicalization_profile') IS NOT 'crm.cjson/v1' "+
   "THEN 1 ELSE 0 END) AS broken FROM crm_privacy_case_records WHERE tenant=?", "tenant-a");
 assert.ok(number(privacySqlState?.n)>0,"Privacy signed owner scenario must exercise canonical snapshots");
 assert.equal(privacySqlState?.broken,0,"Privacy canonicalization profile must survive D1 writes");
 sqlStateRows+=number(privacySqlState?.n);
 console.log(JSON.stringify({event:"crm.native-s2-owner-sql-persistence-contract",result:"PASS",
   proof:"signed generated Protobuf RPC -> original Rust owner -> real workerd D1 SQL paths",
   owners:nativeSqlStateContracts.length+1,checked_rows:sqlStateRows,
   privacy_canonical_profile:true}));
 // The exact production receipt query (not a mocked storage error) must
 // distinguish a proven absent key from an inaccessible receipt table.
 // Use only the existing signed native-ci read-only probe and local D1 fixture.
 // Preparation failure occurs before a command can reach D1 writes;
 // unlike a failed lookup of the original receipt, it is a typed rejection.
 const preparationInput={...saleCreate,dealId:"r471-prepare-"+randomUUID(),
   precondition:{idempotencyKey:"r471-preparation-failure"}};
 await sql("ALTER TABLE crm_sales_pipeline_records RENAME TO fixture_unavailable_deal_catalog");
 try {
   const preparationFailed=await fixtureRpcError(dispatch,DealService.method.createDeal,admin,preparationInput);
   assert.equal(preparationFailed.status,503,formatRpcFailure(preparationFailed));
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(preparationFailed.body).details));
   assert.equal(detail.domainCode,"SALES_WRITE_PREPARATION_FAILED");
   assert.equal(detail.commitOutcome,1);
   assert.equal(detail.requestId,preparationFailed.headers.get("x-request-id"));
 } finally {
   await sql("ALTER TABLE fixture_unavailable_deal_catalog RENAME TO crm_sales_pipeline_records");
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records WHERE tenant=? AND id=?",
   "tenant-a",preparationInput.dealId))[0]?.n,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
   "tenant-a",preparationInput.precondition.idempotencyKey))[0]?.n,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?",
   "tenant-a",preparationInput.dealId))[0]?.n,0);
 const receiptProbe=await call("/api/_ci/original-domain-check",admin);
 assert.equal(receiptProbe.status,200);
 assert.equal(receiptProbe.body.receipt_read_classification,"PROVEN_NO_CHANGE");
 await sql("ALTER TABLE crm_sales_deal_receipts RENAME TO fixture_unavailable_deal_receipts");
 try {
   const unavailable=await call("/api/_ci/original-domain-check",admin);
   assert.equal(unavailable.status,200);
   const unknownReplay=await fixtureRpcError(dispatch,DealService.method.createDeal,admin,saleCreate);
   assert.equal(unknownReplay.status,503);
   const unknownDetail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(unknownReplay.body).details));
   assert.equal(unknownDetail.domainCode,"SALES_COMMIT_UNCONFIRMED");
   assert.equal(unknownDetail.commitOutcome,2);
   assert.equal(unavailable.body.receipt_read_classification,"RECEIPT_READ_FAILED",
     "An inaccessible receipt source must not become a proven CAS conflict");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_deal_receipts RENAME TO crm_sales_deal_receipts");
 }
 const recoveredProbe=await call("/api/_ci/original-domain-check",admin);
 assert.equal(recoveredProbe.status,200);
 assert.equal(recoveredProbe.body.receipt_read_classification,"PROVEN_NO_CHANGE");
 console.log(JSON.stringify({event:"crm.native-s1-reconciliation-read",result:"PASS",
   actual_d1_read_failure:true,restore:true,production_receipt_query:true}));

 // Both concurrent retries must see one committed version and the same original request ID.
 const retry=async(subject:string,write:()=>Promise<{body:{receipt?:MutationReceipt|undefined}}>)=>{
   const replies=await Promise.all([write(),write()]);
   const receipts=replies.map(({body})=>{
     assert.ok(body.receipt,"Retry must return an immutable receipt");
     return body.receipt;
   });
   const originals=receipts.filter(receipt=>!receipt.replayed);
   assert.equal(originals.length,1);
   const original=string(originals[0]?.requestId);
   for(const receipt of receipts){
     assert.equal(receipt.originalRequestId,original);
     assert.equal(receipt.resourceVersion,1n);
   }
   assert.deepEqual(await sql("SELECT request_id FROM crm_audit WHERE tenant_id=? AND subject_id=?",
     "tenant-a",subject),[{request_id:original}]);
 };
 await retry("party-audit-failure",()=>fixtureRpc(dispatch,parties.createParty,admin,
   {partyId:"party-audit-failure",kind:1,displayName:"Rollback",precondition:{idempotencyKey:"party-audit-failed-create"}}));
 await retry("privacy-audit-failed-create",()=>fixtureRpc(dispatch,PrivacyCaseService.method.createPrivacyCase,admin,
   {caseId:"privacy-audit-failed-create",kind:1,precondition:{idempotencyKey:"privacy-audit-failed-create-key"}}));
 await retry(accountRetryInput.accountId,()=>fixtureRpc(dispatch,accounts.createAccount,admin,accountRetryInput));
 await retry(contactRetryInput.contactPointId,()=>fixtureRpc(dispatch,contacts.createContactPoint,admin,contactRetryInput));
}

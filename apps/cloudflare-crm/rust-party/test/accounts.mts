import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { AccountService } from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import type { ListAccountsRequest } from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type AccountsFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 id: string;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
 unknownService: string;
};
export async function verifyAccounts({dispatch,sql,id,admin,viewer,tenantB,knownService,unknownService}: AccountsFixture) {
 const account=AccountService.method;
 const assertUnconfirmed=(failed: Awaited<ReturnType<typeof fixtureRpcError>>) => {
   assert.equal(failed.status,503);
   assert.equal(failed.body.code,"unavailable");
   const detail=decodeFixtureDetail(DomainErrorDetailSchema,records(record(failed.body).details));
   assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
   assert.equal(detail.domainCode,"CUSTOMER_ACCOUNTS_WRITE_UNCONFIRMED");
   assert.equal(detail.requestId,failed.headers.get("x-request-id"));
 };
 const collectAccounts=(input: Partial<Pick<ListAccountsRequest, "status">>)=>collectFixturePages(async pageToken=>{
   const result=await fixtureRpc(dispatch,account.listAccounts,viewer,{...input,page:{pageSize:1,pageToken}});
   return {items:result.body.accounts,nextPageToken:result.body.pageInfo?.nextPageToken};
 });
 const accountId="account-native-"+randomUUID();
 const accountInput={accountId,name:"  Connected   Account  ",primaryPartyId:id,
   precondition:{idempotencyKey:"candidate-v2-account-create-001"}};
 assert.equal((await fixtureRpcError(dispatch,account.createAccount,null,accountInput)).status,401);
 assert.equal((await fixtureRpcError(dispatch,account.createAccount,viewer,accountInput)).status,403);
 assert.equal((await fixtureRpcError(dispatch,account.createAccount,admin,{...accountInput,
   precondition:{idempotencyKey:"candidate-v2-account-version",expectedVersion:1n}})).status,400);
 assert.equal((await fixtureRpcError(dispatch,account.createAccount,admin,{...accountInput,
   primaryPartyId:"no-such-party",
   precondition:{idempotencyKey:"candidate-v2-account-fk"}})).status,400);
 const accountCreated=await fixtureRpc(dispatch,account.createAccount,admin,accountInput,{
    "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
    "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(accountCreated.status,200,formatRpcFailure(accountCreated));
 assert.equal(accountCreated.body.account?.accountId,accountId);
 assert.equal(accountCreated.body.account?.name,"Connected Account");
 assert.equal(accountCreated.body.account?.parties?.[0]?.partyId,id);
 assert.equal(accountCreated.body.account?.parties?.[0]?.role,1);
 assert.equal(accountCreated.body.receipt?.resourceVersion,1n);
 assert.equal(accountCreated.body.receipt?.replayed,false);
 assert.equal(accountCreated.body.receipt?.requestId,accountCreated.headers.get("x-request-id"));
 const accountReplay=await fixtureRpc(dispatch,account.createAccount,knownService,accountInput);
 assert.equal(accountReplay.status,200,formatRpcFailure(accountReplay));
 assert.equal(accountReplay.body.receipt?.replayed,true);
 assert.equal(accountReplay.body.receipt?.originalRequestId,accountCreated.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch,account.createAccount,admin,{...accountInput,
   name:"different idempotent body"})).status,409);
 assert.equal((await fixtureRpcError(dispatch,account.getAccount,tenantB,{accountId})).status,404);
 assert.equal((await fixtureRpc(dispatch,account.getAccount,viewer,{accountId})).body.account?.name,"Connected Account");
 const accountList=await fixtureRpc(dispatch,account.listAccounts,viewer,{page:{pageSize:1}});
 assert.equal(accountList.status,200,formatRpcFailure(accountList));
 assert.equal(accountList.body.accounts.length,1);
 assert.equal(accountList.body.accounts[0]?.accountId,accountId);
 assert.equal((await fixtureRpcError(dispatch,account.listAccounts,viewer,{page:{pageSize:101}})).status,400);
 const accountUpdate={accountId,name:"Updated Account",status:2,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-v2-account-update-001"}};
 assert.equal((await fixtureRpcError(dispatch,account.updateAccount,viewer,accountUpdate)).status,403);
 assert.equal((await fixtureRpcError(dispatch,account.updateAccount,admin,{...accountUpdate,status:0,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-v2-account-invalid"}})).status,400);
 const accountUpdated=await fixtureRpc(dispatch,account.updateAccount,knownService,accountUpdate);
 assert.equal(accountUpdated.status,200,formatRpcFailure(accountUpdated));
 assert.equal(accountUpdated.body.account?.status,2);
 assert.equal(accountUpdated.body.account?.version,2n);
 assert.equal(accountUpdated.body.receipt?.replayed,false);
 const accountReupdate=await fixtureRpc(dispatch,account.updateAccount,admin,accountUpdate);
 assert.equal(accountReupdate.status,200,formatRpcFailure(accountReupdate));
 assert.equal(accountReupdate.body.receipt?.replayed,true);
 assert.equal(accountReupdate.body.receipt?.originalRequestId,accountUpdated.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch,account.updateAccount,admin,{...accountUpdate,
   precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-v2-account-stale"}})).status,409);
 assert.equal((await fixtureRpc(dispatch,account.listAccounts,viewer,{page:{pageSize:1},status:1}))
   .body.accounts.length,0);
 assert.equal((await fixtureRpc(dispatch,account.listAccounts,viewer,{page:{pageSize:1},status:2}))
   .body.accounts.length,1);
 assert.equal((await sql("SELECT version FROM crm_account_records WHERE tenant='tenant-a' "+
   "AND id='"+accountId+"'"))[0]?.version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_account_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v2-account-create-001','candidate-v2-account-update-001')"))[0]?.n,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+accountId+"' AND action LIKE 'accounts.%'"))[0]?.n,2);
 assert.deepEqual((await sql("SELECT action,tenant_id,subject_id,request_id,actor FROM crm_audit WHERE subject_id='"+accountId+"' ORDER BY rowid")),[
   {action:"accounts.create",tenant_id:"tenant-a",subject_id:accountId,
     request_id:accountCreated.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"accounts.update",tenant_id:"tenant-a",subject_id:accountId,
     request_id:accountUpdated.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Cross-actor Account replay preserves original verified actor/request ID");
 const accountSnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_account_records) AS states,"+
   "(SELECT COUNT(*) FROM crm_account_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 const beforeAccountDenials=(await accountSnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await fixtureRpcError(dispatch,account.createAccount,token,accountInput)).status>=400);
   assert.ok((await fixtureRpcError(dispatch,account.updateAccount,token,accountUpdate)).status>=400);
 }
 assert.equal((await fixtureRpcError(dispatch,account.updateAccount,tenantB,accountUpdate)).status,404);
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,account.createAccount,knownService,accountInput)).status,401,
   "Deactivated service cannot replay Account creation");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await fixtureRpcError(dispatch,account.updateAccount,knownService,accountUpdate)).status,403,
   "Current permission must be checked before Account replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.deepEqual((await accountSnapshot()),beforeAccountDenials,
   "Denied/stale/invalid Account commands cannot change state, receipts or audit");
 const accountRetryInput={accountId:"account-audit-rollback",name:"Must roll back",primaryPartyId:id,
   precondition:{idempotencyKey:"account-audit-failed-create"}};
 (await sql("CREATE TRIGGER fixture_fail_account_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('account-audit-rollback','"+accountId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture account audit failure'); END;"));
 try{
   assertUnconfirmed(await fixtureRpcError(dispatch,account.createAccount,admin,accountRetryInput));
   assertUnconfirmed(await fixtureRpcError(dispatch,account.updateAccount,admin,{
     ...accountUpdate,name:"Must roll back",
     precondition:{expectedVersion:2n,idempotencyKey:"account-audit-failed-update"}}));
   assert.deepEqual((await accountSnapshot()),beforeAccountDenials,
     "Account audit INSERT failure rolls back state and receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_account_audit");}
 console.log(JSON.stringify({event:"crm.native-account-actor-audit",result:"PASS",
   human_create:true,service_update:true,cross_actor_replay:true,forged_headers_ignored:true,
   authorization_before_replay:true,tenant_ab:true,sql_failure_rollback:true}));
 console.log(JSON.stringify({event:"crm.native-g3-account-v2",result:"PASS",
   generated_binary:true,owner:"crm.customer-accounts",
   rpc_methods:["CreateAccount","UpdateAccount","GetAccount","ListAccounts"],
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   d1_cas:true,d1_receipts:true,d1_audit:true,published_active:true}));


 // Account filter must run BEFORE LIMIT: active records sort before inactive.
 const accountNextId="account-zzz-inactive-"+randomUUID();
 for(const [index,accountToCreate] of [
   "account-aaa-active-"+randomUUID(),"account-aab-active-"+randomUUID(),accountNextId
 ].entries()){
   const made=await fixtureRpc(dispatch,account.createAccount,admin,{
     accountId:accountToCreate,name:"Paging Account "+index,primaryPartyId:id,
     precondition:{idempotencyKey:"page-account-create-"+index}});
   assert.equal(made.status,200,formatRpcFailure(made));
 }
 const inactiveLater=await fixtureRpc(dispatch,account.updateAccount,admin,{
   accountId:accountNextId,name:"Later Inactive Account",status:2,
   precondition:{expectedVersion:1n,idempotencyKey:"page-account-inactive"}});
 assert.equal(inactiveLater.status,200,formatRpcFailure(inactiveLater));
 const inactiveAccounts=await collectAccounts({status:2});
 assert.deepEqual(inactiveAccounts.map(a=>a.accountId),[accountId,accountNextId]);
 assert.equal((await collectAccounts({status:1})).length,2);
 assert.equal((await collectAccounts({status:2})).length,2);
 const afterAccounts=await fixtureRpc(dispatch,account.listAccounts,viewer,{
   page:{pageSize:1,pageToken:accountNextId},status:2});
 assert.equal(afterAccounts.status,200);
 assert.equal(afterAccounts.body.accounts.length,0);
 assert.ok(!afterAccounts.body.pageInfo?.nextPageToken);
 assert.equal((await fixtureRpc(dispatch,account.listAccounts,tenantB,{page:{pageSize:1},status:2}))
   .body.accounts.length,0);
 console.log(JSON.stringify({event:"crm.native-g3-account-filtered-pagination",result:"PASS"}));

 return {accountId,accountInput,accountCreated,accountNextId,accountRetryInput};
}

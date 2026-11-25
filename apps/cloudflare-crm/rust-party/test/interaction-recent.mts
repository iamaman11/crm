import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { InteractionService } from "../../../../packages/client/gen/crm/activities/v2/interaction_pb.js";
import { fixtureRpc, fixtureRpcError } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type RecentFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 accountId: string; accountNextId: string; dealId: string;
 eventLater: string; eventEarlier: string; eventNote: string; failedId: string;
 sourceSha: string; admin: string; viewer: string; tenantB: string;
};
export async function verifyRecentInteractions({dispatch,sql,accountId,accountNextId,dealId,eventLater,eventEarlier,eventNote,failedId,sourceSha,admin,viewer,tenantB}: RecentFixture) {
 const interaction=InteractionService.method;
 // The SAME Interaction List RPC supports an explicitly authorized
 // tenant-wide recent view; Account/Deal history remains scoped as before.
 const sk06OtherInteraction="sk06-recent-"+randomUUID();
 const sk06RecentInput={interactionId:sk06OtherInteraction,accountId:accountNextId,dealId:"",kind:1,
   text:"SK05 validated interaction "+sk06OtherInteraction,occurredAt:{seconds:1790000000n,nanos:123456789},
   precondition:{idempotencyKey:"sk05-"+sk06OtherInteraction}};
 const sk06Created=await fixtureRpc(dispatch,interaction.createInteraction,admin,sk06RecentInput);
 assert.equal(sk06Created.status,200,formatRpcFailure(sk06Created));
 const sk06RecentAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
 const sk06Recent=await fixtureRpc(dispatch,interaction.listInteractions,viewer,{
   recentAcrossAccounts:true,page:{pageSize:2}
 });
 assert.equal(sk06Recent.status,200);
 assert.deepEqual(sk06Recent.body.interactions.map(event=>event.interactionId),
   [sk06OtherInteraction,eventLater],
   "SK-06 newest interactions span multiple Accounts before keyset LIMIT");
 assert.ok(sk06Recent.body.accountNames[accountNextId]);
 assert.ok(sk06Recent.body.accountNames[accountId]);
 assert.ok(sk06Recent.body.pageInfo?.nextPageToken);
 const sk06NextRecent=await fixtureRpc(dispatch,interaction.listInteractions,viewer,{
   recentAcrossAccounts:true,page:{pageSize:2,
     pageToken:sk06Recent.body.pageInfo.nextPageToken}
 });
 assert.deepEqual(sk06NextRecent.body.interactions.map(event=>event.interactionId),
   [eventNote,failedId],"a recovered SK-05 Interaction is a real fact, not a missing event");
 assert.ok(sk06NextRecent.body.pageInfo?.nextPageToken);
 const sk06ThirdRecent=await fixtureRpc(dispatch,interaction.listInteractions,viewer,{
   recentAcrossAccounts:true,page:{pageSize:2,
     pageToken:sk06NextRecent.body.pageInfo.nextPageToken}
 });
 assert.deepEqual(sk06ThirdRecent.body.interactions.map(event=>event.interactionId),
   [eventEarlier],"tenant-wide keyset must return every immutable event");
 assert.equal(sk06ThirdRecent.body.pageInfo?.nextPageToken??"","");
 assert.deepEqual((await fixtureRpc(dispatch,interaction.listInteractions,admin,{
   recentAcrossAccounts:true,page:{pageSize:2}
 })).body,sk06Recent.body);
 assert.deepEqual((await fixtureRpc(dispatch,interaction.listInteractions,tenantB,{
   recentAcrossAccounts:true,page:{pageSize:3}
 })).body.interactions,[]);
 assert.equal((await fixtureRpcError(dispatch,interaction.listInteractions,viewer,{
   recentAcrossAccounts:true,accountId,page:{pageSize:2}
 })).status,400);
 assert.equal((await fixtureRpcError(dispatch,interaction.listInteractions,viewer,{
   recentAcrossAccounts:true,dealId,page:{pageSize:2}
 })).status,400);
 assert.equal((await fixtureRpcError(dispatch,interaction.listInteractions,viewer,{
   recentAcrossAccounts:false,page:{pageSize:2}
 })).status,400);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n,
   sk06RecentAuditBefore,"recent original Interaction reads are audit-free");
 console.log(JSON.stringify({event:"crm.native-sk06-workday",result:"PASS",source_sha:sourceSha,
   owner:"crm.activities",typed_binary:true,due_range_before_limit:true,
   due_nanosecond_keyset:true,undated:true,tenant_ab:true,viewer_full_fields:true,
   recent_across_accounts:true,account_names:true,invalid_scope_denied:true,
   no_read_audit:true}));

}

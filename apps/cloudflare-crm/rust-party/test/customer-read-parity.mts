import assert from "node:assert/strict";
import {fixtureRpc, fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import {PartyService} from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import {AccountService} from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import {ContactPointService} from "../../../../packages/client/gen/crm/contact_points/v2/contact_point_pb.js";
import {PartyRelationshipService} from "../../../../packages/client/gen/crm/party_relationships/v2/relationship_pb.js";
import {TaskService} from "../../../../packages/client/gen/crm/activities/v2/task_pb.js";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {verifyOverviewReadParity} from "./account-overview.mts";

type Fixture = {
  dispatch: RpcDispatch;
  sql: (statement: string) => Promise<ReadonlyArray<Record<string, unknown>>>;
  admin: string; viewer: string; tenantB: string;
  id: string; accountId: string; contactId: string; rMid: string; dealId: string; linkedTaskId: string;
};

export async function verifyCustomerReadParity({dispatch,sql,admin,viewer,tenantB,
  id,accountId,contactId,rMid,dealId,linkedTaskId}: Fixture) {
  const audit=async()=>(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0]?.n;
  const before=await audit();
  assert.equal(typeof before,"number");
  const parties=PartyService.method,accounts=AccountService.method,contacts=ContactPointService.method;
  const relationships=PartyRelationshipService.method,deals=DealService.method,tasks=TaskService.method;
  let gets=0,lists=0;
  // Compare complete generated business records for both signed roles.
  const get=async<T,>(label: string,read: (identity: string) => Promise<T | undefined>,
    denied: () => Promise<{status: number}>)=>{
    const fromAdmin=await read(admin);
    const fromViewer=await read(viewer);
    assert.ok(fromAdmin&&fromViewer,label+" must return an authorized record");
    assert.deepEqual(fromViewer,fromAdmin,label+" must expose every business field to viewer");
    assert.equal((await denied()).status,404,label+" must not reveal a cross-tenant record");
    gets++;
    return fromViewer;
  };
  const party=await get("Party",async identity=>(await fixtureRpc(dispatch,parties.getParty,identity,{partyId:id})).body.party,
    ()=>fixtureRpcError(dispatch,parties.getParty,tenantB,{partyId:id}));
  const account=await get("Account",async identity=>(await fixtureRpc(dispatch,accounts.getAccount,identity,{accountId})).body.account,
    ()=>fixtureRpcError(dispatch,accounts.getAccount,tenantB,{accountId}));
  await get("ContactPoint",async identity=>(await fixtureRpc(dispatch,contacts.getContactPoint,identity,{contactPointId:contactId})).body.contactPoint,
    ()=>fixtureRpcError(dispatch,contacts.getContactPoint,tenantB,{contactPointId:contactId}));
  await get("Relationship",async identity=>(await fixtureRpc(dispatch,relationships.getPartyRelationship,identity,{relationshipId:rMid})).body.relationship,
    ()=>fixtureRpcError(dispatch,relationships.getPartyRelationship,tenantB,{relationshipId:rMid}));
  const deal=await get("Deal",async identity=>(await fixtureRpc(dispatch,deals.getDeal,identity,{dealId})).body.deal,
    ()=>fixtureRpcError(dispatch,deals.getDeal,tenantB,{dealId}));
  const task=await get("Task",async identity=>(await fixtureRpc(dispatch,tasks.getTask,identity,{taskId:linkedTaskId})).body.task,
    ()=>fixtureRpcError(dispatch,tasks.getTask,tenantB,{taskId:linkedTaskId}));
  const overview=await verifyOverviewReadParity({dispatch,admin,viewer,tenantB,accountId});
  gets++;

  const page={pageSize:5};
  const list=async<T,>(label: string,read: (identity: string) => Promise<T>,length: (value: T) => number)=>{
    const fromAdmin=await read(admin);
    const fromViewer=await read(viewer);
    assert.ok(length(fromAdmin)>0,label+" fixture must exercise populated list");
    assert.deepEqual(fromViewer,fromAdmin,label+" must preserve all business fields, pagination and filters for viewer");
    lists++;
  };
  await list("Party",async identity=>(await fixtureRpc(dispatch,parties.listParties,identity,{page})).body,body=>body.parties.length);
  await list("Account",async identity=>(await fixtureRpc(dispatch,accounts.listAccounts,identity,{page})).body,body=>body.accounts.length);
  await list("ContactPoint",async identity=>(await fixtureRpc(dispatch,contacts.listContactPoints,identity,{partyId:id,page})).body,body=>body.contactPoints.length);
  await list("Relationship",async identity=>(await fixtureRpc(dispatch,relationships.listPartyRelationships,identity,{partyId:id,page})).body,body=>body.relationships.length);
  const {dealCapabilities:adminCapabilities,...adminBusiness}=(await fixtureRpc(dispatch,deals.listDeals,admin,{accountId,page})).body;
  const {dealCapabilities:viewerCapabilities,...viewerBusiness}=(await fixtureRpc(dispatch,deals.listDeals,viewer,{accountId,page})).body;
  assert.ok(adminBusiness.deals.length>0,"Deal fixture must exercise populated list");
  assert.deepEqual(viewerBusiness,adminBusiness,"Deal list business facts and pagination");
  assert.deepEqual(viewerCapabilities.map(x=>x.dealId),viewerBusiness.deals.map(x=>x.dealId));
  assert.deepEqual(adminCapabilities.map(x=>x.dealId),adminBusiness.deals.map(x=>x.dealId));
  assert.ok(adminCapabilities.every(x=>x.capabilities?.manageOpportunityTeam===true));
  assert.ok(viewerCapabilities.every(x=>x.capabilities?.manageOpportunityTeam===false));
  lists++;
  await list("Task",async identity=>(await fixtureRpc(dispatch,tasks.listTasks,identity,{accountId,page})).body,body=>body.tasks.length);

  assert.equal(party.partyId,id);
  assert.ok(account.parties.some(row=>row.partyId===id&&row.role===1));
  assert.equal(deal.accountId,account.accountId);
  assert.equal(deal.amount?.minorUnits,98765n);
  assert.equal(deal.amount?.currencyCode,"PLN");
  assert.equal(task.accountId,account.accountId);
  const {body}=await fixtureRpc(dispatch,deals.listDealEvents,viewer,{dealId,page:{pageSize:25}});
  assert.ok(body.events.some(event=>event.linkedTaskId===linkedTaskId),
    "Original Sales stage event must carry the linked Activities Task ID");
  assert.equal(overview.account?.accountId,account.accountId);
  assert.equal(overview.primaryParty?.partyId,party.partyId);
  assert.ok(overview.deals.some(row=>row.dealId===deal.dealId));
  assert.ok(overview.tasks.some(row=>row.taskId===linkedTaskId));
  assert.equal(await audit(),before,"M1 business reads and denied tenant lookups must not write audit");
  console.log(JSON.stringify({event:"crm.native-m1-read-parity-linked-flow",result:"PASS",
    signed_admin_and_viewer:true,full_wire_fields_equal:true,tenant_b_not_found:true,
    get_owners:gets,list_owners:lists,
    party_account_deal_sales_event_task_customer360:true,read_only:true}));
}

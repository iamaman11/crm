import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {AccountService} from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import {PartyKind,PartyService} from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import {IdentityResolutionService} from "../../../../packages/client/gen/crm/identity_resolution/v2/identity_pb.js";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {LeadService,LeadStatus} from "../../../../packages/client/gen/crm/sales/v2/lead_pb.js";
import {fixtureRpc,fixtureRpcError,fixtureRpcRace} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";

type Fixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string,unknown>>>;
  admin: string; viewer: string; tenantB: string;
};

const precondition=(expectedVersion?:bigint)=>({idempotencyKey:randomUUID(),expectedVersion});

export async function verifySalesLeads({dispatch,sql,admin,viewer,tenantB}:Fixture) {
  const partyId=randomUUID(),accountId=randomUUID(),unrelatedAccountId=randomUUID();
  const unrelatedPartyId=randomUUID();
  for (const [id, name] of [[partyId,"Lead party"],[unrelatedPartyId,"Unrelated Lead party"]] as const) {
    assert.equal((await fixtureRpc(dispatch,PartyService.method.createParty,admin,{
      partyId:id,kind:PartyKind.ORGANIZATION,displayName:name,precondition:precondition(),
    })).status,200);
  }
  for (const [id, owner] of [[accountId,partyId],[unrelatedAccountId,unrelatedPartyId]] as const) {
    assert.equal((await fixtureRpc(dispatch,AccountService.method.createAccount,admin,{
      accountId:id,primaryPartyId:owner,name:"Lead account "+id,precondition:precondition(),
    })).status,200);
  }
  const lead=LeadService.method;
  const leadId=randomUUID(),rejectedId=randomUUID();
  const create={leadId,partyId,name:"Qualified referral",precondition:precondition()};
  assert.equal((await fixtureRpcError(dispatch,lead.createLead,viewer,create)).status,403);
  assert.equal((await fixtureRpcError(dispatch,lead.createLead,tenantB,create)).status,400);
  const created=await fixtureRpc(dispatch,lead.createLead,admin,create);
  assert.equal(created.body.lead?.status,LeadStatus.NEW);
  assert.equal((await fixtureRpc(dispatch,lead.createLead,admin,create)).body.receipt?.replayed,true);
  assert.equal((await fixtureRpcError(dispatch,lead.getLead,tenantB,{leadId})).status,404);
  const adminRead=(await fixtureRpc(dispatch,lead.getLead,admin,{leadId})).body.entry;
  const viewerRead=(await fixtureRpc(dispatch,lead.getLead,viewer,{leadId})).body.entry;
  assert.deepEqual(viewerRead?.lead,adminRead?.lead);
  assert.equal(adminRead?.capabilities?.qualify,true);
  assert.equal(viewerRead?.capabilities?.qualify,false);
  assert.equal((await fixtureRpc(dispatch,lead.listLeads,viewer,{})).body.canCreate,false);
  assert.deepEqual((await fixtureRpc(dispatch,lead.listLeads,tenantB,{})).body.entries,[]);

  const deal={dealId:randomUUID(),name:"Deal from referral",accountId,stage:{
    pipelineId:"pipeline.enterprise",stageId:"lead",
  }};
  assert.equal((await fixtureRpcError(dispatch,lead.convertLead,admin,{
    leadId,deal,precondition:precondition(1n),
  })).status,409);
  assert.equal((await fixtureRpcError(dispatch,lead.qualifyLead,admin,{
    leadId,reason:" ",precondition:precondition(1n),
  })).status,400);
  const decision={leadId,reason:"Confirmed need",precondition:precondition(1n)};
  const qualified=await fixtureRpc(dispatch,lead.qualifyLead,admin,decision);
  assert.equal(qualified.body.lead?.status,LeadStatus.QUALIFIED);
  assert.equal((await fixtureRpc(dispatch,lead.qualifyLead,admin,decision)).body.receipt?.replayed,true);

  // Reject stays terminal and cannot add a Deal or edit its prior decision.
  assert.equal((await fixtureRpc(dispatch,lead.createLead,admin,{
    ...create,leadId:rejectedId,precondition:precondition(),
  })).status,200);
  const denied=await fixtureRpc(dispatch,lead.rejectLead,admin,{
    leadId:rejectedId,reason:"Unrelated request",precondition:precondition(1n),
  });
  assert.equal(denied.body.lead?.status,LeadStatus.REJECTED);
  assert.equal((await fixtureRpcError(dispatch,lead.convertLead,admin,{
    leadId:rejectedId,deal,precondition:precondition(2n),
  })).status,409);

  assert.equal((await fixtureRpcError(dispatch,lead.convertLead,admin,{
    leadId,deal:{...deal,accountId:unrelatedAccountId},precondition:precondition(2n),
  })).status,400);
  assert.equal((await fixtureRpcError(dispatch,lead.convertLead,admin,{
    leadId,deal:{...deal,stage:{...deal.stage,stageId:"invalid-stage"}},precondition:precondition(2n),
  })).status,400);
  assert.equal((await fixtureRpcError(dispatch,DealService.method.getDeal,viewer,{
    dealId:deal.dealId,
  })).status,404);
  assert.equal((await fixtureRpc(dispatch,lead.getLead,viewer,{leadId})).body.entry?.lead?.version,2n);

  const merge=await fixtureRpc(dispatch,IdentityResolutionService.method.mergeParty,admin,{
    operationId:randomUUID(),sourcePartyId:partyId,sourcePartyVersion:1n,
    survivorPartyId:unrelatedPartyId,survivorPartyVersion:1n,reason:"manual_identity_review",
    precondition:precondition(),
  });
  assert.ok(merge.body.receipt);
  const canonicalDeal={...deal,accountId:unrelatedAccountId};
  const first={leadId,deal:canonicalDeal,precondition:precondition(2n)};

  // Abort the Lead receipt trigger after the Deal insert was attempted.
  // Both records and both audits must roll back.
  await sql("CREATE TRIGGER fixture_lead_rollback BEFORE INSERT ON crm_sales_lead_events " +
    "WHEN NEW.status='converted' BEGIN SELECT RAISE(ABORT,'fixture_rollback'); END");
  try {
    assert.equal((await fixtureRpcError(dispatch,lead.convertLead,admin,first)).status,503);
    assert.equal((await fixtureRpcError(dispatch,DealService.method.getDeal,viewer,{
      dealId:deal.dealId,
    })).status,404);
    assert.equal((await fixtureRpc(dispatch,lead.getLead,viewer,{leadId})).body.entry?.lead?.version,2n);
    assert.deepEqual(await sql("SELECT id FROM crm_audit WHERE tenant_id=? AND subject_id=?",
      "tenant-a",deal.dealId),[]);
  } finally {
    await sql("DROP TRIGGER fixture_lead_rollback");
  }

  // Two distinct conversions compete for the same Lead CAS. A retry uses
  // the winning key and returns its original linked Deal and provenance.
  const second={leadId,deal:{...canonicalDeal,dealId:randomUUID()},precondition:precondition(2n)};
  const results=await Promise.all([first,second].map(async input=>({
    input,result:await fixtureRpcRace(dispatch,lead.convertLead,admin,input),
  })));
  assert.deepEqual(results.map(entry=>entry.result.status).sort(),[200,409]);
  const winner=results.find(entry=>entry.result.status===200);
  assert.ok(winner);
  const converted=winner.result.body?.lead;
  assert.equal(converted?.dealId,winner.input.deal.dealId);
  assert.equal(converted?.originalPartyId,partyId);
  assert.equal(converted?.status,LeadStatus.CONVERTED);
  const replay=await fixtureRpc(dispatch,lead.convertLead,admin,winner.input);
  assert.equal(replay.body.receipt?.replayed,true);
  assert.deepEqual(replay.body.lead,converted);
  const receipt=(await fixtureRpc(dispatch,lead.getLead,viewer,{
    leadId,receiptIdempotencyKey:winner.input.precondition.idempotencyKey,
  })).body.confirmedMutation;
  assert.equal(receipt?.originalRequestId,winner.result.headers.get("x-request-id"));
  const events=(await fixtureRpc(dispatch,lead.listLeadEvents,viewer,{leadId})).body.events;
  assert.deepEqual(events.map(event=>event.snapshot?.status),[LeadStatus.NEW,LeadStatus.QUALIFIED,LeadStatus.CONVERTED]);
  assert.equal((await fixtureRpcError(dispatch,lead.listLeadEvents,tenantB,{leadId})).status,404);

  const linked=await sql("SELECT id FROM crm_sales_deal_records WHERE tenant=? AND source_lead_id=?","tenant-a",leadId);
  assert.deepEqual(linked,[{id:winner.input.deal.dealId}]);
  const proof=await sql(
    "SELECT action FROM crm_audit WHERE tenant_id=? AND request_id=? ORDER BY action",
    "tenant-a",receipt?.originalRequestId??"",
  );
  assert.deepEqual(proof,[{action:"sales_deals.create"},{action:"sales_leads.convert"}]);
}

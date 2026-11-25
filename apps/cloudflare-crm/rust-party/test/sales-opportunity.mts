import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {DealService} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import type {DealCapabilities, OpportunityTeamRole} from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import {MemberService} from "../../../../packages/client/gen/crm/access/v2/member_pb.js";
import {binarySalesV2,salesV2Path} from "../../../../packages/client/test/cloudflare-binary-rpc-fixture.ts";
import {fixtureRpc,fixtureRpcError} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {RpcDispatch} from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type {dealCreationInput} from "../../../../packages/client/test/cloudflare-deal-fixture.ts";
import {formatRpcFailure} from "../../scripts/rpc-error-response.mts";

type BindingValue = string | number | null | ArrayBuffer;
type FixtureResult = {success: boolean; results: Record<string,unknown>[]};
type Statement = {bind(...values: BindingValue[]): Statement; all(): Promise<FixtureResult>};
type OpportunityFixture = {
  dispatch: RpcDispatch;
  sql: (statement: string, ...bindings: BindingValue[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
  statement: (query: string, ...bindings: BindingValue[]) => Statement;
  batch: (statements: Statement[]) => Promise<FixtureResult[]>;
  admin: string; viewer: string; tenantB: string; saleCreate: ReturnType<typeof dealCreationInput>;
};
export async function verifySalesOpportunity({dispatch,sql,statement,batch,admin,viewer,tenantB,saleCreate}: OpportunityFixture) {
  const sales=DealService.method;
  const raw=(path: string,identity: string,body: Uint8Array)=>dispatch(path,{
    method:"POST",headers:{"content-type":"application/proto","connect-protocol-version":"1",
      "cf-access-jwt-assertion":identity},body:new Uint8Array(body),
  });
  const id="opportunity-team-qa-"+randomUUID();
  const email="native-viewer@example.test";
  const creditTeamId="opportunity-credit-team-"+randomUUID();
  const creditTeamKey="opportunity-credit-team-"+creditTeamId;
  const creditTeam=await fixtureRpc(dispatch,MemberService.method.createTeam,admin,{
    teamId:creditTeamId,name:"Opportunity credit QA "+creditTeamId,
    roster:{memberEmails:["native-a@example.test"]},
    precondition:{idempotencyKey:creditTeamKey}
  });
  assert.equal(creditTeam.status,200,formatRpcFailure(creditTeam));
  const createQaDeal=async (dealId: string)=>fixtureRpc(dispatch,sales.createDeal,admin,{
    ...saleCreate,dealId,
    owner:{owner:{case:"assigneeIdentity",value:"native-a@example.test"}},
    precondition:{idempotencyKey:"opportunity-create-"+dealId}
  });
  const grant=(dealId: string,role: OpportunityTeamRole | undefined,version: bigint,key: string)=>({dealId,email,...(role===undefined?{}:{role}),
    precondition:{expectedVersion:version,idempotencyKey:"opportunity-"+key+"-"+dealId}});
  const edit=(dealId: string,version: bigint,key: string,fields: {probabilityBasisPoints?: number; owner?: {owner: {case: "assigneeIdentity"; value: string}}}={})=>({dealId,
    name:{operation:{case:"set" as const,value:"Opportunity team "+key}},...fields,
    precondition:{expectedVersion:version,idempotencyKey:"opportunity-edit-"+key+"-"+dealId}});
  const closeInput=(dealId: string,version: bigint,key: string,fields: {creditSelection?: {leaveTeamUnallocated?: boolean}; expectedCreditDecision?: string | undefined}={})=>({dealId,outcome:3 as const,reasonCode:"qa",
    ...fields,precondition:{expectedVersion:version,idempotencyKey:"opportunity-close-"+key+"-"+dealId}});
  const created=await createQaDeal(id);
  assert.equal(created.status,200,formatRpcFailure(created));
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,edit(id,1n,"denied"))).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.setOpportunityParticipant,viewer,grant(id,3,1n,"escalate"))).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.setOpportunityParticipant,admin,grant(id,0,1n,"unknown"))).status,400);
  assert.equal((await fixtureRpcError(dispatch,sales.setOpportunityParticipant,admin,{...grant(id,3,1n,"foreign"),email:"native-b@example.test"})).status,400);

  const observer=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(id,1,1n,"observer"));
  assert.equal(observer.status,200,formatRpcFailure(observer));
  assert.equal(observer.body.deal?.opportunityTeam[0]?.role,1);
  assert.equal((await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(id,1,1n,"observer"))).body.receipt?.replayed,true);
  assert.equal((await fixtureRpcError(dispatch,sales.setOpportunityParticipant,admin,grant(id,3,1n,"stale"))).status,409);
  const assertProjection=async(actor: string,expected: DealCapabilities | undefined)=>{
    const details=await fixtureRpc(dispatch,sales.getDeal,actor,{dealId:id});
    assert.equal(details.status,200,formatRpcFailure(details));
    assert.deepEqual(details.body.capabilities,expected);
    const list=await fixtureRpc(dispatch,sales.listDeals,actor,{search:id,page:{pageSize:25}});
    assert.equal(list.status,200,formatRpcFailure(list));
    assert.deepEqual(list.body.deals,[details.body.deal]);
    assert.deepEqual(list.body.dealCapabilities.map(x=>x.dealId),[id]);
    assert.deepEqual(list.body.dealCapabilities[0]?.capabilities,expected);
    assert.ok(details.body.deal?.stage);
    const stage=await fixtureRpc(dispatch,sales.listStageDeals,actor,{
      pipelineId:details.body.deal.stage.pipelineId,stageId:details.body.deal.stage.stageId,page:{pageSize:100}
    });
    assert.equal(stage.status,200,formatRpcFailure(stage));
    assert.deepEqual(stage.body.dealCapabilities.map(x=>x.dealId),stage.body.deals.map(x=>x.dealId));
    assert.deepEqual(stage.body.dealCapabilities.find(x=>x.dealId===id)?.capabilities,expected);
  };
  const observerRead=(await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:id})).body;
  await assertProjection(viewer,observerRead.capabilities);
  assert.equal(observerRead.capabilities?.close,false);
  assert.equal(observerRead.capabilities?.manageCloseCredit,false);
  assert.ok(observer.body.deal);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeInput(id,observer.body.deal.version,"observer"))).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,edit(id,2n,"observer"))).status,403);

  const contributor=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(id,2,2n,"contributor"));
  assert.equal(contributor.status,200,formatRpcFailure(contributor));
  const contributorRead=(await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:id})).body;
  await assertProjection(viewer,contributorRead.capabilities);
  assert.equal(contributorRead.capabilities?.edit,true);
  assert.equal(contributorRead.capabilities?.editCommercial,false);
  assert.equal(contributorRead.capabilities?.advance,false);
  assert.equal(contributorRead.capabilities?.close,false);
  assert.equal(contributorRead.capabilities?.manageOpportunityTeam,false);
  assert.ok(contributor.body.deal);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeInput(id,contributor.body.deal.version,"contributor"))).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,edit(id,3n,"money",{probabilityBasisPoints:1200}))).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,edit(id,3n,"owner",{owner:{owner:{case:"assigneeIdentity",value:email}}}))).status,403);
  const command=edit(id,3n,"allowed");
  const edited=await fixtureRpc(dispatch,sales.updateDeal,viewer,command);
  assert.equal(edited.status,200,formatRpcFailure(edited));
  assert.equal(edited.body.deal?.version,4n);
  assert.equal((await fixtureRpc(dispatch,sales.updateDeal,viewer,command)).body.receipt?.replayed,true);
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,{...command,dealId:"another-deal"})).status,403);

  const lead=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(id,3,4n,"lead"));
  assert.equal(lead.status,200,formatRpcFailure(lead));
  const leadRead=(await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:id})).body;
  await assertProjection(viewer,leadRead.capabilities);
  assert.equal(leadRead.capabilities?.advance,true);
  assert.equal(leadRead.capabilities?.close,true);
  assert.equal(leadRead.capabilities?.manageCloseCredit,false);
  const adminRead=(await fixtureRpc(dispatch,sales.getDeal,admin,{dealId:id})).body;
  await assertProjection(admin,adminRead.capabilities);
  assert.equal(adminRead.capabilities?.close,true);
  assert.equal(adminRead.capabilities?.manageCloseCredit,true);
  const commercial=await fixtureRpc(dispatch,sales.updateDeal,viewer,edit(id,5n,"commercial",{probabilityBasisPoints:3300}));
  assert.equal(commercial.status,200,formatRpcFailure(commercial));
  assert.equal(commercial.body.deal?.probabilityBasisPoints,3300);

  const automaticSelection={};
  const automaticPreview=await fixtureRpc(dispatch,sales.previewDealCloseCredit,viewer,{
    dealId:id,expectedVersion:6n,selection:automaticSelection
  });
  assert.equal(automaticPreview.status,200,formatRpcFailure(automaticPreview));
  assert.ok(automaticPreview.body.eligibleTeams.some(team=>team.id===creditTeamId));
  assert.equal(automaticPreview.body.creditedTeam?.id,creditTeamId);
  const manualSelection={leaveTeamUnallocated:true};
  const manualPreview=await fixtureRpc(dispatch,sales.previewDealCloseCredit,viewer,{
    dealId:id,expectedVersion:6n,selection:manualSelection
  });
  assert.equal(manualPreview.status,200,formatRpcFailure(manualPreview));
  assert.equal(manualPreview.body.creditedTeam,undefined);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeInput(id,6n,"manual-denied",{
    creditSelection:manualSelection
  }))).status,403);

  const leadCloseInput=closeInput(id,6n,"lead",{
    creditSelection:automaticSelection,
    expectedCreditDecision:automaticPreview.body.expectedDecision
  });
  const leadClose=await fixtureRpc(dispatch,sales.closeDeal,viewer,leadCloseInput);
  assert.equal(leadClose.status,200,formatRpcFailure(leadClose));
  assert.equal(leadClose.body.deal?.version,7n);
  assert.equal(leadClose.body.deal?.status,3);
  assert.equal(leadClose.body.deal?.commercialCloseFact?.creditedTeam?.id,creditTeamId);
  assert.equal(leadClose.body.receipt?.replayed,false);
  assert.equal((await fixtureRpc(dispatch,sales.closeDeal,viewer,leadCloseInput)).body.receipt?.replayed,true);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,admin,leadCloseInput)).status,409,
    "an administrator must not replay a Lead-scoped close receipt");
  assert.equal((await fixtureRpcError(dispatch,sales.reopenDeal,viewer,{
    dealId:id,reasonCode:"qa.reopen",
    precondition:{expectedVersion:7n,idempotencyKey:"opportunity-reopen-denied-"+id}
  })).status,403);
  const reopened=await fixtureRpc(dispatch,sales.reopenDeal,admin,{
    dealId:id,reasonCode:"qa.reopen",
    precondition:{expectedVersion:7n,idempotencyKey:"opportunity-reopen-"+id}
  });
  assert.equal(reopened.status,200,formatRpcFailure(reopened));
  assert.equal(reopened.body.deal?.version,8n);
  assert.equal(reopened.body.deal?.status,1);

  const adminManualPreview=await fixtureRpc(dispatch,sales.previewDealCloseCredit,admin,{
    dealId:id,expectedVersion:8n,selection:manualSelection
  });
  assert.equal(adminManualPreview.status,200,formatRpcFailure(adminManualPreview));
  assert.equal(adminManualPreview.body.creditedTeam,undefined);
  const adminManualClose=await fixtureRpc(dispatch,sales.closeDeal,admin,closeInput(id,8n,"admin-manual",{
    creditSelection:manualSelection,
    expectedCreditDecision:adminManualPreview.body.expectedDecision
  }));
  assert.equal(adminManualClose.status,200,formatRpcFailure(adminManualClose));
  assert.equal(adminManualClose.body.deal?.version,9n);
  assert.equal(adminManualClose.body.deal?.commercialCloseFact?.creditedTeam,undefined);
  const revoke=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(id,undefined,9n,"revoke"));
  assert.equal(revoke.status,200,formatRpcFailure(revoke));
  assert.equal(revoke.body.deal?.opportunityTeam.length,0);
  await assertProjection(viewer,(await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:id})).body.capabilities);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,leadCloseInput)).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,command)).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.updateDeal,viewer,edit(id,10n,"revoked"))).status,403);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,tenantB,closeInput(id,9n,"foreign-tenant"))).status,404);
  const events=(await fixtureRpc(dispatch,sales.listDealEvents,viewer,{dealId:id,page:{pageSize:25}})).body.events;
  assert.deepEqual(events.filter(e=>e.opportunityTeamAfter).map(e=>e.opportunityTeamAfter?.participants.map(m=>m.role)),[[],[3],[2],[1]]);
  assert.equal(events.filter(e=>e.kind===4).length,2);
  assert.equal(events.filter(e=>e.kind===5).length,1);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.opportunity_team'","tenant-a",id))[0]?.n,4);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.close'","tenant-a",id))[0]?.n,2);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.reopen'","tenant-a",id))[0]?.n,1);

  // An ungranted Deal and a revoked grant cannot use the SelfWrite transport.
  const ungrantedId="opportunity-ungranted-"+randomUUID();
  const ungranted=await createQaDeal(ungrantedId);
  assert.equal(ungranted.status,200,formatRpcFailure(ungranted));
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeInput(ungrantedId,1n,"ungranted"))).status,403);
  const revokedId="opportunity-revoked-"+randomUUID();
  const revoked=await createQaDeal(revokedId);
  assert.equal(revoked.status,200,formatRpcFailure(revoked));
  const revokedGrant=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(revokedId,3,1n,"revoked-grant"));
  assert.equal(revokedGrant.status,200,formatRpcFailure(revokedGrant));
  const revokedRemoval=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(revokedId,undefined,2n,"revoked-removal"));
  assert.equal(revokedRemoval.status,200,formatRpcFailure(revokedRemoval));
  const revokedCloseKey="opportunity-close-revoked-"+revokedId;
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeInput(revokedId,3n,"revoked"))).status,403);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",revokedCloseKey))[0]?.n,0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.close'","tenant-a",revokedId))[0]?.n,0);

  // Enrollment version is pinned in the grant; reactivation cannot restore it.
  const versionId="opportunity-member-version-"+randomUUID();
  const versionDeal=await createQaDeal(versionId);
  assert.equal(versionDeal.status,200,formatRpcFailure(versionDeal));
  const versionGrant=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(versionId,3,1n,"version-grant"));
  assert.equal(versionGrant.status,200,formatRpcFailure(versionGrant));
  const memberRow=(await sql("SELECT version FROM crm_users WHERE tenant_id=? AND email=?","tenant-a",email))[0];
  const originalMemberVersion=Number(memberRow?.version);
  assert.ok(Number.isSafeInteger(originalMemberVersion)&&originalMemberVersion>0);
  await sql("UPDATE crm_users SET active=0,version=version+1 WHERE tenant_id=? AND email=?","tenant-a",email);
  await sql("UPDATE crm_users SET active=1,version=version+1 WHERE tenant_id=? AND email=?","tenant-a",email);
  const staleMemberRead=(await fixtureRpc(dispatch,sales.getDeal,viewer,{dealId:versionId})).body;
  assert.equal(staleMemberRead.capabilities?.close,false);
  assert.equal((await fixtureRpcError(dispatch,sales.closeDeal,viewer,closeInput(versionId,2n,"stale-member"))).status,403);
  const staleGrantRemoval=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(versionId,undefined,2n,"stale-member-revoke"));
  assert.equal(staleGrantRemoval.status,200,formatRpcFailure(staleGrantRemoval));
  await sql("UPDATE crm_users SET version=?,active=1 WHERE tenant_id=? AND email=?",originalMemberVersion,"tenant-a",email);

  // A concurrent CloseDeal and grant revocation contend on the same original
  // Deal version and the same version-pinned D1 predicate. Exactly one wins.
  const raceId="opportunity-close-revoke-race-"+randomUUID();
  const raceDeal=await createQaDeal(raceId);
  assert.equal(raceDeal.status,200,formatRpcFailure(raceDeal));
  const raceGrant=await fixtureRpc(dispatch,sales.setOpportunityParticipant,admin,grant(raceId,3,1n,"race-grant"));
  assert.equal(raceGrant.status,200,formatRpcFailure(raceGrant));
  const raceCloseKey="opportunity-close-race-"+raceId;
  const [raceClose,raceRevoke]=await Promise.all([
    raw(salesV2Path("CloseDeal"),viewer,binarySalesV2("CloseDeal",closeInput(raceId,2n,"race"))),
    raw(salesV2Path("SetOpportunityParticipant"),admin,binarySalesV2("SetOpportunityParticipant",grant(raceId,undefined,2n,"race-revoke")))
  ]);
  const closeWon=raceClose.status===200;
  const revokeWon=raceRevoke.status===200;
  assert.notEqual(closeWon,revokeWon,"close and revocation must not both commit or both lose");
  if(closeWon) assert.equal(raceRevoke.status,409);
  else assert.ok([403,409].includes(raceClose.status));
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",raceCloseKey))[0]?.n,closeWon?1:0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND action='closed'","tenant-a",raceId))[0]?.n,closeWon?1:0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.close'","tenant-a",raceId))[0]?.n,closeWon?1:0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.opportunity_team'","tenant-a",raceId))[0]?.n,revokeWon?2:1);

  // Remove only these deterministic QA records after every assertion.
  const qaIds=[id,ungrantedId,revokedId,versionId,raceId];
  await batch(qaIds.flatMap(dealId=>[
    statement("DELETE FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?","tenant-a",dealId),
    statement("DELETE FROM crm_sales_deal_events WHERE tenant=? AND deal_id=?","tenant-a",dealId),
    statement("DELETE FROM crm_sales_deal_receipts WHERE tenant=? AND json_extract(state,'$.deal_id')=?","tenant-a",dealId),
    statement("DELETE FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",dealId),
    statement("DELETE FROM crm_sales_deal_records WHERE tenant=? AND id=?","tenant-a",dealId)
  ]));
  await batch([
    statement("DELETE FROM crm_team_receipts WHERE tenant=? AND key=?","tenant-a",creditTeamKey),
    statement("DELETE FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",creditTeamId),
    statement("DELETE FROM crm_access_teams WHERE tenant=? AND id=?","tenant-a",creditTeamId)
  ]);
  console.log(JSON.stringify({event:"crm.native-opportunity-team-close",result:"PASS",
    explicit_roles:true,lead_close:true,automatic_credit:true,admin_manual_credit:true,
    actor_scoped_receipt:true,revocation:true,member_version_pin:true,concurrent_cas:true,
    reopen_admin_only:true,tenant_isolation:true,history_audit_receipts:true}));
}

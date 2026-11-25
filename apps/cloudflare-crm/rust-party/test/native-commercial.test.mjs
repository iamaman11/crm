// Isolated commercial execution and automation; no browser or earlier suite state.
import assert from "node:assert/strict";
import {test} from "node:test";
import {randomUUID,createHash} from "node:crypto";
import {binaryPartyV3,decodePartyV3,partyV3Path,
 binaryAccountV2,decodeAccountV2,accountV2Path,
 binaryMemberV2,decodeMemberV2,memberV2Path,
 binaryTaskV2,decodeTaskV2,taskV2Path,
 binarySalesV2,decodeSalesV2,salesV2Path,
 binaryInteractionV2,decodeInteractionV2,interactionV2Path,
 binaryAutomationV2,decodeAutomationV2,automationV2Path} from "../../../../scripts/crm-connect-binary-fixture.mjs";
import {withNativeHarness,signedRpc,writeRuntimePart} from "./native.mjs";

test("commercial ownership, routing, analytics and automation on fresh D1",async context=>{
 const evidence=await withNativeHarness(context,async({sql,statement,batch,rpcFetch,assertUnconfirmed,jwt,admin,viewer,businessEvidence})=>{
 const tenantB=jwt("native-b@example.test");
 const describeProtobuf=value=>JSON.stringify(value,(_key,item)=>typeof item==="bigint"?item.toString():item);
 const v3=signedRpc(rpcFetch,partyV3Path,binaryPartyV3,decodePartyV3);
 const accountV2=signedRpc(rpcFetch,accountV2Path,binaryAccountV2,decodeAccountV2);
 const memberV2=signedRpc(rpcFetch,memberV2Path,binaryMemberV2,decodeMemberV2);
 const taskV2=signedRpc(rpcFetch,taskV2Path,binaryTaskV2,decodeTaskV2);
 const salesV2=signedRpc(rpcFetch,salesV2Path,binarySalesV2,decodeSalesV2);
 const interactionV2=signedRpc(rpcFetch,interactionV2Path,binaryInteractionV2,decodeInteractionV2);
 // Minimal prerequisites through the existing typed owners, not copied D1 rows.
 const partyId="commercial-party-"+randomUUID(),accountId="commercial-account-"+randomUUID();
 assert.equal((await v3("CreateParty",admin,{partyId,kind:2,displayName:"Commercial QA",precondition:{idempotencyKey:partyId}})).status,200);
 assert.equal((await accountV2("CreateAccount",admin,{accountId,name:"Commercial QA",primaryPartyId:partyId,precondition:{idempotencyKey:accountId}})).status,200);
const pipelineSeed={pipelineId:"pipeline.enterprise",name:"Enterprise",
  stages:[
    {stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
    {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500},
    {stageId:"won",name:"Historical terminal",probabilityBasisPoints:10000}
  ],precondition:{idempotencyKey:"connect-r7-pipeline-create-1"}};
 assert.equal((await salesV2("CreatePipeline",admin,pipelineSeed)).status,200);
 // SK-07.C: Sales owner shares the Access team, original Deal state and
 // one server-side filter across list, board totals, saved views and forecast.
 {
   const salesTeam="sk07-sales-team-"+randomUUID();
   const salesTeamCreated=await memberV2("CreateTeam",admin,{
     teamId:salesTeam,name:"Sales owner QA "+salesTeam,roster:{memberEmails:["native-viewer@example.test"]},
     precondition:{idempotencyKey:"team-"+salesTeam}
   });
   assert.equal(salesTeamCreated.status,200,describeProtobuf(salesTeamCreated.body));
   const ownerFilter={owner:{case:"teamId",value:salesTeam}};
   const actorOwner={owner:{case:"assigneeIdentity",value:"native-viewer@example.test"}};
   const pId="sk07-sales-owner-"+randomUUID();
   const pMade=await salesV2("CreatePipeline",admin,{pipelineId:pId,name:"Owner filter QA "+pId,
     stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:5000}],
     precondition:{idempotencyKey:"pipeline-"+pId}});
   assert.equal(pMade.status,200,describeProtobuf(pMade.body));
   const input=(id,owner,currency,units)=>({dealId:id,name:"Sales owner QA "+id,accountId,
     stage:{pipelineId:pId,stageId:"lead",ordinal:1},owner,amount:{currencyCode:currency,minorUnits:units},
     precondition:{idempotencyKey:"owner-create-"+id}});
   const ids=["sk07-owner-a-"+randomUUID(),"sk07-owner-b-"+randomUUID(),"sk07-owner-c-"+randomUUID()];
   const inputs=[input(ids[0],ownerFilter,"EUR",10001n),input(ids[1],ownerFilter,"USD",20000n),input(ids[2],actorOwner,"EUR",40000n)];
   for(const request of inputs){
     const result=await salesV2("CreateDeal",admin,request);
     assert.equal(result.status,200,describeProtobuf(result.body));
     assert.equal(result.body.deal?.owner?.owner.case,request.owner.owner.case);
     assert.equal(result.body.deal?.owner?.owner.value,request.owner.owner.value);
   }
   assert.equal((await salesV2("CreateDeal",admin,inputs[0])).body.receipt?.replayed,true);
   const missing=input("sk07-owner-bad-"+randomUUID(),{owner:{case:"teamId",value:"missing-team"}},"EUR",1n);
   assert.equal((await salesV2("CreateDeal",admin,missing)).status,400);
   assert.equal((await salesV2("CreateDeal",viewer,{...inputs[0],dealId:"sk07-owner-denied-"+randomUUID()})).status,403);
   const list=await salesV2("ListDeals",viewer,{ownerFilter,pipelineId:pId,page:{pageSize:1}});
   assert.equal(list.status,200,describeProtobuf(list.body));
   assert.equal(list.body.deals[0]?.dealId,ids[0]);
   const next=await salesV2("ListDeals",viewer,{ownerFilter,pipelineId:pId,page:{pageSize:1,pageToken:list.body.pageInfo.nextPageToken}});
   assert.equal(next.body.deals[0]?.dealId,ids[1]);
   assert.equal((await salesV2("ListDeals",tenantB,{ownerFilter})).body.deals.length,0);
   const board=await salesV2("ListStageDeals",viewer,{pipelineId:pId,stageId:"lead",ownerFilter,page:{pageSize:1}});
   assert.equal(board.body.totalCount,2n);
   assert.deepEqual(board.body.currencyTotals.map(x=>[x.currencyCode,x.totalMinorUnits]),[["EUR","10001"],["USD","20000"]]);
   const metrics=await salesV2("GetPipelineMetrics",viewer,{pipelineId:pId,ownerFilter});
   assert.equal(metrics.status,200,describeProtobuf(metrics.body));
   assert.equal(metrics.body.totalDealCount,2n);
   assert.deepEqual(metrics.body.forecastCurrencies.map(x=>[x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints]),
     [["EUR","10001","50005000"],["USD","20000","100000000"]]);
   const viewId="sk07-owner-view-"+randomUUID();
   const viewCreated=await salesV2("CreateSalesView",admin,{viewId,name:"Team owner view",pipelineId:pId,ownerFilter,
     precondition:{idempotencyKey:"view-"+viewId}});
   assert.equal(viewCreated.status,200,describeProtobuf(viewCreated.body));
   const viewRead=await salesV2("GetSalesView",viewer,{viewId});
   assert.equal(viewRead.body.view?.ownerFilter?.owner.value,salesTeam);
   const noChange={teamId:salesTeam,active:true,precondition:{expectedVersion:1n,idempotencyKey:"noop-"+salesTeam}};
   assert.equal((await memberV2("UpdateTeam",admin,noChange)).status,400);
   const archive={teamId:salesTeam,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+salesTeam}};
   assert.equal((await memberV2("UpdateTeam",admin,archive)).status,409);
   for(const id of ids.slice(0,2)){
     const change={dealId:id,owner:actorOwner,precondition:{expectedVersion:1n,idempotencyKey:"owner-update-"+id}};
     assert.equal((await salesV2("UpdateDeal",viewer,change)).status,403);
     const result=await salesV2("UpdateDeal",admin,change);
     assert.equal(result.status,200,describeProtobuf(result.body));
     assert.equal(result.body.deal?.owner?.owner.value,"native-viewer@example.test");
     assert.equal((await salesV2("UpdateDeal",admin,change)).body.receipt?.replayed,true);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",id))[0].n,2);
   }
   assert.equal((await memberV2("UpdateTeam",admin,archive)).status,200);
   assert.equal((await salesV2("GetPipelineMetrics",viewer,{pipelineId:pId,ownerFilter})).body.totalDealCount,0n);
   assert.equal((await salesV2("GetPipelineMetrics",viewer,{pipelineId:pId,ownerFilter:actorOwner})).body.totalDealCount,3n);
   console.log(JSON.stringify({event:"crm.native-sk07c-sales-owner",result:"PASS",typed_owner:true,tenant_ab:true,
     list_before_limit:true,exact_currency:true,forecast:true,saved_view:true,archive_guard:true,replay:true}));
   // SK-07.C4: no last-human removal/revocation can strand an open pool.
   const suffix=randomUUID(),lastEmail="qa-last-"+suffix+"@example.test",lastTeam="sk07-last-"+suffix;
   const lastJwt=jwt(lastEmail);
   assert.equal((await memberV2("ProvisionMember",admin,{email:lastEmail,role:1,
     precondition:{idempotencyKey:"member-"+suffix}})).status,200);
   assert.equal((await memberV2("CreateTeam",admin,{teamId:lastTeam,name:"Last member QA "+suffix,
     roster:{memberEmails:[lastEmail]},precondition:{idempotencyKey:"team-"+suffix}})).status,200);
   const lastOwner={owner:{case:"teamId",value:lastTeam}};
   const lastTask="sk07-last-task-"+suffix,lastDeal="sk07-last-deal-"+suffix;
   assert.equal((await taskV2("CreateTask",admin,{taskId:lastTask,accountId,subject:"Last member QA",priority:2,
     owner:lastOwner,precondition:{idempotencyKey:"task-"+suffix}})).status,200);
   const empty={teamId:lastTeam,roster:{memberEmails:[]},precondition:{expectedVersion:1n,idempotencyKey:"empty-"+suffix}};
   const deactivate={email:lastEmail,active:false,precondition:{expectedVersion:1n,idempotencyKey:"revoke-"+suffix}};
   assert.equal((await memberV2("UpdateTeam",admin,empty)).status,409);
   assert.equal((await memberV2("UpdateMember",admin,deactivate)).status,409);
   assert.equal((await salesV2("CreateDeal",admin,input(lastDeal,lastOwner,"EUR",12500n))).status,200);
   const claim={taskId:lastTask,precondition:{expectedVersion:1n,idempotencyKey:"claim-"+suffix}};
   const [claimed,revoked]=await Promise.all([taskV2("ClaimTask",lastJwt,claim),memberV2("UpdateMember",admin,deactivate)]);
   assert.equal(claimed.status,200,describeProtobuf(claimed.body));
   assert.equal(revoked.status,409,"open Deal still requires an eligible human");
   assert.equal((await taskV2("CompleteTask",admin,{taskId:lastTask,
     precondition:{expectedVersion:2n,idempotencyKey:"complete-"+suffix}})).status,200);
   assert.equal((await memberV2("UpdateTeam",admin,empty)).status,409,"open Deal alone protects the roster");
   assert.equal((await salesV2("CloseDeal",admin,{dealId:lastDeal,outcome:3,reasonCode:"qa.closed",
     precondition:{expectedVersion:1n,idempotencyKey:"close-"+suffix}})).status,200);
   const archiveLast={teamId:lastTeam,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+suffix}};
   assert.equal((await memberV2("UpdateTeam",admin,archiveLast)).status,200);
   assert.equal((await memberV2("UpdateMember",admin,deactivate)).status,200);
   assert.equal((await memberV2("GetMyMembership",lastJwt,{})).status,401);
   const reopen={dealId:lastDeal,reasonCode:"qa.revisit",precondition:{expectedVersion:2n,idempotencyKey:"reopen-"+suffix}};
   const before=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastDeal))[0].n;
   assert.equal((await salesV2("ReopenDeal",admin,reopen)).status,409);
   assert.equal((await salesV2("ReopenDeal",admin,{...reopen,owner:lastOwner})).status,409);
   assert.equal((await salesV2("ReopenDeal",admin,{...reopen,owner:{owner:{case:"assigneeIdentity",value:lastEmail}}})).status,409);
   assert.equal((await salesV2("ReopenDeal",viewer,{...reopen,owner:actorOwner})).status,403);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastDeal))[0].n,before);
   const reopened=await salesV2("ReopenDeal",admin,{...reopen,owner:actorOwner});
   assert.equal(reopened.status,200,describeProtobuf(reopened.body));
   assert.equal(reopened.body.deal?.version,3n);
   assert.equal(reopened.body.deal?.owner?.owner.value,actorOwner.owner.value);
   assert.equal((await salesV2("ReopenDeal",admin,{...reopen,owner:actorOwner})).body.receipt?.replayed,true);
   assert.equal((await salesV2("ReopenDeal",admin,{...reopen,owner:lastOwner})).status,409,"changed idempotent intent");
   const events=(await salesV2("ListDealEvents",viewer,{dealId:lastDeal,page:{pageSize:10}})).body.events;
   assert.deepEqual(events.map(e=>e.dealVersion),[3n,2n,1n]);
   assert.equal(events[0].kind,5);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastDeal))[0].n,before+1);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastTeam))[0].n,2);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",lastEmail))[0].n,2);

   // A closed team-owned deal and team archive race on the same D1 invariant.
   const raceId="sk07-reopen-race-"+randomUUID(),raceDeal="deal-"+raceId;
   const raceOwner={owner:{case:"teamId",value:raceId}};
   assert.equal((await memberV2("CreateTeam",admin,{teamId:raceId,name:"Reopen race "+raceId,
     roster:{memberEmails:["native-a@example.test"]},precondition:{idempotencyKey:"team-"+raceId}})).status,200);
   assert.equal((await salesV2("CreateDeal",admin,input(raceDeal,raceOwner,"EUR",100n))).status,200);
   assert.equal((await salesV2("CloseDeal",admin,{dealId:raceDeal,outcome:3,reasonCode:"qa.close",
     precondition:{expectedVersion:1n,idempotencyKey:"close-"+raceId}})).status,200);
   const reopenRace={dealId:raceDeal,reasonCode:"qa.reopen",precondition:{expectedVersion:2n,idempotencyKey:"reopen-"+raceId}};
   const archiveRace={teamId:raceId,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+raceId}};
   const [rr,ar]=await Promise.all([salesV2("ReopenDeal",admin,reopenRace),memberV2("UpdateTeam",admin,archiveRace)]);
   assert.equal(Number(rr.status===200)+Number(ar.status===200),1,"reopen and archive cannot both commit");
   assert.equal(rr.status===200?ar.status:rr.status,409);
   if(rr.status===200){
     assert.equal((await salesV2("CloseDeal",admin,{dealId:raceDeal,outcome:3,reasonCode:"qa.cleanup",
       precondition:{expectedVersion:3n,idempotencyKey:"cleanup-"+raceId}})).status,200);
     assert.equal((await memberV2("UpdateTeam",admin,archiveRace)).status,200);
   }
   console.log(JSON.stringify({event:"crm.native-sk07c4-owner-lifecycle",result:"PASS",
     last_member_guard:true,claim_deactivation_race:true,closed_owner_retained:true,typed_reopen_reassign:true,
     reopen_archive_race:true,replay:true,one_event_receipt_audit:true}));
 }

 {
   // SK-07.D1: signed generated RPCs against the original Sales owners.
   const suffix=randomUUID(),root="territory-root-"+suffix,child="territory-child-"+suffix;
   const email="coverage-"+suffix+"@example.test";
   const human={owner:{case:"assigneeIdentity",value:email}};
   const primary={owner:{case:"assigneeIdentity",value:"native-a@example.test"}};
   const pre=(label,version)=>({idempotencyKey:"coverage-"+label+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   assert.equal((await memberV2("ProvisionMember",admin,{email,role:1,precondition:pre("member")})).status,200);
   const configuration={name:"Coverage root "+suffix,defaultOwner:primary};
   const rootCreate={territoryId:root,configuration,precondition:pre("root")};
   const created=await salesV2("CreateTerritory",admin,rootCreate);
   assert.equal(created.status,200,describeProtobuf(created.body));
   assert.equal(created.body.territory?.version,1n);
   assert.equal((await salesV2("CreateTerritory",admin,rootCreate)).body.receipt?.replayed,true);
   const childConfig={name:"Coverage child "+suffix,parentId:root,defaultOwner:primary};
   assert.equal((await salesV2("CreateTerritory",admin,{territoryId:child,configuration:childConfig,precondition:pre("child")})).status,200);
   const read=await salesV2("GetTerritory",viewer,{territoryId:root,receiptIdempotencyKey:rootCreate.precondition.idempotencyKey});
   assert.equal(read.status,200);
   assert.deepEqual(read.body.territory,created.body.territory);
   assert.equal(read.body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await salesV2("GetTerritory",tenantB,{territoryId:root})).status,404);
   assert.equal((await salesV2("ListTerritories",tenantB,{})).body.territories.length,0);
   assert.equal((await salesV2("ListTerritories",viewer,{page:{pageSize:1}})).body.territories.length,1);
   assert.equal((await salesV2("UpdateTerritory",admin,{territoryId:root,
     configuration:{...configuration,parentId:child},precondition:pre("cycle",1n)})).status,409);
   assert.equal((await salesV2("UpdateTerritory",admin,{territoryId:root,
     configuration:{...configuration,archived:true},precondition:pre("blocked-parent",1n)})).status,409);
   const assignmentConfig={territoryId:child,primaryOwner:human,overlays:[{role:1,owner:primary}]};
   const assignmentInput={accountId,configuration:assignmentConfig,precondition:pre("assignment")};
   assert.equal((await salesV2("GetSalesAccountAssignment",viewer,{accountId})).body.assignment,undefined);
   const assignment=await salesV2("CreateSalesAccountAssignment",admin,assignmentInput);
   assert.equal(assignment.status,200,describeProtobuf(assignment.body));
   assert.equal(assignment.body.assignment?.version,1n);
   assert.equal((await salesV2("CreateSalesAccountAssignment",admin,assignmentInput)).body.receipt?.replayed,true);
   const aRead=await salesV2("GetSalesAccountAssignment",viewer,{accountId,receiptIdempotencyKey:assignmentInput.precondition.idempotencyKey});
   assert.deepEqual(aRead.body.assignment,assignment.body.assignment);
   assert.equal(aRead.body.confirmedMutation?.resourceVersion,1n);
   assert.equal((await salesV2("GetSalesAccountAssignment",tenantB,{accountId})).status,404);
   assert.equal((await salesV2("ListSalesAccountAssignments",tenantB,{})).body.assignments.length,0);
   const filtered=await salesV2("ListSalesAccountAssignments",viewer,{territoryId:child,primaryOwnerFilter:human});
   assert.deepEqual(filtered.body.assignments.map(a=>a.accountId),[accountId]);
   assert.equal((await salesV2("UpdateTerritory",admin,{territoryId:child,
     configuration:{...childConfig,archived:true},precondition:pre("blocked-child",1n)})).status,409);
   assert.equal((await memberV2("UpdateMember",admin,{email,active:false,precondition:pre("blocked-member",1n)})).status,409);
   assert.equal((await salesV2("UpdateSalesAccountAssignment",admin,{accountId,
     configuration:{...assignmentConfig,overlays:[{role:1,owner:human}]},precondition:pre("duplicate",1n)})).status,400);
   const before=await sql("SELECT state,version FROM crm_sales_account_assignment_records WHERE tenant=? AND id=?","tenant-a",accountId);
   const count=(await sql("SELECT COUNT(*) n FROM crm_sales_account_assignment_receipts WHERE tenant=?","tenant-a"))[0].n;
   await sql("CREATE TRIGGER fixture_fail_coverage_audit BEFORE INSERT ON crm_audit WHEN NEW.action='sales.account_assignment.update' BEGIN SELECT RAISE(ABORT,'fixture coverage audit rollback'); END;");
   try{
     const fail=await salesV2("UpdateSalesAccountAssignment",admin,{accountId,configuration:{...assignmentConfig,archived:true},precondition:pre("fault",1n)});
     assertUnconfirmed(fail,"SALES_COVERAGE_WRITE_UNCONFIRMED");
     assert.deepEqual(await sql("SELECT state,version FROM crm_sales_account_assignment_records WHERE tenant=? AND id=?","tenant-a",accountId),before);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_account_assignment_receipts WHERE tenant=?","tenant-a"))[0].n,count);
   }finally{await sql("DROP TRIGGER fixture_fail_coverage_audit");}
   const archived=await salesV2("UpdateSalesAccountAssignment",admin,{accountId,
     configuration:{...assignmentConfig,archived:true},precondition:pre("archive-assignment",1n)});
   assert.equal(archived.status,200,describeProtobuf(archived.body));
   assert.equal(archived.body.assignment?.version,2n);
   assert.equal((await salesV2("UpdateSalesAccountAssignment",admin,{accountId,
     configuration:assignmentConfig,precondition:pre("stale",1n)})).status,409);
   assert.equal((await memberV2("UpdateMember",admin,{email,active:false,precondition:pre("deactivate",1n)})).status,200);
   assert.equal((await salesV2("UpdateSalesAccountAssignment",admin,{accountId,
     configuration:assignmentConfig,precondition:pre("inactive-owner",2n)})).status,409);
   assert.equal((await salesV2("UpdateTerritory",admin,{territoryId:child,
     configuration:{...childConfig,archived:true},precondition:pre("archive-child",1n)})).status,200);
   assert.equal((await salesV2("UpdateTerritory",admin,{territoryId:root,
     configuration:{...configuration,archived:true},precondition:pre("archive-root",1n)})).status,200);
   assert.equal((await salesV2("ListTerritories",viewer,{})).body.territories.length,0);
   const archivedTerritories=(await salesV2("ListTerritories",viewer,{includeArchived:true})).body.territories;
   assert.deepEqual(archivedTerritories.filter(t=>[root,child].includes(t.territoryId)).map(t=>t.configuration.archived),[true,true]);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id IN (?,?) AND action LIKE 'sales.territory.%'","tenant-a",root,child))[0].n,4);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action LIKE 'sales.account_assignment.%'","tenant-a",accountId))[0].n,2);
   const raceTeam="coverage-team-"+suffix,raceTerritory="coverage-race-"+suffix;
   const teamOwner={owner:{case:"teamId",value:raceTeam}};
   assert.equal((await memberV2("CreateTeam",admin,{teamId:raceTeam,name:"Coverage race "+suffix,
     roster:{memberEmails:["native-a@example.test"]},precondition:pre("race-team")})).status,200);
   const raceConfig={name:"Coverage race "+suffix,defaultOwner:teamOwner};
   const [assigned,teamArchived]=await Promise.all([
     salesV2("CreateTerritory",admin,{territoryId:raceTerritory,configuration:raceConfig,precondition:pre("race-territory")}),
     memberV2("UpdateTeam",admin,{teamId:raceTeam,active:false,precondition:pre("race-archive",1n)})
   ]);
   assert.equal(Number(assigned.status===200)+Number(teamArchived.status===200),1);
   if(assigned.status===200){
     assert.equal(teamArchived.status,409);
     assert.equal((await salesV2("UpdateTerritory",admin,{territoryId:raceTerritory,
       configuration:{...raceConfig,archived:true},precondition:pre("race-cleanup",1n)})).status,200);
     assert.equal((await memberV2("UpdateTeam",admin,{teamId:raceTeam,active:false,precondition:pre("race-archive",1n)})).status,200);
   }else{
     assert.equal(assigned.status,409);
   }
   console.log(JSON.stringify({event:"crm.native-sk07d1-coverage",result:"PASS",
     typed_ownership:true,hierarchy:true,tenant_ab:true,archive_guard:true,overlay:true,cas_replay:true,atomic_audit_rollback:true}));
 }
 {
   // D2: typed rules, exact money, preview, deterministic dispatch, capacity,
   // replay and receipt/event/audit are tested on the same signed D1 path.
   const suffix=randomUUID(),team="routing-team-"+suffix,pipeline="routing-pipeline-"+suffix;
   const emailA="routing-a-"+suffix+"@example.test",emailB="routing-b-"+suffix+"@example.test";
   const ruleId="routing-rule-"+suffix,secondId="routing-second-"+suffix;
   const territoryId="routing-territory-"+suffix;
   const actor=email=>({owner:{case:"assigneeIdentity",value:email}});
   const target={owner:{case:"teamId",value:team}};
   const pre=(label,version)=>({idempotencyKey:"routing-"+label+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   const call=async(method,identity,input,status=200)=>{
     const response=await salesV2(method,identity,input);
     assert.equal(response.status,status,method+": "+describeProtobuf(response.body));
     return response.body;
   };
   const revision=async tenant=>BigInt((await sql("SELECT CAST(COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?),0) AS TEXT) AS revision",tenant))[0].revision);
   const initialFence=await revision("tenant-a"),initialOtherFence=await revision("tenant-b");
   for(const [label,email] of [["a",emailA],["b",emailB]])assert.equal((await memberV2("ProvisionMember",admin,{email,role:1,precondition:pre("member-"+label)})).status,200);
   assert.equal(await revision("tenant-a"),initialFence+2n);
   assert.equal((await memberV2("CreateTeam",admin,{teamId:team,name:"Routing QA "+suffix,roster:{memberEmails:[emailA,emailB]},precondition:pre("team")})).status,200);
   assert.equal(await revision("tenant-a"),initialFence+3n);
   await call("CreatePipeline",admin,{pipelineId:pipeline,name:"Routing "+suffix,stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:5000}],precondition:pre("pipeline")});
   await call("CreateTerritory",admin,{territoryId,configuration:{name:"Routing territory "+suffix,defaultOwner:actor(emailB)},precondition:pre("territory")});
   // The shared read fence now covers catalog membership as well.
   assert.equal(await revision("tenant-a"),initialFence+5n);
   assert.equal(await revision("tenant-b"),initialOtherFence);
   const stage={pipelineId:pipeline,stageId:"lead",ordinal:1};
   const facts={accountId,stage,territoryId,amount:{currencyCode:"EUR",minorUnits:100n}};
   const cfg={name:"Routing workload "+suffix,priority:10,criteria:{territoryId,pipelineId:pipeline,stageId:"lead",currencyCode:"EUR",minimumMinorUnits:100n,maximumMinorUnits:100n},target,dispatch:2,maximumOpenDeals:1,overflow:1};
   const fallback={...cfg,name:"Routing fallback "+suffix,priority:20,criteria:{pipelineId:pipeline},target:actor(emailB),dispatch:1,overflow:1};
   delete fallback.maximumOpenDeals;
   const req={ruleId,configuration:cfg,precondition:pre("rule")};
   const created=await call("CreateSalesRoutingRule",admin,req);
   assert.equal(created.rule.version,1n);
   assert.equal((await call("CreateSalesRoutingRule",admin,req)).receipt.replayed,true);
   const get=await call("GetSalesRoutingRule",viewer,{ruleId,receiptIdempotencyKey:req.precondition.idempotencyKey});
   assert.deepEqual(get.rule,created.rule);assert.equal(get.confirmedMutation.resourceVersion,1n);
   await call("GetSalesRoutingRule",tenantB,{ruleId},404);
   assert.equal((await call("ListSalesRoutingRules",tenantB,{})).rules.length,0);
   await call("CreateSalesRoutingRule",viewer,{...req,ruleId:"denied-"+suffix,precondition:pre("denied")},403);
   await call("CreateSalesRoutingRule",admin,{ruleId:secondId,configuration:{...fallback,priority:10},precondition:pre("duplicate")},409);
   await call("CreateSalesRoutingRule",admin,{ruleId:secondId,configuration:fallback,precondition:pre("second")});
   assert.equal((await call("ListSalesRoutingRules",viewer,{})).rules.length,2);
   await call("UpdateTerritory",admin,{territoryId,configuration:{name:"Routing territory "+suffix,defaultOwner:actor(emailB),archived:true},precondition:pre("blocked-territory",1n)},409);
   await call("UpdatePipeline",admin,{pipelineId:pipeline,name:"Routing "+suffix,archived:true,stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:5000}],precondition:pre("blocked-pipeline",1n)},409);
   assert.equal((await memberV2("UpdateTeam",admin,{teamId:team,active:false,precondition:pre("blocked-team",1n)})).status,409);
   const before=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n;
   const preview=await call("PreviewSalesRouting",viewer,facts);
   assert.equal(preview.decision.ruleId,ruleId);assert.equal(preview.decision.selectedOwner.owner.value,emailA);
   assert.equal(preview.decision.selectedOpenDeals,0n);
   assert.deepEqual(await call("PreviewSalesRouting",admin,facts),preview);
   assert.equal((await call("PreviewSalesRouting",viewer,{...facts,amount:{currencyCode:"USD",minorUnits:100n}})).decision.ruleId,secondId);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n,before);
   const deal=(label,expected)=>({dealId:"routing-"+label+"-"+suffix,name:"Routing "+label+" "+suffix,accountId,stage,amount:facts.amount,automaticRouting:{territoryId,...(expected?{expectedDecision:expected}:{})},precondition:pre("deal-"+label)});
   const one=deal("one",preview.expectedDecision);
   const first=await call("CreateDeal",admin,one);
   assert.deepEqual(first.deal.initialRouting,preview.decision);
   assert.equal(first.deal.owner.owner.value,emailA);
   const p2=await call("PreviewSalesRouting",viewer,facts);
   assert.equal(p2.decision.selectedOwner.owner.value,emailB);
   const race=[deal("race-a",p2.expectedDecision),deal("race-b",p2.expectedDecision)];
   const results=await Promise.all(race.map(input=>salesV2("CreateDeal",admin,input)));
   assert.equal(results.filter(r=>r.status===200).length,1);
   assert.equal(results.filter(r=>r.status===409).length,1);
   const winner=race[results.findIndex(r=>r.status===200)];
   assert.equal((await call("GetDeal",viewer,{dealId:winner.dealId})).deal.owner.owner.value,emailB);
   await call("PreviewSalesRouting",viewer,facts,409);
   await call("CreateDeal",admin,deal("full"),409);
   const update={ruleId,configuration:{...cfg,overflow:2},precondition:pre("overflow",1n)};
   const ruleBefore=await sql("SELECT state,version FROM crm_sales_routing_rule_records WHERE tenant=? AND id=?","tenant-a",ruleId);
   await sql("CREATE TRIGGER fixture_fail_rule_audit BEFORE INSERT ON crm_audit WHEN NEW.action='sales.routing_rule.update' BEGIN SELECT RAISE(ABORT,'fixture routing rule audit rollback'); END;");
   try{
     assertUnconfirmed(await salesV2("UpdateSalesRoutingRule",admin,{...update,precondition:pre("rule-fault",1n)}),"SALES_ROUTING_RULE_WRITE_UNCONFIRMED");
     assert.deepEqual(await sql("SELECT state,version FROM crm_sales_routing_rule_records WHERE tenant=? AND id=?","tenant-a",ruleId),ruleBefore);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_routing_rule_receipts WHERE tenant=? AND key=?","tenant-a",pre("rule-fault",1n).idempotencyKey))[0].n,0);
   }finally{await sql("DROP TRIGGER fixture_fail_rule_audit");}
   assert.equal((await call("UpdateSalesRoutingRule",admin,update)).rule.version,2n);
   assert.equal((await call("UpdateSalesRoutingRule",admin,update)).receipt.replayed,true);
   assert.equal((await call("CreateDeal",admin,one)).receipt.replayed,true);
   assert.deepEqual((await call("CreateDeal",admin,one)).deal.initialRouting,first.deal.initialRouting);
   await call("CreateDeal",admin,deal("stale",preview.expectedDecision),409);
   const p3=await call("PreviewSalesRouting",viewer,facts);
   assert.equal(p3.decision.capacityOverflow,true);
   assert.equal(p3.decision.selectedOwner.owner.value,team);
   const overflow=deal("overflow",p3.expectedDecision);
   const pooled=await call("CreateDeal",admin,overflow);
   assert.equal(pooled.deal.owner.owner.value,team);
   const manual=deal("manual");delete manual.automaticRouting;manual.owner=actor(emailB);
   const manualSaved=await call("CreateDeal",admin,manual);
   assert.equal(manualSaved.deal.initialRouting.source,4);
   const events=await call("ListDealEvents",viewer,{dealId:one.dealId});
   assert.deepEqual(events.events[0].initialRouting,first.deal.initialRouting);
   const stored=await sql("SELECT json_extract(state,'$.initial_routing.rule_id') rule_id FROM crm_sales_deal_records WHERE tenant=? AND id=?","tenant-a",one.dealId);
   assert.equal(stored[0].rule_id,ruleId);
   await sql("CREATE TRIGGER fixture_fail_routing_audit BEFORE INSERT ON crm_audit WHEN NEW.subject_id='routing-fault-"+suffix+"' BEGIN SELECT RAISE(ABORT,'fixture routing audit rollback'); END;");
   try{
     assertUnconfirmed(await salesV2("CreateDeal",admin,deal("fault")),"SALES_COMMIT_UNCONFIRMED");
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_records WHERE tenant=? AND id=?","tenant-a","routing-fault-"+suffix))[0].n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",pre("deal-fault").idempotencyKey))[0].n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_events WHERE tenant=? AND deal_id=?","tenant-a","routing-fault-"+suffix))[0].n,0);
   }finally{await sql("DROP TRIGGER fixture_fail_routing_audit");}
   await call("UpdateSalesRoutingRule",admin,{ruleId,configuration:{...cfg,overflow:2,archived:true},precondition:pre("archive-rule",2n)});
   await call("UpdateSalesRoutingRule",admin,{ruleId:secondId,configuration:{...fallback,archived:true},precondition:pre("archive-second",1n)});
   assert.equal((await call("PreviewSalesRouting",viewer,facts)).decision.source,3);
   const noTerritory={accountId,stage,amount:facts.amount};
   await call("PreviewSalesRouting",viewer,noTerritory,409);
   const previousAssignment=(await call("GetSalesAccountAssignment",viewer,{accountId})).assignment;
   const beforeAssignmentFence=await revision("tenant-a");
   await call("UpdateSalesAccountAssignment",admin,{accountId,configuration:{territoryId,primaryOwner:actor(emailB),overlays:[],archived:false},precondition:pre("assignment",previousAssignment.version)});
   assert.equal(await revision("tenant-a"),beforeAssignmentFence+1n);
   assert.equal((await call("PreviewSalesRouting",viewer,noTerritory)).decision.source,2);
   await call("PreviewSalesRouting",viewer,{...facts,territoryId:"conflicting-"+suffix},409);
   await call("PreviewSalesRouting",tenantB,facts,409);
   const activeAssignment=(await call("GetSalesAccountAssignment",viewer,{accountId})).assignment;
   await call("PreviewDealCloseCredit",viewer,{dealId:one.dealId,expectedVersion:1n,
     selection:{territoryId:"conflict-"+suffix}},409);
   await call("UpdateSalesAccountAssignment",admin,{accountId,configuration:{...activeAssignment.configuration,archived:true},precondition:pre("assignment-archive",activeAssignment.version)});
   const reassigned=await call("UpdateDeal",admin,{dealId:one.dealId,owner:actor(emailB),precondition:pre("manual-reassign",1n)});
   assert.equal(reassigned.deal.owner.owner.value,emailB);
   assert.deepEqual(reassigned.deal.initialRouting,first.deal.initialRouting);
   // Explicit close credit shares the actual owner resolver and atomic guards.
   const secondTeam="credit-team-"+suffix;
   assert.equal((await memberV2("CreateTeam",admin,{teamId:secondTeam,name:"Credit choice "+suffix,
     roster:{memberEmails:[emailB]},precondition:pre("credit-team")})).status,200);
   const previewInput={dealId:one.dealId,expectedVersion:2n,selection:{teamId:team,territoryId}};
   const creditPreview=await call("PreviewDealCloseCredit",viewer,previewInput);
   assert.equal(creditPreview.eligibleTeams.length,2);
   assert.equal(creditPreview.creditedTeam.id,team);
   assert.equal(creditPreview.creditedTerritory.id,territoryId);
   assert.equal(creditPreview.territoryAssigned,false);
   assert.deepEqual(await call("PreviewDealCloseCredit",admin,previewInput),creditPreview);
   assert.equal((await call("PreviewDealCloseCredit",viewer,{...previewInput,selection:{}})).creditedTeam,undefined);
   assert.equal((await call("PreviewDealCloseCredit",viewer,{...previewInput,selection:{leaveTeamUnallocated:true}})).creditedTeam,undefined);
   await call("PreviewDealCloseCredit",tenantB,previewInput,404);
   await call("PreviewDealCloseCredit",viewer,{...previewInput,selection:{teamId:"missing-"+suffix}},400);
   const closeIntent={dealId:one.dealId,outcome:2,reasonCode:"qa.cleanup",precondition:pre("close-stale",2n),
     creditSelection:previewInput.selection,expectedCreditDecision:creditPreview.expectedDecision};
   await call("CloseDeal",viewer,closeIntent,403);
   assert.equal((await memberV2("UpdateTeam",admin,{teamId:secondTeam,name:"Credit choice revised "+suffix,
     roster:{memberEmails:[emailB]},precondition:pre("credit-team-rename",1n)})).status,200);
   await call("CloseDeal",admin,closeIntent,409);
   assert.equal((await call("GetDeal",viewer,{dealId:one.dealId})).deal.version,2n);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",closeIntent.precondition.idempotencyKey))[0].n,0);
   let replayCreditIntent;
   for(const [index,input] of [one,winner,overflow,manual].entries()){
     const version=index===0?2n:1n;
     const selection=index===0?{teamId:team,territoryId}:{teamId:team};
     const credit=await call("PreviewDealCloseCredit",viewer,{dealId:input.dealId,expectedVersion:version,selection});
     const request={dealId:input.dealId,outcome:index===0?2:3,reasonCode:"qa.cleanup",precondition:pre("close-"+index,version),
       creditSelection:selection,expectedCreditDecision:credit.expectedDecision};
     if(index===0)replayCreditIntent=request;
     const closed=await call("CloseDeal",admin,request);
     const fact=closed.deal.commercialCloseFact;
     assert.equal(fact.closeVersion,version+1n);
     assert.equal(fact.outcome,request.outcome);
     assert.equal(fact.amount.currencyCode,facts.amount.currencyCode);
     assert.equal(fact.amount.minorUnits,facts.amount.minorUnits);
     assert.deepEqual(fact.owner,closed.deal.owner);
     assert.equal(fact.creditedTeam.id,team);
     if(index===0)assert.equal(fact.creditedTerritory.id,territoryId);
     assert.deepEqual((await call("CloseDeal",admin,request)).deal.commercialCloseFact,fact);
     assert.deepEqual((await call("GetDeal",viewer,{dealId:input.dealId})).deal.commercialCloseFact,fact);
     assert.deepEqual((await call("ListDealEvents",viewer,{dealId:input.dealId})).events[0].commercialCloseFact,fact);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_events WHERE tenant=? AND deal_id=?","tenant-a",input.dealId))[0].n,Number(version)+1);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",input.dealId))[0].n,Number(version)+1);
   }
   assert.equal((await memberV2("UpdateTeam",admin,{teamId:secondTeam,active:false,
     precondition:pre("credit-team-archive",2n)})).status,200);
   assert.equal((await call("CloseDeal",admin,replayCreditIntent)).receipt.replayed,true);
   const originalClose=(await call("GetDeal",viewer,{dealId:one.dealId})).deal.commercialCloseFact;
   const reopened=await call("ReopenDeal",admin,{dealId:one.dealId,reasonCode:"qa.revisit",precondition:pre("credit-reopen",3n)});
   assert.equal(reopened.deal.commercialCloseFact,undefined);
   const reclosed=await call("CloseDeal",admin,{dealId:one.dealId,outcome:3,reasonCode:"qa.cleanup",precondition:pre("credit-reclose",4n)});
   assert.equal(reclosed.deal.commercialCloseFact.closeVersion,5n);
   const creditEvents=(await call("ListDealEvents",viewer,{dealId:one.dealId})).events;
   assert.deepEqual(creditEvents[0].commercialCloseFact,reclosed.deal.commercialCloseFact);
   assert.deepEqual(creditEvents[1].commercialCloseFact,originalClose);
   assert.deepEqual(creditEvents[2].commercialCloseFact,originalClose);
   assert.equal(creditEvents[1].kind,5);
   const storedCredit=await sql("SELECT commercial_close_fact FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND deal_version IN (3,4) ORDER BY deal_version","tenant-a",one.dealId);
   assert.equal(storedCredit.length,2);
   assert.deepEqual(JSON.parse(storedCredit[0].commercial_close_fact),JSON.parse(storedCredit[1].commercial_close_fact));
   // D3 quota publication, overlap, exact event net and historical revision.
   const quotaId="quota-"+suffix;
   const year=new Date().getUTCFullYear();
   const quotaCfg={name:"QA quota "+suffix,scope:{scope:{case:"owner",value:target}},pipelineId:pipeline,
     period:{startDate:{year,month:1,day:1},endDate:{year,month:12,day:31},timeZone:"UTC",cadence:3},
     target:{currencyCode:"EUR",minorUnits:1000n},published:false,revisionReason:"qa.draft"};
   const draft={quotaId,configuration:quotaCfg,precondition:pre("quota-draft")};
   await call("CreateSalesQuota",viewer,draft,403);
   const quota=await call("CreateSalesQuota",admin,draft);
   assert.equal(quota.quota.version,1n);
   assert.equal((await call("CreateSalesQuota",admin,draft)).receipt.replayed,true);
   const publish={quotaId,configuration:{...quotaCfg,published:true,revisionReason:"qa.publish"},precondition:pre("quota-publish",1n)};
   const publishedQuota=await call("UpdateSalesQuota",admin,publish);
   assert.equal(publishedQuota.quota.version,2n);
   await call("CreateSalesQuota",admin,{...draft,quotaId:"overlap-"+suffix,configuration:publish.configuration,precondition:pre("quota-overlap")},409);
   await call("GetSalesQuota",tenantB,{quotaId},404);
   assert.equal((await call("ListSalesQuotas",tenantB,{})).quotas.length,0);
   assert.deepEqual(await call("GetSalesQuota",viewer,{quotaId}),await call("GetSalesQuota",admin,{quotaId}));
   assert.equal((await call("ListSalesQuotas",viewer,{})).quotas.some(q=>q.quotaId===quotaId),true);
   const firstQuota=await call("GetSalesQuota",viewer,{quotaId,version:1n,receiptIdempotencyKey:draft.precondition.idempotencyKey});
   assert.equal(firstQuota.quota.configuration.published,false);
   assert.equal(firstQuota.confirmedMutation.resourceVersion,1n);
   const perf=await call("GetSalesQuotaPerformance",viewer,{quotaId,pageSize:1});
   assert.equal(perf.grossMinorUnits,"100");assert.equal(perf.reversalMinorUnits,"100");
   assert.equal(perf.netMinorUnits,"0");assert.equal(perf.remainingMinorUnits,"1000");
   assert.equal(perf.facts.length,1);assert.ok(perf.next);
   const page2=await call("GetSalesQuotaPerformance",viewer,{quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos,eventSequence:perf.eventSequence,after:perf.next,pageSize:1});
   assert.equal(page2.netMinorUnits,perf.netMinorUnits);assert.notDeepEqual(page2.facts[0],perf.facts[0]);
   // Scorecard shares the immutable ledger and exact quota semantics. A
   // monthly goal does not borrow the custom annual target.
   const month=new Date().getUTCMonth()+1;
   const monthlyId="quota-month-"+suffix;
   const monthlyConfig={...publish.configuration,name:"Monthly "+suffix,
     period:{startDate:{year,month,day:1},endDate:{year,month,day:new Date(Date.UTC(year,month,0)).getUTCDate()},timeZone:"UTC",cadence:1}};
   const monthlyQuota=(await call("CreateSalesQuota",admin,{quotaId:monthlyId,configuration:monthlyConfig,precondition:pre("monthly-goal")})).quota;
   const scoreInput={axis:3,currencyCode:"EUR",pipelineId:pipeline,firstDate:{year,month,day:1},timeZone:"UTC",cadence:1,periodCount:2};
   const auditBeforeScore=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n;
   const historyInput={pipelineId:pipeline,timeZone:"UTC",comparePrevious:true,
     period:{startDate:{year,month:1,day:1},endDate:{year,month:12,day:31}}};
   const history=await call("GetSalesHistory",viewer,historyInput);
   assert.ok(history.current);
   const historyDeal=history.current.deals.find(d=>d.dealId===one.dealId);
   assert.equal(historyDeal.status,3);
   assert.equal(historyDeal.finalCloseVersion,5n);
   assert.deepEqual(historyDeal.closeVersions,[3n,5n]);
   assert.deepEqual(historyDeal.reopenVersions,[4n]);
   assert.equal(historyDeal.visits.length,2);
   assert.equal(historyDeal.reasonCode,"qa.cleanup");
   assert.equal(history.current.finalCloses.wonCount,0n);
   assert.equal(history.current.finalCloses.lostCount,4n);
   const historyPaged=await call("GetSalesHistory",viewer,{...historyInput,pageSize:1});
   assert.equal(historyPaged.current.totalDealCount,history.current.totalDealCount);
   assert.deepEqual(historyPaged.current.finalCloses,history.current.finalCloses);
   const historyIds=historyPaged.current.deals.map(d=>d.dealId);
   let historyAfter=historyPaged.current.nextDealId;
   while(historyAfter){
     const page=await call("GetSalesHistory",viewer,{...historyInput,pageSize:1,
       snapshot:historyPaged.snapshot,detail:{afterDealId:historyAfter}});
     assert.equal(page.current,undefined);
     assert.equal(page.detail.deals.length,1);
     historyIds.push(page.detail.deals[0].dealId);
     historyAfter=page.detail.nextDealId;
   }
   assert.deepEqual(historyIds,history.current.deals.map(d=>d.dealId));
   await call("GetSalesHistory",viewer,{...historyInput,timeZone:"Europe/Warsaw",
     snapshot:historyPaged.snapshot,detail:{afterDealId:""}},400);
   const historyExportScope={...historyInput,snapshot:historyPaged.snapshot};
   const historyCsv=await call("ExportSalesAnalytics",admin,{kind:7,scope:{case:"history",value:historyExportScope}});
   assert.equal(historyCsv.csvSha256Hex,createHash("sha256").update(historyCsv.csv).digest("hex"));
   assert.deepEqual(historyCsv.history.current.finalCloses,history.current.finalCloses);
   assert.ok(Buffer.from(historyCsv.csv).toString("utf8").includes("history_window,current,total_deal_count,"));
   const historyDetailCsv=await call("ExportSalesAnalytics",admin,{kind:8,scope:{case:"history",value:{...historyExportScope,detail:{}}}});
   assert.ok(Buffer.from(historyDetailCsv.csv).toString("utf8").includes("historical_deal,"+one.dealId+","));
   await call("ExportSalesAnalytics",viewer,{kind:7,scope:{case:"history",value:historyExportScope}},403);
   const historyAdmin=await call("GetSalesHistory",admin,historyInput);
   assert.deepEqual(historyAdmin.current.finalCloses,history.current.finalCloses);
   assert.deepEqual(historyAdmin.current.deals,history.current.deals);
   await call("GetSalesHistory",tenantB,historyInput,404);
   const tenantHistory=await call("GetSalesHistory",tenantB,{...historyInput,pipelineId:undefined});
   assert.equal(tenantHistory.current.deals.some(d=>d.dealId===one.dealId),false);
   await call("GetSalesHistory",viewer,{...historyInput,timeZone:"Invalid/Zone"},400);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n,auditBeforeScore);
   const score=await call("GetSalesScorecard",viewer,scoreInput);
   assert.equal(score.buckets.length,2);
   assert.equal(score.buckets[0].endExclusiveUnixNanos,score.buckets[1].startUnixNanos);
   const scoreTeam=score.buckets[0].dimensions.find(d=>d.dimensionId===team).cell;
   assert.equal(scoreTeam.quota.quotaId,monthlyId);
   assert.equal(scoreTeam.quota.version,monthlyQuota.version);
   for(const field of ["grossMinorUnits","reversalMinorUnits","netMinorUnits","wonCount","lostCount","reversalCount","remainingMinorUnits","attainmentBasisPoints"])
     assert.equal(scoreTeam[field],perf[field],field);
   assert.equal(score.buckets[1].dimensions.find(d=>d.dimensionId===team).cell.netMinorUnits,"0");
   assert.equal(score.buckets[1].dimensions.find(d=>d.dimensionId===team).cell.quota,undefined);
   assert.equal(score.buckets[0].organization.quota,undefined);
   assert.deepEqual((await call("GetSalesScorecard",admin,scoreInput)).buckets,score.buckets);
   for(const axis of [1,2,4]){
     const other=await call("GetSalesScorecard",viewer,{...scoreInput,axis});
     for(const field of ["grossMinorUnits","reversalMinorUnits","netMinorUnits","wonCount","lostCount","sourceEventCount"])
       assert.equal(other.buckets[0].organization[field],score.buckets[0].organization[field],field);
     if(axis!==1)assert.equal(other.buckets[0].dimensions.reduce((n,d)=>n+BigInt(d.cell.netMinorUnits),0n),BigInt(other.buckets[0].organization.netMinorUnits));
   }
   const otherTenant=await call("GetSalesScorecard",tenantB,scoreInput);
   assert.equal(otherTenant.buckets[0].organization.sourceEventCount,0n);
   assert.equal(otherTenant.buckets[0].organization.netMinorUnits,"0");
   assert.ok(!otherTenant.buckets[0].dimensions.some(d=>d.dimensionId===team));
   await call("GetSalesScorecard",viewer,{...scoreInput,periodCount:25},400);
   await call("GetSalesScorecard",viewer,{...scoreInput,firstDate:{year,month,day:2}},400);
   const scoreDetail={...scoreInput,detail:{bucketIndex:0,organization:false,dimensionId:team,
     asOfUnixNanos:score.asOfUnixNanos,eventSequence:score.eventSequence,expectedEventCount:scoreTeam.sourceEventCount,pageSize:1}};
   const scoreExportRequest={kind:5,scope:{case:"scorecard",value:scoreInput}};
   const scoreExport=await call("ExportSalesAnalytics",admin,scoreExportRequest);
   const scoreCsv=Buffer.from(scoreExport.csv).toString("utf8");
   assert.equal(scoreExport.csvSha256Hex,createHash("sha256").update(scoreExport.csv).digest("hex"));
   assert.equal(scoreExport.filename,"crm-sales-scorecard.csv");
   assert.equal(scoreExport.specificationVersion,"sales-analytics-csv/v1");
   assert.deepEqual(scoreExport.scorecard.buckets,score.buckets);
   assert.ok(scoreCsv.startsWith("section,record_id,field,value\n"));
   assert.ok(scoreCsv.includes("gross_minor_units,100\n"));
   assert.ok(scoreCsv.includes("quota_id,"+monthlyId+"\n"));
   await call("ExportSalesAnalytics",viewer,scoreExportRequest,403);
   await call("ExportSalesAnalytics",admin,{kind:5},400);
   const detailExport=await call("ExportSalesAnalytics",admin,{kind:6,scope:{case:"scorecard",value:scoreDetail}});
   assert.ok(Buffer.from(detailExport.csv).toString("utf8").includes("signed_minor_units,-100\n"));
   await call("ExportSalesAnalytics",admin,{kind:6,scope:{case:"scorecard",value:{...scoreDetail,detail:{...scoreDetail.detail,expectedEventCount:scoreTeam.sourceEventCount+1n}}}},409);
   const firstScorePage=(await call("GetSalesScorecard",viewer,scoreDetail)).detail;
   assert.equal(firstScorePage.facts.length,1);assert.ok(firstScorePage.next);
   assert.equal(firstScorePage.sourceEventCount,scoreTeam.sourceEventCount);
   assert.deepEqual((await call("GetSalesScorecard",admin,scoreDetail)).detail,firstScorePage);
   const nextScorePage=(await call("GetSalesScorecard",viewer,{...scoreDetail,detail:{...scoreDetail.detail,after:firstScorePage.next}})).detail;
   assert.notDeepEqual(nextScorePage.facts[0],firstScorePage.facts[0]);
   await call("GetSalesScorecard",tenantB,scoreDetail,409);
   await call("GetSalesScorecard",viewer,{...scoreDetail,detail:{...scoreDetail.detail,expectedEventCount:scoreTeam.sourceEventCount+1n}},409);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n,auditBeforeScore);
   // Disposable fixture: more than the former 2000-event limit, exact values beyond JS safe integers.
   const scaleCount=2501,scaleAmount=9007199254740993n;
   const scaleInsert=`WITH RECURSIVE nums(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM nums WHERE n<${scaleCount})
     INSERT INTO crm_sales_deal_events(tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,stage_id,
       previous_stage_ordinal,stage_ordinal,previous_status,status,reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version,commercial_close_fact)
     SELECT e.tenant,e.deal_id,10000+n,e.action,e.pipeline_id,e.previous_stage_id,e.stage_id,e.previous_stage_ordinal,e.stage_ordinal,
       e.previous_status,e.status,e.reason_code,e.actor,e.request_id,e.occurred_at_unix_nanos,e.catalog_version,
       json_set(e.commercial_close_fact,'$.close_version',10000+n,'$.amount.minor_units','${scaleAmount}')
       FROM nums CROSS JOIN crm_sales_deal_events e WHERE e.tenant=? AND e.deal_id=? AND e.deal_version=3`;
   let scaleReport,scaledScoreDetail;
   const scaleReportId="report-scale-"+suffix;
   try{
     const beforeEventOnly=await call("GetSalesHistory",viewer,historyInput);
     const eventOnlyFence=await revision("tenant-a");
     await sql(scaleInsert.replace("n<"+scaleCount,"n<120"),"tenant-a",one.dealId);
     assert.equal(await revision("tenant-a"),eventOnlyFence+120n);
     // A deliberately incomplete stream belongs to the current year only;
     // it must not invalidate a historical window before immutable creation.
     await call("GetSalesHistory",viewer,{...historyInput,snapshot:beforeEventOnly.snapshot,
       detail:{afterDealId:""}},409);
     await call("ExportSalesAnalytics",admin,{kind:7,scope:{case:"history",value:{...historyInput,
       snapshot:beforeEventOnly.snapshot}}},409);
     const gappedHistory=await call("GetSalesHistory",viewer,historyInput);
     assert.equal(gappedHistory.current.unknownHistoryCount,1n);
     assert.equal(gappedHistory.current.deals.find(d=>d.dealId===one.dealId).status,undefined);
     assert.equal(gappedHistory.current.deals.find(d=>d.dealId===one.dealId).createdAtUnixNanos,historyDeal.createdAtUnixNanos);
     assert.equal(gappedHistory.previous.unknownHistoryCount,0n);
     assert.equal(gappedHistory.previous.deals.length,0);
     console.log(JSON.stringify({event:"crm.native-history-cutoff-membership",result:"PASS",gap_current:true,gap_future_excluded:true,immutable_creation:true}));
     const pagedScore=await call("GetSalesScorecard",viewer,scoreInput);
     const pagedTeam=pagedScore.buckets[0].dimensions.find(d=>d.dimensionId===team).cell;
     const fullDetail=await call("ExportSalesAnalytics",admin,{kind:6,scope:{case:"scorecard",value:{...scoreDetail,detail:{
       ...scoreDetail.detail,asOfUnixNanos:pagedScore.asOfUnixNanos,eventSequence:pagedScore.eventSequence,expectedEventCount:pagedTeam.sourceEventCount
     }}}});
     const detailCsv=Buffer.from(fullDetail.csv).toString("utf8");
     const versions=detailCsv.split("\n").filter(line=>line.startsWith("actual_event,")&&line.includes(",deal_version,"));
     assert.equal(versions.length,Number(pagedTeam.sourceEventCount));
     assert.equal(new Set(versions).size,versions.length);
     assert.ok(versions.length>100);
     assert.equal(fullDetail.csvSha256Hex,createHash("sha256").update(fullDetail.csv).digest("hex"));
     await sql(scaleInsert.replace("SELECT 1 UNION ALL","SELECT 121 UNION ALL"),"tenant-a",one.dealId);
     const started=Date.now();
     const scaled=await call("GetSalesQuotaPerformance",viewer,{quotaId,pageSize:1});
     const elapsed=Date.now()-started;
     assert.equal(scaled.grossMinorUnits,String(100n+BigInt(scaleCount)*scaleAmount));
     assert.equal(scaled.reversalMinorUnits,"100");
     assert.equal(scaled.netMinorUnits,String(BigInt(scaleCount)*scaleAmount));
     assert.equal(scaled.wonCount,perf.wonCount+BigInt(scaleCount));
     assert.ok(scaled.eventSequence>perf.eventSequence);
     assert.equal(scaled.facts.length,1);
     assert.ok(scaled.next);
     const scoreStarted=Date.now();
     const scaledScore=await call("GetSalesScorecard",viewer,scoreInput);
     const scoreElapsed=Date.now()-scoreStarted;
     const scaledTeam=scaledScore.buckets[0].dimensions.find(d=>d.dimensionId===team).cell;
     assert.equal(scaledTeam.netMinorUnits,scaled.netMinorUnits);
     assert.equal(scaledTeam.sourceEventCount,scoreTeam.sourceEventCount+BigInt(scaleCount));
     assert.deepEqual((await call("GetSalesScorecard",viewer,scoreDetail)).detail,firstScorePage);
     scaledScoreDetail={...scoreDetail,detail:{...scoreDetail.detail,asOfUnixNanos:scaledScore.asOfUnixNanos,
       eventSequence:scaledScore.eventSequence,expectedEventCount:scaledTeam.sourceEventCount}};
     const exactExport=await call("ExportSalesAnalytics",admin,scoreExportRequest);
     assert.ok(Buffer.from(exactExport.csv).toString("utf8").includes("net_minor_units,"+scaled.netMinorUnits+"\n"));
     await call("ExportSalesAnalytics",admin,{kind:6,scope:{case:"scorecard",value:scaledScoreDetail}},429);
     assert.equal((await call("GetSalesScorecard",viewer,scaledScoreDetail)).detail.sourceEventCount,scaledTeam.sourceEventCount);
     console.log(JSON.stringify({event:"crm.native-d3-scorecard-scale",result:"PASS",added_events:scaleCount,periods:2,elapsed_ms:scoreElapsed,exact_i128:true,frozen_membership:true}));
     const frozenRequest={quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos,eventSequence:perf.eventSequence,pageSize:1};
     assert.deepEqual(await call("GetSalesQuotaPerformance",viewer,frozenRequest),perf);
     assert.deepEqual(await call("GetSalesQuotaPerformance",admin,frozenRequest),perf);
     const scaledNext=await call("GetSalesQuotaPerformance",viewer,{quotaId,quotaVersion:2n,
       asOfUnixNanos:scaled.asOfUnixNanos,eventSequence:scaled.eventSequence,after:scaled.next,pageSize:1});
     assert.equal(scaledNext.netMinorUnits,scaled.netMinorUnits);
     assert.notDeepEqual(scaledNext.facts[0],scaled.facts[0]);
     await call("GetSalesQuotaPerformance",tenantB,frozenRequest,404);
     await call("GetSalesQuotaPerformance",viewer,{quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos,after:perf.next},400);
     scaleReport=(await call("SaveSalesPerformanceReport",admin,{reportId:scaleReportId,name:"Scale "+suffix,quotaId,quotaVersion:2n,precondition:pre("report-scale")})).report;
     assert.equal(scaleReport.actuals.netMinorUnits,scaled.netMinorUnits);
     assert.equal((await call("GetSalesPerformanceReport",viewer,{reportId:scaleReportId,pageSize:1})).actualHistoryAvailable,true);
     console.log(JSON.stringify({event:"crm.native-d3-quota-scale",result:"PASS",added_events:scaleCount,
       elapsed_ms:elapsed,exact_i128:true,frozen_membership:true,tenant_ab:true,viewer_parity:true}));
   }finally{
     await sql("DELETE FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND deal_version BETWEEN 10001 AND 12501","tenant-a",one.dealId);
   }
   await call("GetSalesScorecard",viewer,scaledScoreDetail,409);
   await call("UpdateSalesQuota",admin,{quotaId:monthlyId,configuration:{...monthlyConfig,archived:true,revisionReason:"qa.monthly-archive"},precondition:pre("monthly-archive",monthlyQuota.version)});
   const archivedScore=(await call("GetSalesScorecard",viewer,scoreInput)).buckets[0].dimensions.find(d=>d.dimensionId===team).cell;
   assert.equal(archivedScore.quota.configuration.archived,true);
   assert.equal(archivedScore.quota.version,monthlyQuota.version+1n);
   assert.equal(archivedScore.netMinorUnits,scoreTeam.netMinorUnits);
   console.log(JSON.stringify({event:"crm.native-d3-scorecards",result:"PASS",calendar:true,dimensions:true,quota_versions:true,exact_totals:true,detail_paging:true,retention_visible:true,tenant_ab:true,viewer_parity:true,no_read_audit:true}));
   // D3 durable report and forecast: exact estimates, frozen inputs, replay,
   // independent read rights, exclusions, mutation rollback and no new audit on reads.
   const retainedReport=await call("GetSalesPerformanceReport",viewer,{reportId:scaleReportId,pageSize:1});
   assert.deepEqual(retainedReport.report,scaleReport);
   assert.equal(retainedReport.actualHistoryAvailable,false);
   assert.equal(retainedReport.actualFacts.length,0);
   const reportTeam="report-team-"+suffix;
   assert.equal((await memberV2("CreateTeam",admin,{teamId:reportTeam,name:"Report ambiguous "+suffix,
     roster:{memberEmails:[emailB]},precondition:pre("report-team")})).status,200);
   const forecastDate={year,month:6,day:15};
   const forecastInputs=[
     {label:"included-a",amount:{currencyCode:"EUR",minorUnits:1001n},probabilityBasisPoints:5000,forecastCategory:3,expectedCloseDate:forecastDate},
     {label:"included-b",amount:{currencyCode:"EUR",minorUnits:2000n},probabilityBasisPoints:2500,forecastCategory:2,expectedCloseDate:forecastDate},
     {label:"omitted",amount:{currencyCode:"EUR",minorUnits:999n},forecastCategory:4,expectedCloseDate:forecastDate},
     {label:"undated",amount:{currencyCode:"EUR",minorUnits:999n}},
     {label:"other-currency",amount:{currencyCode:"USD",minorUnits:999n},expectedCloseDate:forecastDate},
     {label:"missing",expectedCloseDate:forecastDate},
     {label:"unallocated",owner:actor(emailB),amount:{currencyCode:"EUR",minorUnits:999n},expectedCloseDate:forecastDate},
     {label:"outside",amount:{currencyCode:"EUR",minorUnits:999n},expectedCloseDate:{year:year+1,month:1,day:1}}
   ].map(({label,...fields})=>({dealId:"report-"+label+"-"+suffix,name:"Report "+label+" "+suffix,
     accountId,stage,owner:target,...fields,precondition:pre("forecast-"+label)}));
   for(const input of forecastInputs)await call("CreateDeal",admin,input);
   const previewReport=await call("PreviewSalesPerformanceReport",viewer,{quotaId,quotaVersion:2n,pageSize:1});
   assert.equal(previewReport.report.forecast.openMinorUnits,"3001");
   assert.equal(previewReport.report.forecast.weightedMinorUnitsTimesBasisPoints,"10005000");
   assert.equal(previewReport.report.forecast.weightedCoverageBasisPoints,"10005");
   assert.equal(previewReport.report.forecast.openCoverageBasisPoints,"30010");
   assert.equal(previewReport.report.forecast.includedCount,2n);
   for(const field of ["omittedCount","undatedCount","otherCurrencyCount","missingAmountCount","unallocatedCount"])
     assert.equal(previewReport.report.forecast[field],1n,field);
   assert.equal(previewReport.forecastSources.length,1);
   assert.equal(previewReport.moreForecastSources,true);
   const reportId="report-"+suffix;
   const reportIntent={reportId,name:"Frozen commercial "+suffix,quotaId,quotaVersion:2n,precondition:pre("save-report")};
   await call("SaveSalesPerformanceReport",viewer,reportIntent,403);
   const savedReport=await call("SaveSalesPerformanceReport",admin,reportIntent);
   assert.equal(savedReport.report.forecast.openMinorUnits,"3001");
   assert.equal(savedReport.report.actuals.netMinorUnits,"0");
   assert.equal(savedReport.report.actuals.remainingMinorUnits,"1000");
   assert.equal(savedReport.report.quota.version,2n);
   assert.equal(savedReport.report.formulaVersion,1);
   assert.equal((await call("SaveSalesPerformanceReport",admin,reportIntent)).receipt.replayed,true);
   await call("SaveSalesPerformanceReport",admin,{...reportIntent,name:"Different"},409);
   await call("GetSalesPerformanceReport",tenantB,{reportId},404);
   assert.equal((await call("ListSalesPerformanceReports",tenantB,{})).reports.length,0);
   const readBefore=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n;
   const getReport={reportId,pageSize:1};
   const readReport=await call("GetSalesPerformanceReport",viewer,getReport);
   assert.deepEqual(readReport,await call("GetSalesPerformanceReport",admin,getReport));
   assert.deepEqual(readReport.report,savedReport.report);
   assert.equal(readReport.actualHistoryAvailable,true);
   assert.ok(readReport.actualNext);assert.ok(readReport.forecastNext);
   const compareRequest={...getReport,baselineReportId:scaleReportId};
   const compared=await call("GetSalesPerformanceReport",viewer,compareRequest);
   const reportExportRequest={kind:1,scope:{case:"report",value:{reportId}}};
   const reportExport=await call("ExportSalesAnalytics",admin,reportExportRequest);
   assert.equal(reportExport.csvSha256Hex,createHash("sha256").update(reportExport.csv).digest("hex"));
   assert.deepEqual(await call("ExportSalesAnalytics",admin,reportExportRequest),reportExport);
   await call("ExportSalesAnalytics",tenantB,reportExportRequest,404);
   const comparisonExport=await call("ExportSalesAnalytics",admin,{kind:2,scope:{case:"report",value:{reportId,baselineReportId:scaleReportId}}});
   assert.ok(Buffer.from(comparisonExport.csv).toString("utf8").includes("comparison,"+reportId+",status,comparable"));
   const actualExport=await call("ExportSalesAnalytics",admin,{kind:3,scope:{case:"report",value:{reportId}}});
   assert.ok(Buffer.from(actualExport.csv).toString("utf8").includes("signed_minor_units,-100\n"));
   const forecastExport=await call("ExportSalesAnalytics",admin,{kind:4,scope:{case:"report",value:{reportId}}});
   assert.ok(Buffer.from(forecastExport.csv).toString("utf8").includes("forecast_source,"));
   await call("ExportSalesAnalytics",admin,{kind:3,scope:{case:"report",value:{reportId:scaleReportId}}},409);
   const retainedExport=await call("ExportSalesAnalytics",admin,{kind:1,scope:{case:"report",value:{reportId:scaleReportId}}});
   assert.ok(Buffer.from(retainedExport.csv).toString("utf8").includes("actual_history_available,false\n"));
   console.log(JSON.stringify({event:"crm.native-sales-analytics-export",result:"PASS",sha256:reportExport.csvSha256Hex}));
   assert.deepEqual(compared,await call("GetSalesPerformanceReport",admin,compareRequest));
   assert.deepEqual(compared.comparison.baseline,scaleReport);
   assert.equal(compared.comparison.status,1);
   assert.equal(compared.comparison.delta.netMinorUnits,String(-BigInt(scaleCount)*scaleAmount));
   assert.equal(compared.comparison.delta.targetMinorUnits,"0");
   assert.equal(compared.comparison.delta.weightedCoverageBasisPoints,undefined);
   const reversedComparison=await call("GetSalesPerformanceReport",viewer,{reportId:scaleReportId,baselineReportId:reportId,pageSize:1});
   assert.equal(reversedComparison.comparison.delta.netMinorUnits,String(BigInt(scaleCount)*scaleAmount));
   assert.equal(reversedComparison.actualHistoryAvailable,false);
   assert.equal(reversedComparison.comparison.status,1);
   const identicalComparison=await call("GetSalesPerformanceReport",viewer,{...getReport,baselineReportId:reportId});
   assert.equal(identicalComparison.comparison.delta.netMinorUnits,"0");
   assert.equal(identicalComparison.comparison.delta.weightedCoverageBasisPoints,"0");
   await call("GetSalesPerformanceReport",tenantB,compareRequest,404);
   await call("GetSalesPerformanceReport",viewer,{...getReport,baselineReportId:"missing-"+suffix},404);
   const nextReport=await call("GetSalesPerformanceReport",viewer,{...getReport,actualAfter:readReport.actualNext,forecastAfter:readReport.forecastNext});
   assert.deepEqual(nextReport.report,readReport.report);
   assert.notDeepEqual(nextReport.actualFacts,readReport.actualFacts);
   assert.notDeepEqual(nextReport.forecastSources,readReport.forecastSources);
   const receiptReport=await call("GetSalesPerformanceReport",viewer,{...getReport,receiptIdempotencyKey:reportIntent.precondition.idempotencyKey});
   assert.equal(receiptReport.confirmedMutation.resourceVersion,1n);
   assert.equal(receiptReport.confirmedMutation.originalRequestId,savedReport.receipt.requestId);
   assert.equal((await call("ListSalesPerformanceReports",viewer,{quotaId})).reports.some(r=>r.reportId===reportId),true);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n,readBefore);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",reportId))[0].n,7);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",reportId))[0].n,1);
   // Industrial synchronous capture: >2500 open rows, independent of the
   // closed-event scale proof above. Clone only disposable valid fixture state.
   const prefix="report-scale-open-"+suffix+"-";
   const openScale=2501,limitScale=8193;
   // Fixed-width fixture IDs: indexed binary range, not D1-limited LIKE patterns.
   const fixtureId=n=>prefix+String(n).padStart(6,"0");
   const fixtureFirst=fixtureId(1),fixtureLast=fixtureId(limitScale);
   const scaledReportId="report-open-scale-"+suffix;
   const capacityReportId="report-capacity-"+suffix;
   const cloneOpen=async(first,last)=>sql(
     `WITH RECURSIVE nums(n) AS (SELECT ?1 UNION ALL SELECT n+1 FROM nums WHERE n<?2)
      INSERT INTO crm_sales_deal_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor)
      SELECT d.tenant,?3||printf('%06d',n),d.account_id,json_set(d.state,'$.deal_id',?3||printf('%06d',n)),
        d.version,?3||printf('%06d',n),d.idem_digest,?3||printf('%06d',n),d.actor
      FROM nums CROSS JOIN crm_sales_deal_records d WHERE d.tenant=?4 AND d.id=?5`,
     first,last,prefix,"tenant-a",forecastInputs[0].dealId);
   const beforeFence=await revision("tenant-a"),otherFence=await revision("tenant-b");
   try{
     await cloneOpen(1,openScale);
     // Each inserted Deal and its immutable created event invalidate the shared read fence.
     assert.equal(await revision("tenant-a"),beforeFence+2n*BigInt(openScale));
     assert.equal(await revision("tenant-b"),otherFence);
     const openAmount=3001n+BigInt(openScale)*1001n;
     const weightedAmount=10005000n+BigInt(openScale)*1001n*5000n;
     const captureInput={quotaId,quotaVersion:2n,pageSize:1};
     const started=Date.now();
     const largePreview=await call("PreviewSalesPerformanceReport",viewer,captureInput);
     const previewMs=Date.now()-started;
     assert.equal(largePreview.report.forecast.openMinorUnits,String(openAmount));
     assert.equal(largePreview.report.forecast.weightedMinorUnitsTimesBasisPoints,String(weightedAmount));
     assert.equal(largePreview.report.forecast.includedCount,BigInt(openScale+2));
     assert.equal(largePreview.forecastSources.length,1);
     assert.equal(largePreview.moreForecastSources,true);
     const saveStart=Date.now();
     const largeReport=(await call("SaveSalesPerformanceReport",admin,{...reportIntent,reportId:scaledReportId,precondition:pre("open-scale-report")})).report;
     const saveMs=Date.now()-saveStart;
     assert.deepEqual(largeReport.forecast,largePreview.report.forecast);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",scaledReportId))[0].n,openScale+7);
     const tail=await call("GetSalesPerformanceReport",viewer,{reportId:scaledReportId,forecastAfter:prefix+String(openScale-1).padStart(6,"0"),pageSize:1});
     assert.equal(tail.forecastSources[0].dealId,prefix+String(openScale).padStart(6,"0"));
     assert.equal(tail.forecastSources[0].amount.minorUnits,1001n);
     assert.equal(tail.forecastSources[0].probabilityBasisPoints,5000);
     await call("GetSalesPerformanceReport",tenantB,{reportId:scaledReportId},404);
     const fence=await revision("tenant-a");
     await assert.rejects(()=>batch([
       statement("UPDATE crm_sales_deal_records SET state=state WHERE tenant=? AND id=?","tenant-a",forecastInputs[0].dealId),
       statement("INSERT INTO crm_ci_missing_forecast_table VALUES (1)")
     ]));
     assert.equal(await revision("tenant-a"),fence,"failed batch must roll back the fence");
     // Race a real typed owner command against the capture. The fixture
     // loader cannot bypass the catalogue rank-only guard on existing Deals.
     // Changing a team's eligibility would affect many rows but would also
     // alter unrelated fixture semantics. Use a single real Deal mutation.
     const raceFence=await revision("tenant-a");
     const [race,mutation]=await Promise.all([
       salesV2("PreviewSalesPerformanceReport",viewer,captureInput),
       salesV2("UpdateDeal",admin,{dealId:fixtureId(openScale),probabilityBasisPoints:1000,
         precondition:pre("open-scale-race",1n)})
     ]);
     assert.equal(mutation.status,200,describeProtobuf(mutation.body));
     assert.equal(mutation.body.deal.version,2n);
     assert.equal(mutation.body.deal.probabilityBasisPoints,1000);
     assert.equal(await revision("tenant-a"),raceFence+2n);
     const newWeight=weightedAmount-1001n*4000n;
     assert.ok([200,409].includes(race.status),describeProtobuf(race.body));
     if(race.status===200)assert.ok([String(weightedAmount),String(newWeight)].includes(race.body.report.forecast.weightedMinorUnitsTimesBasisPoints),"mixed capture");
     else assert.match(race.body.message,/SALES_FORECAST_SNAPSHOT_CHANGED/);
     assert.equal((await call("PreviewSalesPerformanceReport",viewer,captureInput)).report.forecast.weightedMinorUnitsTimesBasisPoints,String(newWeight));
     assert.deepEqual((await call("GetSalesPerformanceReport",viewer,{reportId:scaledReportId,pageSize:1})).report,largeReport);
     await cloneOpen(openScale+1,limitScale);
     // The same retained fixture exceeds both obsolete historical limits.
     // Exact summary membership is checked against original physical D1 records.
     const expectedHistory=BigInt((await sql("SELECT COUNT(*) n FROM crm_sales_deal_records WHERE tenant=? AND json_extract(state,'$.stage.pipeline_id')=?",
       "tenant-a",pipeline))[0].n);
     const historyLarge=await call("GetSalesHistory",viewer,{...historyInput,pageSize:5});
     assert.ok(expectedHistory>8192n);
     assert.equal(historyLarge.current.totalDealCount,expectedHistory);
     assert.equal(historyLarge.current.stateAtCutoff.totalCount+historyLarge.current.unknownHistoryCount,expectedHistory);
     assert.equal(historyLarge.current.deals.length,5);
     const historyLargeNext=await call("GetSalesHistory",viewer,{...historyInput,pageSize:5,
       snapshot:historyLarge.snapshot,detail:{afterDealId:historyLarge.current.nextDealId}});
     assert.equal(historyLargeNext.detail.deals.length,5);
     assert.ok(historyLargeNext.detail.deals[0].dealId>historyLarge.current.nextDealId);
     console.log(JSON.stringify({event:"crm.native-history-industrial-scale",result:"PASS",
       deals:String(expectedHistory),page_size:5,exact_membership:true,fenced_detail:true}));
     const oversized=await salesV2("PreviewSalesPerformanceReport",viewer,captureInput);
     assert.equal(oversized.status,429,describeProtobuf(oversized.body));
     assert.match(oversized.body.message,/SALES_FORECAST_CAPTURE_CAPACITY/);
     const refused=await salesV2("SaveSalesPerformanceReport",admin,{...reportIntent,reportId:capacityReportId,precondition:pre("capacity-report")});
     assert.equal(refused.status,429,describeProtobuf(refused.body));
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_reports WHERE tenant=? AND id=?","tenant-a",capacityReportId))[0].n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",capacityReportId))[0].n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",capacityReportId))[0].n,0);
     assert.equal(await revision("tenant-b"),otherFence);
     console.log(JSON.stringify({event:"crm.native-d3-open-forecast-scale",result:"PASS",added_open_deals:openScale,preview_ms:previewMs,save_ms:saveMs,capture_capacity_rejected:true,concurrent_outcome:race.status,exact_weight:true,tenant_fence:true,rollback_fence:true}));
   }catch(error){
     console.error("D3 open forecast scale primary failure:",error);
     throw error;
   }finally{
     await sql("DELETE FROM crm_sales_deal_events WHERE tenant=? AND deal_id>=? AND deal_id<=?","tenant-a",fixtureFirst,fixtureLast);
     await sql("DELETE FROM crm_sales_deal_receipts WHERE tenant=? AND key>=? AND key<=?","tenant-a",fixtureFirst,fixtureLast);
     await sql("DELETE FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",pre("open-scale-race",1n).idempotencyKey);
     await sql("DELETE FROM crm_audit WHERE tenant_id=? AND subject_id>=? AND subject_id<=?","tenant-a",fixtureFirst,fixtureLast);
     await sql("DELETE FROM crm_sales_deal_records WHERE tenant=? AND id>=? AND id<=?","tenant-a",fixtureFirst,fixtureLast);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_records WHERE tenant=? AND id>=? AND id<=?","tenant-a",fixtureFirst,fixtureLast))[0].n,0);
   }
   const faultId="report-fault-"+suffix;
   await sql("CREATE TRIGGER fixture_report_audit_fail BEFORE INSERT ON crm_audit WHEN NEW.action='sales.performance_report.save' BEGIN SELECT RAISE(ABORT,'fixture report audit rollback'); END;");
   try{
     assertUnconfirmed(await salesV2("SaveSalesPerformanceReport",admin,{...reportIntent,reportId:faultId,precondition:pre("report-fault")}),"SALES_PERFORMANCE_REPORT_WRITE_UNCONFIRMED");
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_reports WHERE tenant=? AND id=?","tenant-a",faultId))[0].n,0);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_performance_forecast_sources WHERE tenant=? AND report_id=?","tenant-a",faultId))[0].n,0);
   }finally{await sql("DROP TRIGGER fixture_report_audit_fail");}
   const beforeOwnerFence=await revision("tenant-a");
   const changedForecast=await call("UpdateDeal",admin,{dealId:forecastInputs[0].dealId,amount:{operation:{case:"set",value:{currencyCode:"EUR",minorUnits:9000n}}},precondition:pre("forecast-update",1n)});
   assert.equal(changedForecast.deal.version,2n);
   assert.equal(await revision("tenant-a"),beforeOwnerFence+2n);
   assert.equal(await revision("tenant-b"),otherFence);
   assert.equal((await call("PreviewSalesPerformanceReport",viewer,{quotaId,quotaVersion:2n})).report.forecast.openMinorUnits,"11000");
   assert.deepEqual(await call("GetSalesPerformanceReport",viewer,getReport),readReport);
   for(const [index,input] of forecastInputs.entries())await call("CloseDeal",admin,{dealId:input.dealId,outcome:3,reasonCode:"qa.report.cleanup",precondition:pre("report-close-"+index,index===0?2n:1n)});
   assert.deepEqual(await call("GetSalesPerformanceReport",viewer,getReport),readReport);
   assert.equal((await memberV2("UpdateTeam",admin,{teamId:reportTeam,active:false,precondition:pre("archive-report-team",1n)})).status,200);
   const beforeQuota=await sql("SELECT state,version FROM crm_sales_quota_records WHERE tenant=? AND id=?","tenant-a",quotaId);
   await sql("CREATE TRIGGER fixture_fail_quota_audit BEFORE INSERT ON crm_audit WHEN NEW.action='sales.quota.update' BEGIN SELECT RAISE(ABORT,'fixture quota audit rollback'); END;");
   try{
     assertUnconfirmed(await salesV2("UpdateSalesQuota",admin,{quotaId,configuration:{...publish.configuration,revisionReason:"qa.fault"},precondition:pre("quota-fault",2n)}),"SALES_QUOTA_WRITE_UNCONFIRMED");
     assert.deepEqual(await sql("SELECT state,version FROM crm_sales_quota_records WHERE tenant=? AND id=?","tenant-a",quotaId),beforeQuota);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_quota_revisions WHERE tenant=? AND id=?","tenant-a",quotaId))[0].n,2);
     assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_quota_receipts WHERE tenant=? AND key=?","tenant-a",pre("quota-fault",2n).idempotencyKey))[0].n,0);
   }finally{await sql("DROP TRIGGER fixture_fail_quota_audit");}
   const revised=await call("UpdateSalesQuota",admin,{quotaId,configuration:{...publish.configuration,target:{currencyCode:"EUR",minorUnits:2000n},revisionReason:"qa.revise"},precondition:pre("quota-revise",2n)});
   assert.equal(revised.quota.version,3n);
   const historical=await call("GetSalesQuotaPerformance",viewer,{quotaId,quotaVersion:2n,asOfUnixNanos:perf.asOfUnixNanos});
   assert.equal(historical.remainingMinorUnits,"1000");
   assert.equal((await call("GetSalesQuotaPerformance",viewer,{quotaId})).remainingMinorUnits,"2000");
   await call("UpdateSalesQuota",admin,{quotaId,configuration:{...revised.quota.configuration,archived:true,revisionReason:"qa.archive"},precondition:pre("quota-archive",3n)});
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_quota_revisions WHERE tenant=? AND id=?","tenant-a",quotaId))[0].n,4);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",quotaId))[0].n,4);
   assert.deepEqual(await call("GetSalesPerformanceReport",viewer,getReport),readReport);
   assert.deepEqual((await call("SaveSalesPerformanceReport",admin,reportIntent)).report,savedReport.report);
   assert.deepEqual(await call("ExportSalesAnalytics",admin,reportExportRequest),reportExport);
   console.log(JSON.stringify({event:"crm.native-d3-performance-report",result:"PASS",exact_forecast:true,immutable_report:true,source_paging:true,tenant_ab:true,receipt_replay:true,atomic_audit_rollback:true,no_read_audit:true,retention_visible:true}));
   await call("UpdateTerritory",admin,{territoryId,configuration:{name:"Routing territory "+suffix,defaultOwner:actor(emailB),archived:true},precondition:pre("archive-territory",1n)});
   assert.equal((await memberV2("UpdateTeam",admin,{teamId:team,active:false,precondition:pre("archive-team",1n)})).status,200);
   for(const [label,email] of [["a",emailA],["b",emailB]])assert.equal((await memberV2("UpdateMember",admin,{email,active:false,precondition:pre("deactivate-"+label,1n)})).status,200);
   console.log(JSON.stringify({event:"crm.native-sk07d2-routing",result:"PASS",priority:true,exact_money:true,capacity_race:true,overflow:true,stale_preview:true,immutable_attribution:true,tenant_ab:true,receipt_event_audit:true}));
 }
 // Independent signed viewer and original lifecycle CAS; test at the end
 // so disposable fixtures never change earlier deterministic analytics.
 {
   const suffix=randomUUID(),email="opportunity-"+suffix+"@example.test";
   const token=jwt(email),pid="opportunity-pipeline-"+suffix,id="opportunity-deal-"+suffix;
   const pre=(key,version)=>({idempotencyKey:"opportunity-"+key+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   assert.equal((await memberV2("ProvisionMember",admin,{email,role:1,precondition:pre("member")})).status,200);
   const pipeline=await salesV2("CreatePipeline",admin,{pipelineId:pid,name:"Opportunity QA",
     stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:1000},{stageId:"qualified",name:"Qualified",probabilityBasisPoints:5000}],precondition:pre("pipeline")});
   assert.equal(pipeline.status,200,describeProtobuf(pipeline.body));
   const deal=await salesV2("CreateDeal",admin,{dealId:id,name:"Opportunity QA",accountId,
     stage:{pipelineId:pid,stageId:"lead"},precondition:pre("deal")});
   assert.equal(deal.status,200,describeProtobuf(deal.body));
   const grant=(role,version,key)=>({dealId:id,email,...(role===undefined?{}:{role}),precondition:pre(key,version)});
   const update=(version,key)=>({dealId:id,name:{operation:{case:"set",value:"Opportunity "+key}},precondition:pre(key,version)});
   const lead=await salesV2("SetOpportunityParticipant",admin,grant(3,1n,"lead"));
   assert.equal(lead.status,200,describeProtobuf(lead.body));
   assert.equal(lead.body.deal.opportunityTeam[0].memberVersion,1n);
   const advance={dealId:id,targetStage:{pipelineId:pid,stageId:"qualified"},targetStatus:1,precondition:pre("move",2n)};
   const moved=await salesV2("AdvanceStage",token,advance);
   assert.equal(moved.status,200,describeProtobuf(moved.body));
   assert.equal(moved.body.deal.stage.stageId,"qualified");
   assert.equal(moved.body.deal.version,3n);
   const moveReplay=await salesV2("AdvanceStage",token,advance);
   assert.equal(moveReplay.status,200,describeProtobuf(moveReplay.body));
   assert.equal(moveReplay.body.receipt.replayed,true);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?","tenant-a",id))[0].n,1);
   assert.equal((await memberV2("UpdateMember",admin,{email,active:false,precondition:pre("deactivate",1n)})).status,200);
   assert.equal((await salesV2("UpdateDeal",token,update(3n,"inactive"))).status,401);
   assert.equal((await memberV2("UpdateMember",admin,{email,active:true,precondition:pre("reactivate",2n)})).status,200);
   assert.equal((await salesV2("GetDeal",token,{dealId:id})).body.capabilities.edit,false);
   assert.equal((await salesV2("UpdateDeal",token,update(3n,"stale-grant"))).status,403);
   const renewed=await salesV2("SetOpportunityParticipant",admin,grant(3,3n,"renew"));
   assert.equal(renewed.status,200,describeProtobuf(renewed.body));
   assert.equal(renewed.body.deal.opportunityTeam[0].memberVersion,3n);
   const [revoke,write]=await Promise.all([
     salesV2("SetOpportunityParticipant",admin,grant(undefined,4n,"revoke")),
     salesV2("UpdateDeal",token,update(4n,"race"))
   ]);
   assert.equal([revoke,write].filter(r=>r.status===200).length,1);
   assert.ok([403,409].includes((revoke.status===200?write:revoke).status));
   if(write.status===200){
     const removed=await salesV2("SetOpportunityParticipant",admin,grant(undefined,5n,"revoke-after-race"));
     assert.equal(removed.status,200,describeProtobuf(removed.body));
   }
   const final=(await salesV2("GetDeal",token,{dealId:id})).body;
   assert.equal(final.deal.opportunityTeam.length,0);
   assert.equal(final.capabilities.edit,false);
   assert.equal((await salesV2("UpdateDeal",token,update(final.deal.version,"after-revoke"))).status,403);
   assert.equal((await salesV2("AdvanceStage",token,advance)).status,403);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?","tenant-a",id))[0].n,1);
   const events=(await salesV2("ListDealEvents",viewer,{dealId:id,page:{pageSize:25}})).body.events;
   assert.equal(events.find(e=>e.kind===3).actor,email);
   console.log(JSON.stringify({event:"crm.native-opportunity-delegation",result:"PASS",signed_viewer:true,stage_link_once:true,member_version_pin:true,reactivation_no_restore:true,revocation_cas:true,race:[revoke.status,write.status]}));
 }
 // Cross-owner fixed template with separate fixtures after earlier analytics.
 {
   const suffix=randomUUID(),leadEmail="coord-"+suffix+"@example.test",targetEmail="target-"+suffix+"@example.test",otherEmail="outside-"+suffix+"@example.test";
   const lead=jwt(leadEmail),target=jwt(targetEmail),outside=jwt(otherEmail);
   const tid="coord-team-"+suffix,otherTeam="other-team-"+suffix;
   const pre=(key,version)=>({idempotencyKey:"coord-"+key+"-"+suffix,...(version===undefined?{}:{expectedVersion:version})});
   for(const [key,email] of [["lead",leadEmail],["target",targetEmail],["outside",otherEmail]])assert.equal((await memberV2("ProvisionMember",admin,{email,role:1,precondition:pre(key)})).status,200);
   const team=await memberV2("CreateTeam",admin,{teamId:tid,name:"Coordinator "+suffix,roster:{memberEmails:[leadEmail,targetEmail]},precondition:pre("team")});
   assert.equal(team.status,200,describeProtobuf(team.body));
   assert.equal((await memberV2("CreateTeam",admin,{teamId:otherTeam,name:"Outside "+suffix,roster:{memberEmails:[otherEmail]},precondition:pre("otherteam")})).status,200);
   const grant={teamId:tid,queueCoordinator:{email:leadEmail,enabled:true},precondition:pre("grant",1n)};
   assert.equal((await memberV2("UpdateTeam",lead,grant)).status,403);
   assert.equal((await memberV2("UpdateTeam",admin,{...grant,queueCoordinator:{email:otherEmail,enabled:true},precondition:pre("foreign",1n)})).status,400);
   const granted=await memberV2("UpdateTeam",admin,grant);
   assert.equal(granted.status,200,describeProtobuf(granted.body));
   assert.equal(granted.body.team.queueCoordinators[0].memberVersion,1n);
   const taskId="coord-task-"+suffix,dealId="coord-deal-"+suffix,taskRace="coord-race-"+suffix;
   const owner={owner:{case:"teamId",value:tid}},foreignOwner={owner:{case:"teamId",value:otherTeam}};
   for(const [id,own] of [[taskId,owner],[taskRace,owner],["foreign-task-"+suffix,foreignOwner]])assert.equal((await taskV2("CreateTask",admin,{taskId:id,accountId,subject:"Coordinator "+id,priority:2,owner:own,precondition:pre("task-"+id)})).status,200);
   assert.equal((await salesV2("CreateDeal",admin,{dealId,name:"Coordinator "+suffix,accountId,stage:{pipelineId:"pipeline.enterprise",stageId:"lead"},owner,precondition:pre("deal")})).status,200);
   const taskRead=(await taskV2("GetTask",lead,{taskId})).body;
   const dealRead=(await salesV2("GetDeal",lead,{dealId})).body;
   assert.deepEqual(taskRead.teamDistribution,dealRead.teamDistribution);
   assert.deepEqual(taskRead.teamDistribution.eligibleMemberEmails,[leadEmail,targetEmail].sort());
   assert.equal((await taskV2("GetTask",target,{taskId})).body.teamDistribution,undefined);
   assert.equal((await taskV2("GetTask",outside,{taskId})).body.teamDistribution,undefined);
   let scope=taskRead.teamDistribution.scope;
   const currentGrant=async()=>(await memberV2("GetTeam",viewer,{teamId:tid})).body.team.queueCoordinators[0];
   assert.equal((await currentGrant()).effective,true);
   assert.equal((await memberV2("ListTeams",viewer,{page:{pageSize:100}})).body.teams.find(t=>t.teamId===tid).queueCoordinators[0].effective,true);
   const effects=async()=>(await sql("SELECT (SELECT COUNT(*) FROM crm_audit) AS audit,(SELECT COUNT(*) FROM crm_activity_task_receipts) AS tasks,(SELECT COUNT(*) FROM crm_sales_deal_receipts) AS deals,(SELECT COUNT(*) FROM crm_sales_deal_events) AS events"))[0];
   const beforeDenied=await effects();
   const targetOwner={owner:{case:"assigneeIdentity",value:targetEmail}};
   const taskMove=(id,key,owner=targetOwner,version=1n)=>({taskId:id,owner,teamDistribution:scope,precondition:pre(key,version)});
   const dealMove={dealId,owner:targetOwner,teamDistribution:scope,precondition:pre("deal-move",1n)};
   assert.equal((await taskV2("UpdateTask",target,taskMove(taskId,"nonlead"))).status,403);
   assert.equal((await taskV2("UpdateTask",lead,{...taskMove(taskId,"field"),subject:{operation:{case:"set",value:"Forbidden"}}})).status,403);
   assert.equal((await taskV2("UpdateTask",lead,taskMove(taskId,"outside",{owner:{case:"assigneeIdentity",value:otherEmail}}))).status,403);
   assert.equal((await taskV2("UpdateTask",lead,taskMove("foreign-task-"+suffix,"cross-team"))).status,403);
   assert.equal((await salesV2("UpdateDeal",lead,{...dealMove,probabilityBasisPoints:5000,precondition:pre("money",1n)})).status,403);
   assert.equal((await salesV2("UpdateDeal",lead,{...dealMove,owner:{owner:{case:"assigneeIdentity",value:otherEmail}},precondition:pre("outside-deal",1n)})).status,403);
   assert.equal((await taskV2("UpdateTask",tenantB,taskMove(taskId,"cross-tenant-task"))).status,403);
   assert.equal((await salesV2("UpdateDeal",tenantB,{...dealMove,precondition:pre("cross-tenant-deal",1n)})).status,403);
   assert.deepEqual(await effects(),beforeDenied);
   const command=taskMove(taskId,"move");
   const moved=await taskV2("UpdateTask",lead,command);
   assert.equal(moved.status,200,describeProtobuf(moved.body));
   assert.equal(moved.body.task.owner.owner.value,targetEmail);
   assert.equal((await taskV2("UpdateTask",lead,command)).body.receipt.replayed,true);
   const movedDeal=await salesV2("UpdateDeal",lead,dealMove);
   assert.equal(movedDeal.status,200,describeProtobuf(movedDeal.body));
   assert.equal(movedDeal.body.deal.owner.owner.value,targetEmail);
   assert.equal((await salesV2("UpdateDeal",lead,dealMove)).body.receipt.replayed,true);
   assert.equal((await salesV2("GetDeal",lead,{dealId})).body.teamDistribution,undefined);
   const afterMoves=await effects();
   assert.equal((await taskV2("UpdateTask",lead,taskMove(taskId,"already-personal",targetOwner,2n))).status,403);
   assert.equal((await salesV2("UpdateDeal",lead,{...dealMove,precondition:pre("already-personal-deal",2n)})).status,403);
   assert.deepEqual(await effects(),afterMoves);
   assert.equal((await memberV2("UpdateMember",admin,{email:leadEmail,active:false,precondition:pre("deactivate",1n)})).status,200);
   assert.equal((await currentGrant()).effective,false);
   assert.equal((await memberV2("UpdateMember",admin,{email:leadEmail,active:true,precondition:pre("reactivate",2n)})).status,200);
   assert.equal((await currentGrant()).effective,false);
   assert.equal((await taskV2("GetTask",lead,{taskId:taskRace})).body.teamDistribution,undefined);
   assert.equal((await taskV2("UpdateTask",lead,taskMove(taskRace,"stale-enrollment"))).status,403);
   const renewed=await memberV2("UpdateTeam",admin,{...grant,precondition:pre("renew",2n)});
   assert.equal(renewed.status,200,describeProtobuf(renewed.body));
   assert.equal((await currentGrant()).memberVersion,3n);
   assert.equal((await currentGrant()).effective,true);
   const roster=async(emails,key,version)=>memberV2("UpdateTeam",admin,{teamId:tid,roster:{memberEmails:emails},precondition:pre(key,version)});
   assert.equal((await roster([leadEmail],"remove-target",3n)).status,200);
   scope=(await taskV2("GetTask",lead,{taskId:taskRace})).body.teamDistribution.scope;
   assert.equal((await taskV2("UpdateTask",lead,taskMove(taskRace,"removed-target"))).status,403);
   assert.equal((await roster([leadEmail,targetEmail],"restore-target",4n)).status,200);
   assert.equal((await roster([targetEmail],"remove-coordinator",5n)).status,200);
   assert.equal(await currentGrant(),undefined);
   assert.equal((await roster([leadEmail,targetEmail],"restore-coordinator",6n)).status,200);
   assert.equal((await taskV2("GetTask",lead,{taskId:taskRace})).body.teamDistribution,undefined);
   assert.equal((await memberV2("UpdateTeam",admin,{...grant,precondition:pre("regrant",7n)})).status,200);
   scope=(await taskV2("GetTask",lead,{taskId:taskRace})).body.teamDistribution.scope;
   const revoke={teamId:tid,queueCoordinator:{email:leadEmail,enabled:false},precondition:pre("revoke",8n)};
   const [removed,race]=await Promise.all([memberV2("UpdateTeam",admin,revoke),taskV2("UpdateTask",lead,taskMove(taskRace,"race"))]);
   assert.equal(removed.status,200,describeProtobuf(removed.body));
   assert.ok([200,403,409].includes(race.status),describeProtobuf(race.body));
   assert.equal((await taskV2("UpdateTask",lead,command)).status,403);
   assert.equal((await salesV2("UpdateDeal",lead,dealMove)).status,403);
   assert.equal((await memberV2("GetTeam",viewer,{teamId:tid})).body.team.queueCoordinators.length,0);
   console.log(JSON.stringify({event:"crm.native-team-distribution",result:"PASS",shared_access_template:true,task_deal:true,scoped_targets:true,owner_only:true,replay:true,revocation:true,effective_projection:true,enrollment_pin:true,roster_removal:true,tenant_isolation:true,rejected_side_effects:false,race:race.status}));
 }
 // SK-08A: the only first executable use case is a signed administrator
 // explicitly running a published rule for one original Interaction.
 // The write delegates to the original Task aggregate, never direct SQL.
 {
   const suffix=randomUUID(),ruleId="automation-rule-"+suffix;
   const sourceId="automation-source-"+suffix;
   const person="automation-person-"+suffix+"@example.test";
   const rkey=(label,version)=>({idempotencyKey:"automation-"+label+"-"+suffix,
     ...(version===undefined?{}:{expectedVersion:version})});
   const automationV2=async(method,token,payload)=>{
     const response=await rpcFetch(automationV2Path(method),{method:"POST",
       headers:{"content-type":"application/proto","connect-protocol-version":"1",
         ...(token?{"cf-access-jwt-assertion":token}:{})},
       body:binaryAutomationV2(method,payload)});
     return {status:response.status,headers:response.headers,
       body:response.status<400
         ?decodeAutomationV2(method,new Uint8Array(await response.arrayBuffer()))
         :await response.json()};
   };
   assert.equal((await memberV2("ProvisionMember",admin,{
     email:person,role:1,precondition:rkey("provision")
   })).status,200);
   const put={ruleId,trigger:1,enabled:true,
     taskSubject:"SK08A customer follow-up",assigneeEmail:person,
     dueAfterSeconds:3600n,precondition:rkey("rule")};
   assert.equal((await automationV2("PutAutomationRule",viewer,put)).status,403);
   assert.equal((await automationV2("PutAutomationRule",admin,{
     ...put,trigger:2,precondition:rkey("unsupported")
   })).status,400);
   const created=await automationV2("PutAutomationRule",admin,put);
   assert.equal(created.status,200,describeProtobuf(created.body));
   assert.equal(created.body.rule.version,1n);
   assert.equal((await automationV2("PutAutomationRule",admin,put)).body.receipt.replayed,true);
   assert.equal((await automationV2("PutAutomationRule",admin,{
     ...put,taskSubject:"Another intent, same key"
   })).status,409);
   assert.equal((await automationV2("GetAutomationRule",tenantB,{ruleId})).status,404);
   const read=await automationV2("GetAutomationRule",viewer,{
     ruleId,receiptIdempotencyKey:put.precondition.idempotencyKey
   });
   assert.equal(read.status,200);
   assert.deepEqual(read.body.rule,created.body.rule);
   assert.equal(read.body.confirmedMutation?.resourceVersion,1n);
   const all=await automationV2("ListAutomationRules",viewer,{page:{pageSize:100}});
   assert.equal(all.status,200);
   assert.ok(all.body.rules.some(value=>value.ruleId===ruleId));
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_rule_versions WHERE tenant=? AND rule_id=?",
     "tenant-a",ruleId))[0].n,1);
   const interaction=(id,label)=>({
     interactionId:id,accountId,kind:1,text:"Automated interaction "+label,
     occurredAt:{seconds:1780000000n,nanos:0},
     precondition:rkey("interaction-"+label)
   });
   assert.equal((await interactionV2("CreateInteraction",admin,
     interaction(sourceId,"one"))).status,200);
   const runInput=(id,keyLabel)=>({ruleId,interactionId:id,
     expectedRuleVersion:1n,precondition:rkey(keyLabel)});
   const firstRun=runInput(sourceId,"run");
   assert.equal((await automationV2("RunInteractionAutomation",viewer,firstRun)).status,403);
   assert.equal((await automationV2("RunInteractionAutomation",tenantB,firstRun)).status,404);
   const startCount=(await sql("SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0].n;
   const executed=await automationV2("RunInteractionAutomation",admin,firstRun);
   assert.equal(executed.status,200,describeProtobuf(executed.body));
   assert.equal(executed.body.receipt.resourceVersion,1n);
   assert.equal(executed.body.run.status,2);
   assert.equal(executed.body.run.disposition,1);
   assert.equal(executed.body.run.attemptCount,1n);
   assert.equal(executed.body.run.canCancel,false);
   const taskId=executed.body.run.taskId;
   const task=(await taskV2("GetTask",viewer,{taskId})).body.task;
   assert.equal(task.accountId,accountId);
   assert.equal(task.subject,put.taskSubject);
   assert.equal(task.owner.owner.value,person);
   assert.equal(task.status,1);
   assert.ok(task.dueAt.seconds>0n);
   const replay=await automationV2("RunInteractionAutomation",admin,firstRun);
   assert.equal(replay.status,200,describeProtobuf(replay.body));
   assert.equal(replay.body.receipt.replayed,true);
   assert.equal(replay.body.run.taskId,taskId);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0].n,startCount+1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",taskId))[0].n,1);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     "tenant-a",taskId))[0].n,1);
   const proof=await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:sourceId,receiptIdempotencyKey:firstRun.precondition.idempotencyKey
   });
   assert.equal(proof.status,200);
   assert.equal(proof.body.confirmedMutation.resourceVersion,1n);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:sourceId,receiptIdempotencyKey:"wrong-request-key"
   })).body.confirmedMutation,undefined);
   assert.ok((await automationV2("ListAutomationRuns",viewer,{ruleId,page:{pageSize:25}}))
     .body.runs.some(x=>x.taskId===taskId&&x.status===2));
   assert.equal((await automationV2("ListAutomationRuns",tenantB,{ruleId,page:{pageSize:25}}))
     .body.runs.length,0);
   // A pending run preserves the exact snapshot after human revocation. It
   // cannot create a Task until that human is reactivated by signed Access.
   const second="automation-second-"+suffix;
   assert.equal((await interactionV2("CreateInteraction",admin,
     interaction(second,"two"))).status,200);
   assert.equal((await memberV2("UpdateMember",admin,{
     email:person,active:false,precondition:rkey("revoke",1n)
   })).status,200);
   const alias=runInput(sourceId,"run-alias");
   const aliasReplay=await automationV2("RunInteractionAutomation",admin,alias);
   assert.equal(aliasReplay.status,200,describeProtobuf(aliasReplay.body));
   assert.equal(aliasReplay.body.receipt.replayed,true);
   assert.equal(aliasReplay.body.run.taskId,taskId);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:sourceId,receiptIdempotencyKey:alias.precondition.idempotencyKey
   })).body.confirmedMutation.resourceVersion,1n);
   assert.equal((await automationV2("RunInteractionAutomation",admin,{
     ...alias,expectedRuleVersion:2n
   })).status,409);
   assert.equal((await automationV2("RunInteractionAutomation",admin,{
     ...firstRun,interactionId:second
   })).status,409);
   assert.equal((await automationV2("RunInteractionAutomation",admin,{
     ...alias,interactionId:second
   })).status,409);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:second
   })).status,404);
   const secondInput=runInput(second,"run-two");
   assert.equal((await automationV2("RunInteractionAutomation",admin,secondInput)).status,400);
   const pending=await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:second});
   assert.equal(pending.status,200);
   assert.equal(pending.body.run.status,1);
   assert.equal(pending.body.run.disposition,3);
   assert.equal(pending.body.run.attemptCount,0n);
   assert.equal(pending.body.run.canCancel,true);
   assert.equal(pending.body.confirmedMutation,undefined);
   const waitingList=await automationV2("ListAutomationRuns",viewer,{ruleId,page:{pageSize:25}});
   assert.equal(waitingList.body.runs.find(x=>x.interactionId===second).disposition,3);
   assert.equal((await memberV2("UpdateMember",admin,{
     email:person,active:true,precondition:rkey("reactivate",2n)
   })).status,200);
   const ready=await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:second});
   assert.equal(ready.body.run.disposition,2);
   assert.equal((await automationV2("RunInteractionAutomation",admin,{
     ...runInput(second,"wrong-pinned-version"),expectedRuleVersion:2n
   })).status,400);
   const recovered=await automationV2("RunInteractionAutomation",admin,secondInput);
   assert.equal(recovered.status,200,describeProtobuf(recovered.body));
   assert.equal(recovered.body.run.status,2);
   assert.equal((await automationV2("RunInteractionAutomation",admin,secondInput))
     .body.receipt.replayed,true);
   // Concurrent distinct sources with a single request key: at most one
   // claim and one original Task. The losing source must have no run row.
   const raceA="automation-key-race-a-"+suffix,raceB="automation-key-race-b-"+suffix;
   for(const [id,label] of [[raceA,"key-a"],[raceB,"key-b"]])
     assert.equal((await interactionV2("CreateInteraction",admin,interaction(id,label))).status,200);
   const keyRace=await Promise.all([raceA,raceB].map(id=>
     automationV2("RunInteractionAutomation",admin,runInput(id,"shared-key"))));
   assert.deepEqual(keyRace.map(x=>x.status).sort((a,b)=>a-b),[200,409]);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND interaction_id IN (?,?)",
     "tenant-a",ruleId,raceA,raceB))[0].n,1);

   // Native D1 task-audit failure: actual write rolls back, attempt is
   // durably bounded and the original intent remains available for recovery.
   const exhausted="automation-exhausted-"+suffix;
   assert.equal((await interactionV2("CreateInteraction",admin,interaction(exhausted,"exhausted"))).status,200);
   const exhaustedInput=runInput(exhausted,"exhausted-run");
   await sql("CREATE TRIGGER fixture_fail_automation_task BEFORE INSERT ON crm_audit "+
     "WHEN NEW.action='activity_tasks.create' BEGIN SELECT RAISE(ABORT,'fixture task rollback'); END;");
   let failed;
   try {
     for(let i=1;i<=5;i++){
       assert.equal((await automationV2("RunInteractionAutomation",admin,exhaustedInput)).status,503);
       failed=(await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:exhausted})).body.run;
       assert.equal(failed.attemptCount,BigInt(i));
       assert.equal(failed.disposition,i===5?8:7);
       assert.equal((await automationV2("RunInteractionAutomation",admin,exhaustedInput)).status,400);
       // Seed elapsed backoff in the same native D1; no wall-clock sleeps.
       if(i<5)await sql("UPDATE crm_activity_automation_runs SET next_attempt_at_unix_millis=0 WHERE tenant=? AND task_id=?","tenant-a",failed.taskId);
     }
   }finally{await sql("DROP TRIGGER fixture_fail_automation_task");}
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?","tenant-a",failed.taskId))[0].n,0);
   const cancelInput={ruleId,interactionId:exhausted,expectedRuleVersion:1n,precondition:rkey("cancel-exhausted")};
   assert.equal((await automationV2("CancelAutomationRun",viewer,cancelInput)).status,403);
   assert.equal((await automationV2("CancelAutomationRun",tenantB,cancelInput)).status,404);
   assert.equal((await automationV2("CancelAutomationRun",admin,{
     ...cancelInput,precondition:exhaustedInput.precondition
   })).status,409);
   const cancelled=await automationV2("CancelAutomationRun",admin,cancelInput);
   assert.equal(cancelled.status,200,describeProtobuf(cancelled.body));
   assert.equal(cancelled.body.run.status,3);
   assert.equal(cancelled.body.run.disposition,9);
   assert.equal(cancelled.body.run.canCancel,false);
   assert.equal((await automationV2("CancelAutomationRun",admin,cancelInput)).body.receipt.replayed,true);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:exhausted,receiptIdempotencyKey:cancelInput.precondition.idempotencyKey
   })).body.confirmedMutation.originalRequestId,cancelled.body.receipt.originalRequestId);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:exhausted,receiptIdempotencyKey:exhaustedInput.precondition.idempotencyKey
   })).body.confirmedMutation,undefined);
   assert.equal((await automationV2("RunInteractionAutomation",admin,exhaustedInput)).status,400);
   await assert.rejects(()=>sql(
     "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) "+
     "SELECT tenant,?,account_id,state,version,?,idem_digest,request_id,actor FROM crm_activity_task_records WHERE tenant=? AND id=?",
     failed.taskId,failed.taskId,"tenant-a",taskId),/AUTOMATION_TASK_LEASE_REQUIRED/);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activities.automation.run.cancel'","tenant-a",failed.taskId))[0].n,1);
   assert.equal((await automationV2("CancelAutomationRun",admin,{
     ruleId,interactionId:sourceId,expectedRuleVersion:1n,precondition:rkey("cancel-committed")
   })).status,400);
   // Two real signed calls to one run may return a replay or contention.
   // Neither can create a second Task or consume another successful attempt.
   const leaseRace="automation-lease-race-"+suffix;
   assert.equal((await interactionV2("CreateInteraction",admin,interaction(leaseRace,"lease-race"))).status,200);
   const leaseResults=await Promise.all(["lease-a","lease-b"].map(key=>
     automationV2("RunInteractionAutomation",admin,runInput(leaseRace,key))));
   assert.ok(leaseResults.some(x=>x.status===200));
   assert.ok(leaseResults.every(x=>[200,409].includes(x.status)));
   const leaseRun=(await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:leaseRace})).body.run;
   assert.equal(leaseRun.attemptCount,1n);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'","tenant-a",leaseRun.taskId))[0].n,1);
   // Keep one durable claim without a Task; test the commit-time D1 guard
   // against the exact pending row after rule revocation, not a mock.
   const blocked="automation-blocked-"+suffix;
   assert.equal((await interactionV2("CreateInteraction",admin,interaction(blocked,"blocked"))).status,200);
   assert.equal((await memberV2("UpdateMember",admin,{
     email:person,active:false,precondition:rkey("revoke-blocked",3n)
   })).status,200);
   const blockedInput=runInput(blocked,"blocked-run");
   assert.equal((await automationV2("RunInteractionAutomation",admin,blockedInput)).status,400);
   assert.equal((await memberV2("UpdateMember",admin,{
     email:person,active:true,precondition:rkey("reactivate-blocked",4n)
   })).status,200);
   const blockedRun=(await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:blocked
   })).body.run;
   assert.equal(blockedRun.status,1);
   // Native durable lease recovery fixture: seed a previously acquired lease
   // as if its worker stopped; verify the live API and physical write fence.
   await sql("UPDATE crm_activity_automation_runs SET attempt_count=1,lease_request_id=?,lease_actor=?,lease_until_unix_millis=unixepoch('now')*1000+60000,next_attempt_at_unix_millis=unixepoch('now')*1000+65000 WHERE tenant=? AND task_id=?",
     "fixture-lease-"+suffix,person,"tenant-a",blockedRun.taskId);
   assert.equal((await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:blocked})).body.run.disposition,6);
   assert.equal((await automationV2("RunInteractionAutomation",admin,blockedInput)).status,409);
   await assert.rejects(()=>sql(
     "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) "+
     "SELECT tenant,?,account_id,state,version,?,idem_digest,request_id,actor FROM crm_activity_task_records WHERE tenant=? AND id=?",
     blockedRun.taskId,blockedRun.taskId,"tenant-a",taskId),/AUTOMATION_TASK_LEASE_REQUIRED/);
   await sql("UPDATE crm_activity_automation_runs SET lease_until_unix_millis=0,next_attempt_at_unix_millis=0 WHERE tenant=? AND task_id=?",
     "tenant-a",blockedRun.taskId);
   assert.equal((await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:blocked})).body.run.disposition,2);
   // Native D1 aborts only the run finalization audit; original Task and
   // receipt were committed in their own atomic owner statement.
   const reconcileSource="automation-reconcile-"+suffix;
   assert.equal((await interactionV2("CreateInteraction",admin,
     interaction(reconcileSource,"reconcile"))).status,200);
   const reconcileInput=runInput(reconcileSource,"reconcile-run");
   await sql("CREATE TRIGGER fixture_fail_automation_commit BEFORE INSERT ON crm_audit "+
     "WHEN NEW.action='activities.automation.run.committed' "+
     "BEGIN SELECT RAISE(ABORT,'fixture run finalization rollback'); END;");
   try {
     assert.equal((await automationV2("RunInteractionAutomation",admin,reconcileInput)).status,503);
   } finally {await sql("DROP TRIGGER fixture_fail_automation_commit");}
   const unreconciled=await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:reconcileSource,receiptIdempotencyKey:reconcileInput.precondition.idempotencyKey
   });
   assert.equal(unreconciled.body.run.status,1);
   assert.equal(unreconciled.body.run.disposition,5);
   assert.equal(unreconciled.body.run.canCancel,false);
   assert.equal((await automationV2("CancelAutomationRun",admin,{
     ruleId,interactionId:reconcileSource,expectedRuleVersion:1n,precondition:rkey("cancel-receipt")
   })).status,400);
   assert.equal(unreconciled.body.confirmedMutation,undefined);
   const reconcileTask=unreconciled.body.run.taskId;
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",reconcileTask))[0].n,1);
   const disabled=await automationV2("PutAutomationRule",admin,{
     ...put,enabled:false,precondition:rkey("disable",1n)
   });
   assert.equal(disabled.status,200,describeProtobuf(disabled.body));
   assert.equal(disabled.body.rule.version,2n);
   const terminal=await automationV2("GetAutomationRun",viewer,{ruleId,interactionId:blocked});
   assert.equal(terminal.body.run.disposition,4);
   const reconcileAfterDisable=await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:reconcileSource
   });
   assert.equal(reconcileAfterDisable.body.run.disposition,5);
   assert.equal((await automationV2("RunInteractionAutomation",viewer,reconcileInput)).status,403);
   const reconciled=await automationV2("RunInteractionAutomation",admin,reconcileInput);
   assert.equal(reconciled.status,200,describeProtobuf(reconciled.body));
   assert.equal(reconciled.body.run.disposition,1);
   assert.equal(reconciled.body.receipt.replayed,true);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:reconcileSource,receiptIdempotencyKey:reconcileInput.precondition.idempotencyKey
   })).body.confirmedMutation.resourceVersion,1n);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='activity_tasks.create'",
     "tenant-a",reconcileTask))[0].n,1);
   // Attempt the same physical Task INSERT boundary after disabling the rule.
   // A real D1 trigger, not the earlier Rust check, must reject it.
   await assert.rejects(()=>sql(
     "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) "+
     "SELECT tenant,?,account_id,state,version,?,idem_digest,request_id,actor "+
     "FROM crm_activity_task_records WHERE tenant=? AND id=?",
     blockedRun.taskId,blockedRun.taskId,"tenant-a",taskId),/AUTOMATION_RULE_NOT_ENABLED_OR_STALE/);
   assert.equal((await automationV2("RunInteractionAutomation",admin,blockedInput)).status,400);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id=?",
     "tenant-a",blockedRun.taskId))[0].n,0);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
     "tenant-a",blockedRun.taskId))[0].n,0);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_automation_rule_versions WHERE tenant=? AND rule_id=?",
     "tenant-a",ruleId))[0].n,2);
   const third="automation-third-"+suffix;
   assert.equal((await interactionV2("CreateInteraction",admin,
     interaction(third,"three"))).status,200);
   const countBeforeDenied=(await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0].n;
   assert.equal((await automationV2("RunInteractionAutomation",admin,{
     ...runInput(third,"run-three"),expectedRuleVersion:2n
   })).status,400);
   assert.equal((await sql(
     "SELECT COUNT(*) n FROM crm_activity_task_records WHERE tenant=? AND id LIKE 'automation-%'",
     "tenant-a"))[0].n,countBeforeDenied);
   assert.equal((await automationV2("GetAutomationRun",viewer,{
     ruleId,interactionId:third
   })).status,404);
   console.log(JSON.stringify({event:"crm.native-sk08a-rule-run",result:"PASS",
     generated_connect:true,signed_access:true,original_task_owner:true,
     exact_source_receipt:true,tenant_ab:true,viewer_write_denied:true,
     rule_version_cas:true,single_task_audit:true,exact_replay:true,
     durable_pending:true,revocation_recovery:true,disabled_fail_closed:true,
     request_key_conflict:true,concurrent_key_conflict:true,alias_receipt:true,
     atomic_rule_guard:true,typed_disposition:true,pinned_retry:true,
     original_receipt_reconciliation_after_disable:true,bounded_attempts:true,backoff:true,
     cancellation_receipt:true,cancelled_task_fence:true,concurrent_lease:true,
     expired_lease_readiness:true,stale_writer_fence:true,production_cron_disabled:true}));
 }
 return businessEvidence();
 });
 writeRuntimePart(import.meta.url,evidence);
});

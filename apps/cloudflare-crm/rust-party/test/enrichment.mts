import assert from "node:assert/strict";
import { EnrichmentConfigurationService, RawPayloadPolicy } from "../../../../packages/client/gen/crm/customer_enrichment/v2/config_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { collectFixturePages, decodeFixtureDetail, fixtureRpc, fixtureRpcError, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records } from "../../scripts/json-boundary.mts";
import { formatRpcFailure } from "../../scripts/rpc-error-response.mts";

type EnrichmentFixture = {
 dispatch: RpcDispatch;
 sql: (statement: string, ...bindings: string[]) => Promise<ReadonlyArray<Record<string, unknown>>>;
 admin: string;
 viewer: string;
 tenantB: string;
 knownService: string;
 unknownService: string;
};

export async function verifyEnrichment({dispatch,sql,admin,viewer,tenantB,knownService,unknownService}: EnrichmentFixture) {
 const methods=EnrichmentConfigurationService.method;
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
 const profileInput={
   providerKey:"company_registry",adapterKind:"registry_http_v1",adapterContractVersion:"1.0.0",
   purposeCodes:["customer_profile_enrichment","due_diligence"],
   licenseId:"Registry commercial data licence v3",permittedUseClass:"customer_master_review",
   residencyRegion:"eu",retentionDays:30,rawPayloadPolicy:1,
   credentialHandleAliases:["registry_primary"],
   effectiveAt:{seconds:1n,nanos:123000000},expiresAt:{seconds:2n},
   precondition:{idempotencyKey:"enrichment-profile-main"}
 };
 const enrichmentAuditBefore=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const partyCountBefore=await count("SELECT COUNT(*) AS n FROM crm_party_records");
 for(const bad of [
   {rawPayloadPolicy:0},{rawPayloadPolicy:999 as RawPayloadPolicy},{effectiveAt:undefined},
   {effectiveAt:{seconds:1n,nanos:1}},{effectiveAt:{seconds:-1n}},
   {effectiveAt:{seconds:253402300800n}},{effectiveAt:{seconds:1n,nanos:1000000000}},
   {retentionDays:3651},{expiresAt:{seconds:0n}},
   {precondition:{idempotencyKey:"enrichment-invalid-version",expectedVersion:1n}}
 ]){
   const result=await fixtureRpcError(dispatch, methods.publishProviderProfile, admin, {...profileInput,...bad});
   assert.equal(result.body.code,"invalid_argument",formatRpcFailure(result));
 }
 const profileResult=await fixtureRpc(dispatch, methods.publishProviderProfile, admin, profileInput, {
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test",
   "x-request-id":"forged-enrichment-request"
 });
 assert.equal(profileResult.status,200,formatRpcFailure(profileResult));
 assert.ok(profileResult.body.profile?.effectiveAt && profileResult.body.receipt);
 const profileId=profileResult.body.profile.profileVersionId;
 assert.ok(profileId);
 assert.equal(profileResult.body.profile.effectiveAt.nanos,123000000);
 assert.equal(profileResult.body.receipt.resourceVersion,1n);
 assert.equal(profileResult.body.receipt.requestId,profileResult.headers.get("x-request-id"));
 assert.equal(profileResult.body.receipt?.replayed,false);
 assert.equal((await fixtureRpc(dispatch, methods.publishProviderProfile, knownService, profileInput)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch, methods.publishProviderProfile, admin, {...profileInput,retentionDays:31})).body.code,"aborted");
 assert.equal((await fixtureRpc(dispatch, methods.getProviderProfile, viewer, {profileVersionId:profileId})).body.profile?.profileVersionId,profileId);
 assert.equal((await fixtureRpcError(dispatch, methods.getProviderProfile, tenantB, {profileVersionId:profileId})).body.code,"not_found");
 const enrichmentAuditBeforeProfileReceiptReads=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const profileReceiptRead=await fixtureRpc(dispatch, methods.getProviderProfile, viewer, {
   profileVersionId:"",receiptIdempotencyKey:profileInput.precondition.idempotencyKey});
 assert.equal(profileReceiptRead.status,200,formatRpcFailure(profileReceiptRead));
 assert.equal(profileReceiptRead.body.profile?.profileVersionId,profileId);
 assert.equal(profileReceiptRead.body.confirmedMutation?.originalRequestId,profileResult.headers.get("x-request-id"));
 assert.equal(profileReceiptRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal(profileReceiptRead.body.confirmedMutation?.requestId,profileReceiptRead.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch, methods.getProviderProfile, tenantB, {
   profileVersionId:"",receiptIdempotencyKey:profileInput.precondition.idempotencyKey})).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch, methods.getProviderProfile, viewer, {profileVersionId:""})).body.code,"invalid_argument");
 assert.equal((await fixtureRpcError(dispatch, methods.getProviderProfile, viewer, {
   profileVersionId:"",receiptIdempotencyKey:"short"})).body.code,"invalid_argument");
 assert.equal((await fixtureRpcError(dispatch, methods.getProviderProfile, viewer, {
   profileVersionId:"",receiptIdempotencyKey:"enrichment-profile-missing"})).body.code,"not_found");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),enrichmentAuditBeforeProfileReceiptReads,
   "Profile receipt lookup is read-only");
 const mappingInput={
   mappingKey:"party_display_name",providerProfileVersionId:profileId,
   providerResponseFieldPath:"organization.legal_name",maximumSuggestionsPerResponse:1,
   confidenceRequired:true,precondition:{idempotencyKey:"enrichment-mapping-main"}
 };
 assert.equal((await fixtureRpcError(dispatch, methods.publishMapping, tenantB, mappingInput)).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch, methods.publishMapping, admin, {...mappingInput,maximumSuggestionsPerResponse:0})).body.code,"invalid_argument");
 const mappingResult=await fixtureRpc(dispatch, methods.publishMapping, knownService, mappingInput);
 assert.equal(mappingResult.status,200,formatRpcFailure(mappingResult));
 assert.ok(mappingResult.body.mapping && mappingResult.body.receipt);
 const mappingId=mappingResult.body.mapping.mappingVersionId;
 assert.equal(mappingResult.body.mapping.providerProfileVersionId,profileId);
 assert.equal(mappingResult.body.receipt?.replayed,false);
 assert.equal((await fixtureRpc(dispatch, methods.publishMapping, admin, mappingInput)).body.receipt?.replayed,true);
 assert.equal((await fixtureRpcError(dispatch, methods.publishMapping, admin, {...mappingInput,confidenceRequired:false})).body.code,"aborted");
 assert.equal((await fixtureRpcError(dispatch, methods.publishMapping, admin, {...mappingInput,
   precondition:{idempotencyKey:"enrichment-profile-main"}})).body.code,"aborted");
 assert.equal((await fixtureRpc(dispatch, methods.getMapping, viewer, {mappingVersionId:mappingId})).body.mapping?.mappingVersionId,mappingId);
 assert.equal((await fixtureRpcError(dispatch, methods.getMapping, tenantB, {mappingVersionId:mappingId})).body.code,"not_found");
 const enrichmentAuditBeforeMappingReceiptReads=await count("SELECT COUNT(*) AS n FROM crm_audit");
 const mappingReceiptRead=await fixtureRpc(dispatch, methods.getMapping, viewer, {
   mappingVersionId:"",receiptIdempotencyKey:mappingInput.precondition.idempotencyKey});
 assert.equal(mappingReceiptRead.status,200,formatRpcFailure(mappingReceiptRead));
 assert.equal(mappingReceiptRead.body.mapping?.mappingVersionId,mappingId);
 assert.equal(mappingReceiptRead.body.confirmedMutation?.originalRequestId,mappingResult.headers.get("x-request-id"));
 assert.equal(mappingReceiptRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal(mappingReceiptRead.body.confirmedMutation?.requestId,mappingReceiptRead.headers.get("x-request-id"));
 assert.equal((await fixtureRpcError(dispatch, methods.getMapping, tenantB, {
   mappingVersionId:"",receiptIdempotencyKey:mappingInput.precondition.idempotencyKey})).body.code,"not_found");
 assert.equal((await fixtureRpcError(dispatch, methods.getMapping, viewer, {
   mappingVersionId:"",receiptIdempotencyKey:"enrichment-profile-main"})).body.code,"not_found");
 assert.equal((await fixtureRpc(dispatch, methods.getMapping, viewer, {
   mappingVersionId:mappingId,receiptIdempotencyKey:"enrichment-profile-main"
 })).body.confirmedMutation,undefined,"a profile receipt cannot confirm a mapping");
 assert.equal((await fixtureRpcError(dispatch, methods.getProviderProfile, viewer, {
   profileVersionId:"",receiptIdempotencyKey:mappingInput.precondition.idempotencyKey
 })).body.code,"not_found","a mapping receipt cannot confirm a provider profile");
 assert.equal((await fixtureRpcError(dispatch, methods.getMapping, viewer, {mappingVersionId:""})).body.code,"invalid_argument");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),enrichmentAuditBeforeMappingReceiptReads,
   "Mapping receipt lookup is read-only");
 await sql("ALTER TABLE crm_enrichment_receipts RENAME TO fixture_unavailable_enrichment_receipts");
 try {
   assert.equal((await sendFixtureRpc(dispatch, methods.getProviderProfile, viewer, {
     profileVersionId:"",receiptIdempotencyKey:profileInput.precondition.idempotencyKey}, 503)).status,503,
     "an unavailable receipt source must not confirm a profile write");
   assert.equal((await fixtureRpc(dispatch, methods.getProviderProfile, viewer, {profileVersionId:profileId})).status,200,
     "ordinary immutable Get by ID remains available without receipt recovery");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_enrichment_receipts RENAME TO crm_enrichment_receipts");
 }
 assert.deepEqual((await sql("SELECT action,subject_id,request_id,actor FROM crm_audit WHERE "+
   "tenant_id='tenant-a' AND subject_id IN ('"+profileId+"','"+mappingId+"') ORDER BY rowid")),[
   {action:"customer_enrichment.profile_publish",subject_id:profileId,
     request_id:profileResult.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"customer_enrichment.mapping_publish",subject_id:mappingId,
     request_id:mappingResult.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Immutable definition audit must retain each original signed Access actor and request");
 assert.deepEqual((await sql("SELECT created_by FROM crm_enrichment_profiles WHERE id='"+profileId+"'")),
   [{created_by:"native-a@example.test"}]);
 assert.deepEqual((await sql("SELECT created_by FROM crm_enrichment_mappings WHERE id='"+mappingId+"'")),
   [{created_by:"service:ci-fixture.access"}]);
 const enrichmentSnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(id,tenant,state,created_by)) "+
   " FROM crm_enrichment_profiles ORDER BY id) AS profiles,"+
   "(SELECT json_group_array(json_array(id,tenant,state,created_by)) "+
   " FROM crm_enrichment_mappings ORDER BY id) AS mappings,"+
   "(SELECT COUNT(*) FROM crm_enrichment_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 const enrichmentBeforeDenials=(await enrichmentSnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await fixtureRpcError(dispatch, methods.publishProviderProfile, token, profileInput)).status>=400);
   assert.ok((await fixtureRpcError(dispatch, methods.publishMapping, token, mappingInput)).status>=400);
 }
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await sendFixtureRpc(dispatch, methods.publishMapping, knownService, mappingInput, 401)).status,401,
   "Inactive service cannot replay a committed Enrichment mapping");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await sendFixtureRpc(dispatch, methods.publishMapping, knownService, mappingInput, 403)).status,403,
   "Permission must precede replay of immutable mapping");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.deepEqual(await enrichmentSnapshot(),enrichmentBeforeDenials,"Denied calls cannot write definition, receipt or audit");
 const tenantAEnrichment=async()=>(await sql("SELECT "+
   "(SELECT COUNT(*) FROM crm_enrichment_profiles WHERE tenant='tenant-a') AS profiles,"+
   "(SELECT COUNT(*) FROM crm_enrichment_mappings WHERE tenant='tenant-a') AS mappings,"+
   "(SELECT COUNT(*) FROM crm_enrichment_receipts WHERE tenant='tenant-a') AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit WHERE tenant_id='tenant-a') AS audits"));
 const tenantABeforeOtherTenant=(await tenantAEnrichment());
 assert.equal((await fixtureRpc(dispatch, methods.publishProviderProfile, tenantB, profileInput)).status,200,
   "Separate tenant may publish its own independent immutable definition");
 assert.equal((await fixtureRpcError(dispatch, methods.getMapping, tenantB, {mappingVersionId:mappingId})).body.code,"not_found");
 assert.deepEqual((await tenantAEnrichment()),tenantABeforeOtherTenant,
   "Tenant B must not mutate tenant A's state, receipts or audit");
 assert.deepEqual((await sql("SELECT created_by FROM crm_enrichment_profiles "+
   "WHERE tenant='tenant-a' AND id='"+profileId+"'")),[
   {created_by:"native-a@example.test"}
 ]);
 // Failing audit INSERT must roll back immutable definition + receipt, for both kinds.
 // No test hook in production Worker; fixture trigger is installed only in isolated D1.
 (await sql("CREATE TRIGGER fixture_fail_enrichment_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action LIKE 'customer_enrichment.%' "+
   "BEGIN SELECT RAISE(ABORT,'fixture enrichment audit failure'); END;"));
 const beforeAuditFailure=(await enrichmentSnapshot());
 try{
   await assertUnconfirmed(await sendFixtureRpc(dispatch, methods.publishProviderProfile, admin, {
     ...profileInput,retentionDays:37,
     precondition:{idempotencyKey:"enrichment-profile-audit-failure"}
   }, 503),"ENRICHMENT_WRITE_UNCONFIRMED");
   await assertUnconfirmed(await sendFixtureRpc(dispatch, methods.publishMapping, knownService, {
     ...mappingInput,providerResponseFieldPath:"organization.alternate_legal_name",
     precondition:{idempotencyKey:"enrichment-mapping-audit-failure"}
   }, 503),"ENRICHMENT_WRITE_UNCONFIRMED");
   assert.deepEqual((await enrichmentSnapshot()),beforeAuditFailure,
     "Audit failure must atomically roll back published state and receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_enrichment_audit");}
 console.log(JSON.stringify({event:"crm.native-r2a-enrichment-actor-audit",result:"PASS",
   human_profile:true,service_mapping:true,cross_actor_replay:true,
   forged_headers_ignored:true,authorization_before_replay:true,
   tenant_ab:true,sql_failure_rollback:true,immutable_created_by:true}));
 for(const index of [1,2]){
   const published=await fixtureRpc(dispatch, methods.publishProviderProfile, admin, {
     ...profileInput,retentionDays:30+index,
     precondition:{idempotencyKey:"enrichment-profile-page-"+index}});
   assert.equal(published.status,200,formatRpcFailure(published));
   assert.ok(published.body.profile);
   const mapped=await fixtureRpc(dispatch, methods.publishMapping, admin, {
     ...mappingInput,providerProfileVersionId:published.body.profile.profileVersionId,
     precondition:{idempotencyKey:"enrichment-mapping-page-"+index}});
   assert.equal(mapped.status,200,formatRpcFailure(mapped));
 }
 const profiles=await collectFixturePages(async pageToken=>{
   const {body}=await fixtureRpc(dispatch,methods.listProviderProfiles,viewer,{page:{pageSize:1,pageToken}});
   return {items:body.profiles,nextPageToken:body.pageInfo?.nextPageToken};
 });
 const mappings=await collectFixturePages(async pageToken=>{
   const {body}=await fixtureRpc(dispatch,methods.listMappings,viewer,{page:{pageSize:1,pageToken}});
   return {items:body.mappings,nextPageToken:body.pageInfo?.nextPageToken};
 });
 assert.equal(profiles.length,3);assert.equal(mappings.length,3);
 assert.equal(new Set(profiles.map(x=>x.profileVersionId)).size,3);
 assert.equal(new Set(mappings.map(x=>x.mappingVersionId)).size,3);
 for (const page of [{pageSize:101},{pageToken:"bad/id"}]) {
   const profilesError=await fixtureRpcError(dispatch,methods.listProviderProfiles,viewer,{page});
   const mappingsError=await fixtureRpcError(dispatch,methods.listMappings,viewer,{page});
   assert.equal(profilesError.body.code,"invalid_argument");
   assert.equal(mappingsError.body.code,"invalid_argument");
 }
 const isolatedProfiles=await fixtureRpc(dispatch,methods.listProviderProfiles,tenantB,{page:{pageSize:1}});
 assert.equal(isolatedProfiles.body.profiles.length,1,"Tenant B sees only its own deliberate profile");
 assert.equal(isolatedProfiles.body.profiles[0]?.profileVersionId,profileId);
 const isolatedMappings=await fixtureRpc(dispatch,methods.listMappings,tenantB,{page:{pageSize:1}});
 assert.equal(isolatedMappings.body.mappings.length,0,"Tenant B cannot see tenant A mappings");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_enrichment_receipts WHERE tenant='tenant-a'"),6);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_enrichment_profiles WHERE tenant='tenant-a'"),3);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_enrichment_mappings WHERE tenant='tenant-a'"),3);
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_audit"),enrichmentAuditBefore+7,
   "six tenant-a publishes and one isolated tenant-b publish write audit; replay and denials never do");
 assert.equal(await count("SELECT COUNT(*) AS n FROM crm_party_records"),partyCountBefore);
 console.log(JSON.stringify({event:"crm.native-enrichment-v2",result:"PASS",
   original_owner:"crm.customer-enrichment",immutable:true,tenant_ab:true,replay:true,
   page_bounds:true,d1_audit:true,timestamp_precision:true,provider_execution:"NOT_CONNECTED"}));


}

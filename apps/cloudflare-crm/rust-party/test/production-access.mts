// Feasibility gate: the actual production WASM must authenticate through its
// ordinary Access/JWKS path. Only the external public-key response is replaced
// in Node, never in Worker code or in its production configuration.
import assert from "node:assert/strict";
import {createHash,createSign,generateKeyPairSync} from "node:crypto";
import {readFileSync,writeFileSync} from "node:fs";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import {mock} from "node:test";
import {createTestHarness} from "wrangler";
import { fixtureRpc, sendFixtureRpc } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import type { RpcDispatch } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { PartyService, PartyKind } from "../../../../packages/client/gen/crm/parties/v3/party_pb.js";
import { AccountService } from "../../../../packages/client/gen/crm/accounts/v2/account_pb.js";
import { DealService, DealStatus } from "../../../../packages/client/gen/crm/sales/v2/deal_pb.js";
import { DomainErrorDetailSchema, MutationCommitOutcome } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import type { MutationReceipt } from "../../../../packages/client/gen/crm/core/v2/operation_pb.js";
import { decodeFixtureDetail } from "../../../../packages/client/test/cloudflare-rpc-fixture.ts";
import { record, records, string } from "../../scripts/json-boundary.mts";

const root=dirname(dirname(fileURLToPath(import.meta.url)));
const sha=string(process.env.CRM_SOURCE_SHA);
assert.match(sha,/^[a-f0-9]{40}$/);
const hash=(path: string)=>createHash("sha256").update(readFileSync(join(root,path))).digest("hex");
const artifact=()=>({
  wasm:hash("build/index_bg.wasm"),
  entry:hash("build/index.js"),
});
const before=artifact();
const issuer="https://royal-limit-ea43.cloudflareaccess.com";
const audience="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const {publicKey,privateKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const other=generateKeyPairSync("rsa",{modulusLength:2048}).privateKey;
const kid="production-artifact-local-fixture";
const originalJwk={...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"};
const jwks={keys:[originalJwk]};
const now=Math.floor(Date.now()/1000);
const admin="production-artifact-admin@example.test";
const viewer="production-artifact-viewer@example.test";
function token(identity: string,claims: Record<string, unknown>={},key=privateKey,header: Record<string, unknown>={}){
  const raw=[
    {alg:"RS256",kid,typ:"JWT",...header},
    {iss:issuer,aud:audience,iat:now-10,exp:now+600,sub:identity,email:identity,...claims},
  ].map(value=>Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(key).toString("base64url");
}
let jwksRequests=0,providerStatus=200;
const fetchOriginal=globalThis.fetch.bind(globalThis);
mock.method(globalThis,"fetch",async(...args: Parameters<typeof fetch>)=>{
  const [input, init] = args;
  const url=typeof input==="string"?input:input instanceof URL?input.href:input.url;
  if(url===issuer+"/cdn-cgi/access/certs"){
    assert.equal(init?.method??(typeof input === "string" || input instanceof URL ? "GET" : input.method),"GET");
    jwksRequests++;
    return Response.json(jwks,{status:providerStatus});
  }
  return fetchOriginal(input,init);
});
const configPath=join(root,"wrangler.jsonc");
const config=JSON.parse(readFileSync(configPath,"utf8"));
assert.equal(config.vars?.CRM_ACCESS_AUDIENCE,audience,
  "the local production-artifact harness must use the committed production Access audience");
assert.equal(config.d1_databases.length,1);
assert.equal(config.d1_databases[0].database_id,"00000000-0000-4000-8000-000000000001");
assert.equal(config.main,"./build/index.js");
const harness=createTestHarness({
  root,
  workers:[{configPath,vars:{GIT_SHA:sha,CRM_CI_LOCAL_ONLY:"no"}}],
});
try{
  await harness.listen();
  const worker=harness.getWorker("crm-rust-party-ci-local");
  await worker.applyD1Migrations("DB");
  const env=await worker.getEnv();
  assert.equal(env.CRM_CI_LOCAL_ONLY,"no");
  assert.equal(env.CRM_ACCESS_AUDIENCE,audience);
  assert.equal(env.CRM_NATIVE_CI_JWKS,undefined);
  const db=env.DB;
  assert.ok(db);
  await db.prepare("INSERT INTO crm_tenants(id,name) VALUES(?,?)")
    .bind("artifact-proof","Artifact proof").run();
  await db.prepare("INSERT INTO crm_users(email,tenant_id,role,active) VALUES(?,?,?,1),(?,?,?,1)")
    .bind(admin,"artifact-proof","admin",viewer,"artifact-proof","viewer").run();
  async function call(path: string,assertion: string | null,status: number){
    const response=await worker.fetch(path,{headers:assertion?{"cf-access-jwt-assertion":assertion}:{}});
    assert.equal(response.status,status,path);
    assert.equal(response.headers.get("x-crm-source-sha"),sha);
    return record(await response.json());
  }
  const initial=jwksRequests;
  await call("/api/session",null,401);
  await call("/api/session",token(admin,{aud:"wrong-audience"}),401);
  await call("/api/session",token(admin,{aud:"2d60128cb04e0d2bc181457656441c8c4c778d6ff0b1533af3333c77197d331a"}),401);
  await call("/api/session",token(admin,{exp:now-1}),401);
  assert.equal(jwksRequests,initial,"invalid claims must fail before JWKS fetch");
  assert.equal((await call("/api/session",token(admin),200)).role,"admin");
  assert.equal(jwksRequests,initial+1,"first valid key must be fetched once");
  assert.equal((await call("/api/session",token(viewer),200)).role,"viewer");
  assert.equal((await call("/api/version",token(admin),200)).source,sha);
  await call("/api/session",token(admin,{},other),401);
  assert.equal(jwksRequests,initial+1,"bad signatures must be rejected using the cached key");
  await call("/api/session",token(admin,{},privateKey,{kid:"unknown"}),401);
  assert.equal(jwksRequests,initial+2,"unknown kid must refresh JWKS and fail closed if absent");
  const rotated=generateKeyPairSync("rsa",{modulusLength:2048});
  const rotatedKid="production-artifact-rotated";
  const rotatedJwk={...rotated.publicKey.export({format:"jwk"}),kid:rotatedKid,alg:"RS256",use:"sig"};
  jwks.keys=[originalJwk,rotatedJwk];
  const rotatedAdmin=token(admin,{},rotated.privateKey,{kid:rotatedKid});
  await call("/api/session",rotatedAdmin,200);
  assert.equal(jwksRequests,initial+3,"rotated kid must refresh JWKS exactly once");
  await call("/api/session",token(admin),200);
  await call("/api/session",rotatedAdmin,200);
  assert.equal(jwksRequests,initial+3,"both current and previous keys must reuse imported CryptoKeys");
  await call("/api/session",token("",{common_name:"unassigned-artifact.access"}),401);
  await call("/api/_ci/original-domain-check",token(admin),404);
  await call("/api/doctor",null,401);
  // Compare the actual release binary's registered handlers and policy to
  // the full signed suite's already validated Buf/owner/runtime evidence.
  const doctor=await worker.fetch("/api/doctor",{headers:{"cf-access-jwt-assertion":token(admin)}});
  assert.equal(doctor.headers.get("x-crm-source-sha"),sha);
  const diagnostic=record(await doctor.json());
  assert.equal(diagnostic.source,sha);
  assert.equal(record(diagnostic.database).status,"PASS");
  assert.equal(doctor.status,record(diagnostic.release).source_version==="PASS"?200:503);
  assert.equal(record(diagnostic.rpc).compiled_policy_check,true);
  const runtime=record(JSON.parse(readFileSync(string(process.env.CRM_RPC_RUNTIME_PROOF),"utf8")));
  assert.equal(runtime.source_sha,sha);
  assert.equal(runtime.result,"PASS");
  assert.deepEqual(diagnostic.rpc,runtime.rpc,"production and fully tested router/policy/owners must match");
  const viewerDoctor=await worker.fetch("/api/doctor",{headers:{"cf-access-jwt-assertion":token(viewer)}});
  assert.equal(record(await viewerDoctor.json()).rpc,null);
  providerStatus=503;
  await call("/api/session",token(admin,{},privateKey,{kid:"provider-unavailable"}),401);
  assert.equal(jwksRequests,initial+4,
    "an unknown kid must refresh even when the provider is unavailable, then fail closed");
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_users").first()).n,2);
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_tenants").first()).n,1);
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_audit").first()).n,0);
  // Extend the same artifact proof through the actual generated binary RPCs.
  // No direct insertion of domain state; all fixtures are made by original owners.
  providerStatus=200;
  const partyId="production-artifact-party",accountId="production-artifact-account";
  const pipelineId="production.artifact",dealId="production-artifact-deal";
  const pre=(key: string,expectedVersion?: bigint)=>({idempotencyKey:"artifact-"+key,
    ...(expectedVersion===undefined?{}:{expectedVersion})});
  const dispatch: RpcDispatch = async (path, options) => {
    const response = await worker.fetch(path, options);
    assert.equal(response.headers.get("x-crm-source-sha"), sha);
    return response;
  };
  const committed = (reply: {body: {receipt?: MutationReceipt | undefined}; headers: Pick<Headers, "get">}, version: bigint) => {
    assert.equal(reply.body.receipt?.resourceVersion,version);
    assert.equal(reply.body.receipt?.requestId,reply.headers.get("x-request-id"));
    assert.equal(reply.body.receipt?.replayed,false);
  };
  const party = PartyService.method;
  const account = AccountService.method;
  const sales = DealService.method;
  committed(await fixtureRpc(dispatch,party.createParty,token(admin),{
    partyId,kind:PartyKind.ORGANIZATION,displayName:"Artifact Party",precondition:pre("party")}),1n);
  committed(await fixtureRpc(dispatch,account.createAccount,token(admin),{
    accountId,primaryPartyId:partyId,name:"Artifact Account",precondition:pre("account")}),1n);
  committed(await fixtureRpc(dispatch,sales.createPipeline,token(admin),{pipelineId,name:"Artifact Pipeline",
    stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
      {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500}],
    precondition:pre("pipeline")}),1n);
  const create={dealId,accountId,name:"Artifact Deal",
    stage:{pipelineId,stageId:"lead",ordinal:1},probabilityBasisPoints:1000,
    precondition:pre("deal")};
  const count=async(table: string)=>(await db.prepare("SELECT COUNT(*) AS n FROM "+table).first()).n;
  const snapshot=async()=>Promise.all([
    db.prepare("SELECT state,version FROM crm_sales_deal_records WHERE tenant=? AND id=?")
      .bind("artifact-proof",dealId).first(),
    count("crm_sales_deal_receipts"),count("crm_sales_deal_events"),
    count("crm_sales_link_delivery_records"),count("crm_activity_task_records"),
    count("crm_activity_task_receipts"),count("crm_audit"),
  ]);
  const beforeCreate=await snapshot();
  await sendFixtureRpc(dispatch,sales.createDeal,null,create,401);
  await sendFixtureRpc(dispatch,sales.createDeal,token(viewer),create,403);
  assert.deepEqual(await snapshot(),beforeCreate);
  const created=await fixtureRpc(dispatch,sales.createDeal,token(admin),create);
  const createdRequestId=created.headers.get("x-request-id");
  committed(created,1n);
  const get={dealId,receiptIdempotencyKey:create.precondition.idempotencyKey};
  const confirmed=await fixtureRpc(dispatch,sales.getDeal,token(viewer),get);
  assert.equal(confirmed.body.confirmedMutation?.resourceVersion,1n);
  assert.equal(confirmed.body.confirmedMutation?.originalRequestId,createdRequestId);
  const missing={dealId,receiptIdempotencyKey:"artifact-missing-receipt"};
  assert.equal((await fixtureRpc(dispatch,sales.getDeal,token(viewer),missing)).body.confirmedMutation,undefined);
  // Real D1 receipt read failure is not the same as a proven absent key.
  // These are public production RPCs, not the old CI-only diagnostic route.
  const beforeUnavailable=await snapshot();
  await db.prepare("ALTER TABLE crm_sales_deal_receipts RENAME TO fixture_artifact_unavailable_receipts").run();
  try{
    const unavailable=await sendFixtureRpc(dispatch,sales.getDeal,token(viewer),get,503);
    const unavailableBody=record(await unavailable.json());
    assert.equal(unavailableBody.code,"unavailable");
    assert.equal(unavailableBody.message,"SALES_RECEIPT_READ_FAILED");
    const absent=await sendFixtureRpc(dispatch,sales.getDeal,token(viewer),missing,503);
    assert.equal(record(await absent.json()).message,"SALES_RECEIPT_READ_FAILED");
    const replayUnavailable=await sendFixtureRpc(dispatch,sales.createDeal,token(admin),create,503);
    const detail = decodeFixtureDetail(DomainErrorDetailSchema,
      records(record(await replayUnavailable.json()).details));
    assert.equal(detail.domainCode,"SALES_COMMIT_UNCONFIRMED");
    assert.equal(detail.commitOutcome,MutationCommitOutcome.UNCONFIRMED);
    assert.equal(detail.requestId,replayUnavailable.headers.get("x-request-id"));
  }finally{
    await db.prepare("ALTER TABLE fixture_artifact_unavailable_receipts RENAME TO crm_sales_deal_receipts").run();
  }
  assert.deepEqual(await snapshot(),beforeUnavailable);
  assert.equal((await fixtureRpc(dispatch,sales.getDeal,token(viewer),missing)).body.confirmedMutation,undefined);
  assert.equal((await fixtureRpc(dispatch,sales.getDeal,token(viewer),get)).body.confirmedMutation?.originalRequestId,
    createdRequestId);
  const replay=await fixtureRpc(dispatch,sales.createDeal,token(admin),create);
  assert.equal(replay.body.receipt?.replayed,true);
  assert.equal(replay.body.receipt?.originalRequestId,createdRequestId);
  assert.deepEqual(await snapshot(),beforeUnavailable);
  const advance={dealId,targetStage:{pipelineId,stageId:"qualified",ordinal:2},
    targetStatus:DealStatus.OPEN,precondition:pre("advance",1n)};
  const moved=await fixtureRpc(dispatch,sales.advanceStage,token(admin),advance);
  committed(moved,2n);
  const movedRequestId=moved.headers.get("x-request-id");
  const linked=await db.prepare("SELECT deal_version,disposition,task_id,request_id FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?")
    .bind("artifact-proof",dealId).first();
  assert.equal(linked?.deal_version,2);
  assert.equal(linked?.disposition,"applied");
  assert.equal(linked?.request_id,movedRequestId);
  assert.ok(linked?.task_id);
  const task=await db.prepare("SELECT version,state FROM crm_activity_task_records WHERE tenant=? AND id=?")
    .bind("artifact-proof",linked.task_id).first();
  assert.equal(task?.version,1);
  assert.ok(records(record(JSON.parse(string(task.state))).related_resources)
    .some(ref=>ref.resource_id===dealId&&ref.version===2));
  const afterAdvance=await snapshot();
  assert.equal((await fixtureRpc(dispatch,sales.advanceStage,token(admin),advance)).body.receipt?.replayed,true);
  await sendFixtureRpc(dispatch,sales.updateDeal,token(admin),{dealId,name:{operation:{case:"set",value:"Stale"}},
    precondition:pre("stale",1n)},409);
  assert.deepEqual(await snapshot(),afterAdvance,"replay/stale write must not change D1");
  const actions=(await db.prepare("SELECT action FROM crm_audit WHERE tenant_id=? AND request_id=? ORDER BY action")
    .bind("artifact-proof",movedRequestId).all()).results;
  const names=records(actions).map(row=>row.action);
  assert.deepEqual(names,["activity_tasks.create","sales_activities_link.applied","sales_deals.advance"]);
  console.log(JSON.stringify({event:"crm.production-rpc-d1-artifact",result:"PASS",
    source_sha:sha,generated_binary:true,role_denial:true,real_receipt_read_failure:true,
    replay:true,cas:true,atomic_link:true,request_correlation:true}));
  assert.deepEqual(artifact(),before,"production bundle changed during test");
  const evidence={event:"crm.production-access-artifact",result:"PASS",source_sha:sha,
    production_features:["native"],artifact:before,jwks_requests:jwksRequests,
    access_key_cache:{result:"PASS",ttl_seconds:300,rotation_refresh:true,
      previous_key_overlap:true,unknown_kid_denied:true,provider_failure_denied:true},
    signed_admin_viewer:true,invalid_signature_denied:true,wrong_claims_denied:true,
    unknown_service_denied:true,provider_failure_denied:true,ci_endpoint_absent:true,
    unchanged_d1:true,compiled_router_policy_parity:true,production_rpc_d1:{generated_binary:true,role_denial:true,
      receipt_read_failure:true,replay:true,cas:true,atomic_link:true},
    scope:"production Access and selected RPC/D1 parity; full native-ci suite still mandatory"};
  writeFileSync(string(process.env.CRM_PRODUCTION_ACCESS_PROOF),JSON.stringify(evidence)+"\n");
  console.log(JSON.stringify(evidence));
}finally{
  try{await harness.close();}finally{mock.restoreAll();}
}

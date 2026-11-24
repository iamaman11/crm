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
import {
  binaryPartyV3,decodePartyV3,partyV3Path,
  binaryAccountV2,decodeAccountV2,accountV2Path,
  binarySalesV2,decodeSalesV2,salesV2Path,decodeDomainErrorDetail,
} from "../../../../scripts/crm-connect-binary-fixture.mjs";

const root=dirname(dirname(fileURLToPath(import.meta.url)));
const sha=process.env.CRM_SOURCE_SHA;
assert.match(sha??"",/^[a-f0-9]{40}$/);
const hash=path=>createHash("sha256").update(readFileSync(join(root,path))).digest("hex");
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
const jwks={keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]};
const now=Math.floor(Date.now()/1000);
const admin="production-artifact-admin@example.test";
const viewer="production-artifact-viewer@example.test";
function token(identity,claims={},key=privateKey,header={}){
  const raw=[
    {alg:"RS256",kid,typ:"JWT",...header},
    {iss:issuer,aud:audience,iat:now-10,exp:now+600,sub:identity,email:identity,...claims},
  ].map(value=>Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(key).toString("base64url");
}
let jwksRequests=0,providerStatus=200;
const fetchOriginal=globalThis.fetch.bind(globalThis);
mock.method(globalThis,"fetch",async(input,init)=>{
  const url=typeof input==="string"?input:input instanceof URL?input.href:input.url;
  if(url===issuer+"/cdn-cgi/access/certs"){
    assert.equal(init?.method??input?.method??"GET","GET");
    jwksRequests++;
    return Response.json(jwks,{status:providerStatus});
  }
  return fetchOriginal(input,init);
});
const configPath=join(root,"wrangler.jsonc");
const config=JSON.parse(readFileSync(configPath,"utf8"));
assert.equal(config.d1_databases.length,1);
assert.equal(config.d1_databases[0].database_id,"00000000-0000-4000-8000-000000000001");
assert.equal(config.main,"./build/index.js");
let directClosed=false;
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
  assert.equal(env.CRM_NATIVE_CI_JWKS,undefined);
  const db=env.DB;
  assert.ok(db);
  await db.prepare("INSERT INTO crm_tenants(id,name) VALUES(?,?)")
    .bind("artifact-proof","Artifact proof").run();
  await db.prepare("INSERT INTO crm_users(email,tenant_id,role,active) VALUES(?,?,?,1),(?,?,?,1)")
    .bind(admin,"artifact-proof","admin",viewer,"artifact-proof","viewer").run();
  async function call(path,assertion,status){
    const response=await worker.fetch(path,{headers:assertion?{"cf-access-jwt-assertion":assertion}:{}});
    assert.equal(response.status,status,path);
    assert.equal(response.headers.get("x-crm-source-sha"),sha);
    return response.json();
  }
  const initial=jwksRequests;
  await call("/api/session",null,401);
  await call("/api/session",token(admin,{aud:"wrong-audience"}),401);
  await call("/api/session",token(admin,{exp:now-1}),401);
  assert.equal(jwksRequests,initial,"invalid claims must fail before JWKS fetch");
  assert.equal((await call("/api/session",token(admin),200)).role,"admin");
  assert.equal((await call("/api/session",token(viewer),200)).role,"viewer");
  assert.equal((await call("/api/version",token(admin),200)).source,sha);
  await call("/api/session",token(admin,{},other),401);
  await call("/api/session",token(admin,{},privateKey,{kid:"unknown"}),401);
  await call("/api/session",token("",{common_name:"unassigned-artifact.access"}),401);
  await call("/api/_ci/original-domain-check",token(admin),404);
  await call("/api/doctor",null,401);
  // Compare the actual release binary's registered handlers and policy to
  // the full signed suite's already validated Buf/owner/runtime evidence.
  const doctor=await worker.fetch("/api/doctor",{headers:{"cf-access-jwt-assertion":token(admin)}});
  assert.equal(doctor.headers.get("x-crm-source-sha"),sha);
  const diagnostic=await doctor.json();
  assert.equal(diagnostic.source,sha);
  assert.equal(diagnostic.database.status,"PASS");
  assert.equal(doctor.status,diagnostic.release.source_version==="PASS"?200:503);
  assert.equal(diagnostic.rpc.compiled_policy_check,true);
  const runtime=JSON.parse(readFileSync(process.env.CRM_RPC_RUNTIME_PROOF,"utf8"));
  assert.equal(runtime.source_sha,sha);
  assert.equal(runtime.result,"PASS");
  assert.deepEqual(diagnostic.rpc,runtime.rpc,"production and fully tested router/policy/owners must match");
  const viewerDoctor=await worker.fetch("/api/doctor",{headers:{"cf-access-jwt-assertion":token(viewer)}});
  assert.equal((await viewerDoctor.json()).rpc,null);
  providerStatus=503;
  await call("/api/session",token(admin),401);
  assert.ok(jwksRequests>=8,"production verifier must actually fetch JWKS");
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_users").first()).n,2);
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_tenants").first()).n,1);
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_audit").first()).n,0);
  // Extend the same artifact proof through the actual generated binary RPCs.
  // No direct insertion of domain state; all fixtures are made by original owners.
  providerStatus=200;
  const partyId="production-artifact-party",accountId="production-artifact-account";
  const pipelineId="production.artifact",dealId="production-artifact-deal";
  const pre=(key,expectedVersion)=>({idempotencyKey:"artifact-"+key,
    ...(expectedVersion===undefined?{}:{expectedVersion})});
  const rpc=(path,encode,decode,target=worker)=>async(method,identity,input,status=200,fault)=>{
    const response=await target.fetch(path(method),{method:"POST",
      headers:{"content-type":"application/proto","connect-protocol-version":"1",
        ...(identity?{"cf-access-jwt-assertion":token(identity)}:{}),
        ...(fault?{"x-test-d1-fault":fault}:{})},
      body:encode(method,input)});
    assert.equal(response.status,status,method);
    assert.equal(response.headers.get("x-crm-source-sha"),sha);
    const body=status<400?decode(method,new Uint8Array(await response.arrayBuffer())):await response.json();
    return {body,requestId:response.headers.get("x-request-id")};
  };
  const party=rpc(partyV3Path,binaryPartyV3,decodePartyV3);
  const account=rpc(accountV2Path,binaryAccountV2,decodeAccountV2);
  const sales=rpc(salesV2Path,binarySalesV2,decodeSalesV2);
  const committed=(reply,version)=>{
    assert.equal(reply.body.receipt?.resourceVersion,version);
    assert.equal(reply.body.receipt?.requestId,reply.requestId);
    assert.equal(reply.body.receipt?.replayed,false);
  };
  committed(await party("CreateParty",admin,{partyId,kind:2,displayName:"Artifact Party",
    precondition:pre("party")}),1n);
  committed(await account("CreateAccount",admin,{accountId,primaryPartyId:partyId,
    name:"Artifact Account",precondition:pre("account")}),1n);
  committed(await sales("CreatePipeline",admin,{pipelineId,name:"Artifact Pipeline",
    stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
      {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500}],
    precondition:pre("pipeline")}),1n);
  const create={dealId,accountId,name:"Artifact Deal",
    stage:{pipelineId,stageId:"lead",ordinal:1},probabilityBasisPoints:1000,
    precondition:pre("deal")};
  const count=async(table)=>(await db.prepare("SELECT COUNT(*) AS n FROM "+table).first()).n;
  const snapshot=async()=>Promise.all([
    db.prepare("SELECT state,version FROM crm_sales_deal_records WHERE tenant=? AND id=?")
      .bind("artifact-proof",dealId).first(),
    count("crm_sales_deal_receipts"),count("crm_sales_deal_events"),
    count("crm_sales_link_delivery_records"),count("crm_activity_task_records"),
    count("crm_activity_task_receipts"),count("crm_audit"),
  ]);
  const beforeCreate=await snapshot();
  await sales("CreateDeal",null,create,401);
  await sales("CreateDeal",viewer,create,403);
  assert.deepEqual(await snapshot(),beforeCreate);
  const created=await sales("CreateDeal",admin,create);
  committed(created,1n);
  const get={dealId,receiptIdempotencyKey:create.precondition.idempotencyKey};
  const confirmed=await sales("GetDeal",viewer,get);
  assert.equal(confirmed.body.confirmedMutation?.resourceVersion,1n);
  assert.equal(confirmed.body.confirmedMutation?.originalRequestId,created.requestId);
  const missing={dealId,receiptIdempotencyKey:"artifact-missing-receipt"};
  assert.equal((await sales("GetDeal",viewer,missing)).body.confirmedMutation,undefined);
  // Real D1 receipt read failure is not the same as a proven absent key.
  // These are public production RPCs, not the old CI-only diagnostic route.
  const beforeUnavailable=await snapshot();
  await db.prepare("ALTER TABLE crm_sales_deal_receipts RENAME TO fixture_artifact_unavailable_receipts").run();
  try{
    const unavailable=await sales("GetDeal",viewer,get,503);
    assert.equal(unavailable.body.code,"unavailable");
    assert.equal(unavailable.body.message,"SALES_RECEIPT_READ_FAILED");
    const absent=await sales("GetDeal",viewer,missing,503);
    assert.equal(absent.body.message,"SALES_RECEIPT_READ_FAILED");
    const replayUnavailable=await sales("CreateDeal",admin,create,503);
    const detail=decodeDomainErrorDetail(replayUnavailable.body.details.find(
      d=>d.type==="crm.core.v2.DomainErrorDetail"));
    assert.equal(detail.domainCode,"SALES_COMMIT_UNCONFIRMED");
    assert.equal(detail.commitOutcome,2);
    assert.equal(detail.requestId,replayUnavailable.requestId);
  }finally{
    await db.prepare("ALTER TABLE fixture_artifact_unavailable_receipts RENAME TO crm_sales_deal_receipts").run();
  }
  assert.deepEqual(await snapshot(),beforeUnavailable);
  assert.equal((await sales("GetDeal",viewer,missing)).body.confirmedMutation,undefined);
  assert.equal((await sales("GetDeal",viewer,get)).body.confirmedMutation?.originalRequestId,
    created.requestId);
  const replay=await sales("CreateDeal",admin,create);
  assert.equal(replay.body.receipt?.replayed,true);
  assert.equal(replay.body.receipt?.originalRequestId,created.requestId);
  assert.deepEqual(await snapshot(),beforeUnavailable);
  const advance={dealId,targetStage:{pipelineId,stageId:"qualified",ordinal:2},
    targetStatus:1,precondition:pre("advance",1n)};
  const moved=await sales("AdvanceStage",admin,advance);
  committed(moved,2n);
  const linked=await db.prepare("SELECT deal_version,disposition,task_id,request_id FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?")
    .bind("artifact-proof",dealId).first();
  assert.equal(linked?.deal_version,2);
  assert.equal(linked?.disposition,"applied");
  assert.equal(linked?.request_id,moved.requestId);
  assert.ok(linked?.task_id);
  const task=await db.prepare("SELECT version,state FROM crm_activity_task_records WHERE tenant=? AND id=?")
    .bind("artifact-proof",linked.task_id).first();
  assert.equal(task?.version,1);
  assert.ok(JSON.parse(task.state).related_resources.some(ref=>ref.resource_id===dealId&&ref.version===2));
  const afterAdvance=await snapshot();
  assert.equal((await sales("AdvanceStage",admin,advance)).body.receipt?.replayed,true);
  await sales("UpdateDeal",admin,{dealId,name:{operation:{case:"set",value:"Stale"}},
    precondition:pre("stale",1n)},409);
  assert.deepEqual(await snapshot(),afterAdvance,"replay/stale write must not change D1");
  const actions=(await db.prepare("SELECT action FROM crm_audit WHERE tenant_id=? AND request_id=? ORDER BY action")
    .bind("artifact-proof",moved.requestId).all()).results.map(row=>row.action);
  assert.deepEqual(actions,["activity_tasks.create","sales_activities_link.applied","sales_deals.advance"]);
  console.log(JSON.stringify({event:"crm.production-rpc-d1-artifact",result:"PASS",
    source_sha:sha,generated_binary:true,role_denial:true,real_receipt_read_failure:true,
    replay:true,cas:true,atomic_link:true,request_correlation:true}));
  assert.deepEqual(artifact(),before,"production bundle changed during test");
  // Keep the ordinary production-entrypoint proof above. A separate local
  // harness imports the very same bundle and intercepts only a committed D1
  // acknowledgement. No product code or database implementation is replaced.
  await harness.close();
  directClosed=true;
  const faultHarness=createTestHarness({
    root,
    workers:[{config:{
      ...config,main:join(root,"test/d1-fault-worker.mjs"),
      vars:{GIT_SHA:sha,CRM_CI_LOCAL_ONLY:"no"},
      assets:{...config.assets,directory:join(root,config.assets.directory)},
      d1_databases:config.d1_databases.map(binding=>({
        ...binding,migrations_dir:join(root,binding.migrations_dir)
      })),
    }}],
  });
  try{
    await faultHarness.listen();
    const target=faultHarness.getWorker(config.name);
    await target.applyD1Migrations("DB");
    const faultDb=(await target.getEnv()).DB;
    assert.ok(faultDb);
    await faultDb.prepare("INSERT INTO crm_tenants(id,name) VALUES(?,?)")
      .bind("artifact-proof","Artifact fault proof").run();
    await faultDb.prepare("INSERT INTO crm_users(email,tenant_id,role,active) VALUES(?,?,?,1)")
      .bind(admin,"artifact-proof","admin").run();
    const faultParty=rpc(partyV3Path,binaryPartyV3,decodePartyV3,target);
    const faultAccount=rpc(accountV2Path,binaryAccountV2,decodeAccountV2,target);
    const faultSales=rpc(salesV2Path,binarySalesV2,decodeSalesV2,target);
    committed(await faultParty("CreateParty",admin,{partyId,kind:2,displayName:"Fault Party",
      precondition:pre("party")}),1n);
    committed(await faultAccount("CreateAccount",admin,{accountId,primaryPartyId:partyId,
      name:"Fault Account",precondition:pre("account")}),1n);
    committed(await faultSales("CreatePipeline",admin,{pipelineId,name:"Fault Pipeline",
      stages:[{stageId:"lead",name:"Lead",probabilityBasisPoints:1000}],
      precondition:pre("pipeline")}),1n);
    const exactState=async input=>{
      const row=await faultDb.prepare("SELECT version,request_id FROM crm_sales_deal_records WHERE tenant=? AND id=?")
        .bind("artifact-proof",input.dealId).first();
      const receipts=await faultDb.prepare("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?")
        .bind("artifact-proof",input.precondition.idempotencyKey).first();
      const audits=await faultDb.prepare("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?")
        .bind("artifact-proof",input.dealId).first();
      assert.equal(row?.version,1);
      assert.equal(receipts.n,1);
      assert.equal(audits.n,1);
      return row.request_id;
    };
    const lost={...create,dealId:"artifact-lost-ack",precondition:pre("lost-ack")};
    const reconciled=await faultSales("CreateDeal",admin,lost,200,"lost-ack");
    assert.equal(reconciled.body.receipt?.replayed,true);
    assert.equal(reconciled.body.receipt?.resourceVersion,1n);
    assert.equal(reconciled.body.receipt?.originalRequestId,reconciled.requestId);
    assert.equal(await exactState(lost),reconciled.requestId);
    const unreadable={...create,dealId:"artifact-unreadable-receipt",precondition:pre("unreadable-receipt")};
    let uncertain;
    try{
      uncertain=await faultSales("CreateDeal",admin,unreadable,503,"receipt-read");
      const detail=decodeDomainErrorDetail(uncertain.body.details.find(
        d=>d.type==="crm.core.v2.DomainErrorDetail"));
      assert.equal(detail.domainCode,"SALES_COMMIT_UNCONFIRMED");
      assert.equal(detail.commitOutcome,2);
      assert.equal(detail.requestId,uncertain.requestId);
      assert.ok(await faultDb.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='fixture_fault_receipts'").first(),
        "real receipt relation must have been made unavailable after commit");
      assert.equal((await faultDb.prepare("SELECT version FROM crm_sales_deal_records WHERE tenant=? AND id=?")
        .bind("artifact-proof",unreadable.dealId).first()).version,1);
    }finally{
      if(await faultDb.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='fixture_fault_receipts'").first())
        await faultDb.prepare("ALTER TABLE fixture_fault_receipts RENAME TO crm_sales_deal_receipts").run();
    }
    assert.equal(await exactState(unreadable),uncertain.requestId);
    const recovered=await faultSales("CreateDeal",admin,unreadable);
    assert.equal(recovered.body.receipt?.replayed,true);
    assert.equal(recovered.body.receipt?.resourceVersion,1n);
    assert.equal(recovered.body.receipt?.originalRequestId,uncertain.requestId);
    assert.equal(await exactState(unreadable),uncertain.requestId);
    console.log(JSON.stringify({event:"crm.production-d1-postcommit-fault",result:"PASS",
      source_sha:sha,real_d1:true,compiled_release:true,lost_ack_reconciled:true,
      committed_before_read_failure:true,typed_unconfirmed:true,one_receipt_one_audit:true}));
  }finally{await faultHarness.close();}
  assert.deepEqual(artifact(),before,"production bundle changed during fault test");
  const evidence={event:"crm.production-access-artifact",result:"PASS",source_sha:sha,
    production_features:["native"],artifact:before,jwks_requests:jwksRequests,
    signed_admin_viewer:true,invalid_signature_denied:true,wrong_claims_denied:true,
    unknown_service_denied:true,provider_failure_denied:true,ci_endpoint_absent:true,
    unchanged_d1:true,compiled_router_policy_parity:true,production_d1_postcommit_fault:true,production_rpc_d1:{generated_binary:true,role_denial:true,
      receipt_read_failure:true,replay:true,cas:true,atomic_link:true},
    scope:"production Access and selected RPC/D1 parity; full native-ci suite still mandatory"};
  writeFileSync(process.env.CRM_PRODUCTION_ACCESS_PROOF,JSON.stringify(evidence)+"\n");
  console.log(JSON.stringify(evidence));
}finally{
  try{if(!directClosed)await harness.close();}finally{mock.restoreAll();}
}

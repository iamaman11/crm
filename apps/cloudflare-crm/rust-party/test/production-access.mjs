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
  providerStatus=503;
  await call("/api/session",token(admin),401);
  assert.ok(jwksRequests>=8,"production verifier must actually fetch JWKS");
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_users").first()).n,2);
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_tenants").first()).n,1);
  assert.equal((await db.prepare("SELECT COUNT(*) AS n FROM crm_audit").first()).n,0);
  assert.deepEqual(artifact(),before,"production bundle changed during test");
  const evidence={event:"crm.production-access-artifact",result:"PASS",source_sha:sha,
    production_features:["native"],artifact:before,jwks_requests:jwksRequests,
    signed_admin_viewer:true,invalid_signature_denied:true,wrong_claims_denied:true,
    unknown_service_denied:true,provider_failure_denied:true,ci_endpoint_absent:true,
    unchanged_d1:true,scope:"production Access feasibility; full native-ci suite still mandatory"};
  writeFileSync(process.env.CRM_PRODUCTION_ACCESS_PROOF,JSON.stringify(evidence)+"\n");
  console.log(JSON.stringify(evidence));
}finally{
  try{await harness.close();}finally{mock.restoreAll();}
}

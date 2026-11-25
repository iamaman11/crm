import {execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign} from "node:crypto";
import {writeFileSync,readFileSync,readdirSync} from "node:fs";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import {test} from "node:test";
import type {TestContext} from "node:test";
import {createTestHarness} from "wrangler";
import type {TestHarness,WorkerHandle} from "wrangler";
import {record, records, string, strings} from "../../scripts/json-boundary.mts";
import {formatRpcFailure,readRpcError} from "../../scripts/rpc-error-response.mts";
import {decodeDomainErrorDetail,decodeRpcOutput} from "../../../../packages/client/test/cloudflare-binary-rpc-fixture.ts";

// Each native subtest owns its own harness, fixture D1, signed identity and evidence.
// The first subtest returns only the compiled policy/denial proof; the second
// starts clean and requires the same compiled inventory before sealing evidence.
type BindingValue = string | number | null | ArrayBuffer;
type FixtureResult = {success: boolean; results: Record<string,unknown>[]};
type Statement = {bind(...values: BindingValue[]): Statement; all(): Promise<FixtureResult>};
type FixtureDb = {
 prepare(query: string): Statement;
 batch(statements: Statement[]): Promise<FixtureResult[]>;
};
type Policy = {permission: string};
type RpcInventory = {compiled_policy_check: true; registered_methods: string[]; policies: [string,Policy][]};
type DenialProof = {rpc: Record<string,unknown>; unauthenticatedDenials: string[]; viewerDenials: string[]};
type Claims = Record<string, unknown>;
type Sign = import("node:crypto").KeyObject;
type HarnessScenario = {
 cwd: string; base: string; worker: Omit<WorkerHandle,"getEnv" | "applyD1Migrations">; testHarness: TestHarness;
 sql: (query: string,...params: BindingValue[]) => Promise<Record<string,unknown>[]>;
 statement: (query: string,...params: BindingValue[]) => Statement;
 batch: (statements: Statement[]) => Promise<FixtureResult[]>;
 partySnapshot: () => Promise<Record<string,unknown>[]>;
 rpcFetch: (path: string,options?: Parameters<TestHarness["fetch"]>[1]) => ReturnType<TestHarness["fetch"]>;
 call: (path: string,token: string | null,method?: string) => Promise<{status:number;body:Record<string,unknown>}>;
 sourceSha: string; admin: string; viewer: string; tenantB: string; knownService: string; unknownService: string;
 jwt: (email: string,signingKey?: Sign,signingKid?: string) => string;
 serviceJwt: (name: string,signingKey?: Sign,signingKid?: string) => string;
 signedJwt: (claims: Claims,signingKey?: Sign,signingKid?: string) => string;
 privateKey: Sign;
};
type Scenarios = (fixture: HarnessScenario) => Promise<void>;

async function runNative(context: TestContext,scenarios: Scenarios,denialProof?: DenialProof): Promise<DenialProof | undefined>{
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
let base: string,fixtureDb: FixtureDb | undefined;
const successfulCalls=new Set<string>();
const mutationPaths=new Set<string>();
const writesWithReceipt=new Set<string>();
// Wrangler owns the Worker, local D1 migrations and fixture binding. Every
// live fixture read/write and the actual Rust Worker use that same D1 binding.
const statement=(query: string,...params: BindingValue[])=>{
 assert.ok(fixtureDb,"D1 fixture is not initialized");
 const prepared=fixtureDb.prepare(query);
 return params.length?prepared.bind(...params):prepared;
};
const sql=async(query: string,...params: BindingValue[])=>{
 const result=await statement(query,...params).all();
 assert.equal(result.success,true);
 return result.results.map(row=>({...row}));
};
const batch=async(statements: Statement[])=>{
 assert.ok(fixtureDb,"D1 fixture is not initialized");
 const results=await fixtureDb.batch(statements);
 assert.equal(results.length,statements.length);
 assert.ok(results.every(result=>result.success));
 return results;
};
const rpcFetch=async(path: string,options?: Parameters<TestHarness["fetch"]>[1])=>{
 // Use Wrangler's native route-dispatch API for RPC integration tests.
 // Chromium still exercises the actual listening HTTP server. Never retry a mutation.
 let response;
 try {response=await testHarness.fetch(base+path,options);}
 catch(error) {
   // No mutation retry: an interrupted Connect request can have an unknown outcome.
   // A separate read-only probe distinguishes a closed keepalive connection
   // from a stopped workerd process, without hiding the original failure.
   let probe;
   try {
     const health=await globalThis.fetch(base+"/api/parties",{
       method:"GET",headers:{connection:"close"},signal:AbortSignal.timeout(3000)});
     probe={reachable:true,status:health.status};
     await health.arrayBuffer();
   }catch(failure){probe={reachable:false,error:failure instanceof Error?failure.name:"UnknownError"};}
   const evidence={event:"crm.native-ci.transport-failure",result:"FAIL",
     method:options?.method??"GET",path,
     probe,
     error:error instanceof Error?error.name:"UnknownError"};
   console.error(JSON.stringify(evidence));
   throw new Error("Native workerd transport failure: "+JSON.stringify(evidence));
 }
 if(path.startsWith("/crm.")&&response.ok){
   if(mutationPaths.has(path)){
     const {receipt,changed}=record(decodeRpcOutput(path,new Uint8Array(await response.clone().arrayBuffer())));
     if(changed===false){
       assert.equal(receipt,undefined,path+" unchanged command claimed a write");
     }else{
       const {requestId,resourceVersion}=record(receipt);
       assert.ok(typeof requestId==="string"&&requestId===response.headers.get("x-request-id"),path);
       assert.ok(typeof resourceVersion==="bigint"&&resourceVersion>0n,path+" missing committed version");
       writesWithReceipt.add(path);
     }
   }
   successfulCalls.add(path);
 }
 if(path.startsWith("/crm.")&&!response.ok){
   const error=record(await readRpcError(response.clone(),path,options?.method??"GET"));
   if(mutationPaths.has(path)){
     const wireDetail=records(error.details).find(d=>d.type==="crm.core.v2.DomainErrorDetail");
     assert.ok(wireDetail,"Missing typed mutation outcome for "+path);
     const detail=decodeDomainErrorDetail({type:string(wireDetail.type),value:string(wireDetail.value)});
     assert.ok(detail.domainCode && [1,2].includes(detail.commitOutcome),path);
     assert.ok(detail.requestId && detail.requestId===response.headers.get("x-request-id"),path);
     if([401,403].includes(response.status))assert.equal(detail.commitOutcome,1,path);
   }
   assert.equal(response.headers.get("x-crm-error-code"),error.code,
     "RPC diagnostics must preserve the actual Connect error code");
 }
 return response;
};
const sourceSha=execFileSync("git",["rev-parse","HEAD"],{cwd,encoding:"utf8"}).trim();
assert.equal(process.env.CRM_SOURCE_SHA,sourceSha,"runtime evidence must identify tested checkout");
const runtimeProof=string(process.env.CRM_RPC_RUNTIME_PROOF);
// SQL fixture access goes through the same native D1 binding as CRM.
const partySnapshot=async()=>(await sql("SELECT "+
  "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_party_records) AS states,"+
  "(SELECT COUNT(*) FROM crm_party_receipts) AS receipts,"+
  "(SELECT COUNT(*) FROM crm_party_import_receipts) AS imports,"+
  "(SELECT COUNT(*) FROM crm_audit) AS audits"));
const {privateKey,publicKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const kid="local-signed-rsa-fixture";
const originalJwk={...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"};
const jwks={keys:[originalJwk]};
const iss="https://royal-limit-ea43.cloudflareaccess.com";
const aud="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const now=Math.floor(Date.now()/1000);
function signedJwt(identityClaims: Claims,signingKey=privateKey,signingKid=kid){
 const parts=[{alg:"RS256",kid:signingKid,typ:"JWT"},{iss,aud,exp:now+3600,iat:now-10,...identityClaims}]
   .map(x=>Buffer.from(JSON.stringify(x)).toString("base64url"));
 const raw=parts.join(".");
 return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(signingKey).toString("base64url");
}
const jwt=(email: string,signingKey=privateKey,signingKid=kid)=>signedJwt({sub:email,email},signingKey,signingKid);
const serviceJwt=(common_name: string,signingKey=privateKey,signingKid=kid)=>signedJwt({sub:"",common_name},signingKey,signingKid);
const configPath=join(cwd,"wrangler.jsonc");
const config=record(JSON.parse(readFileSync(configPath,"utf8")));
const databases=records(config.d1_databases);
assert.equal(databases.length,1);
assert.equal(record(databases[0]).binding,"DB");
assert.equal(record(databases[0]).database_id,"00000000-0000-4000-8000-000000000001");
assert.equal(record(config.vars).CRM_CI_LOCAL_ONLY,"yes");
// Use the same provider boundary as production-access.mjs. Do not inject
// authentication keys into the Worker or bypass its normal JWKS fetch.
const testHarness=createTestHarness({
 root:cwd,
 workers:[{configPath,vars:{GIT_SHA:sourceSha}}]
});
let jwksRequests=0;
const fetchOriginal=globalThis.fetch.bind(globalThis);
context.mock.method(globalThis,"fetch",async(input: Parameters<typeof fetch>[0],init?: Parameters<typeof fetch>[1])=>{
 const url=typeof input==="string"?input:input instanceof URL?input.href:input.url;
 if(url===iss+"/cdn-cgi/access/certs"){
   assert.equal(init?.method??(typeof input==="string"||input instanceof URL?"GET":input.method),"GET");
   jwksRequests++;
   return Response.json(jwks);
 }
 return fetchOriginal(input,init);
});
try{
 const {url}=await testHarness.listen();
 base=url.origin;
 const worker=testHarness.getWorker<Record<string,unknown> & {DB: FixtureDb}>("crm-rust-party-ci-local");
 await testHarness.getWorker("crm-rust-party-ci-local").applyD1Migrations("DB");
 const env=await worker.getEnv();
 assert.equal(env.CRM_CI_LOCAL_ONLY,"yes");
 assert.equal(env.CRM_NATIVE_CI_JWKS,undefined);
 assert.equal(env.GIT_SHA,sourceSha);
 fixtureDb=env.DB;
 assert.ok(fixtureDb,"Wrangler test harness did not expose the configured D1 binding");
 assert.equal(record((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0]).n,0,
   "the harness must start with a clean isolated D1 database");
 await sql("INSERT INTO crm_tenants VALUES(?,?),(?,?)","tenant-a","A","tenant-b","B");
 await sql("INSERT INTO crm_users(email,tenant_id,role,active) VALUES"+
   "(?,?,?,1),(?,?,?,1),(?,?,?,1),(?,?,?,1)",
   "native-a@example.test","tenant-a","admin",
   "native-viewer@example.test","tenant-a","viewer",
   "native-b@example.test","tenant-b","admin",
   "service:ci-fixture.access","tenant-a","admin");
 // This is a fixture row created in isolated D1 before the Worker assertions,
 // not a second implementation of CRM's Party persistence.
 const state=JSON.stringify({party_id:"binary-rpc-proof-01",kind:"organization",
   display_name:"Original Rust Party",created_at_unix_nanos:10,
   updated_at_unix_nanos:10,version:1});
 await sql("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) "+
   "VALUES(?,?,?,?,?,?,?,?,?)","tenant-a","binary-rpc-proof-01",state,1,
   "fixture-key-01","fixture-digest-01",null,"native-a@example.test","Original Rust Party");
 // The isolated denial harness proves idempotent migrations once. The
 // separate business harness still initializes its own fresh D1 and checks
 // the full migration ledger, without repeating that same reapply proof.
 if(denialProof===undefined)await testHarness.getWorker("crm-rust-party-ci-local").applyD1Migrations("DB");
 assert.equal(record((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0]).n,2,
   "Wrangler D1 must preserve tenant fixtures");
 assert.equal(record((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE id='binary-rpc-proof-01'"))[0]).n,1,
   "Wrangler D1 must preserve original business data");
 const migrationDir=join(cwd,string(record(databases[0]).migrations_dir));
 const migrations=readdirSync(migrationDir).filter(name=>name.endsWith(".sql")).sort();
 assert.deepEqual((await sql("SELECT name FROM d1_migrations ORDER BY name")).map(row=>row.name),migrations,
   "local D1 must have applied the entire configured migration history once");
 console.log(JSON.stringify({event:"crm.native-ci.d1-runtime",source_sha:sourceSha,
   runtime:"Wrangler createTestHarness/workerd",fixture_access:"D1 binding",
   worker_count:1,d1_count:1}));
 async function call(path: string,token: string | null,method="GET"){
   const response=await rpcFetch(path,{method,
     headers:{...(token?{"cf-access-jwt-assertion":token}:{}),"content-type":"application/json"},
     ...(method==="POST"?{body:"{}"}:{})});
   return {status:response.status,body:record(await response.json())};
 }
 assert.equal((await call("/api/parties",null)).status,401);
 const admin=jwt("native-a@example.test"),viewer=jwt("native-viewer@example.test");
 const knownService=serviceJwt("ci-fixture.access");
 const unknownService=serviceJwt("unassigned-fixture.access");
 assert.equal((await call("/api/session",knownService)).status,200,
   "An explicitly enrolled service must resolve its existing tenant");
 assert.equal((await call("/api/session",unknownService)).status,401,
   "An unknown signed service must not auto-create an admin tenant");
 assert.equal((await call("/api/session",signedJwt({
   sub:"",common_name:"ci-fixture.access",aud:"wrong-app-audience"
 }))).status,401,"Cross-application service JWT must be denied");
 if(denialProof!==undefined){
 assert.equal(jwksRequests,1,
   "the same Access signing key must be fetched once and reused for later signed requests");
 const rotated=generateKeyPairSync("rsa",{modulusLength:2048});
 const rotatedKid="local-signed-rsa-rotated";
 const rotatedJwk={...rotated.publicKey.export({format:"jwk"}),kid:rotatedKid,alg:"RS256",use:"sig"};
 jwks.keys=[originalJwk,rotatedJwk];
 const rotatedService=serviceJwt("ci-fixture.access",rotated.privateKey,rotatedKid);
 assert.equal((await call("/api/session",rotatedService)).status,200);
 assert.equal(jwksRequests,2,"new kid must cause exactly one JWKS refresh");
 assert.equal((await call("/api/session",knownService)).status,200,
   "the previous Access key remains valid during the published overlap window");
 assert.equal((await call("/api/session",rotatedService)).status,200);
 const unknownKidToken=signedJwt({sub:"",common_name:"ci-fixture.access"},privateKey,"local-signed-rsa-unknown");
 assert.equal((await call("/api/session",unknownKidToken)).status,401,
   "a kid absent from refreshed JWKS must fail closed");
 assert.equal(jwksRequests,3,"unknown kid must refresh once and then be rejected");
 assert.equal((await call("/api/session",rotatedService)).status,200);
 assert.equal(jwksRequests,3,"the refreshed CryptoKey must be reused without another provider fetch");
 console.log(JSON.stringify({event:"crm.access-jwks-cache",result:"PASS",jwks_requests:jwksRequests,
   cached_key_reuse:true,rotated_kid_refresh:true,previous_key_overlap:true,unknown_kid_denied:true}));
 }

 const version=await rpcFetch("/api/version",{headers:{"cf-access-jwt-assertion":admin}});
 assert.equal(version.status,200);
 assert.equal(version.headers.get("x-crm-source-sha"),sourceSha,"Worker must receive the tested checkout SHA");
 assert.equal(record(await version.json()).source,sourceSha);
 const compiled=await call("/api/_ci/original-domain-check",admin);
 assert.equal(compiled.status,200,formatRpcFailure(compiled));
 assert.equal(compiled.body.result,"PASS");
 // The normal runtime doctor exposes the compiled inventory to admins.
 // Local workerd has no deployed version metadata: never fabricate it.
 const doctor=await call("/api/doctor",admin);
 assert.equal(doctor.body.source,sourceSha);
 assert.equal(record(doctor.body.database).status,"PASS");
 assert.equal(doctor.status,record(doctor.body.release).source_version==="PASS"?200:503);
 const inventory=record(doctor.body.rpc);
 assert.equal(inventory.compiled_policy_check,true);
 assert.ok(Array.isArray(inventory.policies));
 const policies=inventory.policies.map(([path,rawRule]:unknown[]): [string,Policy]=>{
   const rule=record(rawRule);
   const permission=string(rule.permission);
   assert.ok(["Read","Write","AdminRead","SelfWrite"].includes(permission));
   return [string(path),{permission}];
 });
 const rpc: RpcInventory={compiled_policy_check:true,registered_methods:strings(inventory.registered_methods),policies};
 assert.equal((await call("/api/doctor",viewer)).body.rpc,null);
 assert.equal((await call("/api/doctor",null)).status,401);
 assert.ok(rpc.registered_methods.length>0);
 assert.equal(new Set(rpc.registered_methods).size,rpc.registered_methods.length);
 assert.equal(new Map(rpc.policies).size,rpc.policies.length,"duplicate compiled policy");
 assert.deepEqual(rpc.policies.map(([path])=>path).sort(),[...rpc.registered_methods].sort());
 for(const [path,rule] of rpc.policies){
   if(rule.permission==="Write"||rule.permission==="SelfWrite")mutationPaths.add(path);
 }
 // The denial matrix requires no business scenario state. Execute it once
 // in a separate native harness; do not repeat it in the business subtest.
 if(denialProof===undefined){
   const beforeDenials=await partySnapshot();
   const unauthenticatedDenials=[],viewerDenials=[];
   for(const [path,rule] of rpc.policies){
     const unauthenticated=await rpcFetch(path,{method:"POST",
       headers:{"content-type":"application/proto","connect-protocol-version":"1"},
       body:new Uint8Array()});
     assert.equal(unauthenticated.status,401,"missing identity must deny "+path);
     assert.equal(record(await unauthenticated.json()).code,"unauthenticated",path);
     unauthenticatedDenials.push(path);
     if(rule.permission==="Write"||rule.permission==="AdminRead"){
       const denied=await rpcFetch(path,{method:"POST",
         headers:{"content-type":"application/proto","connect-protocol-version":"1",
           "cf-access-jwt-assertion":viewer},body:new Uint8Array()});
       assert.equal(denied.status,403,"viewer must not access admin RPC "+path);
       assert.equal(record(await denied.json()).code,"permission_denied",path);
       viewerDenials.push(path);
     }
   }

   // Only technical HTTP endpoints remain; business paths must not bypass Connect.
   for(const path of ["/api/parties","/api/accounts",
       "/api/customer-data-operations/parties.csv","/api/manual-business",
       "/api/version/other"]){
     assert.equal((await call(path,admin)).status,404,path);
   }
   for(const path of ["/crm.parties.v2.PartyService/GetParty",
       "/crm.sales.v1.DealService/GetDeal","/crm.sales.v2.DealService/NotRegistered",
       "/crm.future.v9.FutureService/NotRegistered"]){
     const denied=await call(path,admin,"POST");
     assert.equal(denied.status,501,path);
     assert.equal(denied.body.code,"unimplemented");
   }
   assert.deepEqual(await partySnapshot(),beforeDenials,"denied routes must not mutate the fixture");
   assert.equal(jwksRequests,1,
     "the isolated denial harness must fetch one JWKS for its repeated signed policy checks");
   console.log(JSON.stringify({event:"crm.access-jwks-cache-policy",result:"PASS",jwks_requests:jwksRequests}));
   return {rpc:inventory,unauthenticatedDenials,viewerDenials};
 }
 assert.deepEqual(inventory,denialProof.rpc,"isolated denial and business harnesses must expose identical compiled RPC/owner/policy inventory");
 const {unauthenticatedDenials,viewerDenials}=denialProof;
 const tenantB=jwt("native-b@example.test");
 await scenarios({cwd,base,worker,testHarness,sql,statement,batch,partySnapshot,rpcFetch,call,
   sourceSha,admin,viewer,tenantB,knownService,unknownService,jwt,serviceJwt,signedJwt,privateKey});
 assert.deepEqual([...writesWithReceipt].sort(),[...mutationPaths].sort(),
   "every write method must exercise a committed response with a typed receipt");
 assert.deepEqual([...successfulCalls].sort(),[...rpc.registered_methods].sort(),
   "every compiled RPC must have a successful executed binary call");
 writeFileSync(runtimeProof,JSON.stringify({
   source_sha:sourceSha,result:"PASS",features:["native-ci"],rpc:inventory,
   committed_rpc_calls:[...writesWithReceipt].sort(),
   successful_rpc_calls:[...successfulCalls].sort(),
   unauthenticated_denials:unauthenticatedDenials.sort(),
   viewer_denials:viewerDenials.sort(),
   fixture_access:"D1 binding",runtime:"Wrangler createTestHarness/workerd",
   jwks_requests:jwksRequests,
   access_key_cache:{result:"PASS",ttl_seconds:300,rotation_refresh:true,
     previous_key_overlap:true,unknown_kid_denied:true},
   authentication:"production JWKS path",
  scenarios:"existing signed workerd assertions completed"
 },null,2)+"\n");
 console.log(JSON.stringify({event:"crm.signed-connect-acceptance",
   result:"PASS",admin:true,viewer:true,unauthenticated_denied:true,
   legacy_mutations:0,verified_auth:true}));
}catch(error){
 testHarness.debug();
 throw error;
}finally{
 try{await testHarness.close();}finally{context.mock.restoreAll();}
}
}

// Serial first: fetch interception is process-global. A later concurrency
// experiment needs process isolation, not overlapping context mocks.
export function registerNativeTests(scenarios: Scenarios){
test("CRM signed workerd integration",async context=>{
 let denialProof: DenialProof | undefined;
 await context.test("compiled policy and unknown routes on isolated D1",async subtest=>{
   denialProof=await runNative(subtest,scenarios);
 });
 assert.ok(denialProof,"isolated policy denial proof required before business acceptance");
 await context.test("signed business owners, D1 and Chromium on fresh D1",async subtest=>{
   await runNative(subtest,scenarios,denialProof);
 });
});
}

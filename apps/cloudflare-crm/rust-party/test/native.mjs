// Shared native Wrangler fixture: each test file owns a fresh Worker and D1.
import {execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign} from "node:crypto";
import {readFileSync,readdirSync} from "node:fs";
import {createTestHarness} from "wrangler";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import {decodeDomainErrorDetail,decodeRpcOutput} from "../../../../scripts/crm-connect-binary-fixture.mjs";
export async function withNativeHarness(context,scenario){
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
let base,fixtureDb;
const successfulCalls=new Set();
const mutationPaths=new Set();
const writesWithReceipt=new Set();
function assertUnconfirmed(result, domainCode){
 assert.equal(result.status,503);
 assert.equal(result.body.code,"unavailable");
 const detail=decodeDomainErrorDetail(result.body.details.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
 assert.equal(detail.commitOutcome,2);
 assert.equal(detail.domainCode,domainCode);
 assert.equal(detail.requestId,result.headers.get("x-request-id"));
}

// Wrangler owns the Worker, local D1 migrations and fixture binding. Every
// live fixture read/write and the actual Rust Worker use that same D1 binding.
const statement=(query,...params)=>{
 assert.ok(fixtureDb,"D1 fixture is not initialized");
 const prepared=fixtureDb.prepare(query);
 return params.length?prepared.bind(...params):prepared;
};
const sql=async(query,...params)=>{
 const result=await statement(query,...params).all();
 assert.equal(result.success,true);
 return result.results.map(row=>({...row}));
};
const batch=async(statements)=>{
 const results=await fixtureDb.batch(statements);
 assert.equal(results.length,statements.length);
 assert.ok(results.every(result=>result.success));
 return results;
};
const rpcFetch=async(path,options)=>{
 // Keep every CI RPC on a fresh HTTP connection. Workerd may close an idle
 // socket between requests; reusing that socket fails with UND_ERR_SOCKET.
 // This does NOT retry a mutation with an unknown outcome.
 const requestOptions={...options,headers:{...options?.headers,connection:"close"}};
 let response;
 try {response=await globalThis.fetch(base+path,requestOptions);}
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
   }catch(failure){probe={reachable:false,error:String(failure)};}
   const evidence={event:"crm.native-ci.transport-failure",result:"FAIL",
     method:options?.method??"GET",path,
     probe,
     error:error instanceof Error?error.message:String(error),
     cause:error instanceof Error&&error.cause?String(error.cause):null};
   console.error(JSON.stringify(evidence));
   throw new Error("Native workerd transport failure: "+JSON.stringify(evidence),{cause:error});
 }
 if(path.startsWith("/crm.")&&response.ok){
   if(mutationPaths.has(path)){
     const {receipt,changed}=decodeRpcOutput(path,new Uint8Array(await response.clone().arrayBuffer()));
     if(changed===false){
       assert.equal(receipt,undefined,path+" unchanged command claimed a write");
     }else{
       assert.ok(receipt,path+" missing typed receipt");
       assert.ok(receipt.requestId && receipt.requestId===response.headers.get("x-request-id"),path);
       assert.ok(receipt.resourceVersion>0n,path+" missing committed version");
       writesWithReceipt.add(path);
     }
   }
   successfulCalls.add(path);
 }
 if(path.startsWith("/crm.")&&!response.ok){
   const raw=await response.clone().text();
   let error;
   try{error=JSON.parse(raw);}
   catch{
     const evidence={event:"crm.native-ci.non-connect-response",path,
       method:options?.method??"GET",status:response.status,
       content_type:response.headers.get("content-type"),
       body_excerpt:raw.slice(0,600),request_id:response.headers.get("x-request-id")};
     console.error(JSON.stringify(evidence));
     throw new Error("Non-Connect RPC response: "+JSON.stringify(evidence));
   }
   if(mutationPaths.has(path)){
     const detail=decodeDomainErrorDetail(error.details?.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
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
assert.ok(process.env.CRM_RPC_RUNTIME_PROOF,"CI runtime output path required");
// SQL fixture access goes through the same native D1 binding as CRM.
const partySnapshot=async()=>(await sql("SELECT "+
  "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_party_records) AS states,"+
  "(SELECT COUNT(*) FROM crm_party_receipts) AS receipts,"+
  "(SELECT COUNT(*) FROM crm_party_import_receipts) AS imports,"+
  "(SELECT COUNT(*) FROM crm_audit) AS audits"));
const {privateKey,publicKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const kid="local-signed-rsa-fixture";
const jwks={keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]};
const iss="https://royal-limit-ea43.cloudflareaccess.com";
const aud="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const now=Math.floor(Date.now()/1000);
function signedJwt(identityClaims){
 const parts=[{alg:"RS256",kid,typ:"JWT"},{
   iss,aud,exp:now+3600,iat:now-10,...identityClaims
 }].map(x=>Buffer.from(JSON.stringify(x)).toString("base64url"));
 const raw=parts.join(".");
 return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(privateKey).toString("base64url");
}
const jwt=email=>signedJwt({sub:email,email});
const serviceJwt=common_name=>signedJwt({sub:"",common_name});
const configPath=join(cwd,"wrangler.jsonc");
const config=JSON.parse(readFileSync(configPath,"utf8"));
assert.equal(config.d1_databases.length,1);
assert.equal(config.d1_databases[0].binding,"DB");
assert.equal(config.d1_databases[0].database_id,"00000000-0000-4000-8000-000000000001");
assert.equal(config.vars.CRM_CI_LOCAL_ONLY,"yes");
// Use the same provider boundary as production-access.mjs. Do not inject
// authentication keys into the Worker or bypass its normal JWKS fetch.
const testHarness=createTestHarness({
 root:cwd,
 workers:[{configPath,vars:{GIT_SHA:sourceSha}}]
});
let jwksRequests=0;
const fetchOriginal=globalThis.fetch.bind(globalThis);
context.mock.method(globalThis,"fetch",async(input,init)=>{
 const url=typeof input==="string"?input:input instanceof URL?input.href:input.url;
 if(url===iss+"/cdn-cgi/access/certs"){
   assert.equal(init?.method??input?.method??"GET","GET");
   jwksRequests++;
   return Response.json(jwks);
 }
 return fetchOriginal(input,init);
});
try{
 const {url}=await testHarness.listen();
 base=url.origin;
 const worker=testHarness.getWorker("crm-rust-party-ci-local");
 await worker.applyD1Migrations("DB");
 const env=await worker.getEnv();
 assert.equal(env.CRM_CI_LOCAL_ONLY,"yes");
 assert.equal(env.CRM_NATIVE_CI_JWKS,undefined);
 assert.equal(env.GIT_SHA,sourceSha);
 fixtureDb=env.DB;
 assert.ok(fixtureDb,"Wrangler test harness did not expose the configured D1 binding");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0].n,0,
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
 // Reapplying through Wrangler must preserve the existing D1 data and ledger.
 await worker.applyD1Migrations("DB");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0].n,2,
   "reapplying Wrangler migrations must preserve tenant fixtures");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE id='binary-rpc-proof-01'"))[0].n,1,
   "reapplying Wrangler migrations must preserve business data");
 const migrationDir=join(cwd,config.d1_databases[0].migrations_dir);
 const migrations=readdirSync(migrationDir).filter(name=>name.endsWith(".sql")).sort();
 assert.deepEqual((await sql("SELECT name FROM d1_migrations ORDER BY name")).map(row=>row.name),migrations,
   "local D1 must have applied the entire configured migration history once");
 console.log(JSON.stringify({event:"crm.native-ci.d1-runtime",source_sha:sourceSha,
   runtime:"Wrangler createTestHarness/workerd",fixture_access:"D1 binding",
   worker_count:1,d1_count:1}));
 async function call(path,token,method="GET"){
   const response=await rpcFetch(path,{method,
     headers:{...(token?{"cf-access-jwt-assertion":token}:{}),"content-type":"application/json"},
     ...(method==="POST"?{body:"{}"}:{})});
   return {status:response.status,body:await response.json()};
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
 assert.ok(jwksRequests>=2,"signed service fixtures must exercise the ordinary JWKS provider");
 const version=await rpcFetch("/api/version",{headers:{"cf-access-jwt-assertion":admin}});
 assert.equal(version.status,200);
 assert.equal(version.headers.get("x-crm-source-sha"),sourceSha,"Worker must receive the tested checkout SHA");
 assert.equal((await version.json()).source,sourceSha);
 const compiled=await call("/api/_ci/original-domain-check",admin);
 assert.equal(compiled.status,200,JSON.stringify(compiled.body));
 assert.equal(compiled.body.result,"PASS");
 // The normal runtime doctor exposes the compiled inventory to admins.
 // Local workerd has no deployed version metadata: never fabricate it.
 const doctor=await call("/api/doctor",admin);
 assert.equal(doctor.body.source,sourceSha);
 assert.equal(doctor.body.database.status,"PASS");
 assert.equal(doctor.status,doctor.body.release.source_version==="PASS"?200:503);
 const rpc=doctor.body.rpc;
 assert.equal(rpc.compiled_policy_check,true);
 assert.equal((await call("/api/doctor",viewer)).body.rpc,null);
 assert.equal((await call("/api/doctor",null)).status,401);
 assert.ok(rpc.registered_methods.length>0);
 assert.equal(new Set(rpc.registered_methods).size,rpc.registered_methods.length);
 assert.equal(new Map(rpc.policies).size,rpc.policies.length,"duplicate compiled policy");
 assert.deepEqual(rpc.policies.map(([path])=>path).sort(),[...rpc.registered_methods].sort());
 for(const [path,rule] of rpc.policies){
   if(rule.permission==="Write"||rule.permission==="SelfWrite")mutationPaths.add(path);
 }
 return await scenario({cwd,base,sourceSha,sql,statement,batch,rpcFetch,partySnapshot,assertUnconfirmed,
   jwt,admin,viewer,knownService,unknownService,call,rpc,worker,testHarness,
   successfulCalls,mutationPaths,writesWithReceipt,jwksRequests:()=>jwksRequests});
}catch(error){
 testHarness.debug();
 throw error;
}finally{
 try{await testHarness.close();}finally{context.mock.restoreAll();}
}
}

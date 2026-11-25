// Full generated binary Connect contract and actual React against signed workerd/D1.
import {execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,randomUUID,createHash} from "node:crypto";
import {writeFileSync,readFileSync,readdirSync} from "node:fs";
import {createTestHarness} from "wrangler";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import {mock} from "node:test";
import {chromium} from "playwright";
import {typedBrowser} from "./typed-browser.mjs";
import {installUiDiagnostics,assertUiPanel} from "./ui-diagnostics.mjs";
import {binaryPartyV3,decodePartyV3,partyV3Path,
  binaryAccountV2,decodeAccountV2,accountV2Path,
  binaryContactV2,decodeContactV2,contactV2Path,
  binaryRelationshipV2,decodeRelationshipV2,relationshipV2Path,
  binaryTaskV2,decodeTaskV2,taskV2Path,
  binaryMemberV2,decodeMemberV2,memberV2Path,
  binaryInteractionV2,decodeInteractionV2,interactionV2Path,
  binaryAutomationV2,decodeAutomationV2,automationV2Path,
  binaryQualityV2,decodeQualityV2,qualityV2Path,
  binaryIdentityV2,decodeIdentityV2,identityV2Path,
  binaryConsentV2,decodeConsentV2,consentV2Path,
  binarySalesV2,decodeSalesV2,salesV2Path,
  binaryPrivacyV2,decodePrivacyV2,privacyV2Path,
  binaryEnrichmentV2,decodeEnrichmentV2,enrichmentV2Path,
  binaryCsvV2,decodeCsvV2,csvV2Path,
  binaryOverviewV2,decodeOverviewV2,overviewV2Path,decodeDomainErrorDetail,decodeRpcOutput} from "../../../../scripts/crm-connect-binary-fixture.mjs";
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
mock.method(globalThis,"fetch",async(input,init)=>{
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
 const unauthenticatedDenials=[],viewerDenials=[];
 for(const [path,rule] of rpc.policies){
   const unauthenticated=await rpcFetch(path,{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1"},
     body:new Uint8Array()});
   assert.equal(unauthenticated.status,401,"missing identity must deny "+path);
   assert.equal((await unauthenticated.json()).code,"unauthenticated",path);
   unauthenticatedDenials.push(path);
   if(rule.permission==="Write"||rule.permission==="AdminRead"){
     const denied=await rpcFetch(path,{method:"POST",
       headers:{"content-type":"application/proto","connect-protocol-version":"1",
         "cf-access-jwt-assertion":viewer},body:new Uint8Array()});
     assert.equal(denied.status,403,"viewer must not access admin RPC "+path);
     assert.equal((await denied.json()).code,"permission_denied",path);
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
 const tenantB=jwt("native-b@example.test");
 const describeProtobuf=value=>JSON.stringify(value,
   (_key,item)=>typeof item==="bigint"?item.toString():item);
 const v3=async(method,token,payload,extraHeaders={})=>{
   const response=await rpcFetch(partyV3Path(method),{method:"POST",
     headers:{...extraHeaders,"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryPartyV3(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodePartyV3(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 // A real binary Connect paginated scan: every nonfinal cursor must carry a
 // matching record. A filtered empty page with a cursor is a data-loss bug.
 const collectPages=async(rpc,method,field,filter={})=>{
   let token="",items=[];
   const cursors=new Set();
   // R7's real board creates more than 25 Deals before the Account 360
   // composition scan. Retain a finite guard and strict cursor-cycle checks,
   // but never mistake the former 16-page fixture cap for a server failure.
   const maxPages=256;
   for(let attempt=0;attempt<maxPages;attempt++){
     const result=await rpc(method,viewer,{...filter,page:{pageSize:1,pageToken:token}});
     assert.equal(result.status,200,describeProtobuf(result.body));
     const found=result.body[field]??[];
     assert.ok(found.length<=1,"page_size=1 must bound the filtered result");
     const next=result.body.pageInfo?.nextPageToken??"";
     if(next){
       assert.equal(found.length,1,"filtered cursor skipped a matching row");
       assert(!cursors.has(next),"cursor cycle in filtered pagination");
       cursors.add(next);
     }
     items.push(...found);
     if(!next)return items;
     token=next;
   }
   throw Error("filtered pagination never reached its last page: "+
     JSON.stringify({method,field,rows:items.length,lastCursor:token}));
 };
 const id="native-v3-"+randomUUID();
 const creation={partyId:id,kind:2,displayName:"  Connect   Native  Rust ",
   precondition:{idempotencyKey:"candidate-v3-create-001"}};
 assert.equal((await v3("CreateParty",null,creation)).status,401);
 assert.equal((await v3("CreateParty",viewer,creation)).status,403);
 assert.equal((await v3("CreateParty",admin,{...creation,kind:0,
   precondition:{idempotencyKey:"candidate-v3-invalid-kind"}})).status,400);
 assert.equal((await v3("CreateParty",admin,{...creation,
   precondition:{idempotencyKey:"candidate-v3-invalid-pre",
     expectedVersion:1n}})).status,400);
 const created=await v3("CreateParty",admin,creation,{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test","x-request-id":"forged-request"});
 assert.equal(created.status,200,describeProtobuf(created.body));
 assert.equal(created.body.party?.partyId,id);
 assert.equal(created.body.party?.displayName,"Connect Native Rust");
 assert.equal(created.body.receipt?.resourceVersion,1n);
 assert.equal(created.body.receipt?.replayed,false);
 assert.equal(created.body.receipt?.requestId,created.headers.get("x-request-id"));
 const replay=await v3("CreateParty",knownService,creation);
 assert.equal(replay.status,200);
 assert.equal(replay.body.receipt?.replayed,true);
 assert.equal((await v3("CreateParty",admin,{...creation,
   displayName:"different replay"})).status,409);
 assert.equal((await v3("GetParty",tenantB,{partyId:id})).status,404);
 assert.equal((await v3("GetParty",knownService,{partyId:id})).status,200,
   "Enrolled service and human admin must read the same tenant");
 const read=await v3("GetParty",viewer,{partyId:id});
 assert.equal(read.status,200);
 assert.equal(read.body.party?.displayName,"Connect Native Rust");
 const listing=await v3("ListParties",viewer,{page:{pageSize:1}});
 assert.equal(listing.status,200,describeProtobuf(listing.body));
 assert.equal(listing.body.parties.length,1);
 assert.ok(listing.body.pageInfo?.nextPageToken);
 assert.equal((await v3("ListParties",viewer,{page:{pageSize:101}})).status,400);
 const updatedInput={partyId:id,displayName:"Updated via native v3",
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-v3-update-001"}};
 assert.equal((await v3("UpdateParty",viewer,updatedInput)).status,403);
 const updated=await v3("UpdateParty",knownService,updatedInput);
 assert.equal(updated.status,200,describeProtobuf(updated.body));
 assert.equal(updated.body.party?.version,2n);
 assert.equal(updated.body.party?.displayName,"Updated via native v3");
 assert.equal(updated.body.receipt?.resourceVersion,2n);
 assert.equal(updated.body.receipt?.replayed,false);
 const again=await v3("UpdateParty",admin,updatedInput);
 assert.equal(again.status,200);
 assert.equal(again.body.receipt?.replayed,true);
 assert.equal((await v3("UpdateParty",admin,{...updatedInput,
   precondition:{expectedVersion:1n,
      idempotencyKey:"candidate-v3-stale-version"}})).status,409);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0].n,2,
   "Unknown signed service must not create another tenant");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_users"))[0].n,4,
   "Unknown signed service must not insert an admin actor");
 assert.equal((await sql("SELECT tenant_id FROM crm_users WHERE email='service:ci-fixture.access'"))[0].tenant_id,
   "tenant-a","Explicit service enrollment must retain the owner's tenant");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE id='binary-rpc-proof-01'"))[0].n,1,
   "Repeated migrations apply must preserve existing business data");
 assert.equal((await sql("SELECT version FROM crm_party_records WHERE id='"+id+"'"))[0].version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v3-create-001','candidate-v3-update-001')"))[0].n,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+id+"'"))[0].n,2);
 assert.equal((await v3("GetParty",admin,{partyId:id})).body.party?.version,2n);
 assert.deepEqual((await sql("SELECT action,tenant_id,subject_id,request_id,actor FROM crm_audit WHERE subject_id='"+id+"' ORDER BY rowid")),[
   {action:"parties.create",tenant_id:"tenant-a",subject_id:id,
     request_id:created.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"parties.update",tenant_id:"tenant-a",subject_id:id,
     request_id:updated.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Cross-actor replay must retain the original audit identity and request");
 const partyBeforeDenials=(await partySnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await v3("CreateParty",token,creation)).status>=400);
   assert.ok((await v3("UpdateParty",token,updatedInput)).status>=400);
 }
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await v3("CreateParty",knownService,creation)).status,401,
   "Inactive actor cannot retrieve a previously committed mutation by replay");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await v3("UpdateParty",knownService,updatedInput)).status,403,
   "Current permission is required before replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.equal((await v3("UpdateParty",tenantB,updatedInput)).status,404);
 assert.equal((await v3("UpdateParty",admin,{...updatedInput,displayName:"   ",
   precondition:{expectedVersion:2n,idempotencyKey:"party-invalid-name"}})).status,400);
 assert.deepEqual((await partySnapshot()),partyBeforeDenials,"Denied/invalid operations must not change Party state, receipts or audit");

 // This fixture rejects the audit INSERT after the owner has written state and receipt.
 // No test hook or failure path is compiled into the production Worker.
 (await sql("CREATE TRIGGER fixture_fail_party_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('party-audit-failure','"+id+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;"));
 try{
   assertUnconfirmed(await v3("CreateParty",admin,{partyId:"party-audit-failure",kind:1,displayName:"Rollback",
     precondition:{idempotencyKey:"party-audit-failed-create"}}),"PARTIES_WRITE_UNCONFIRMED");
   assertUnconfirmed(await v3("UpdateParty",admin,{...updatedInput,displayName:"Must roll back",
     precondition:{expectedVersion:2n,idempotencyKey:"party-audit-failed-update"}}),"PARTIES_WRITE_UNCONFIRMED");
   assert.deepEqual((await partySnapshot()),partyBeforeDenials,"SQL audit failure must roll back the state and receipt too");
 }finally{await sql("DROP TRIGGER fixture_fail_party_audit");}
 console.log(JSON.stringify({event:"crm.native-party-actor-audit",result:"PASS",
   human_create:true,service_update:true,cross_actor_replay:true,authorization_before_replay:true,
   forged_headers_ignored:true,sql_failure_rollback:true,tenant_ab:true}));

 // Interleave nonmatching person rows between matching organizations.
 // The old Rust post-LIMIT filter produced an empty middle page here.
 const pagingPersonId="native-page-person-a-"+randomUUID();
 for(const [index,personId] of [pagingPersonId,"native-page-person-b-"+randomUUID()].entries()){
   const made=await v3("CreateParty",admin,{
     partyId:personId,kind:1,displayName:"Paging Person "+index,
     precondition:{idempotencyKey:"page-person-create-"+index}});
   assert.equal(made.status,200,describeProtobuf(made.body));
 }
 const organizations=await collectPages(v3,"ListParties","parties",{kind:2});
 assert.deepEqual(organizations.map(p=>p.partyId),["binary-rpc-proof-01",id]);
 const personPages=await collectPages(v3,"ListParties","parties",{kind:1});
 assert.equal(personPages.length,2);
 assert.deepEqual(personPages.map(p=>p.partyId),
   [...personPages.map(p=>p.partyId)].sort());
 assert.ok(personPages.some(p=>p.partyId===pagingPersonId));
 const beyondParty=await v3("ListParties",viewer,{kind:2,page:{pageSize:1,pageToken:id}});
 assert.equal(beyondParty.status,200);
 assert.equal(beyondParty.body.parties.length,0);
 assert.ok(!beyondParty.body.pageInfo?.nextPageToken);
 assert.equal((await v3("ListParties",tenantB,{kind:2,page:{pageSize:1}}))
   .body.parties.length,0);
 console.log(JSON.stringify({event:"crm.native-g3-party-filtered-pagination",result:"PASS"}));
 // Accounts v2: original Account aggregate and D1 trigger, not a pilot record.
 const accountV2=async(method,token,payload,extraHeaders={})=>{
   const response=await rpcFetch(accountV2Path(method),{method:"POST",
     headers:{...extraHeaders,"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryAccountV2(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodeAccountV2(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 const accountId="account-native-"+randomUUID();
 const accountInput={accountId,name:"  Connected   Account  ",primaryPartyId:id,
   precondition:{idempotencyKey:"candidate-v2-account-create-001"}};
 assert.equal((await accountV2("CreateAccount",null,accountInput)).status,401);
 assert.equal((await accountV2("CreateAccount",viewer,accountInput)).status,403);
 assert.equal((await accountV2("CreateAccount",admin,{...accountInput,
   precondition:{idempotencyKey:"candidate-v2-account-version",expectedVersion:1n}})).status,400);
 assert.equal((await accountV2("CreateAccount",admin,{...accountInput,
   primaryPartyId:"no-such-party",
   precondition:{idempotencyKey:"candidate-v2-account-fk"}})).status,400);
 const accountCreated=await accountV2("CreateAccount",admin,accountInput,{
    "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
    "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(accountCreated.status,200,describeProtobuf(accountCreated.body));
 assert.equal(accountCreated.body.account?.accountId,accountId);
 assert.equal(accountCreated.body.account?.name,"Connected Account");
 assert.equal(accountCreated.body.account?.parties?.[0]?.partyId,id);
 assert.equal(accountCreated.body.account?.parties?.[0]?.role,1);
 assert.equal(accountCreated.body.receipt?.resourceVersion,1n);
 assert.equal(accountCreated.body.receipt?.replayed,false);
 assert.equal(accountCreated.body.receipt?.requestId,accountCreated.headers.get("x-request-id"));
 const accountReplay=await accountV2("CreateAccount",knownService,accountInput);
 assert.equal(accountReplay.status,200,describeProtobuf(accountReplay.body));
 assert.equal(accountReplay.body.receipt?.replayed,true);
 assert.equal(accountReplay.body.receipt.originalRequestId,accountCreated.headers.get("x-request-id"));
 assert.equal((await accountV2("CreateAccount",admin,{...accountInput,
   name:"different idempotent body"})).status,409);
 assert.equal((await accountV2("GetAccount",tenantB,{accountId})).status,404);
 assert.equal((await accountV2("GetAccount",viewer,{accountId})).body.account?.name,"Connected Account");
 const accountList=await accountV2("ListAccounts",viewer,{page:{pageSize:1}});
 assert.equal(accountList.status,200,describeProtobuf(accountList.body));
 assert.equal(accountList.body.accounts.length,1);
 assert.equal(accountList.body.accounts[0].accountId,accountId);
 assert.equal((await accountV2("ListAccounts",viewer,{page:{pageSize:101}})).status,400);
 const accountUpdate={accountId,name:"Updated Account",status:2,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-v2-account-update-001"}};
 assert.equal((await accountV2("UpdateAccount",viewer,accountUpdate)).status,403);
 assert.equal((await accountV2("UpdateAccount",admin,{...accountUpdate,status:0,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-v2-account-invalid"}})).status,400);
 const accountUpdated=await accountV2("UpdateAccount",knownService,accountUpdate);
 assert.equal(accountUpdated.status,200,describeProtobuf(accountUpdated.body));
 assert.equal(accountUpdated.body.account?.status,2);
 assert.equal(accountUpdated.body.account?.version,2n);
 assert.equal(accountUpdated.body.receipt?.replayed,false);
 const accountReupdate=await accountV2("UpdateAccount",admin,accountUpdate);
 assert.equal(accountReupdate.status,200,describeProtobuf(accountReupdate.body));
 assert.equal(accountReupdate.body.receipt?.replayed,true);
 assert.equal(accountReupdate.body.receipt.originalRequestId,accountUpdated.headers.get("x-request-id"));
 assert.equal((await accountV2("UpdateAccount",admin,{...accountUpdate,
   precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-v2-account-stale"}})).status,409);
 assert.equal((await accountV2("ListAccounts",viewer,{page:{pageSize:1},status:1}))
   .body.accounts.length,0);
 assert.equal((await accountV2("ListAccounts",viewer,{page:{pageSize:1},status:2}))
   .body.accounts.length,1);
 assert.equal((await sql("SELECT version FROM crm_account_records WHERE tenant='tenant-a' "+
   "AND id='"+accountId+"'"))[0].version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_account_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v2-account-create-001','candidate-v2-account-update-001')"))[0].n,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+accountId+"' AND action LIKE 'accounts.%'"))[0].n,2);
 assert.deepEqual((await sql("SELECT action,tenant_id,subject_id,request_id,actor FROM crm_audit WHERE subject_id='"+accountId+"' ORDER BY rowid")),[
   {action:"accounts.create",tenant_id:"tenant-a",subject_id:accountId,
     request_id:accountCreated.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"accounts.update",tenant_id:"tenant-a",subject_id:accountId,
     request_id:accountUpdated.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Cross-actor Account replay preserves original verified actor/request ID");
 const accountSnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_account_records) AS states,"+
   "(SELECT COUNT(*) FROM crm_account_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 const beforeAccountDenials=(await accountSnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await accountV2("CreateAccount",token,accountInput)).status>=400);
   assert.ok((await accountV2("UpdateAccount",token,accountUpdate)).status>=400);
 }
 assert.equal((await accountV2("UpdateAccount",tenantB,accountUpdate)).status,404);
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await accountV2("CreateAccount",knownService,accountInput)).status,401,
   "Deactivated service cannot replay Account creation");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await accountV2("UpdateAccount",knownService,accountUpdate)).status,403,
   "Current permission must be checked before Account replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.deepEqual((await accountSnapshot()),beforeAccountDenials,
   "Denied/stale/invalid Account commands cannot change state, receipts or audit");
 const accountRetryInput={accountId:"account-audit-rollback",name:"Must roll back",primaryPartyId:id,
   precondition:{idempotencyKey:"account-audit-failed-create"}};
 (await sql("CREATE TRIGGER fixture_fail_account_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('account-audit-rollback','"+accountId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture account audit failure'); END;"));
 try{
   assertUnconfirmed(await accountV2("CreateAccount",admin,accountRetryInput),"CUSTOMER_ACCOUNTS_WRITE_UNCONFIRMED");
   assertUnconfirmed(await accountV2("UpdateAccount",admin,{
     ...accountUpdate,name:"Must roll back",
     precondition:{expectedVersion:2n,idempotencyKey:"account-audit-failed-update"}}),"CUSTOMER_ACCOUNTS_WRITE_UNCONFIRMED");
   assert.deepEqual((await accountSnapshot()),beforeAccountDenials,
     "Account audit INSERT failure rolls back state and receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_account_audit");}
 console.log(JSON.stringify({event:"crm.native-account-actor-audit",result:"PASS",
   human_create:true,service_update:true,cross_actor_replay:true,forged_headers_ignored:true,
   authorization_before_replay:true,tenant_ab:true,sql_failure_rollback:true}));
 console.log(JSON.stringify({event:"crm.native-g3-account-v2",result:"PASS",
   generated_binary:true,owner:"crm.customer-accounts",
   rpc_methods:["CreateAccount","UpdateAccount","GetAccount","ListAccounts"],
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   d1_cas:true,d1_receipts:true,d1_audit:true,published_active:true}));


 // Account filter must run BEFORE LIMIT: active records sort before inactive.
 const accountNextId="account-zzz-inactive-"+randomUUID();
 for(const [index,accountToCreate] of [
   "account-aaa-active-"+randomUUID(),"account-aab-active-"+randomUUID(),accountNextId
 ].entries()){
   const made=await accountV2("CreateAccount",admin,{
     accountId:accountToCreate,name:"Paging Account "+index,primaryPartyId:id,
     precondition:{idempotencyKey:"page-account-create-"+index}});
   assert.equal(made.status,200,describeProtobuf(made.body));
 }
 const inactiveLater=await accountV2("UpdateAccount",admin,{
   accountId:accountNextId,name:"Later Inactive Account",status:2,
   precondition:{expectedVersion:1n,idempotencyKey:"page-account-inactive"}});
 assert.equal(inactiveLater.status,200,describeProtobuf(inactiveLater.body));
 const inactiveAccounts=await collectPages(accountV2,"ListAccounts","accounts",{status:2});
 assert.deepEqual(inactiveAccounts.map(a=>a.accountId),[accountId,accountNextId]);
 assert.equal((await collectPages(accountV2,"ListAccounts","accounts",{status:1})).length,2);
 assert.equal((await collectPages(accountV2,"ListAccounts","accounts",{status:2})).length,2);
 const afterAccounts=await accountV2("ListAccounts",viewer,{
   page:{pageSize:1,pageToken:accountNextId},status:2});
 assert.equal(afterAccounts.status,200);
 assert.equal(afterAccounts.body.accounts.length,0);
 assert.ok(!afterAccounts.body.pageInfo?.nextPageToken);
 assert.equal((await accountV2("ListAccounts",tenantB,{page:{pageSize:1},status:2}))
   .body.accounts.length,0);
 console.log(JSON.stringify({event:"crm.native-g3-account-filtered-pagination",result:"PASS"}));

 // ContactPoints v2 uses the original Party-owned contact aggregate, NEVER
 // the removed pilot crm_contacts table. No browser-written verified=true.
 const contactV2=async(method,token,payload,extraHeaders={})=>{
   const response=await rpcFetch(contactV2Path(method),{method:"POST",
     headers:{...extraHeaders,"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryContactV2(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodeContactV2(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 const contactId="contact-native-"+randomUUID();
 const contactInput={contactPointId:contactId,partyId:id,kind:1,
   value:"  contact@example.com  ",preferred:true,
   validFrom:{seconds:1700000000n,nanos:123000000},
   validUntil:{seconds:1800000000n,nanos:456000000},
   precondition:{idempotencyKey:"candidate-v2-contact-create-001"}};
 assert.equal((await contactV2("CreateContactPoint",null,contactInput)).status,401);
 assert.equal((await contactV2("CreateContactPoint",viewer,contactInput)).status,403);
 assert.equal((await contactV2("CreateContactPoint",admin,{
   ...contactInput,kind:0,
   precondition:{idempotencyKey:"candidate-v2-contact-invalid-kind"}})).status,400);
 assert.equal((await contactV2("CreateContactPoint",admin,{
   ...contactInput,partyId:"missing-party",
   precondition:{idempotencyKey:"candidate-v2-contact-invalid-party"}})).status,400);
 for(const [index,value] of [
   "Ada..Tag@example.test","Ada<>@example.test","Ada@exa_mple.test",
   "Ada@example.test.","Ada@[127.0.0.1]","Ада@example.test"
 ].entries()){
   const denied=await contactV2("CreateContactPoint",admin,{
     ...contactInput,value,precondition:{idempotencyKey:"contact-email-invalid-"+index}});
   assert.equal(denied.status,400,describeProtobuf(denied.body));
 }
 for(const [index,value] of [
   "2025550102","+999123456789","+11111111111",
   "+1 202 555 0102 ext 42","+1 202 555 0102 junk"
 ].entries()){
   const denied=await contactV2("CreateContactPoint",admin,{
     ...contactInput,kind:2,value,precondition:{idempotencyKey:"contact-phone-invalid-"+index}});
   assert.equal(denied.status,400,describeProtobuf(denied.body));
 }
 const contactCreated=await contactV2("CreateContactPoint",admin,contactInput,{
    "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
    "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(contactCreated.status,200,describeProtobuf(contactCreated.body));
 assert.equal(contactCreated.body.contactPoint?.contactPointId,contactId);
 assert.equal(contactCreated.body.contactPoint?.partyId,id);
 assert.equal(contactCreated.body.contactPoint?.verified,false);
 assert.equal(contactCreated.body.contactPoint?.normalizedValue,"contact@example.com");
 assert.equal(contactCreated.body.contactPoint?.validFrom?.seconds,1700000000n);
 assert.equal(contactCreated.body.contactPoint?.validFrom?.nanos,123000000);
 assert.equal(contactCreated.body.contactPoint?.validUntil?.seconds,1800000000n);
 assert.equal(contactCreated.body.contactPoint?.validUntil?.nanos,456000000);
 assert.equal(contactCreated.body.receipt?.resourceVersion,1n);
 assert.equal(contactCreated.body.receipt?.replayed,false);
 assert.equal(contactCreated.body.receipt?.requestId,contactCreated.headers.get("x-request-id"));
 const contactReplay=await contactV2("CreateContactPoint",knownService,contactInput);
 assert.equal(contactReplay.status,200,describeProtobuf(contactReplay.body));
 assert.equal(contactReplay.body.receipt?.replayed,true);
 assert.equal(contactReplay.body.receipt.originalRequestId,contactCreated.headers.get("x-request-id"));
 assert.equal((await contactV2("CreateContactPoint",admin,{...contactInput,
   value:"different@example.com"})).status,409);
 assert.equal((await contactV2("GetContactPoint",tenantB,{contactPointId:contactId})).status,404);
 assert.equal((await contactV2("GetContactPoint",viewer,{contactPointId:contactId}))
   .body.contactPoint?.verified,false);
 const contactList=await contactV2("ListContactPoints",viewer,{page:{pageSize:1},partyId:id});
 assert.equal(contactList.status,200,describeProtobuf(contactList.body));
 assert.equal(contactList.body.contactPoints?.[0]?.contactPointId,contactId);
 assert.equal((await contactV2("ListContactPoints",viewer,{page:{pageSize:101}})).status,400);
 assert.equal((await contactV2("ListContactPoints",viewer,{partyId:"missing"}))
   .body.contactPoints.length,0);
 const contactUpdate={contactPointId:contactId,value:"next@example.org",status:1,
   preferred:false,
   validFrom:{operation:{case:"set",value:{seconds:1760000000n,nanos:987654321}}},
   validUntil:{operation:{case:"clear",value:true}},
   precondition:{
     expectedVersion:1n,idempotencyKey:"candidate-v2-contact-update-001"}};
 assert.equal((await contactV2("UpdateContactPoint",viewer,contactUpdate)).status,403);
 assert.equal((await contactV2("UpdateContactPoint",admin,{...contactUpdate,status:0,
   precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-v2-contact-invalid-status"}})).status,400);
 const contactUpdated=await contactV2("UpdateContactPoint",knownService,contactUpdate);
 assert.equal(contactUpdated.status,200,describeProtobuf(contactUpdated.body));
 assert.equal(contactUpdated.body.contactPoint?.version,2n);
 assert.equal(contactUpdated.body.contactPoint?.verified,false);
 assert.equal(contactUpdated.body.contactPoint?.normalizedValue,"next@example.org");
 assert.equal(contactUpdated.body.contactPoint?.validFrom?.seconds,1760000000n);
 assert.equal(contactUpdated.body.contactPoint?.validFrom?.nanos,987654321);
 assert.equal(contactUpdated.body.contactPoint?.validUntil,undefined,
   "explicit clear timestamp patch must remove validUntil");
 assert.equal(contactUpdated.body.receipt?.resourceVersion,2n);
 const contactReupdate=await contactV2("UpdateContactPoint",admin,contactUpdate);
 assert.equal(contactReupdate.status,200,describeProtobuf(contactReupdate.body));
 assert.equal(contactReupdate.body.receipt?.replayed,true);
 assert.equal(contactReupdate.body.receipt.originalRequestId,contactUpdated.headers.get("x-request-id"));
 assert.equal((await contactV2("UpdateContactPoint",admin,{...contactUpdate,
   precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-v2-contact-stale"}})).status,409);
 assert.equal((await sql("SELECT version FROM crm_contact_point_records WHERE tenant='tenant-a' "+
   "AND id='"+contactId+"'"))[0].version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_contact_point_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v2-contact-create-001','candidate-v2-contact-update-001')"))[0].n,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"' AND action LIKE 'contact_points.%'"))[0].n,2);
 assert.deepEqual((await sql("SELECT action,tenant_id,subject_id,request_id,actor FROM crm_audit WHERE subject_id='"+contactId+"' ORDER BY rowid")),[
   {action:"contact_points.create",tenant_id:"tenant-a",subject_id:contactId,
     request_id:contactCreated.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"contact_points.update",tenant_id:"tenant-a",subject_id:contactId,
     request_id:contactUpdated.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
 ],"Cross-actor contact replay must preserve signed author/request ID");
 const contactSnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_contact_point_records) AS states,"+
   "(SELECT COUNT(*) FROM crm_contact_point_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 const beforeContactDenials=(await contactSnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await contactV2("CreateContactPoint",token,contactInput)).status>=400);
   assert.ok((await contactV2("UpdateContactPoint",token,contactUpdate)).status>=400);
 }
 assert.equal((await contactV2("UpdateContactPoint",tenantB,contactUpdate)).status,404);
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await contactV2("CreateContactPoint",knownService,contactInput)).status,401,
   "Inactive service cannot replay ContactPoint create");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await contactV2("UpdateContactPoint",knownService,contactUpdate)).status,403,
   "Current permission is required before ContactPoint replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.deepEqual((await contactSnapshot()),beforeContactDenials,
   "Denied, stale and validation errors cannot change contact state, receipts or audit");
 const contactRetryInput={contactPointId:"contact-audit-rollback",partyId:id,kind:1,value:"rollback@example.org",preferred:false,
   precondition:{idempotencyKey:"contact-audit-failed-create"}};
 (await sql("CREATE TRIGGER fixture_fail_contact_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('contact-audit-rollback','"+contactId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture contact audit failure'); END;"));
 try{
   assertUnconfirmed(await contactV2("CreateContactPoint",admin,contactRetryInput),"CONTACT_POINTS_WRITE_UNCONFIRMED");
   assertUnconfirmed(await contactV2("UpdateContactPoint",admin,{
     ...contactUpdate,value:"rollback@example.org",
     precondition:{expectedVersion:2n,idempotencyKey:"contact-audit-failed-update"}}),"CONTACT_POINTS_WRITE_UNCONFIRMED");
   assert.deepEqual((await contactSnapshot()),beforeContactDenials,
     "Contact audit SQL failure must roll back both state and receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_contact_audit");}
 console.log(JSON.stringify({event:"crm.native-contact-actor-audit",result:"PASS",
   human_create:true,service_update:true,cross_actor_replay:true,forged_headers_ignored:true,
   authorization_before_replay:true,tenant_ab:true,sql_failure_rollback:true}));
 // Filtered ContactPoint list uses the EXISTING D1 party_id column.
 // Nonmatches sort before our two matches, exposing the old LIMIT bug.
 const contactLastId="contact-zzz-ours-"+randomUUID();
 for(const [index,pointToCreate] of [
   ["contact-aaa-other-"+randomUUID(),pagingPersonId],
   ["contact-aab-other-"+randomUUID(),pagingPersonId],
   [contactLastId,id]
 ].entries()){
   const made=await contactV2("CreateContactPoint",admin,{
     contactPointId:pointToCreate[0],partyId:pointToCreate[1],kind:index===0?2:1,
     value:index===0?"+1 202 555 0102":"page-"+index+"@example.org",preferred:false,
     precondition:{idempotencyKey:"page-contact-create-"+index}});
   assert.equal(made.status,200,describeProtobuf(made.body));
   if(index===0){
     assert.equal(made.body.contactPoint?.normalizedValue,"+12025550102");
     assert.equal(made.body.contactPoint?.verified,false);
     const read=await contactV2("GetContactPoint",viewer,{contactPointId:pointToCreate[0]});
     assert.equal(read.status,200,describeProtobuf(read.body));
     assert.equal(read.body.contactPoint?.normalizedValue,"+12025550102");
   }
 }
 const filteredContacts=await collectPages(contactV2,"ListContactPoints",
   "contactPoints",{partyId:id});
 assert.deepEqual(filteredContacts.map(p=>p.contactPointId),[contactId,contactLastId]);
 assert.equal((await collectPages(contactV2,"ListContactPoints",
   "contactPoints",{partyId:"missing-party"})).length,0);
 assert.equal((await contactV2("ListContactPoints",tenantB,{
   page:{pageSize:1},partyId:id})).body.contactPoints.length,0);
 const beyondContact=await contactV2("ListContactPoints",viewer,{
   partyId:id,page:{pageSize:1,pageToken:contactLastId}});
 assert.equal(beyondContact.status,200);
 assert.equal(beyondContact.body.contactPoints.length,0);
 assert.ok(!beyondContact.body.pageInfo?.nextPageToken);

 // Omitted timestamp patches mean KEEP. Replay the SAME request after a
 // later successful update which changes both current timestamp fields.
 const keepRequest={contactPointId:contactId,value:"keep-timestamps@example.org",
   status:1,preferred:false,
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-v2-contact-keep-001"}};
 const keep=await contactV2("UpdateContactPoint",admin,keepRequest);
 assert.equal(keep.status,200,describeProtobuf(keep.body));
 assert.equal(keep.body.contactPoint?.version,3n);
 assert.equal(keep.body.contactPoint?.validFrom?.seconds,1760000000n);
 assert.equal(keep.body.contactPoint?.validUntil,undefined);
 const later=await contactV2("UpdateContactPoint",admin,{
   ...keepRequest,value:"later-timestamps@example.org",
   validFrom:{operation:{case:"set",value:{seconds:1770000000n,nanos:3}}},
   validUntil:{operation:{case:"set",value:{seconds:1870000000n,nanos:9}}},
   precondition:{expectedVersion:3n,idempotencyKey:"candidate-v2-contact-later-001"}});
 assert.equal(later.status,200,describeProtobuf(later.body));
 assert.equal(later.body.contactPoint?.version,4n);
 const beforeReplay=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"'"))[0].n;
 const restoredReceipt=await contactV2("UpdateContactPoint",admin,keepRequest);
 assert.equal(restoredReceipt.status,200,describeProtobuf(restoredReceipt.body));
 assert.equal(restoredReceipt.body.receipt?.replayed,true);
 assert.equal(restoredReceipt.body.receipt?.resourceVersion,3n);
 assert.equal(restoredReceipt.body.contactPoint?.version,3n);
 assert.equal(restoredReceipt.body.contactPoint?.validFrom?.seconds,1760000000n);
 assert.equal(restoredReceipt.body.contactPoint?.validUntil,undefined);
 assert.equal((await contactV2("UpdateContactPoint",admin,{
   ...keepRequest,value:"not-the-same-request@example.org"})).status,409);
 assert.equal((await contactV2("UpdateContactPoint",admin,{
   ...keepRequest,precondition:{expectedVersion:2n,
     idempotencyKey:"candidate-v2-contact-stale-after-later"}})).status,409);
 const currentPoint=await contactV2("GetContactPoint",viewer,{contactPointId:contactId});
 assert.equal(currentPoint.body.contactPoint?.version,4n);
 assert.equal(currentPoint.body.contactPoint?.validFrom?.seconds,1770000000n);
 assert.equal(currentPoint.body.contactPoint?.validUntil?.seconds,1870000000n);
 assert.equal((await sql("SELECT version FROM crm_contact_point_records WHERE tenant='tenant-a' "+
   "AND id='"+contactId+"'"))[0].version,4);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"'"))[0].n,beforeReplay);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_contact_point_receipts "+
   "WHERE tenant='tenant-a' AND key IN ("+
   "'candidate-v2-contact-keep-001','candidate-v2-contact-later-001')"))[0].n,2);
 console.log(JSON.stringify({event:"crm.native-g3-contact-pagination-interleaved-replay",
   result:"PASS",same_original_receipt:true,no_extra_write:true,tenant_isolated:true}));

 // Product-first M1-04: narrow by a specific channel AND lifecycle status,
 // not post-LIMIT filtering that loses items on later pages.
 const contactLastInactive=await contactV2("UpdateContactPoint",admin,{
   contactPointId:contactLastId,value:"page-2@example.org",preferred:false,status:2,
   precondition:{expectedVersion:1n,idempotencyKey:"contact-filter-inactive-last"}});
 assert.equal(contactLastInactive.status,200,describeProtobuf(contactLastInactive.body));
 const contactFiltersAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 assert.deepEqual((await collectPages(contactV2,"ListContactPoints","contactPoints",{
   partyId:id,kind:1,status:1
 })).map(p=>p.contactPointId),[contactId]);
 assert.deepEqual((await collectPages(contactV2,"ListContactPoints","contactPoints",{
   partyId:id,kind:1,status:2
 })).map(p=>p.contactPointId),[contactLastId]);
 assert.deepEqual((await collectPages(contactV2,"ListContactPoints","contactPoints",{
   partyId:id,kind:1
 })).map(p=>p.contactPointId),[contactId,contactLastId]);
 assert.equal((await collectPages(contactV2,"ListContactPoints","contactPoints",{
   partyId:id,kind:2,status:1
 })).length,0);
 const otherPhone=await collectPages(contactV2,"ListContactPoints","contactPoints",{
   partyId:pagingPersonId,kind:2,status:1
 });
 assert.equal(otherPhone.length,1);
 assert.equal(otherPhone[0].normalizedValue,"+12025550102");
 for(const filter of [{kind:0},{kind:99},{status:0},{status:99}]){
   const denied=await contactV2("ListContactPoints",viewer,filter);
   assert.equal(denied.status,400,describeProtobuf(denied.body));
 }
 assert.equal((await contactV2("ListContactPoints",tenantB,{partyId:id,kind:1,status:1}))
   .body.contactPoints.length,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   contactFiltersAudit,"Filtering ContactPoints must not write audit");
 console.log(JSON.stringify({event:"crm.native-m1-contact-kind-status",result:"PASS",
   exact_kind:true,exact_status:true,composed_before_cursor:true,tenant:true,
   invalid_enum_rejected:true,read_only:true}));
 console.log(JSON.stringify({event:"crm.native-g3-contact-points-v2",result:"PASS",
   generated_binary:true,owner:"crm.contact-points",
   rpc_methods:["CreateContactPoint","UpdateContactPoint","GetContactPoint","ListContactPoints"],
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   no_browser_verification:true,d1_cas:true,d1_receipts:true,d1_audit:true,
   published_active:true}));

 // G3 Relationships v2: original Rust domain owner and one D1, binary
 // Protobuf descriptor, signed JWT + tenant A/B + viewer rights.
 const relationV2=async(method,token,payload)=>{
   const response=await rpcFetch(relationshipV2Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryRelationshipV2(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodeRelationshipV2(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 const rOther="relationship-aaa-other-"+randomUUID();
 const rMid="relationship-mid-ours-"+randomUUID();
 const rLast="relationship-zzz-ours-"+randomUUID();
 const relType={code:"employment",directionality:1,fromRole:"employer",toRole:"employee"};
 const relationInput=(relationshipId,fromPartyId,toPartyId,seq)=>({
   relationshipId,fromPartyId,toPartyId,type:relType,
   validFrom:{seconds:1700000000n,nanos:1000},
   validUntil:{seconds:1800000000n,nanos:2000},
   precondition:{idempotencyKey:"candidate-rel-create-"+seq}
 });
 const otherRelation=relationInput(rOther,"binary-rpc-proof-01",pagingPersonId,"other");
 const midRelation=relationInput(rMid,id,pagingPersonId,"mid");
 const lastRelation=relationInput(rLast,pagingPersonId,id,"last");
 assert.equal((await relationV2("CreatePartyRelationship",null,midRelation)).status,401);
 assert.equal((await relationV2("CreatePartyRelationship",viewer,midRelation)).status,403);
 assert.equal((await relationV2("CreatePartyRelationship",admin,{
   ...midRelation,fromPartyId:id,toPartyId:id,
   precondition:{idempotencyKey:"candidate-rel-self"}})).status,400);
 assert.equal((await relationV2("CreatePartyRelationship",admin,{
   ...midRelation,fromPartyId:"missing-party",
   precondition:{idempotencyKey:"candidate-rel-missing"}})).status,400);
 assert.equal((await relationV2("CreatePartyRelationship",admin,{
   ...midRelation,type:{...relType,directionality:0},
   precondition:{idempotencyKey:"candidate-rel-invalid-type"}})).status,400);
 const otherCreated=await relationV2("CreatePartyRelationship",admin,otherRelation);
 assert.equal(otherCreated.status,200,describeProtobuf(otherCreated.body));
 const midCreated=await relationV2("CreatePartyRelationship",admin,midRelation);
 assert.equal(midCreated.status,200,describeProtobuf(midCreated.body));
 assert.equal(midCreated.body.relationship?.relationshipId,rMid);
 assert.equal(midCreated.body.relationship?.type?.code,"employment");
 assert.equal(midCreated.body.relationship?.validFrom?.nanos,1000);
 assert.equal(midCreated.body.receipt?.resourceVersion,1n);
 assert.equal(midCreated.body.receipt?.requestId,midCreated.headers.get("x-request-id"));
 const lastCreated=await relationV2("CreatePartyRelationship",admin,lastRelation);
 assert.equal(lastCreated.status,200,describeProtobuf(lastCreated.body));
 const midReplay=await relationV2("CreatePartyRelationship",admin,midRelation);
 assert.equal(midReplay.status,200,describeProtobuf(midReplay.body));
 assert.equal(midReplay.body.receipt?.replayed,true);
 assert.equal((await relationV2("CreatePartyRelationship",admin,{
   ...midRelation,toPartyId:"binary-rpc-proof-01"})).status,409);
 assert.equal((await relationV2("GetPartyRelationship",tenantB,{relationshipId:rMid})).status,404);
 assert.equal((await relationV2("GetPartyRelationship",viewer,{relationshipId:rMid}))
   .body.relationship?.fromPartyId,id);
 const relList=await collectPages(relationV2,"ListPartyRelationships","relationships",{partyId:id});
 assert.deepEqual(relList.map(r=>r.relationshipId),[rMid,rLast]);
 assert.equal((await collectPages(relationV2,"ListPartyRelationships","relationships",
   {partyId:"missing-party"})).length,0);
 assert.equal((await relationV2("ListPartyRelationships",tenantB,{
   partyId:id,page:{pageSize:1}})).body.relationships.length,0);
 assert.equal((await relationV2("ListPartyRelationships",viewer,{
   partyId:id,page:{pageSize:101}})).status,400);
 const afterRel=await relationV2("ListPartyRelationships",viewer,{
   partyId:id,page:{pageSize:1,pageToken:rLast}});
 assert.equal(afterRel.status,200,describeProtobuf(afterRel.body));
 assert.equal(afterRel.body.relationships.length,0);
 assert.ok(!afterRel.body.pageInfo?.nextPageToken);
 const relUpdate={relationshipId:rMid,status:2,
   validFrom:{operation:{case:"set",value:{seconds:1710000000n,nanos:123}}},
   validUntil:{operation:{case:"clear",value:true}},
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-rel-update-01"}};
 assert.equal((await relationV2("UpdatePartyRelationship",viewer,relUpdate)).status,403);
 assert.equal((await relationV2("UpdatePartyRelationship",admin,{
   ...relUpdate,status:0,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-rel-invalid-status"}})).status,400);
 const updatedRel=await relationV2("UpdatePartyRelationship",knownService,relUpdate);
 assert.equal(updatedRel.status,200,describeProtobuf(updatedRel.body));
 assert.equal(updatedRel.body.relationship?.version,2n);
 assert.equal(updatedRel.body.relationship?.status,2);
 assert.equal(updatedRel.body.relationship?.validFrom?.seconds,1710000000n);
 assert.equal(updatedRel.body.relationship?.validUntil,undefined);
 // After the update this relationship is INACTIVE; the other one is ACTIVE.
 const relationFilterAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 assert.deepEqual((await collectPages(relationV2,"ListPartyRelationships","relationships",{
   partyId:id,status:1
 })).map(r=>r.relationshipId),[rLast]);
 assert.deepEqual((await collectPages(relationV2,"ListPartyRelationships","relationships",{
   partyId:id,status:2
 })).map(r=>r.relationshipId),[rMid]);
 assert.equal((await collectPages(relationV2,"ListPartyRelationships","relationships",{
   partyId:"missing-party",status:2
 })).length,0);
 for(const invalidStatus of [0,99]){
   const denied=await relationV2("ListPartyRelationships",viewer,{status:invalidStatus});
   assert.equal(denied.status,400,describeProtobuf(denied.body));
 }
 assert.equal((await relationV2("ListPartyRelationships",tenantB,{partyId:id,status:2}))
   .body.relationships.length,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   relationFilterAudit,"Relationship status reads never write audit");
 console.log(JSON.stringify({event:"crm.native-m1-relationship-status",result:"PASS",
   active_inactive_exact:true,before_cursor:true,tenant:true,
   invalid_enum_rejected:true,read_only:true}));
 const relFirstReplay=await relationV2("UpdatePartyRelationship",admin,relUpdate);
 assert.equal(relFirstReplay.status,200);
 assert.equal(relFirstReplay.body.receipt?.replayed,true);
 const relKeep={relationshipId:rMid,status:1,
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-rel-keep-02"}};
 const relKeepApplied=await relationV2("UpdatePartyRelationship",admin,relKeep);
 assert.equal(relKeepApplied.status,200,describeProtobuf(relKeepApplied.body));
 assert.equal(relKeepApplied.body.relationship?.version,3n);
 assert.equal(relKeepApplied.body.relationship?.validFrom?.seconds,1710000000n);
 assert.equal(relKeepApplied.body.relationship?.validUntil,undefined);
 const relLater=await relationV2("UpdatePartyRelationship",admin,{
   ...relKeep,status:2,validFrom:{operation:{case:"set",
     value:{seconds:1720000000n,nanos:987}}},
   validUntil:{operation:{case:"set",value:{seconds:1820000000n,nanos:654}}},
   precondition:{expectedVersion:3n,idempotencyKey:"candidate-rel-later-03"}});
 assert.equal(relLater.status,200,describeProtobuf(relLater.body));
 assert.equal(relLater.body.relationship?.version,4n);
 const relAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"+
   " AND subject_id='"+rMid+"'"))[0].n;
 const relKeepReplay=await relationV2("UpdatePartyRelationship",admin,relKeep);
 assert.equal(relKeepReplay.status,200,describeProtobuf(relKeepReplay.body));
 assert.equal(relKeepReplay.body.receipt?.replayed,true);
 assert.equal(relKeepReplay.body.receipt?.resourceVersion,3n);
 assert.equal(relKeepReplay.body.relationship?.validFrom?.seconds,1710000000n);
 assert.equal(relKeepReplay.body.relationship?.validUntil,undefined);
 assert.equal((await relationV2("UpdatePartyRelationship",admin,{
   ...relKeep,status:2})).status,409,"same key with different intent rejected");
 assert.equal((await relationV2("UpdatePartyRelationship",admin,{
   ...relKeep,precondition:{expectedVersion:2n,
     idempotencyKey:"candidate-rel-stale-04"}})).status,409);
 const currentRel=await relationV2("GetPartyRelationship",viewer,{relationshipId:rMid});
 assert.equal(currentRel.body.relationship?.version,4n);
 assert.equal(currentRel.body.relationship?.validFrom?.seconds,1720000000n);
 assert.equal(currentRel.body.relationship?.validUntil?.seconds,1820000000n);
 assert.equal((await sql("SELECT version FROM crm_party_relationship_records WHERE tenant='tenant-a'"+
   " AND id='"+rMid+"'"))[0].version,4);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"+
   " AND subject_id='"+rMid+"'"))[0].n,relAuditBefore);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_relationship_receipts WHERE tenant='tenant-a'"+
   " AND key IN ('candidate-rel-create-mid','candidate-rel-update-01',"+
   " 'candidate-rel-keep-02','candidate-rel-later-03')"))[0].n,4);
 
  // R2a #408: real signed human/service authors, replay and atomic SQL rollback.
  assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+rMid+"' ORDER BY rowid")),[
    {action:"party_relationships.create",actor:"native-a@example.test",
      request_id:midCreated.headers.get("x-request-id")},
    {action:"party_relationships.update",actor:"service:ci-fixture.access",
      request_id:updatedRel.headers.get("x-request-id")},
    {action:"party_relationships.update",actor:"native-a@example.test",
      request_id:relKeepApplied.headers.get("x-request-id")},
    {action:"party_relationships.update",actor:"native-a@example.test",
      request_id:relLater.headers.get("x-request-id")}
  ],"Cross-actor replay never replaces original Relationship audit author");
  const relSnapshot=async()=>(await sql("SELECT "+
    "(SELECT state FROM crm_party_relationship_records WHERE tenant='tenant-a' AND id='"+rMid+"') AS state,"+
    "(SELECT COUNT(*) FROM crm_party_relationship_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"));
  const beforeRelFault=(await relSnapshot());
  (await sql("CREATE TRIGGER fixture_fail_rel_actor BEFORE INSERT ON crm_audit "+
    "WHEN NEW.subject_id='"+rMid+"' BEGIN SELECT RAISE(ABORT,'fixture relation audit'); END;"));
  try{
    assertUnconfirmed(await relationV2("UpdatePartyRelationship",admin,{relationshipId:rMid,status:1,
      precondition:{expectedVersion:4n,idempotencyKey:"candidate-rel-actor-failed"}}),"PARTY_RELATIONSHIPS_WRITE_UNCONFIRMED");
    assert.deepEqual((await relSnapshot()),beforeRelFault);
  }finally{await sql("DROP TRIGGER fixture_fail_rel_actor");}
  console.log(JSON.stringify({event:"crm.native-r2a-relationship-actor-audit",result:"PASS",
    signed_cross_actor:true,failed_audit_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-relationship-v2",
   result:"PASS",rpc_methods:["CreatePartyRelationship","UpdatePartyRelationship",
     "GetPartyRelationship","ListPartyRelationships"],
   generated_binary:true,original_owner:"crm.party-relationships",
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,temporal_patch:true,replay_after_interleaving:true,
   no_extra_audit:true,published_active:true}));

 // Activities v2: real signed workerd binary Protobuf against the ORIGINAL
 // crm-activities Task state, native one-D1 account FK, receipt and audit.
 const memberV2=async(method,token,payload)=>{
   const response=await rpcFetch(memberV2Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryMemberV2(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodeMemberV2(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 const taskV2=async(method,token,payload)=>{
   const response=await rpcFetch(taskV2Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryTaskV2(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodeTaskV2(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 const taskOther="task-aaa-other-"+randomUUID();
 const taskMid="task-mid-ours-"+randomUUID();
 const taskLast="task-zzz-ours-"+randomUUID();
 const taskInput=(taskId,accountId,sequence)=>({
   taskId,accountId,subject:"Check original Account "+sequence,
   description:"Signed actor task "+sequence,priority:2,
   dueAt:{seconds:1860000000n,nanos:987654321},
   reminderAt:{seconds:1850000000n,nanos:123456789},
   precondition:{idempotencyKey:"candidate-task-create-"+sequence}
 });
 const taskOtherInput={...taskInput(taskOther,accountNextId,"other"),description:undefined,priority:4};
 const taskMidInput=taskInput(taskMid,accountId,"mid");
 const taskLastInput=taskInput(taskLast,accountId,"last");
 assert.equal((await taskV2("CreateTask",null,taskMidInput)).status,401);
 assert.equal((await taskV2("CreateTask",viewer,taskMidInput)).status,403);
 assert.equal((await taskV2("CreateTask",admin,{...taskMidInput,
   priority:0,precondition:{idempotencyKey:"candidate-task-invalid-priority"}})).status,400);
 assert.equal((await taskV2("CreateTask",admin,{...taskMidInput,
   accountId:"missing-account",
   precondition:{idempotencyKey:"candidate-task-missing-account"}})).status,400);
 assert.equal((await taskV2("CreateTask",tenantB,{...taskMidInput,
   precondition:{idempotencyKey:"candidate-task-foreign-account"}})).status,400);
 assert.equal((await taskV2("CreateTask",admin,{...taskMidInput,
   reminderAt:{seconds:1700000000n,nanos:0},
   precondition:{idempotencyKey:"candidate-task-past-reminder"}})).status,400);
 const taskCreated=await taskV2("CreateTask",admin,taskMidInput);
 assert.equal(taskCreated.status,200,describeProtobuf(taskCreated.body));
 assert.equal(taskCreated.body.task?.taskId,taskMid);
 assert.equal(taskCreated.body.task?.accountId,accountId);
 assert.equal(taskCreated.body.task?.status,1);
 assert.equal(taskCreated.body.task?.priority,2);
 assert.equal(taskCreated.body.task?.description,"Signed actor task mid");
 assert.equal(taskCreated.body.task?.dueAt?.nanos,987654321);
 assert.equal(taskCreated.body.task?.reminderAt?.nanos,123456789);
 assert.equal(taskCreated.body.receipt?.resourceVersion,1n);
 assert.equal(taskCreated.body.receipt?.requestId,taskCreated.headers.get("x-request-id"));
 const taskReplay=await taskV2("CreateTask",admin,taskMidInput);
 assert.equal(taskReplay.status,200);
 assert.equal(taskReplay.body.receipt?.replayed,true);
 assert.equal((await taskV2("CreateTask",admin,{
   ...taskMidInput,subject:"Different same key"})).status,409);
 assert.equal((await taskV2("GetTask",tenantB,{taskId:taskMid})).status,404);
 assert.equal((await taskV2("GetTask",viewer,{taskId:taskMid}))
   .body.task?.accountId,accountId);
 const taskOtherCreated=await taskV2("CreateTask",admin,taskOtherInput);
 assert.equal(taskOtherCreated.status,200,describeProtobuf(taskOtherCreated.body));
 assert.equal(taskOtherCreated.body.task?.description,"",
   "optional CreateTask.description absent is canonical empty Task.description");
 assert.equal((await taskV2("CreateTask",admin,taskLastInput)).status,200);
 const taskAuditBeforeFilters=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 // Original Task subject search: signed, before paging, composed with existing filters.
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{search:"Check original Account mid"})).map(t=>t.taskId),[taskMid]);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{search:"Check original Account",page:{pageSize:1}})).map(t=>t.taskId),[taskOther,taskMid,taskLast].sort());
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{search:"Check original Account",priority:2,accountId})).map(t=>t.taskId),[taskMid,taskLast]);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{search:"task-mid-ours"})).map(t=>t.taskId),[taskMid]);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",{search:"Signed actor task mid"})).length,0,"description must not broaden Task subject search");
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",{search:"check original account"})).length,0,"search follows case-sensitive UI contract");
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",{search:"' OR 1=1 --"})).length,0,"search never interpolates SQL");
 assert.equal((await taskV2("ListTasks",tenantB,{search:"Check original Account"})).body.tasks.length,0);
 const namedTasks=(await taskV2("ListTasks",viewer,{search:"Check original Account mid"})).body;
 assert.equal(namedTasks.tasks[0].taskId,taskMid);
 const namedAccount=(await sql("SELECT search_name FROM crm_account_records WHERE tenant=? AND id=?","tenant-a",accountId))[0];
 assert.deepEqual(Object.entries(namedTasks.accountNames),[[accountId,namedAccount.search_name]]);
 assert.equal((await taskV2("ListTasks",viewer,{search:"x".repeat(201)})).status,400);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{priority:2})).map(t=>t.taskId),[taskMid,taskLast]);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{priority:4})).map(t=>t.taskId),[taskOther]);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",{priority:4,accountId})).length,0);
 assert.equal((await taskV2("ListTasks",tenantB,{priority:4})).body.tasks.length,0);
 assert.equal((await taskV2("ListTasks",viewer,{priority:4})).body.tasks[0].taskId,taskOther);
 for(const priority of [0,99]){
   const invalid=await taskV2("ListTasks",viewer,{priority});
   assert.equal(invalid.status,400);assert.equal(invalid.body.code,"invalid_argument");
   assert.equal(invalid.body.message,"invalid TaskPriority filter");
   assert.ok(invalid.headers.get("x-request-id"));
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,taskAuditBeforeFilters,
   "Priority filters and invalid read requests must not create audit writes");
 const taskFilter=await collectPages(taskV2,"ListTasks","tasks",
   {accountId,status:1});
 assert.deepEqual(taskFilter.map(t=>t.taskId),[taskMid,taskLast]);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",
   {accountId:"missing-account"})).length,0);
 assert.equal((await taskV2("ListTasks",tenantB,{
   accountId,page:{pageSize:1}})).body.tasks.length,0);
 assert.equal((await taskV2("ListTasks",viewer,{
   page:{pageSize:101}})).status,400);
 // UpdateTask: original Rust invariants and atomic state/receipt/audit.
 const updateKey="candidate-task-update-other";
 const baseUpdate={taskId:taskOther,precondition:{expectedVersion:1n,idempotencyKey:updateKey}};
 const taskRow=async()=>(await sql("SELECT state,version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"'"))[0];
 const taskAudits=async()=>(await sql("SELECT action,request_id FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+taskOther+"' ORDER BY rowid"));
 const beforeTask=(await taskRow()),beforeTaskAudit=(await taskAudits());
 const patchSubject=value=>({operation:{case:"set",value}});
 assert.equal((await taskV2("UpdateTask",null,{...baseUpdate,subject:patchSubject("No auth")})).status,401);
 assert.equal((await taskV2("UpdateTask",viewer,{...baseUpdate,subject:patchSubject("No permission")})).status,403);
 assert.equal((await taskV2("UpdateTask",tenantB,{...baseUpdate,subject:patchSubject("Wrong tenant")})).status,404);
 assert.equal((await taskV2("UpdateTask",admin,baseUpdate)).status,400);
 assert.equal((await taskV2("UpdateTask",admin,{...baseUpdate,subject:{operation:{case:"clear",value:true}}})).status,400);
 assert.equal((await taskV2("UpdateTask",admin,{...baseUpdate,priority:99})).status,400);
 assert.equal((await taskV2("UpdateTask",admin,{...baseUpdate,priority:0})).status,400);
 const rejected=await taskV2("UpdateTask",admin,{...baseUpdate,
   subject:patchSubject("Must not persist"),description:{operation:{case:"clear",value:true}},
   priority:1,
   dueAt:{operation:{case:"set",value:{seconds:1840000000n,nanos:0}}}});
 assert.equal(rejected.status,400,describeProtobuf(rejected.body));
 assert.deepEqual((await taskRow()),beforeTask,"late rejection must not persist an earlier field change");
 assert.deepEqual((await taskAudits()),beforeTaskAudit,"rejection must not create audit");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a' AND key='"+updateKey+"'"))[0].n,0);
 const updateTask={...baseUpdate,subject:patchSubject("Edited original Task"),
   description:{operation:{case:"set",value:"Edited through Connect"}},
   priority:3,dueAt:{operation:{case:"clear",value:true}}};
 const updatedTask=await taskV2("UpdateTask",knownService,updateTask);
 assert.equal(updatedTask.status,200,describeProtobuf(updatedTask.body));
 assert.equal(updatedTask.body.task.subject,"Edited original Task");
 assert.equal(updatedTask.body.task.description,"Edited through Connect");
 assert.equal(updatedTask.body.task.priority,3);
 assert.equal(updatedTask.body.task.dueAt,undefined);
 assert.equal(updatedTask.body.task.version,2n);
 assert.equal(updatedTask.body.receipt.resourceVersion,2n);
 assert.equal(updatedTask.body.receipt.requestId,updatedTask.headers.get("x-request-id"));
 assert.deepEqual((await taskAudits()).map(x=>x.action),["activity_tasks.create","activity_tasks.update"]);
 assert.equal((await taskAudits())[1].request_id,updatedTask.headers.get("x-request-id"));
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{priority:3,accountId:accountNextId})).map(x=>x.taskId),[taskOther]);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",{priority:4,accountId:accountNextId})).length,0);
 const replayTask=await taskV2("UpdateTask",admin,updateTask);
 assert.equal(replayTask.status,200);
 assert.equal(replayTask.body.receipt.replayed,true);
 assert.equal((await taskAudits()).length,2,"replay cannot duplicate audit");
 assert.equal((await taskV2("UpdateTask",admin,{...updateTask,subject:patchSubject("Same key, different payload")})).status,409);
 assert.equal((await taskV2("UpdateTask",admin,{...updateTask,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-update-stale"}})).status,409);
 assert.equal((await taskAudits()).length,2,"stale version must not create audit");
 const secondTaskEdit=await taskV2("UpdateTask",admin,{taskId:taskOther,
   description:{operation:{case:"clear",value:true}},priority:2,
   dueAt:{operation:{case:"set",value:{seconds:1870000000n,nanos:987654321}}},
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-task-update-second"}});
 assert.equal(secondTaskEdit.status,200,describeProtobuf(secondTaskEdit.body));
 assert.equal(secondTaskEdit.body.task.description,"");
 assert.equal(secondTaskEdit.body.task.dueAt.nanos,987654321);
 assert.equal(secondTaskEdit.body.task.version,3n);
 assert.deepEqual((await taskAudits()).map(x=>x.action),["activity_tasks.create","activity_tasks.update","activity_tasks.update"]);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a' AND key IN ('candidate-task-create-other','candidate-task-update-other','candidate-task-update-second')"))[0].n,3);
 const taskComplete={taskId:taskMid,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-complete-mid"}};
 assert.equal((await taskV2("CompleteTask",viewer,taskComplete)).status,403);
 assert.equal((await taskV2("CompleteTask",admin,{
   ...taskComplete,precondition:{expectedVersion:0n,
     idempotencyKey:"candidate-task-invalid-version"}})).status,400);
 const completed=await taskV2("CompleteTask",knownService,taskComplete);
 assert.equal(completed.status,200,describeProtobuf(completed.body));
 assert.equal(completed.body.changed,true);
 assert.equal(completed.body.task?.status,2);
 assert.equal(completed.body.task?.version,2n);
 assert.ok(completed.body.task?.completedAt?.seconds>0n);
 assert.equal(completed.body.task?.reminderAt,undefined);
 assert.equal(completed.body.receipt?.resourceVersion,2n);
 const completedReplay=await taskV2("CompleteTask",admin,taskComplete);
 assert.equal(completedReplay.status,200,describeProtobuf(completedReplay.body));
 assert.equal(completedReplay.body.receipt?.replayed,true);
 assert.equal(completedReplay.body.task?.version,2n);
 const beforeNoop=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+taskMid+"'"))[0].n;
 const unchangedTask=await taskV2("CompleteTask",admin,{
   taskId:taskMid,
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-task-unchanged"}});
 assert.equal(unchangedTask.status,200,describeProtobuf(unchangedTask.body));
 assert.equal(unchangedTask.body.changed,false);
 assert.equal(unchangedTask.body.receipt,undefined,
   "unchanged completion must not claim persisted D1 receipt");
 assert.equal((await taskV2("CompleteTask",admin,{
   ...taskComplete,precondition:{expectedVersion:1n,
     idempotencyKey:"candidate-task-stale"}})).status,409);
 const lastComplete=await taskV2("CompleteTask",admin,{
   taskId:taskLast,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-complete-last"}});
 assert.equal(lastComplete.status,200,describeProtobuf(lastComplete.body));
 assert.equal(lastComplete.body.changed,true);
 const completedTasks=await collectPages(taskV2,"ListTasks","tasks",
   {accountId,status:2});
 assert.deepEqual(completedTasks.map(t=>t.taskId),[taskMid,taskLast]);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{accountId,status:2,priority:2})).map(t=>t.taskId),[taskMid,taskLast]);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",
   {accountId,status:1})).length,0);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",
   {accountId:accountNextId,status:1})).length,1);
 assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='tenant-a' "+
   "AND id='"+taskMid+"'"))[0].version,2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+taskMid+"'"))[0].n,beforeNoop);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts "+
   "WHERE tenant='tenant-a' AND key IN ('candidate-task-create-mid',"+
   "'candidate-task-complete-mid')"))[0].n,2);
 
  assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+taskOther+"' ORDER BY rowid")),[
    {action:"activity_tasks.create",actor:"native-a@example.test",
      request_id:taskOtherCreated.headers.get("x-request-id")},
    {action:"activity_tasks.update",actor:"service:ci-fixture.access",
      request_id:updatedTask.headers.get("x-request-id")},
    {action:"activity_tasks.update",actor:"native-a@example.test",
      request_id:secondTaskEdit.headers.get("x-request-id")}
  ],"Task update replay keeps first signed author and trace");
  assert.deepEqual((await sql("SELECT action,actor FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+taskMid+"' ORDER BY rowid")),[
    {action:"activity_tasks.create",actor:"native-a@example.test"},
    {action:"activity_tasks.complete",actor:"service:ci-fixture.access"}
  ],"Task complete and update stay distinct; no-op produces no extra audit");
  const taskActorSnapshot=async()=>(await sql("SELECT "+
    "(SELECT state FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"') AS state,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"));
  const beforeTaskFault=(await taskActorSnapshot());
  (await sql("CREATE TRIGGER fixture_fail_task_actor BEFORE INSERT ON crm_audit "+
    "WHEN NEW.subject_id='"+taskOther+"' BEGIN SELECT RAISE(ABORT,'fixture task audit'); END;"));
  try{
    assertUnconfirmed(await taskV2("UpdateTask",admin,{taskId:taskOther,
      subject:patchSubject("Must not persist"),
      precondition:{expectedVersion:3n,idempotencyKey:"candidate-task-actor-failed"}}),"ACTIVITIES_TASK_WRITE_UNCONFIRMED");
    assert.deepEqual((await taskActorSnapshot()),beforeTaskFault);
  }finally{await sql("DROP TRIGGER fixture_fail_task_actor");}
  console.log(JSON.stringify({event:"crm.native-r2a-task-actor-audit",result:"PASS",
    update_complete_actor:true,no_op:true,failed_audit_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-activities-tasks-v2",
   result:"PASS",rpc_methods:["CreateTask","UpdateTask","ScheduleReminder","CompleteTask","GetTask","ListTasks"],
   generated_binary:true,original_owner:"crm.activities",signed_access:true,
   tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,account_fk:true,d1_cas:true,
   no_op_without_write:true,replay:true,d1_receipts:true,d1_audit:true,
   published_active:true}));


 // M1-08: restore the ORIGINAL crm-activities Task::schedule_reminder through
 // the existing generated TaskService, one Task table and atomic D1 trigger.
 const scheduleTask={taskId:taskOther,reminderAt:{seconds:1860000000n,nanos:123456789},
  precondition:{expectedVersion:3n,idempotencyKey:"candidate-task-reminder-first"}};
 const beforeReminder=await taskActorSnapshot();
 assert.equal((await taskV2("ScheduleReminder",null,scheduleTask)).status,401);
 assert.equal((await taskV2("ScheduleReminder",viewer,scheduleTask)).status,403);
 assert.equal((await taskV2("ScheduleReminder",tenantB,scheduleTask)).status,404);
 assert.equal((await taskV2("ScheduleReminder",admin,{
  taskId:taskOther,precondition:{expectedVersion:3n,idempotencyKey:"candidate-reminder-missing"}
 })).status,400);
 assert.equal((await taskV2("ScheduleReminder",admin,{
  ...scheduleTask,reminderAt:{seconds:1700000000n,nanos:0},
  precondition:{expectedVersion:3n,idempotencyKey:"candidate-reminder-past"}
 })).status,400);
 assert.equal((await taskV2("ScheduleReminder",admin,{
  ...scheduleTask,reminderAt:{seconds:1880000000n,nanos:0},
  precondition:{expectedVersion:3n,idempotencyKey:"candidate-reminder-after-due"}
 })).status,400);
 assert.equal((await taskV2("ScheduleReminder",admin,{
  ...scheduleTask,precondition:{expectedVersion:2n,idempotencyKey:"candidate-reminder-stale"}
 })).status,409);
 assert.equal((await taskV2("ScheduleReminder",admin,{
  taskId:taskMid,reminderAt:scheduleTask.reminderAt,
  precondition:{expectedVersion:2n,idempotencyKey:"candidate-reminder-completed"}
 })).status,409);
 assert.deepEqual(await taskActorSnapshot(),beforeReminder,
  "Invalid/denied/stale/completed reminder must not modify D1, audit or receipts");
 const scheduled=await taskV2("ScheduleReminder",admin,scheduleTask);
 assert.equal(scheduled.status,200,describeProtobuf(scheduled.body));
 assert.equal(scheduled.body.changed,true);
 assert.equal(scheduled.body.task?.version,4n);
 assert.equal(scheduled.body.task?.reminderAt?.seconds,1860000000n);
 assert.equal(scheduled.body.task?.reminderAt?.nanos,123456789);
 assert.equal(scheduled.body.receipt?.resourceVersion,4n);
 assert.equal(scheduled.body.receipt?.requestId,scheduled.headers.get("x-request-id"));
 assert.equal((await taskAudits()).at(-1).action,"activity_tasks.reminder_scheduled");
 assert.equal((await taskAudits()).at(-1).request_id,scheduled.headers.get("x-request-id"));
 const reminderReplayed=await taskV2("ScheduleReminder",knownService,scheduleTask);
 assert.equal(reminderReplayed.status,200,describeProtobuf(reminderReplayed.body));
 assert.equal(reminderReplayed.body.receipt?.replayed,true);
 assert.equal(reminderReplayed.body.receipt?.resourceVersion,4n);
 assert.equal((await taskAudits()).filter(x=>x.action==="activity_tasks.reminder_scheduled").length,1);
 assert.equal((await taskV2("ScheduleReminder",admin,{
  ...scheduleTask,reminderAt:{seconds:1860000001n,nanos:0}
 })).status,409,"same key with different reminder intent must reject");
 const beforeNoChange=await taskActorSnapshot();
 const unchangedReminder=await taskV2("ScheduleReminder",admin,{
  ...scheduleTask,precondition:{expectedVersion:4n,idempotencyKey:"candidate-reminder-same"}
 });
 assert.equal(unchangedReminder.status,200,describeProtobuf(unchangedReminder.body));
 assert.equal(unchangedReminder.body.changed,false);
 assert.equal(unchangedReminder.body.receipt,undefined);
 assert.equal(unchangedReminder.body.task?.version,4n);
 assert.deepEqual(await taskActorSnapshot(),beforeNoChange,
  "same reminder time must not create a version, receipt or audit");
 const movedReminder=await taskV2("ScheduleReminder",knownService,{
  ...scheduleTask,reminderAt:{seconds:1861000000n,nanos:222333444},
  precondition:{expectedVersion:4n,idempotencyKey:"candidate-reminder-reschedule"}
 });
 assert.equal(movedReminder.status,200,describeProtobuf(movedReminder.body));
 assert.equal(movedReminder.body.task?.version,5n);
 assert.equal(movedReminder.body.task?.reminderAt?.seconds,1861000000n);
 assert.equal((await taskAudits()).filter(x=>x.action==="activity_tasks.reminder_scheduled").length,2);
 assert.equal((await taskV2("GetTask",viewer,{taskId:taskOther})).body.task?.version,5n);
 assert.equal((await sql("SELECT version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"'"))[0].version,5);
 const beforeReminderFault=await taskActorSnapshot();
 await sql("CREATE TRIGGER fixture_fail_reminder_audit BEFORE INSERT ON crm_audit "+
  "WHEN NEW.subject_id='"+taskOther+"' AND NEW.action='activity_tasks.reminder_scheduled' "+
  "BEGIN SELECT RAISE(ABORT,'fixture reminder audit'); END;");
 try{
  const denied=await taskV2("ScheduleReminder",admin,{
    ...scheduleTask,reminderAt:{seconds:1862000000n,nanos:0},
    precondition:{expectedVersion:5n,idempotencyKey:"candidate-reminder-audit-rollback"}
  });
  assert.equal(denied.status,503,describeProtobuf(denied.body));
  assert.equal(denied.body.message,"ACTIVITIES_TASK_WRITE_UNCONFIRMED");
  assert.deepEqual(await taskActorSnapshot(),beforeReminderFault,
    "Late reminder audit failure must roll back Task state and original receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_reminder_audit");}
 console.log(JSON.stringify({event:"crm.native-m1-08-reminder",result:"PASS",
  domain:"crm.activities.Task::schedule_reminder",post_create:true,reschedule:true,
  no_op_without_write:true,completed_denied:true,after_due_denied:true,
  stale_cas:true,replay:true,tenant_ab:true,receipt_audit_atomic:true}));

 // The same original Task owner filters overdue instants BEFORE the keyset
 // LIMIT, preserving the typed status/priority/account/search/Deal filters.
 const dueSeeds=[
   ["task-overdue-a-"+randomUUID(),"Native overdue A",2,1600000000n],
   ["task-overdue-z-"+randomUUID(),"Native overdue Z",4,1640995200n],
   ["task-due-future-"+randomUUID(),"Native due future",4,1900000000n],
   ["task-no-due-"+randomUUID(),"Native unscheduled",2,null],
   ["task-overdue-completed-"+randomUUID(),"Native already completed",2,1600000000n]
 ];
 for(const [taskId,subject,priority,due] of dueSeeds){
   const created=await taskV2("CreateTask",admin,{taskId,accountId:accountNextId,subject,priority,
     ...(due===null?{}:{dueAt:{seconds:due,nanos:0}}),
     precondition:{idempotencyKey:"candidate-overdue-create-"+taskId}});
   assert.equal(created.status,200,describeProtobuf(created.body));
 }
 const [dueA,dueZ,dueFuture,dueNone,dueComplete]=dueSeeds.map(x=>x[0]);
 const completeSeed=await taskV2("CompleteTask",admin,{taskId:dueComplete,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-overdue-complete"}});
 assert.equal(completeSeed.status,200,describeProtobuf(completeSeed.body));
 const overdueAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 const overdueList=async(input={})=>(await collectPages(taskV2,"ListTasks","tasks",
   {overdueOnly:true,page:{pageSize:1},...input})).map(x=>x.taskId);
 assert.deepEqual((await overdueList()).sort(),[dueA,dueZ].sort());
 assert.deepEqual(await overdueList({accountId:accountNextId}),[dueA,dueZ].sort());
 assert.deepEqual(await overdueList({accountId}),[]);
 assert.deepEqual(await overdueList({priority:4}),[dueZ]);
 assert.deepEqual(await overdueList({status:2}),[]);
 assert.deepEqual(await overdueList({search:"Native overdue A"}),[dueA]);
 assert.deepEqual(await overdueList({dealId:"missing-deal"}),[]);
 assert.equal((await taskV2("ListTasks",tenantB,{overdueOnly:true})).body.tasks.length,0);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",
   {overdueOnly:true,accountId:accountNextId,page:{pageSize:1}})).map(x=>x.taskId),
   (await collectPages(taskV2,"ListTasks","tasks",
   {overdueOnly:true,accountId:accountNextId,page:{pageSize:1}})).map(x=>x.taskId));
 const viewerOverdue=await taskV2("ListTasks",viewer,{overdueOnly:true,accountId:accountNextId});
 const adminOverdue=await taskV2("ListTasks",admin,{overdueOnly:true,accountId:accountNextId});
 assert.deepEqual(viewerOverdue.body,adminOverdue.body);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   overdueAuditBefore,"Overdue owner reads never mutate audit");
 assert.equal((await taskV2("GetTask",viewer,{taskId:dueFuture})).status,200);
 assert.equal((await taskV2("GetTask",viewer,{taskId:dueNone})).status,200);
 const resolvedOverdue=await taskV2("CompleteTask",admin,{taskId:dueA,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-overdue-resolved"}});
 assert.equal(resolvedOverdue.status,200);
 assert.deepEqual(await overdueList(),[dueZ],"Completion removes overdue Task from server results");
 console.log(JSON.stringify({event:"crm.native-task-overdue-original-owner",result:"PASS",
   owner:"crm.activities",server_time:true,original_cursor:true,tenant:true,viewer:true,
   status:true,priority:true,account:true,search:true,deal:true,no_read_audit:true}));

 // Original read-only DataQuality and Identity Resolution, signed binary
 // Connect from the exact generated descriptor, with tenant A/B and viewer.
 const signedReadOnly=(path,encode,decode)=>async(method,token,payload,extraHeaders={})=>{
   const r=await rpcFetch(path(method),{method:"POST",headers:{
     ...extraHeaders,"content-type":"application/proto","connect-protocol-version":"1",
     ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:encode(method,payload)});
   return {status:r.status,headers:r.headers,
     body:r.status<400?decode(method,new Uint8Array(await r.arrayBuffer())):await r.json()};
 };
 const qualityV2=signedReadOnly(qualityV2Path,binaryQualityV2,decodeQualityV2);
 const identityV2=signedReadOnly(identityV2Path,binaryIdentityV2,decodeIdentityV2);
 assert.equal((await qualityV2("EvaluatePartyName",null,{partyId:id})).status,401);
 assert.equal((await qualityV2("EvaluatePartyName",tenantB,{partyId:id})).status,404);
 assert.equal((await qualityV2("EvaluatePartyName",viewer,{partyId:""})).status,400);
 assert.equal((await qualityV2("EvaluatePartyName",viewer,{partyId:"missing-party"})).status,404);
 const quality=await qualityV2("EvaluatePartyName",viewer,{partyId:id});
 assert.equal(quality.status,200,describeProtobuf(quality.body));
 assert.equal(quality.body.party?.partyId,id);
 assert.ok(quality.body.ruleSetVersion);
 assert.equal(quality.body.checks.length,2);
 assert.equal(quality.body.failedCount,0);
 assert.ok(quality.body.checks.every(c=>c.passed));
 const shortQualityId="quality-short-"+randomUUID();
 const shortCreated=await v3("CreateParty",admin,{
   partyId:shortQualityId,kind:1,displayName:"A",
   precondition:{idempotencyKey:"quality-short-create"}});
 assert.equal(shortCreated.status,200,describeProtobuf(shortCreated.body));
 const poorQuality=await qualityV2("EvaluatePartyName",viewer,{partyId:shortQualityId});
 assert.equal(poorQuality.status,200,describeProtobuf(poorQuality.body));
 assert.equal(poorQuality.body.failedCount,1);
 assert.equal(poorQuality.body.checks.filter(x=>!x.passed).length,1);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+shortQualityId+"'"))[0].n,1,
   "quality read must not append audit or finding");
 console.log(JSON.stringify({event:"crm.native-g3-data-quality-v2",
   result:"PASS",rpc_methods:["EvaluatePartyName"],generated_binary:true,
   original_owner:"crm.data-quality",tenant_ab:true,viewer_read:true,
   no_duplicate_rules:true,no_writes:true,published_active:true}));

 const beforeCandidates=await identityV2("FindSharedEmailCandidates",viewer,{page:{pageSize:1}});
 assert.equal(beforeCandidates.status,200,describeProtobuf(beforeCandidates.body));
 assert.equal(beforeCandidates.body.candidates.length,0);
 const auditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 for(const [index,partyId] of [pagingPersonId,"binary-rpc-proof-01"].entries()){
   const contact=await contactV2("CreateContactPoint",admin,{
     contactPointId:"shared-email-"+index+"-"+randomUUID(),
     partyId,kind:1,value:"later-timestamps@example.org",preferred:false,
     precondition:{idempotencyKey:"shared-email-create-"+index}});
   assert.equal(contact.status,200,describeProtobuf(contact.body));
   assert.equal(contact.body.contactPoint?.verified,false);
 }
 const auditAfter=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 assert.equal(auditAfter,auditBefore+2);
 assert.equal((await identityV2("FindSharedEmailCandidates",null,{})).status,401);
 assert.equal((await identityV2("FindSharedEmailCandidates",viewer,{page:{pageSize:101}})).status,400);
 assert.equal((await identityV2("FindSharedEmailCandidates",viewer,{
   page:{pageSize:1,pageToken:"invalid"}})).status,400);
 assert.equal((await identityV2("FindSharedEmailCandidates",tenantB,{
   page:{pageSize:1}})).body.candidates.length,0);
 const matches=await collectPages(identityV2,"FindSharedEmailCandidates","candidates");
 assert.equal(matches.length,3,"three canonical pair observations");
 assert.ok(matches.every(c=>!c.bothContactsVerified));
 const pairs=matches.map(c=>[c.firstParty?.partyId,c.secondParty?.partyId].join("|"));
 assert.equal(new Set(pairs).size,3);
 assert.ok(matches.every(c=>c.firstParty.partyId<c.secondParty.partyId));
 const compare={firstPartyId:pagingPersonId,secondPartyId:id};
 assert.equal((await identityV2("CompareParties",null,compare)).status,401);
 assert.equal((await identityV2("CompareParties",tenantB,compare)).status,404);
 assert.equal((await identityV2("CompareParties",viewer,{
   firstPartyId:id,secondPartyId:id})).status,400);
 assert.equal((await identityV2("CompareParties",viewer,{
   firstPartyId:id,secondPartyId:"missing-party"})).status,404);
 const compared=await identityV2("CompareParties",viewer,compare);
 assert.equal(compared.status,200,describeProtobuf(compared.body));
 assert.equal(compared.body.firstParty?.partyId,pagingPersonId);
 assert.equal(compared.body.secondParty?.partyId,id);
 assert.equal(compared.body.samePartyId,false);
 assert.equal(compared.body.identityConfirmed,false);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   auditAfter,"read-only identity operations must not write audit");
 console.log(JSON.stringify({event:"crm.native-g3-identity-resolution-v2",
   result:"PASS",rpc_methods:["FindSharedEmailCandidates","CompareParties"],
   generated_binary:true,original_owner:"crm.identity-resolution",
   tenant_ab:true,viewer_read:true,pagination:true,no_identity_verdict:true,
   no_writes:true,published_active:true}));

 // L-03: a governed canonical merge is reversible lineage, not data reparenting.
 // The unchanged downstream D1 owner tables prove no Account/Contact/Deal/Task loss.
 const mergeSourceId="native-merge-source-"+randomUUID();
 const mergeSurvivorId="native-merge-survivor-"+randomUUID();
 for(const [p,name] of [[mergeSourceId,"Merge source"],[mergeSurvivorId,"Merge survivor"]]){
   const created=await v3("CreateParty",admin,{
     partyId:p,kind:1,displayName:name,
     precondition:{idempotencyKey:"identity-l03-party-"+randomUUID()}});
   assert.equal(created.status,200,describeProtobuf(created.body));
 }
 const mergeOpId="native-merge-"+randomUUID();
 const mergeKey="identity-l03-merge-"+randomUUID();
 const intent={
   operationId:mergeOpId,sourcePartyId:mergeSourceId,sourcePartyVersion:1n,
   survivorPartyId:mergeSurvivorId,survivorPartyVersion:1n,
   reason:"manual_identity_review",precondition:{idempotencyKey:mergeKey}
 };
 const downstreamTables=["crm_account_records","crm_contact_point_records",
   "crm_party_relationship_records","crm_sales_deal_records",
   "crm_activity_task_records","crm_consent_records"];
 const beforeDownstream=await Promise.all(downstreamTables.map(async table=>
   (await sql("SELECT COUNT(*) AS n FROM "+table))[0].n));
 const beforeLineageAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 assert.equal((await identityV2("MergeParty",viewer,intent)).status,403);
 assert.equal((await identityV2("MergeParty",tenantB,intent)).status,404);
 assert.equal((await identityV2("MergeParty",admin,{
   ...intent,sourcePartyVersion:2n,
   precondition:{idempotencyKey:"l03-stale-"+randomUUID()}
 })).status,409);
 await sql("CREATE TRIGGER fixture_identity_rollback BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action='identity_resolution.merge' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;");
 try {
   assertUnconfirmed(await identityV2("MergeParty",admin,intent),"IDENTITY_MERGE_COMMIT_UNCONFIRMED");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",intent.precondition.idempotencyKey))[0].n,0);
 } finally {await sql("DROP TRIGGER fixture_identity_rollback");}
 const merged=await identityV2("MergeParty",admin,intent);
 if(merged.status!==200){
   const rowProof=await sql("SELECT status,version FROM crm_identity_merge_records WHERE tenant='tenant-a' AND id='"+mergeOpId+"'");
   const receiptProof=await sql("SELECT original_request_id FROM crm_identity_merge_receipts WHERE tenant='tenant-a' AND key='"+mergeKey+"'");
   const partyProof=await sql("SELECT id,version FROM crm_party_records WHERE tenant='tenant-a' AND id IN ('"+mergeSourceId+"','"+mergeSurvivorId+"')");
   console.error(JSON.stringify({event:"crm.l03.commit-proof-failure",status:merged.status,
     domain_error:merged.body?.message,rows:rowProof,receipts:receiptProof.length,parties:partyProof}));
 }
 assert.equal(merged.status,200,describeProtobuf(merged.body));
 assert.equal(merged.body.operation?.active,true);
 assert.equal(merged.body.operation?.version,1n);
 assert.equal(merged.body.receipt?.replayed,false);
 assert.equal(merged.body.receipt?.resourceVersion,1n);
 assert.equal((await identityV2("MergeParty",admin,intent)).body.receipt?.replayed,true);
 assert.equal((await identityV2("MergeParty",admin,{...intent,
   reason:"different_reason"})).status,409);
 assert.equal((await identityV2("MergeParty",admin,{...intent,
   operationId:"l03-double-"+randomUUID(),
   precondition:{idempotencyKey:"identity-l03-duplicate-"+randomUUID()}
 })).status,409,"active source cannot be merged twice");
 assert.equal((await identityV2("MergeParty",admin,{
   ...intent,operationId:"l03-reverse-"+randomUUID(),
   sourcePartyId:mergeSurvivorId,survivorPartyId:mergeSourceId,
   precondition:{idempotencyKey:"identity-l03-reverse-"+randomUUID()}
 })).status,409,"cycle or redirected survivor must fail");
 const got=await identityV2("GetMergeOperation",viewer,{
   operationId:mergeOpId,receiptIdempotencyKey:mergeKey
 });
 assert.equal(got.status,200,describeProtobuf(got.body));
 assert.equal(got.body.operation?.operationId,mergeOpId);
 assert.equal(got.body.confirmedMutation?.resourceVersion,1n);
 assert.ok(got.body.confirmedMutation?.originalRequestId);
 assert.equal((await identityV2("GetMergeOperation",tenantB,{operationId:mergeOpId})).status,404);
 const history=await identityV2("ListMergeOperationsByParty",viewer,{
   partyId:mergeSourceId,page:{pageSize:10}
 });
 assert.equal(history.status,200,describeProtobuf(history.body));
 assert.equal(history.body.operations.length,1);
 assert.equal(history.body.operations[0].operationId,mergeOpId);
 assert.equal((await identityV2("ListMergeOperationsByParty",tenantB,{
   partyId:mergeSourceId,page:{pageSize:10}
 })).status,404);
 const resolved=await identityV2("ResolveCanonicalParty",viewer,{partyId:mergeSourceId});
 assert.equal(resolved.status,200,describeProtobuf(resolved.body));
 assert.equal(resolved.body.canonicalPartyId,mergeSurvivorId);
 assert.deepEqual(resolved.body.partyPath,[mergeSourceId,mergeSurvivorId]);
 assert.deepEqual(resolved.body.mergeOperationPath,[mergeOpId]);
 assert.equal((await identityV2("ResolveCanonicalParty",tenantB,{
   partyId:mergeSourceId
 })).status,404);
 const unmergeKey="identity-l03-unmerge-"+randomUUID();
 const undo={operationId:mergeOpId,sourcePartyVersion:1n,survivorPartyVersion:1n,
   reason:"incorrect_merge",precondition:{expectedVersion:1n,idempotencyKey:unmergeKey}};
 assert.equal((await identityV2("UnmergeParty",viewer,undo)).status,403);
 assert.equal((await identityV2("UnmergeParty",admin,{
   ...undo,precondition:{expectedVersion:2n,idempotencyKey:"l03-undo-stale-"+randomUUID()}
 })).status,409);
 await sql("CREATE TRIGGER fixture_identity_rollback BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action='identity_resolution.unmerge' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;");
 try {
   assertUnconfirmed(await identityV2("UnmergeParty",admin,undo),"IDENTITY_UNMERGE_COMMIT_UNCONFIRMED");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts WHERE tenant=? AND key=?",
     "tenant-a",undo.precondition.idempotencyKey))[0].n,0);
 } finally {await sql("DROP TRIGGER fixture_identity_rollback");}
 const undone=await identityV2("UnmergeParty",admin,undo);
 assert.equal(undone.status,200,describeProtobuf(undone.body));
 assert.equal(undone.body.operation?.active,false);
 assert.equal(undone.body.operation?.version,2n);
 assert.equal((await identityV2("UnmergeParty",admin,undo)).body.receipt?.replayed,true);
 const reverted=await identityV2("ResolveCanonicalParty",viewer,{partyId:mergeSourceId});
 assert.equal(reverted.status,200,describeProtobuf(reverted.body));
 assert.equal(reverted.body.canonicalPartyId,mergeSourceId);
 assert.deepEqual(reverted.body.partyPath,[mergeSourceId]);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_identity_merge_records "+
   "WHERE tenant='tenant-a' AND id='"+mergeOpId+"' AND status='unmerged' AND version=2"))[0].n,1);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_identity_merge_receipts "+
   "WHERE tenant='tenant-a'"))[0].n,2,"one immutable receipt per original mutation");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   beforeLineageAudit+2,"one merge and one unmerge audit, reads and replays do not write");
 const afterDownstream=await Promise.all(downstreamTables.map(async table=>
   (await sql("SELECT COUNT(*) AS n FROM "+table))[0].n));
 assert.deepEqual(afterDownstream,beforeDownstream,"canonical lineage never rewires other owners");
 console.log(JSON.stringify({event:"crm.native-l03-merge-lineage",result:"PASS",
   generated_binary:true,role_policy:true,tenant_isolation:true,stale_party_guard:true,
   replay_receipt:true,immutable_history:true,canonical_resolve:true,
   reversible:true,downstream_unchanged:true,audit_atomic:true}));

// Original crm-consents v2: operator may DENY, never self-attest GRANT.
// Every RPC is generated binary Protobuf through signed native workerd/D1.
const consentV2=signedReadOnly(consentV2Path,binaryConsentV2,decodeConsentV2);
const consentScope={partyId:id,contactPointId:contactId,
  purpose:"marketing.newsletter",channel:1};
assert.equal((await consentV2("EvaluateConsent",null,consentScope)).status,401);
assert.equal((await consentV2("EvaluateConsent",viewer,{
  ...consentScope,channel:0})).status,400);
assert.equal((await consentV2("EvaluateConsent",tenantB,consentScope)).status,400);
const noGrant=await consentV2("EvaluateConsent",viewer,consentScope);
assert.equal(noGrant.status,200,describeProtobuf(noGrant.body));
assert.equal(noGrant.body.allowed,false,"no source-backed GRANT is available");
assert.equal(noGrant.body.reason,4,"NO_APPLICABLE_GRANT");
const denyInput={authorizationId:"consent-mid-"+randomUUID(),
  ...consentScope,jurisdiction:"eu",
  precondition:{idempotencyKey:"connect-consent-deny-mid"}};
assert.equal((await consentV2("SuppressCommunication",null,denyInput)).status,401);
assert.equal((await consentV2("SuppressCommunication",viewer,denyInput)).status,403);
assert.equal((await consentV2("SuppressCommunication",tenantB,denyInput)).status,400);
assert.equal((await consentV2("SuppressCommunication",admin,{
  ...denyInput,channel:0,
  precondition:{idempotencyKey:"connect-consent-invalid-channel"}})).status,400);
const suppressed=await consentV2("SuppressCommunication",admin,denyInput);
assert.equal(suppressed.status,200,describeProtobuf(suppressed.body));
assert.equal(suppressed.body.authorization?.effect,2,"DENY only");
assert.equal(suppressed.body.authorization?.status,1,"ACTIVE");
assert.equal(suppressed.body.authorization?.partyId,id);
assert.equal(suppressed.body.receipt?.resourceVersion,1n);
assert.equal(suppressed.body.receipt?.replayed,false);
assert.equal(suppressed.body.receipt?.requestId,suppressed.headers.get("x-request-id"));
const consentReplay=await consentV2("SuppressCommunication",admin,denyInput);
assert.equal(consentReplay.status,200,describeProtobuf(consentReplay.body));
assert.equal(consentReplay.body.receipt?.replayed,true);
assert.equal((await consentV2("SuppressCommunication",admin,{
  ...denyInput,purpose:"marketing.different"})).status,409);
const consentDenied=await consentV2("EvaluateConsent",viewer,consentScope);
assert.equal(consentDenied.status,200,describeProtobuf(consentDenied.body));
assert.equal(consentDenied.body.allowed,false);
assert.equal(consentDenied.body.reason,2,"ACTIVE_DENY");
const otherScope=await consentV2("EvaluateConsent",admin,{
  ...consentScope,purpose:"marketing.unrelated"});
assert.equal(otherScope.status,200);
assert.equal(otherScope.body.allowed,false);
assert.equal(otherScope.body.reason,4);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+denyInput.authorizationId+"'"))[0].n,1,
  "repeat suppression and consent evaluation must not duplicate audit");
const originalList=await consentV2("ListConsentAuthorizations",viewer,{page:{pageSize:1}});
assert.equal(originalList.status,200,describeProtobuf(originalList.body));
assert.equal(originalList.body.authorizations.length,1);
assert.equal((await consentV2("ListConsentAuthorizations",tenantB,{page:{pageSize:1}}))
  .body.authorizations.length,0);
assert.equal((await consentV2("ListConsentAuthorizations",viewer,{page:{pageSize:101}}))
  .status,400);
const otherId="consent-aaa-other-"+randomUUID();
const lastId="consent-zzz-own-"+randomUUID();
for(const [index,authorizationId,partyId] of [
  [0,otherId,pagingPersonId],[1,lastId,id]
]){
  const inserted=await consentV2("SuppressCommunication",admin,{
    authorizationId,partyId,purpose:"marketing.other",channel:1,jurisdiction:"eu",
    precondition:{idempotencyKey:"connect-consent-additional-"+index}});
  assert.equal(inserted.status,200,describeProtobuf(inserted.body));
}
const filteredConsents=await collectPages(consentV2,"ListConsentAuthorizations",
  "authorizations",{partyId:id});
assert.deepEqual(filteredConsents.map(x=>x.authorizationId),
  [denyInput.authorizationId,lastId]);
const allConsents=await collectPages(consentV2,"ListConsentAuthorizations","authorizations");
assert.equal(allConsents.length,3);
assert.equal((await consentV2("ListConsentAuthorizations",viewer,{
  page:{pageSize:1,pageToken:lastId},partyId:id})).body.authorizations.length,0);
assert.equal((await consentV2("ListConsentAuthorizations",viewer,{
  page:{pageSize:1},partyId:"missing-party"})).body.authorizations.length,0);
console.log(JSON.stringify({event:"crm.native-g3-consents-v2",result:"PASS",
  rpc_methods:["EvaluateConsent","SuppressCommunication","ListConsentAuthorizations"],
  generated_binary:true,original_owner:"crm.consents",signed_access:true,
  tenant_ab:true,viewer_read:true,viewer_write_denied:true,operator_deny_only:true,
  no_grant_ingress:true,before_limit_filtered_pagination:true,
  d1_receipts:true,no_extra_audit:true,published_active:true}));

// R3a: signed human self-consent and distinct enrolled dev test-agent path.
 const r3Contact="r3-contact-"+randomUUID();
 const r3Create=await contactV2("CreateContactPoint",admin,{
   contactPointId:r3Contact,partyId:id,kind:1,value:"native-a@example.test",
   preferred:false,precondition:{idempotencyKey:"r3-create-"+r3Contact}});
 assert.equal(r3Create.status,200,describeProtobuf(r3Create.body));
 const r3Verify={contactPointId:r3Contact,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-verify-"+r3Contact}};
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",viewer,r3Verify)).status,403);
 await sql("CREATE TRIGGER fixture_fail_verify_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.action='contact_points.verify' BEGIN SELECT RAISE(ABORT,'verification audit unavailable'); END;");
 try{
   assertUnconfirmed(await contactV2("VerifyOwnEmailContactPoint",admin,r3Verify),"CONTACT_POINTS_WRITE_UNCONFIRMED");
 }finally{await sql("DROP TRIGGER fixture_fail_verify_audit");}
 const verified=await contactV2("VerifyOwnEmailContactPoint",admin,r3Verify);
 assert.equal(verified.status,200,describeProtobuf(verified.body));
 assert.equal(verified.body.contactPoint?.verified,true);
 assert.equal(verified.body.contactPoint?.version,2n);
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",admin,r3Verify)).body.receipt?.replayed,true);
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",admin,{
   ...r3Verify,precondition:{expectedVersion:1n,
     idempotencyKey:"r3-stale-verify-"+r3Contact}})).status,409,
   "Stale verified-contact version must reject without any extra mutation");
 const r3Id="r3-consent-"+randomUUID();
 const r3Scope={partyId:id,contactPointId:r3Contact,purpose:"r3.test.newsletter",channel:1};
 const r3Grant={authorizationId:r3Id,partyId:id,contactPointId:r3Contact,
   purpose:r3Scope.purpose,jurisdiction:"eu",precondition:{idempotencyKey:"r3-grant-"+r3Id}};
 assert.equal((await consentV2("GrantVerifiedEmailConsent",knownService,r3Grant)).status,403);
 assert.equal((await consentV2("GrantVerifiedEmailConsent",viewer,r3Grant)).status,403);
 const granted=await consentV2("GrantVerifiedEmailConsent",admin,r3Grant);
 assert.equal(granted.status,200,describeProtobuf(granted.body));
 assert.equal(granted.body.authorization?.effect,1);
 assert.equal((await consentV2("GrantVerifiedEmailConsent",admin,r3Grant)).body.receipt?.replayed,true);
 assert.equal((await consentV2("EvaluateConsent",viewer,r3Scope)).body.allowed,true);
 const r3Withdraw={authorizationId:r3Id,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-withdraw-"+r3Id}};
 assert.equal((await consentV2("WithdrawConsentAuthorization",viewer,r3Withdraw)).status,403);
 const revoked=await consentV2("WithdrawConsentAuthorization",knownService,r3Withdraw);
 assert.equal(revoked.status,200,describeProtobuf(revoked.body));
 assert.equal(revoked.body.authorization?.status,2);
 assert.equal((await consentV2("WithdrawConsentAuthorization",knownService,r3Withdraw)).body.receipt?.replayed,true);
 assert.equal((await consentV2("WithdrawConsentAuthorization",admin,{
   ...r3Withdraw,precondition:{expectedVersion:1n,
     idempotencyKey:"r3-stale-withdraw-"+r3Id}})).status,409,
   "Stale withdrawal cannot create another audit or receipt");
 const deniedAfter=await consentV2("EvaluateConsent",viewer,r3Scope);
 assert.equal(deniedAfter.body.allowed,false);
 assert.equal(deniedAfter.body.reason,3);
 const secondId="r3-consent-renew-"+randomUUID();
 const second={...r3Grant,authorizationId:secondId,
   precondition:{idempotencyKey:"r3-grant2-"+secondId}};
 assert.equal((await consentV2("GrantVerifiedEmailConsent",admin,second)).status,200);
 assert.equal((await consentV2("EvaluateConsent",viewer,r3Scope)).body.allowed,true);

 // #471 S5b Consent: the existing read endpoint proves the original receipt,
 // even after the authorization has advanced to a later version.
 const consentReceiptInput={authorizationId:r3Id,
   receiptIdempotencyKey:r3Grant.precondition.idempotencyKey};
 assert.equal((await consentV2("GetConsentAuthorization",null,consentReceiptInput)).status,401);
 const consentAuditBefore=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE subject_id=?",r3Id))[0].n;
 const recoveredGrant=await consentV2("GetConsentAuthorization",viewer,consentReceiptInput);
 assert.equal(recoveredGrant.status,200,describeProtobuf(recoveredGrant.body));
 assert.equal(recoveredGrant.body.authorization?.status,2,
   "Get returns the current withdrawn authorization");
 assert.equal(recoveredGrant.body.confirmedMutation?.originalRequestId,
   granted.headers.get("x-request-id"));
 assert.equal(recoveredGrant.body.confirmedMutation?.resourceVersion,1n,
   "receipt evidence retains the original grant version, not the current withdrawn version");
 assert.equal(recoveredGrant.body.confirmedMutation?.requestId,
   recoveredGrant.headers.get("x-request-id"));
 assert.equal((await consentV2("GetConsentAuthorization",tenantB,consentReceiptInput)).status,404);
 assert.equal((await consentV2("GetConsentAuthorization",viewer,{
   ...consentReceiptInput,receiptIdempotencyKey:"short"})).status,400);
 const missingConsentReceipt=await consentV2("GetConsentAuthorization",viewer,{
   authorizationId:r3Id,receiptIdempotencyKey:"consent-missing-receipt-01"});
 assert.equal(missingConsentReceipt.status,200);
 assert.equal(missingConsentReceipt.body.confirmedMutation,undefined,
   "a missing key is not evidence of a committed mutation");
 const wrongConsentResource=await consentV2("GetConsentAuthorization",viewer,{
   authorizationId:secondId,receiptIdempotencyKey:r3Grant.precondition.idempotencyKey});
 assert.equal(wrongConsentResource.status,200);
 assert.equal(wrongConsentResource.body.confirmedMutation,undefined,
   "a receipt for another authorization cannot confirm this resource");
 await sql("UPDATE crm_consent_receipts SET original_request_id=NULL WHERE tenant=? AND key=?",
   "tenant-a",r3Grant.precondition.idempotencyKey);
 try {
   const legacyConsent=await consentV2("GetConsentAuthorization",viewer,consentReceiptInput);
   assert.equal(legacyConsent.status,200);
   assert.equal(legacyConsent.body.confirmedMutation,undefined,
     "legacy receipts without original provenance are not certified");
 } finally {
   await sql("UPDATE crm_consent_receipts SET original_request_id=? WHERE tenant=? AND key=?",
     granted.headers.get("x-request-id"),"tenant-a",r3Grant.precondition.idempotencyKey);
 }
 await sql("ALTER TABLE crm_consent_receipts RENAME TO fixture_unavailable_consent_receipts");
 try {
   const unavailableConsent=await consentV2("GetConsentAuthorization",viewer,consentReceiptInput);
   assert.equal(unavailableConsent.status,503,
     "an inaccessible receipt source must remain unconfirmed");
   assert.equal((await consentV2("GetConsentAuthorization",viewer,{
     authorizationId:r3Id})).status,200,
     "ordinary Get remains available without receipt recovery");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_consent_receipts RENAME TO crm_consent_receipts");
 }
 const restoredConsent=await consentV2("GetConsentAuthorization",viewer,consentReceiptInput);
 assert.equal(restoredConsent.status,200);
 assert.equal(restoredConsent.body.confirmedMutation?.originalRequestId,
   granted.headers.get("x-request-id"));
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE subject_id=?",r3Id))[0].n,
   consentAuditBefore,"read-only receipt recovery must never append an audit row");
 console.log(JSON.stringify({event:"crm.native-s5b-consent-commit-provenance",result:"PASS",
   original_request_id:true,original_version:true,tenant_isolation:true,
   wrong_resource_rejected:true,legacy_null_fail_closed:true,
   receipt_read_failure_unconfirmed:true,read_only:true}));

 const failingGrantId="r3-audit-fault-"+randomUUID();
 const failingKey="r3-audit-fault-key-"+failingGrantId;
 const beforeGrantFault=(await sql("SELECT "+
   "(SELECT COUNT(*) FROM crm_consent_records) AS records,"+
   "(SELECT COUNT(*) FROM crm_consent_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit) AS audits"));
 (await sql("CREATE TRIGGER fixture_fail_r3_grant_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+failingGrantId+"' AND NEW.action='consents.grant' "+
   "BEGIN SELECT RAISE(ABORT,'fixture R3 consent grant audit failure'); END;"));
 try{
   assertUnconfirmed(await consentV2("GrantVerifiedEmailConsent",admin,{
     ...r3Grant,authorizationId:failingGrantId,
     precondition:{idempotencyKey:failingKey}}),"CONSENTS_WRITE_UNCONFIRMED");
   assert.deepEqual((await sql("SELECT "+
     "(SELECT COUNT(*) FROM crm_consent_records) AS records,"+
     "(SELECT COUNT(*) FROM crm_consent_receipts) AS receipts,"+
     "(SELECT COUNT(*) FROM crm_audit) AS audits")),beforeGrantFault,
     "Consent state, receipt and audit must roll back together on late audit failure");
 } finally {await sql("DROP TRIGGER fixture_fail_r3_grant_audit");}
 const changed=await contactV2("UpdateContactPoint",admin,{
   contactPointId:r3Contact,value:"other@example.org",status:1,preferred:false,
   precondition:{idempotencyKey:"r3-change-"+r3Contact,expectedVersion:2n}});
 assert.equal(changed.status,200,describeProtobuf(changed.body));
 assert.equal(changed.body.contactPoint?.verified,false);
 assert.equal((await consentV2("EvaluateConsent",viewer,r3Scope)).body.allowed,false,
   "Old grant must be ineffective after email change");
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+r3Id+"' ORDER BY rowid")),[
   {action:"consents.grant",actor:"native-a@example.test"},
   {action:"consents.withdraw",actor:"service:ci-fixture.access"}
 ]);
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+r3Contact+"' ORDER BY rowid")),[
   {action:"contact_points.create",actor:"native-a@example.test"},
   {action:"contact_points.verify",actor:"native-a@example.test"},
   {action:"contact_points.update",actor:"native-a@example.test"}
 ]);
 console.log(JSON.stringify({event:"crm.native-r3-signed-email-consent",result:"PASS",
   human_email:true,service_denied:true,grant_withdraw:true,replay:true,
   changed_contact_invalidates_grant:true,actor_audit:true,
   atomic_grant_rollback:true,stale_versions_denied:true}));

 // The same real signed admin service can accept a QA cycle through the UI.
 // It must not use human Access evidence, nor grant on human-verified records.
 const agentContact="r3-agent-contact-"+randomUUID();
 const agentCreated=await contactV2("CreateContactPoint",knownService,{
   contactPointId:agentContact,partyId:id,kind:1,value:"qa-agent@example.test",
   preferred:false,precondition:{idempotencyKey:"r3-agent-create-"+agentContact}});
 assert.equal(agentCreated.status,200,describeProtobuf(agentCreated.body));
 const agentVerify={contactPointId:agentContact,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-agent-verify-"+agentContact}};
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",viewer,agentVerify)).status,403);
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",admin,agentVerify)).status,403);
 const agentVerified=await contactV2("VerifyOwnEmailContactPoint",knownService,agentVerify);
 assert.equal(agentVerified.status,200,describeProtobuf(agentVerified.body));
 assert.equal(agentVerified.body.contactPoint?.verified,true);
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",knownService,agentVerify)).body.receipt?.replayed,true);
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",knownService,{
   ...agentVerify,precondition:{expectedVersion:1n,
     idempotencyKey:"r3-agent-stale-"+agentContact}})).status,409);
 const agentId="r3-agent-consent-"+randomUUID();
 const agentScope={partyId:id,contactPointId:agentContact,
   purpose:"r3.dev.agent",channel:1};
 const agentGrant={authorizationId:agentId,partyId:id,
   contactPointId:agentContact,purpose:agentScope.purpose,
   jurisdiction:"eu",precondition:{idempotencyKey:"r3-agent-grant-"+agentId}};
 assert.equal((await consentV2("GrantVerifiedEmailConsent",viewer,agentGrant)).status,403);
 assert.equal((await consentV2("GrantVerifiedEmailConsent",admin,agentGrant)).status,403);
 const agentGranted=await consentV2("GrantVerifiedEmailConsent",knownService,agentGrant);
 assert.equal(agentGranted.status,200,describeProtobuf(agentGranted.body));
 assert.equal(agentGranted.body.authorization?.effect,1);
 assert.equal((await consentV2("GrantVerifiedEmailConsent",knownService,agentGrant)).body.receipt?.replayed,true);
 assert.equal((await consentV2("EvaluateConsent",viewer,agentScope)).body.allowed,true);
 const agentState=(await sql("SELECT state FROM crm_consent_records WHERE id='"+agentId+"'"))[0].state;
 assert.ok(agentState.includes("crm.dev.agent"),"Service grant requires distinct test-agent provenance");
 const agentWithdraw={authorizationId:agentId,
   precondition:{expectedVersion:1n,idempotencyKey:"r3-agent-withdraw-"+agentId}};
 assert.equal((await consentV2("WithdrawConsentAuthorization",knownService,agentWithdraw)).status,200);
 assert.equal((await consentV2("EvaluateConsent",viewer,agentScope)).body.allowed,false);
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+agentId+"' ORDER BY rowid")),[
   {action:"consents.grant",actor:"service:ci-fixture.access"},
   {action:"consents.withdraw",actor:"service:ci-fixture.access"}
 ]);
 assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE subject_id='"+agentContact+"' ORDER BY rowid")),[
   {action:"contact_points.create",actor:"service:ci-fixture.access"},
   {action:"contact_points.verify",actor:"service:ci-fixture.access"}
 ]);
 console.log(JSON.stringify({event:"crm.native-r3-dev-agent-consent",result:"PASS",
   signed_service_admin:true,test_provenance:true,non_impersonation:true,
   grant_withdraw:true,replay:true,cas:true,actor_audit:true}));

 // Original crm-sales v2: generated binary Connect, original Deal aggregate,
// one tenant D1, exact money, CAS, receipt replay, and atomic stage->Activity.
const salesV2=signedReadOnly(salesV2Path,binarySalesV2,decodeSalesV2);
// The authoritative pipeline catalog is created before any new Deal write.
// Stage order is owner-assigned, never supplied as an untrusted rank.
const pipelineSeed={pipelineId:"pipeline.enterprise",name:"Enterprise",
  stages:[
    {stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
    {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500},
    {stageId:"won",name:"Historical terminal",probabilityBasisPoints:10000}
  ],precondition:{idempotencyKey:"connect-r7-pipeline-create-1"}};
assert.equal((await salesV2("CreatePipeline",viewer,pipelineSeed)).status,403);
assert.equal((await salesV2("CreatePipeline",admin,{
  ...pipelineSeed,stages:[pipelineSeed.stages[0],pipelineSeed.stages[0]],
  precondition:{idempotencyKey:"connect-r7-pipeline-duplicate-stage"}
})).status,400);
const pipelineCreated=await salesV2("CreatePipeline",admin,pipelineSeed);
assert.equal(pipelineCreated.status,200,describeProtobuf(pipelineCreated.body));
assert.equal(pipelineCreated.body.pipeline?.version,1n);
assert.deepEqual(pipelineCreated.body.pipeline?.stages.map(s=>s.ordinal),[1,2,3]);
const pipelineCreateProof=await salesV2("GetPipeline",viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"connect-r7-pipeline-create-1"
});
assert.equal(pipelineCreateProof.status,200,describeProtobuf(pipelineCreateProof.body));
assert.equal(pipelineCreateProof.body.confirmedMutation?.originalRequestId,
  pipelineCreated.body.receipt?.requestId);
assert.equal(pipelineCreateProof.body.confirmedMutation?.resourceVersion,1n);
assert.notEqual(pipelineCreateProof.body.confirmedMutation?.requestId,
  pipelineCreated.body.receipt?.requestId);
assert.equal((await sql("SELECT original_request_id FROM crm_sales_pipeline_receipts "+
  "WHERE tenant='tenant-a' AND key='connect-r7-pipeline-create-1'"))[0].original_request_id,
  pipelineCreated.body.receipt?.requestId);
assert.equal((await salesV2("GetPipeline",viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"short"
})).status,400);
assert.equal((await salesV2("CreatePipeline",admin,pipelineSeed)).body.receipt?.replayed,true);
assert.equal((await salesV2("CreatePipeline",admin,{...pipelineSeed,name:"modified"})).status,409);
assert.equal((await salesV2("GetPipeline",tenantB,{pipelineId:"pipeline.enterprise"})).status,404);
assert.equal((await salesV2("GetPipeline",viewer,{pipelineId:"pipeline.enterprise"})).status,200);
const pipelineOther=await salesV2("CreatePipeline",admin,{
  pipelineId:"pipeline.other",name:"Other",stages:[pipelineSeed.stages[0]],
  precondition:{idempotencyKey:"connect-r7-pipeline-create-2"}});
assert.equal(pipelineOther.status,200,describeProtobuf(pipelineOther.body));
const wrongPipelineProof=await salesV2("GetPipeline",viewer,{
  pipelineId:"pipeline.other",receiptIdempotencyKey:"connect-r7-pipeline-create-1"
});
assert.equal(wrongPipelineProof.status,200);
assert.equal(wrongPipelineProof.body.confirmedMutation,undefined,
  "A receipt for another pipeline must never be disclosed as commit evidence");
const pipelineList=await salesV2("ListPipelines",viewer,{page:{pageSize:1}});
assert.equal(pipelineList.status,200,describeProtobuf(pipelineList.body));
assert.equal(pipelineList.body.pipelines.length,1);
assert.ok(pipelineList.body.pageInfo?.nextPageToken);
const currencyRows=pipelineList.body.currencies;
assert.equal(currencyRows.length,165);
const currencyByCode=new Map(currencyRows.map(row=>[row.currencyCode,row.minorUnitExponent]));
assert.equal(currencyByCode.size,currencyRows.length);
for(const [code,exponent] of [["JPY",0],["USD",2],["EUR",2],["KWD",3],["CLF",4]])
  assert.equal(currencyByCode.get(code),exponent);
for(const code of ["ZZZ","XXX","XAU","BGN"])assert.equal(currencyByCode.has(code),false);
const pipelineNext=await salesV2("ListPipelines",viewer,{page:{pageSize:1,pageToken:pipelineList.body.pageInfo.nextPageToken}});
assert.equal(pipelineNext.status,200,describeProtobuf(pipelineNext.body));
assert.equal(pipelineNext.body.pipelines.length,1);
assert.equal(pipelineNext.body.currencies.length,0);
const pipelineUpdated=await salesV2("UpdatePipeline",admin,{
  pipelineId:"pipeline.enterprise",name:"Enterprise managed",
  stages:pipelineSeed.stages,archived:false,
  precondition:{expectedVersion:1n,idempotencyKey:"connect-r7-pipeline-rename-1"}});
assert.equal(pipelineUpdated.status,200,describeProtobuf(pipelineUpdated.body));
assert.equal(pipelineUpdated.body.pipeline?.version,2n);
const pipelineUpdateProof=await salesV2("GetPipeline",viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"connect-r7-pipeline-rename-1"
});
assert.equal(pipelineUpdateProof.status,200);
assert.equal(pipelineUpdateProof.body.confirmedMutation?.originalRequestId,
  pipelineUpdated.body.receipt?.requestId);
assert.equal(pipelineUpdateProof.body.confirmedMutation?.resourceVersion,2n);
const oldPipelineCreateProof=await salesV2("GetPipeline",viewer,{
  pipelineId:"pipeline.enterprise",receiptIdempotencyKey:"connect-r7-pipeline-create-1"
});
assert.equal(oldPipelineCreateProof.body.confirmedMutation?.originalRequestId,
  pipelineCreated.body.receipt?.requestId);
assert.equal(oldPipelineCreateProof.body.confirmedMutation?.resourceVersion,1n,
  "A later update must not overwrite immutable create evidence");
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id='"+
  pipelineUpdateProof.body.confirmedMutation?.requestId+"'"))[0].n,0,
  "Receipt lookup is read-only and must not append audit");
assert.equal((await salesV2("UpdatePipeline",admin,{
  pipelineId:"pipeline.enterprise",name:"stale",
  stages:pipelineSeed.stages,archived:false,
  precondition:{expectedVersion:1n,idempotencyKey:"connect-r7-pipeline-stale"}
})).status,409);
// R7.3 named views: one signed Rust owner, tenant D1, replay, CAS, archived
// visibility, negative inputs and atomic audit rollback.
const viewAId="sales-view-"+randomUUID();
const viewBId="sales-view-"+randomUUID();
const viewAInput={
  viewId:viewAId,name:"Open enterprise",pipelineId:"pipeline.enterprise",
  status:1,accountId,
  precondition:{idempotencyKey:"r73-view-create-1"}
};
assert.equal((await salesV2("CreateSalesView",viewer,viewAInput)).status,403);
assert.equal((await salesV2("CreateSalesView",admin,{
  ...viewAInput,status:0,precondition:{idempotencyKey:"r73-view-invalid-status"}
})).status,400);
assert.equal((await salesV2("CreateSalesView",admin,{
  ...viewAInput,pipelineId:"pipeline.unknown",
  precondition:{idempotencyKey:"r73-view-unknown-pipeline"}
})).status,404);
assert.equal((await salesV2("CreateSalesView",admin,{
  ...viewAInput,accountId:"bad/id",
  precondition:{idempotencyKey:"r73-view-invalid-account"}
})).status,400);
const viewCreated=await salesV2("CreateSalesView",admin,viewAInput);
assert.equal(viewCreated.status,200,describeProtobuf(viewCreated.body));
assert.equal(viewCreated.body.view?.version,1n);
const viewCreateProof=await salesV2("GetSalesView",viewer,{
  viewId:viewAId,receiptIdempotencyKey:"r73-view-create-1"
});
assert.equal(viewCreateProof.status,200);
assert.equal(viewCreateProof.body.confirmedMutation?.originalRequestId,
  viewCreated.body.receipt?.requestId);
assert.equal(viewCreateProof.body.confirmedMutation?.resourceVersion,1n);
assert.equal((await sql("SELECT original_request_id FROM crm_sales_saved_view_receipts "+
  "WHERE tenant='tenant-a' AND key='r73-view-create-1'"))[0].original_request_id,
  viewCreated.body.receipt?.requestId);
assert.equal(viewCreated.body.view?.status,1);
assert.equal(viewCreated.body.view?.accountId,accountId);
assert.equal((await salesV2("CreateSalesView",admin,viewAInput)).body.receipt?.replayed,true);
assert.equal((await salesV2("CreateSalesView",admin,{
  ...viewAInput,name:"different"
})).status,409);
assert.equal((await salesV2("GetSalesView",tenantB,{viewId:viewAId})).status,404);
assert.equal((await salesV2("ListSalesViews",tenantB,{page:{pageSize:10}})).body.views.length,0);
assert.equal((await salesV2("GetSalesView",viewer,{viewId:viewAId})).body.view?.viewId,viewAId);
const viewBCreated=await salesV2("CreateSalesView",admin,{
  viewId:viewBId,name:"Won across pipelines",status:2,
  precondition:{idempotencyKey:"r73-view-create-2"}
});
assert.equal(viewBCreated.status,200,describeProtobuf(viewBCreated.body));
const wrongViewProof=await salesV2("GetSalesView",viewer,{
  viewId:viewBId,receiptIdempotencyKey:"r73-view-create-1"
});
assert.equal(wrongViewProof.status,200);
assert.equal(wrongViewProof.body.confirmedMutation,undefined,
  "A receipt for another saved view must never be disclosed");
const firstViewsPage=await salesV2("ListSalesViews",viewer,{page:{pageSize:1}});
assert.equal(firstViewsPage.status,200,describeProtobuf(firstViewsPage.body));
assert.equal(firstViewsPage.body.views.length,1);
assert.ok(firstViewsPage.body.pageInfo?.nextPageToken);
const secondViewsPage=await salesV2("ListSalesViews",viewer,{
  page:{pageSize:1,pageToken:firstViewsPage.body.pageInfo.nextPageToken}
});
assert.equal(secondViewsPage.body.views.length,1);
assert.equal(secondViewsPage.body.pageInfo?.nextPageToken,"");
assert.deepEqual(new Set([firstViewsPage.body.views[0].viewId,secondViewsPage.body.views[0].viewId]),
  new Set([viewAId,viewBId]));
const viewUpdate={
  viewId:viewAId,name:"Won in enterprise",pipelineId:"pipeline.enterprise",
  status:2,precondition:{expectedVersion:1n,idempotencyKey:"r73-view-update-1"}
};
const updatedView=await salesV2("UpdateSalesView",admin,viewUpdate);
assert.equal(updatedView.status,200,describeProtobuf(updatedView.body));
assert.equal(updatedView.body.view?.version,2n);
const viewUpdateProof=await salesV2("GetSalesView",viewer,{
  viewId:viewAId,receiptIdempotencyKey:"r73-view-update-1"
});
assert.equal(viewUpdateProof.status,200);
assert.equal(viewUpdateProof.body.confirmedMutation?.originalRequestId,
  updatedView.body.receipt?.requestId);
assert.equal(viewUpdateProof.body.confirmedMutation?.resourceVersion,2n);
const oldViewCreateProof=await salesV2("GetSalesView",viewer,{
  viewId:viewAId,receiptIdempotencyKey:"r73-view-create-1"
});
assert.equal(oldViewCreateProof.body.confirmedMutation?.originalRequestId,
  viewCreated.body.receipt?.requestId);
assert.equal(oldViewCreateProof.body.confirmedMutation?.resourceVersion,1n);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id='"+
  viewUpdateProof.body.confirmedMutation?.requestId+"'"))[0].n,0,
  "Saved-view receipt lookup is read-only");
assert.equal(updatedView.body.view?.accountId,undefined);
assert.equal((await salesV2("UpdateSalesView",admin,viewUpdate)).body.receipt?.replayed,true);
assert.equal((await salesV2("UpdateSalesView",admin,{
  ...viewUpdate,name:"stale",precondition:{
    expectedVersion:1n,idempotencyKey:"r73-view-update-stale"}
})).status,409);
const beforeFailedView=(await sql("SELECT state,version FROM crm_sales_saved_view_records WHERE tenant='tenant-a' AND id='"+viewAId+"'"));
const beforeFailedReceipts=(await sql("SELECT COUNT(*) AS n FROM crm_sales_saved_view_receipts WHERE tenant='tenant-a'"))[0].n;
(await sql("CREATE TRIGGER fixture_fail_saved_view_audit BEFORE INSERT ON crm_audit "+
  "WHEN NEW.action='sales_view.update' AND NEW.subject_id='"+viewAId+"' "+
  "BEGIN SELECT RAISE(ABORT,'fixture view audit rollback'); END;"));
try {
  const failed=await salesV2("UpdateSalesView",admin,{
    ...viewUpdate,name:"Failed write",precondition:{
      expectedVersion:2n,idempotencyKey:"r73-view-audit-rollback"}
  });
  assertUnconfirmed(failed,"SALES_VIEW_WRITE_UNCONFIRMED");
  assert.deepEqual((await sql("SELECT state,version FROM crm_sales_saved_view_records "+
    "WHERE tenant='tenant-a' AND id='"+viewAId+"'")),beforeFailedView);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_saved_view_receipts "+
    "WHERE tenant='tenant-a'"))[0].n,beforeFailedReceipts);
} finally {await sql("DROP TRIGGER fixture_fail_saved_view_audit");}
const archivedView=await salesV2("UpdateSalesView",admin,{
  ...viewUpdate,archived:true,precondition:{
    expectedVersion:2n,idempotencyKey:"r73-view-archive"}
});
assert.equal(archivedView.status,200,describeProtobuf(archivedView.body));
assert.equal(archivedView.body.view?.archived,true);
assert.deepEqual((await salesV2("ListSalesViews",viewer,{})).body.views.map(x=>x.viewId),[viewBId]);
assert.equal((await salesV2("ListSalesViews",viewer,{includeArchived:true})).body.views.length,2);
assert.deepEqual((await sql("SELECT action,actor FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+viewAId+"' ORDER BY rowid")).map(x=>x.action),
  ["sales_view.create","sales_view.update","sales_view.update"]);
console.log(JSON.stringify({event:"crm.native-r73-saved-views",result:"PASS",
  typed_filters:true,tenant_isolation:true,keyset:true,cas:true,
  replay:true,signed_audit:true,archive:true,atomic_rollback:true}));
const dealId="sales-mid-"+randomUUID();
const stageLead={pipelineId:"pipeline.enterprise",stageId:"lead",ordinal:1};
const stageQualified={pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:2};
const saleCreate={dealId,name:"Connect Deal",accountId,
  stage:{...stageLead,ordinal:999},amount:{minorUnits:12500n,currencyCode:"USD"},
  probabilityBasisPoints:2500,primaryContactId:contactId,
  expectedCloseDate:{year:2027,month:2,day:28},
  precondition:{idempotencyKey:"connect-sales-create-main"}};
const foreignSaleContactId="sales-contact-foreign-"+randomUUID();
const foreignSaleContact=await contactV2("CreateContactPoint",admin,{
  contactPointId:foreignSaleContactId,partyId:pagingPersonId,kind:1,
  value:"sales-foreign@example.test",preferred:false,
  precondition:{idempotencyKey:"connect-r72-foreign-contact-create"}
});
assert.equal(foreignSaleContact.status,200,describeProtobuf(foreignSaleContact.body));
const denialAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0].n;
assert.equal((await salesV2("CreateDeal",null,saleCreate)).status,401);
assert.equal((await salesV2("CreateDeal",viewer,saleCreate)).status,403);
for(const currencyCode of ["ZZZ","XXX","XAU","BGN"]){
  const rejected=await salesV2("CreateDeal",admin,{
    ...saleCreate,amount:{minorUnits:1n,currencyCode},
    precondition:{idempotencyKey:"connect-sales-unsupported-currency-"+currencyCode}
  });
  assert.equal(rejected.status,400,describeProtobuf(rejected.body));
}
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,stage:undefined,
  precondition:{idempotencyKey:"connect-sales-no-stage"}})).status,400);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,accountId:"missing-account",
  precondition:{idempotencyKey:"connect-sales-no-account"}})).status,400);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,probabilityBasisPoints:10001,
  precondition:{idempotencyKey:"connect-sales-invalid-probability"}})).status,400);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,stage:{pipelineId:"pipeline.enterprise",stageId:"unknown",ordinal:1},
  precondition:{idempotencyKey:"connect-sales-invalid-catalog-stage"}})).status,400);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,precondition:{idempotencyKey:"connect-sales-create-invalid-version",
    expectedVersion:1n}})).status,400);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,primaryContactId:foreignSaleContactId,
  precondition:{idempotencyKey:"connect-r72-foreign-contact-create-denied"}
})).status,400,"a ContactPoint owned by another Party cannot be attached to this Account");
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,primaryContactId:"foreign-tenant-contact-id",
  precondition:{idempotencyKey:"connect-r72-missing-contact-denied"}
})).status,400);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,expectedCloseDate:{year:2027,month:2,day:29},
  precondition:{idempotencyKey:"connect-r72-bad-leapdate-denied"}
})).status,400,"calendar validation must be in original Rust, not a JS Date");
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0].n,denialAuditBefore,
  "invalid Contact/CalendarDate must not create a Deal or append audit");
const invalidDealName=await salesV2("CreateDeal",admin,{
  ...saleCreate,name:"",precondition:{idempotencyKey:"r471-invalid-deal-name"}});
assert.equal(invalidDealName.status,400);
const invalidNameDetail=decodeDomainErrorDetail(invalidDealName.body.details.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
assert.equal(invalidNameDetail.domainCode,"SALES_DEAL_NAME_INVALID");
assert.equal(invalidNameDetail.commitOutcome,1);
assert.equal(invalidNameDetail.requestId,invalidDealName.headers.get("x-request-id"));
assert.equal(invalidNameDetail.fieldViolations[0].fieldPath,"deal.name");
const salesCreated=await salesV2("CreateDeal",admin,saleCreate);
assert.equal(salesCreated.status,200,describeProtobuf(salesCreated.body));
assert.equal(salesCreated.body.deal?.dealId,dealId);
assert.equal(salesCreated.body.deal?.name,"Connect Deal");
assert.equal(salesCreated.body.deal?.stage?.ordinal,1,
  "The catalog, not caller ordinal 999, assigns the rank");
assert.equal(salesCreated.body.deal?.amount?.minorUnits,12500n);
assert.equal(salesCreated.body.deal?.amount?.currencyCode,"USD");
assert.equal(salesCreated.body.deal?.status,1);
assert.equal(salesCreated.body.deal?.primaryContactId,contactId);
assert.equal(salesCreated.body.deal?.expectedCloseDate?.year,2027);
assert.equal(salesCreated.body.deal?.expectedCloseDate?.month,2);
assert.equal(salesCreated.body.deal?.expectedCloseDate?.day,28);
assert.ok(salesCreated.body.deal?.createdAt);
assert.ok(salesCreated.body.deal?.updatedAt);
assert.equal(salesCreated.body.receipt?.resourceVersion,1n);
assert.equal(salesCreated.body.receipt?.replayed,false);
assert.equal(salesCreated.body.receipt?.requestId,salesCreated.headers.get("x-request-id"));
const saleReplayed=await salesV2("CreateDeal",admin,saleCreate);
assert.equal(saleReplayed.status,200);
assert.equal(saleReplayed.body.receipt?.replayed,true);
assert.equal(salesCreated.body.receipt?.originalRequestId,salesCreated.headers.get("x-request-id"));
assert.equal(saleReplayed.body.receipt?.originalRequestId,salesCreated.headers.get("x-request-id"));
assert.equal(saleReplayed.body.receipt?.requestId,saleReplayed.headers.get("x-request-id"));
assert.notEqual(saleReplayed.body.receipt?.requestId,saleReplayed.body.receipt?.originalRequestId);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,name:"changed idempotency payload"})).status,409);
assert.equal((await salesV2("GetDeal",tenantB,{dealId})).status,404);
assert.equal((await salesV2("GetDeal",viewer,{dealId})).body.deal?.dealId,dealId);
assert.equal((await salesV2("GetDeal",viewer,{dealId:"bad/invalid"})).status,400);
// Recovery reads the original receipt without executing a write.
const receiptRead=await salesV2("GetDeal",admin,{dealId,
  receiptIdempotencyKey:saleCreate.precondition.idempotencyKey});
assert.equal(receiptRead.status,200);
assert.equal(receiptRead.body.confirmedMutation?.originalRequestId,salesCreated.headers.get("x-request-id"));
assert.equal(receiptRead.body.confirmedMutation?.resourceVersion,1n);
assert.equal(Object.hasOwn(receiptRead.body.confirmedMutation,"replayed"),false);
assert.equal(receiptRead.body.confirmedMutation?.requestId,receiptRead.headers.get("x-request-id"));
assert.equal((await salesV2("GetDeal",admin,{dealId,receiptIdempotencyKey:"r471-unknown-key"})).body.confirmedMutation,undefined);
assert.equal((await salesV2("GetDeal",tenantB,{dealId,receiptIdempotencyKey:saleCreate.precondition.idempotencyKey})).status,404);
assert.equal((await salesV2("GetDeal",null,{dealId,receiptIdempotencyKey:saleCreate.precondition.idempotencyKey})).status,401);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id=?",
  receiptRead.headers.get("x-request-id")))[0].n,0);
// Opportunity Team Lead may close only a specifically granted Deal. The
// original Sales owner keeps authorization, attribution, CAS, receipts and audit.
{
  const id="opportunity-team-qa-"+randomUUID();
  const email="native-viewer@example.test";
  const creditTeamId="opportunity-credit-team-"+randomUUID();
  const creditTeamKey="opportunity-credit-team-"+creditTeamId;
  const creditTeam=await memberV2("CreateTeam",admin,{
    teamId:creditTeamId,name:"Opportunity credit QA "+creditTeamId,
    roster:{memberEmails:["native-a@example.test"]},
    precondition:{idempotencyKey:creditTeamKey}
  });
  assert.equal(creditTeam.status,200,describeProtobuf(creditTeam.body));
  const createQaDeal=async dealId=>salesV2("CreateDeal",admin,{
    ...saleCreate,dealId,
    owner:{owner:{case:"assigneeIdentity",value:"native-a@example.test"}},
    precondition:{idempotencyKey:"opportunity-create-"+dealId}
  });
  const grant=(dealId,role,version,key)=>({dealId,email,...(role===undefined?{}:{role}),
    precondition:{expectedVersion:version,idempotencyKey:"opportunity-"+key+"-"+dealId}});
  const edit=(dealId,version,key,fields={})=>({dealId,
    name:{operation:{case:"set",value:"Opportunity team "+key}},...fields,
    precondition:{expectedVersion:version,idempotencyKey:"opportunity-edit-"+key+"-"+dealId}});
  const closeInput=(dealId,version,key,fields={})=>({dealId,outcome:3,reasonCode:"qa",
    ...fields,precondition:{expectedVersion:version,idempotencyKey:"opportunity-close-"+key+"-"+dealId}});
  const created=await createQaDeal(id);
  assert.equal(created.status,200,describeProtobuf(created.body));
  assert.equal((await salesV2("UpdateDeal",viewer,edit(id,1n,"denied"))).status,403);
  assert.equal((await salesV2("SetOpportunityParticipant",viewer,grant(id,3,1n,"escalate"))).status,403);
  assert.equal((await salesV2("SetOpportunityParticipant",admin,grant(id,0,1n,"unknown"))).status,400);
  assert.equal((await salesV2("SetOpportunityParticipant",admin,{...grant(id,3,1n,"foreign"),email:"native-b@example.test"})).status,400);

  const observer=await salesV2("SetOpportunityParticipant",admin,grant(id,1,1n,"observer"));
  assert.equal(observer.status,200,describeProtobuf(observer.body));
  assert.equal(observer.body.deal.opportunityTeam[0].role,1);
  assert.equal((await salesV2("SetOpportunityParticipant",admin,grant(id,1,1n,"observer"))).body.receipt.replayed,true);
  assert.equal((await salesV2("SetOpportunityParticipant",admin,grant(id,3,1n,"stale"))).status,409);
  const assertProjection=async(actor,expected)=>{
    const details=await salesV2("GetDeal",actor,{dealId:id});
    assert.equal(details.status,200,describeProtobuf(details.body));
    assert.deepEqual(details.body.capabilities,expected);
    const list=await salesV2("ListDeals",actor,{search:id,page:{pageSize:25}});
    assert.equal(list.status,200,describeProtobuf(list.body));
    assert.deepEqual(list.body.deals,[details.body.deal]);
    assert.deepEqual(list.body.dealCapabilities.map(x=>x.dealId),[id]);
    assert.deepEqual(list.body.dealCapabilities[0].capabilities,expected);
    const stage=await salesV2("ListStageDeals",actor,{
      pipelineId:details.body.deal.stage.pipelineId,stageId:details.body.deal.stage.stageId,page:{pageSize:100}
    });
    assert.equal(stage.status,200,describeProtobuf(stage.body));
    assert.deepEqual(stage.body.dealCapabilities.map(x=>x.dealId),stage.body.deals.map(x=>x.dealId));
    assert.deepEqual(stage.body.dealCapabilities.find(x=>x.dealId===id)?.capabilities,expected);
  };
  const observerRead=(await salesV2("GetDeal",viewer,{dealId:id})).body;
  await assertProjection(viewer,observerRead.capabilities);
  assert.equal(observerRead.capabilities.close,false);
  assert.equal(observerRead.capabilities.manageCloseCredit,false);
  assert.equal((await salesV2("CloseDeal",viewer,closeInput(id,observer.body.deal.version,"observer"))).status,403);
  assert.equal((await salesV2("UpdateDeal",viewer,edit(id,2n,"observer"))).status,403);

  const contributor=await salesV2("SetOpportunityParticipant",admin,grant(id,2,2n,"contributor"));
  assert.equal(contributor.status,200,describeProtobuf(contributor.body));
  const contributorRead=(await salesV2("GetDeal",viewer,{dealId:id})).body;
  await assertProjection(viewer,contributorRead.capabilities);
  assert.equal(contributorRead.capabilities.edit,true);
  assert.equal(contributorRead.capabilities.editCommercial,false);
  assert.equal(contributorRead.capabilities.advance,false);
  assert.equal(contributorRead.capabilities.close,false);
  assert.equal(contributorRead.capabilities.manageOpportunityTeam,false);
  assert.equal((await salesV2("CloseDeal",viewer,closeInput(id,contributor.body.deal.version,"contributor"))).status,403);
  assert.equal((await salesV2("UpdateDeal",viewer,edit(id,3n,"money",{probabilityBasisPoints:1200}))).status,403);
  assert.equal((await salesV2("UpdateDeal",viewer,edit(id,3n,"owner",{owner:{owner:{case:"assigneeIdentity",value:email}}}))).status,403);
  const command=edit(id,3n,"allowed");
  const edited=await salesV2("UpdateDeal",viewer,command);
  assert.equal(edited.status,200,describeProtobuf(edited.body));
  assert.equal(edited.body.deal.version,4n);
  assert.equal((await salesV2("UpdateDeal",viewer,command)).body.receipt.replayed,true);
  assert.equal((await salesV2("UpdateDeal",viewer,{...command,dealId:"another-deal"})).status,403);

  const lead=await salesV2("SetOpportunityParticipant",admin,grant(id,3,4n,"lead"));
  assert.equal(lead.status,200,describeProtobuf(lead.body));
  const leadRead=(await salesV2("GetDeal",viewer,{dealId:id})).body;
  await assertProjection(viewer,leadRead.capabilities);
  assert.equal(leadRead.capabilities.advance,true);
  assert.equal(leadRead.capabilities.close,true);
  assert.equal(leadRead.capabilities.manageCloseCredit,false);
  const adminRead=(await salesV2("GetDeal",admin,{dealId:id})).body;
  await assertProjection(admin,adminRead.capabilities);
  assert.equal(adminRead.capabilities.close,true);
  assert.equal(adminRead.capabilities.manageCloseCredit,true);
  const commercial=await salesV2("UpdateDeal",viewer,edit(id,5n,"commercial",{probabilityBasisPoints:3300}));
  assert.equal(commercial.status,200,describeProtobuf(commercial.body));
  assert.equal(commercial.body.deal.probabilityBasisPoints,3300);

  const automaticSelection={};
  const automaticPreview=await salesV2("PreviewDealCloseCredit",viewer,{
    dealId:id,expectedVersion:6n,selection:automaticSelection
  });
  assert.equal(automaticPreview.status,200,describeProtobuf(automaticPreview.body));
  assert.ok(automaticPreview.body.eligibleTeams.some(team=>team.id===creditTeamId));
  assert.equal(automaticPreview.body.creditedTeam?.id,creditTeamId);
  const manualSelection={leaveTeamUnallocated:true};
  const manualPreview=await salesV2("PreviewDealCloseCredit",viewer,{
    dealId:id,expectedVersion:6n,selection:manualSelection
  });
  assert.equal(manualPreview.status,200,describeProtobuf(manualPreview.body));
  assert.equal(manualPreview.body.creditedTeam,undefined);
  assert.equal((await salesV2("CloseDeal",viewer,closeInput(id,6n,"manual-denied",{
    creditSelection:manualSelection
  }))).status,403);

  const leadCloseInput=closeInput(id,6n,"lead",{
    creditSelection:automaticSelection,
    expectedCreditDecision:automaticPreview.body.expectedDecision
  });
  const leadClose=await salesV2("CloseDeal",viewer,leadCloseInput);
  assert.equal(leadClose.status,200,describeProtobuf(leadClose.body));
  assert.equal(leadClose.body.deal.version,7n);
  assert.equal(leadClose.body.deal.status,3);
  assert.equal(leadClose.body.deal.commercialCloseFact?.creditedTeam?.id,creditTeamId);
  assert.equal(leadClose.body.receipt?.replayed,false);
  assert.equal((await salesV2("CloseDeal",viewer,leadCloseInput)).body.receipt?.replayed,true);
  assert.equal((await salesV2("CloseDeal",admin,leadCloseInput)).status,409,
    "an administrator must not replay a Lead-scoped close receipt");
  assert.equal((await salesV2("ReopenDeal",viewer,{
    dealId:id,reasonCode:"qa.reopen",
    precondition:{expectedVersion:7n,idempotencyKey:"opportunity-reopen-denied-"+id}
  })).status,403);
  const reopened=await salesV2("ReopenDeal",admin,{
    dealId:id,reasonCode:"qa.reopen",
    precondition:{expectedVersion:7n,idempotencyKey:"opportunity-reopen-"+id}
  });
  assert.equal(reopened.status,200,describeProtobuf(reopened.body));
  assert.equal(reopened.body.deal.version,8n);
  assert.equal(reopened.body.deal.status,1);

  const adminManualPreview=await salesV2("PreviewDealCloseCredit",admin,{
    dealId:id,expectedVersion:8n,selection:manualSelection
  });
  assert.equal(adminManualPreview.status,200,describeProtobuf(adminManualPreview.body));
  assert.equal(adminManualPreview.body.creditedTeam,undefined);
  const adminManualClose=await salesV2("CloseDeal",admin,closeInput(id,8n,"admin-manual",{
    creditSelection:manualSelection,
    expectedCreditDecision:adminManualPreview.body.expectedDecision
  }));
  assert.equal(adminManualClose.status,200,describeProtobuf(adminManualClose.body));
  assert.equal(adminManualClose.body.deal.version,9n);
  assert.equal(adminManualClose.body.deal.commercialCloseFact?.creditedTeam,undefined);
  const revoke=await salesV2("SetOpportunityParticipant",admin,grant(id,undefined,9n,"revoke"));
  assert.equal(revoke.status,200,describeProtobuf(revoke.body));
  assert.equal(revoke.body.deal.opportunityTeam.length,0);
  await assertProjection(viewer,(await salesV2("GetDeal",viewer,{dealId:id})).body.capabilities);
  assert.equal((await salesV2("CloseDeal",viewer,leadCloseInput)).status,403);
  assert.equal((await salesV2("UpdateDeal",viewer,command)).status,403);
  assert.equal((await salesV2("UpdateDeal",viewer,edit(id,10n,"revoked"))).status,403);
  assert.equal((await salesV2("CloseDeal",tenantB,closeInput(id,9n,"foreign-tenant"))).status,404);
  const events=(await salesV2("ListDealEvents",viewer,{dealId:id,page:{pageSize:25}})).body.events;
  assert.deepEqual(events.filter(e=>e.opportunityTeamAfter).map(e=>e.opportunityTeamAfter.participants.map(m=>m.role)),[[],[3],[2],[1]]);
  assert.equal(events.filter(e=>e.kind===4).length,2);
  assert.equal(events.filter(e=>e.kind===5).length,1);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.opportunity_team'","tenant-a",id))[0].n,4);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.close'","tenant-a",id))[0].n,2);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.reopen'","tenant-a",id))[0].n,1);

  // An ungranted Deal and a revoked grant cannot use the SelfWrite transport.
  const ungrantedId="opportunity-ungranted-"+randomUUID();
  const ungranted=await createQaDeal(ungrantedId);
  assert.equal(ungranted.status,200,describeProtobuf(ungranted.body));
  assert.equal((await salesV2("CloseDeal",viewer,closeInput(ungrantedId,1n,"ungranted"))).status,403);
  const revokedId="opportunity-revoked-"+randomUUID();
  const revoked=await createQaDeal(revokedId);
  assert.equal(revoked.status,200,describeProtobuf(revoked.body));
  const revokedGrant=await salesV2("SetOpportunityParticipant",admin,grant(revokedId,3,1n,"revoked-grant"));
  assert.equal(revokedGrant.status,200,describeProtobuf(revokedGrant.body));
  const revokedRemoval=await salesV2("SetOpportunityParticipant",admin,grant(revokedId,undefined,2n,"revoked-removal"));
  assert.equal(revokedRemoval.status,200,describeProtobuf(revokedRemoval.body));
  const revokedCloseKey="opportunity-close-revoked-"+revokedId;
  assert.equal((await salesV2("CloseDeal",viewer,closeInput(revokedId,3n,"revoked"))).status,403);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",revokedCloseKey))[0].n,0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.close'","tenant-a",revokedId))[0].n,0);

  // Enrollment version is pinned in the grant; reactivation cannot restore it.
  const versionId="opportunity-member-version-"+randomUUID();
  const versionDeal=await createQaDeal(versionId);
  assert.equal(versionDeal.status,200,describeProtobuf(versionDeal.body));
  const versionGrant=await salesV2("SetOpportunityParticipant",admin,grant(versionId,3,1n,"version-grant"));
  assert.equal(versionGrant.status,200,describeProtobuf(versionGrant.body));
  const memberRow=(await sql("SELECT version FROM crm_users WHERE tenant_id=? AND email=?","tenant-a",email))[0];
  const originalMemberVersion=Number(memberRow.version);
  assert.ok(Number.isSafeInteger(originalMemberVersion)&&originalMemberVersion>0);
  await sql("UPDATE crm_users SET active=0,version=version+1 WHERE tenant_id=? AND email=?","tenant-a",email);
  await sql("UPDATE crm_users SET active=1,version=version+1 WHERE tenant_id=? AND email=?","tenant-a",email);
  const staleMemberRead=(await salesV2("GetDeal",viewer,{dealId:versionId})).body;
  assert.equal(staleMemberRead.capabilities.close,false);
  assert.equal((await salesV2("CloseDeal",viewer,closeInput(versionId,2n,"stale-member"))).status,403);
  const staleGrantRemoval=await salesV2("SetOpportunityParticipant",admin,grant(versionId,undefined,2n,"stale-member-revoke"));
  assert.equal(staleGrantRemoval.status,200,describeProtobuf(staleGrantRemoval.body));
  await sql("UPDATE crm_users SET version=?,active=1 WHERE tenant_id=? AND email=?",originalMemberVersion,"tenant-a",email);

  // A concurrent CloseDeal and grant revocation contend on the same original
  // Deal version and the same version-pinned D1 predicate. Exactly one wins.
  const raceId="opportunity-close-revoke-race-"+randomUUID();
  const raceDeal=await createQaDeal(raceId);
  assert.equal(raceDeal.status,200,describeProtobuf(raceDeal.body));
  const raceGrant=await salesV2("SetOpportunityParticipant",admin,grant(raceId,3,1n,"race-grant"));
  assert.equal(raceGrant.status,200,describeProtobuf(raceGrant.body));
  const raceCloseKey="opportunity-close-race-"+raceId;
  const [raceClose,raceRevoke]=await Promise.all([
    salesV2("CloseDeal",viewer,closeInput(raceId,2n,"race")),
    salesV2("SetOpportunityParticipant",admin,grant(raceId,undefined,2n,"race-revoke"))
  ]);
  const closeWon=raceClose.status===200;
  const revokeWon=raceRevoke.status===200;
  assert.notEqual(closeWon,revokeWon,"close and revocation must not both commit or both lose");
  if(closeWon) assert.equal(raceRevoke.status,409);
  else assert.ok([403,409].includes(raceClose.status));
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?","tenant-a",raceCloseKey))[0].n,closeWon?1:0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_sales_deal_events WHERE tenant=? AND deal_id=? AND action='closed'","tenant-a",raceId))[0].n,closeWon?1:0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.close'","tenant-a",raceId))[0].n,closeWon?1:0);
  assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action='sales_deals.opportunity_team'","tenant-a",raceId))[0].n,revokeWon?2:1);

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
const updateSale={dealId,
  name:{operation:{case:"set",value:"Updated Connect Deal"}},
  amount:{operation:{case:"set",value:{minorUnits:98765n,currencyCode:"PLN"}}},
  probabilityBasisPoints:6750,
  primaryContactId:{operation:{case:"clear",value:true}},
  expectedCloseDate:{operation:{case:"set",value:{year:2028,month:2,day:29}}},
  precondition:{expectedVersion:1n,idempotencyKey:"connect-sales-update-main"}};
assert.equal((await salesV2("UpdateDeal",viewer,updateSale)).status,403);
assert.equal((await salesV2("UpdateDeal",admin,{
  ...updateSale,precondition:{expectedVersion:100n,
    idempotencyKey:"connect-sales-stale-version"}})).status,409);
const saleUpdated=await salesV2("UpdateDeal",knownService,updateSale);
assert.equal(saleUpdated.status,200,describeProtobuf(saleUpdated.body));
assert.equal(saleUpdated.body.deal?.name,"Updated Connect Deal");
assert.equal(saleUpdated.body.deal?.amount?.minorUnits,98765n);
assert.equal(saleUpdated.body.deal?.amount?.currencyCode,"PLN");
assert.equal(saleUpdated.body.deal?.probabilityBasisPoints,6750);
assert.equal(saleUpdated.body.deal?.primaryContactId,"");
assert.equal(saleUpdated.body.deal?.expectedCloseDate?.year,2028);
assert.equal(saleUpdated.body.deal?.expectedCloseDate?.month,2);
assert.equal(saleUpdated.body.deal?.expectedCloseDate?.day,29);
assert.ok(saleUpdated.body.deal?.updatedAt);
assert.equal(saleUpdated.body.receipt?.resourceVersion,2n);
assert.equal(saleUpdated.body.receipt?.replayed,false);
// A later record version is not the original command's receipt version.
const originalAfterUpdate=await salesV2("GetDeal",admin,{dealId,
  receiptIdempotencyKey:saleCreate.precondition.idempotencyKey});
assert.equal(originalAfterUpdate.status,200);
assert.equal(originalAfterUpdate.body.deal?.version,2n);
assert.equal(originalAfterUpdate.body.confirmedMutation?.resourceVersion,1n);
assert.equal(originalAfterUpdate.body.confirmedMutation?.originalRequestId,salesCreated.headers.get("x-request-id"));
const saleUpdateReplayed=await salesV2("UpdateDeal",admin,updateSale);
assert.equal(saleUpdateReplayed.status,200);
assert.equal(saleUpdateReplayed.body.receipt?.replayed,true);
assert.equal(saleUpdateReplayed.body.receipt?.originalRequestId,saleUpdated.headers.get("x-request-id"));
assert.equal((await salesV2("UpdateDeal",admin,{
  ...updateSale,probabilityBasisPoints:100})).status,409);
assert.equal((await salesV2("UpdateDeal",admin,{
  dealId,primaryContactId:{operation:{case:"set",value:foreignSaleContactId}},
  precondition:{expectedVersion:2n,idempotencyKey:"connect-r72-foreign-contact-update-denied"}
})).status,400);
assert.equal((await salesV2("UpdateDeal",admin,{
  dealId,expectedCloseDate:{operation:{case:"set",value:{year:2027,month:13,day:1}}},
  precondition:{expectedVersion:2n,idempotencyKey:"connect-r72-invalid-month-update"}
})).status,400);
assert.equal((await salesV2("GetDeal",viewer,{dealId})).body.deal?.version,2n);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0].n,2,
  "failed sales contact/date updates do not append audit");
console.log(JSON.stringify({event:"crm.native-r72-linked-contact-date",result:"PASS",
  create_contact:true,create_expected_close_date:true,update_contact_clear:true,
  leap_date_validated_in_rust:true,wrong_account_rejected:true,
  missing_contact_rejected:true,cas_and_replay:true,audit_rollback:true}));
const advanceSale={dealId,targetStage:stageQualified,targetStatus:1,
  precondition:{expectedVersion:2n,idempotencyKey:"connect-sales-advance-main"}};
assert.equal((await salesV2("AdvanceStage",viewer,advanceSale)).status,403);
assert.equal((await salesV2("AdvanceStage",admin,{
  ...advanceSale,targetStage:stageLead,
  precondition:{expectedVersion:2n,idempotencyKey:"connect-sales-regress"}})).status,400);
const taskCountBefore=(await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0].n;
const advancedSale=await salesV2("AdvanceStage",admin,advanceSale);
assert.equal(advancedSale.status,200,describeProtobuf(advancedSale.body));
assert.equal(advancedSale.body.deal?.stage?.stageId,"qualified");
assert.equal(advancedSale.body.deal?.version,3n);
assert.equal(advancedSale.body.receipt?.resourceVersion,3n);
assert.equal(advancedSale.body.receipt?.replayed,false);
const taskCountAfter=(await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0].n;
assert.equal(taskCountAfter,taskCountBefore+1,
  "original Sales stage event must create one Activities Task atomically");
const advanceReplayed=await salesV2("AdvanceStage",admin,advanceSale);
assert.equal(advanceReplayed.status,200);
assert.equal(advanceReplayed.body.receipt?.replayed,true);
assert.equal(advanceReplayed.body.receipt?.originalRequestId,advancedSale.headers.get("x-request-id"));
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0].n,taskCountAfter);
// Signed workerd cross-owner operational check: a real stage move delivered
// one original Task. Only its OPEN status and authoritative dueAt may mark
// the Deal as having an overdue linked action.
const movedEvent=(await salesV2("ListDealEvents",viewer,{
  dealId,page:{pageSize:5}})).body.events[0];
assert.ok(movedEvent.linkedTaskId);
const overdueLinkedTask=await taskV2("UpdateTask",admin,{
  taskId:movedEvent.linkedTaskId,
  dueAt:{operation:{case:"set",value:{seconds:1577836800n,nanos:0}}},
  precondition:{expectedVersion:1n,idempotencyKey:"r73b-linked-task-overdue"}
});
assert.equal(overdueLinkedTask.status,200,describeProtobuf(overdueLinkedTask.body));
const operationalRead=await salesV2("GetPipelineMetrics",viewer,{
  pipelineId:"pipeline.enterprise",status:1,accountId});
assert.equal(operationalRead.status,200,describeProtobuf(operationalRead.body));
const qualifiedOp=operationalRead.body.operational.find(x=>x.stageId==="qualified");
assert.ok(qualifiedOp,"the moved OPEN Deal belongs to qualified");
assert.equal(qualifiedOp.overdueLinkedTaskCount,1n);
assert.equal(qualifiedOp.knownStageAgeCount,1n);
assert.equal((await salesV2("AdvanceStage",admin,{
  dealId,targetStage:{pipelineId:"pipeline.enterprise",stageId:"fictional-won",ordinal:3},
  targetStatus:2,closeReasonCode:"contract-signed",
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-fake-stage-close"}
})).status,400,"closing cannot fabricate a terminal stage");
assert.equal((await salesV2("CloseDeal",admin,{
  dealId,outcome:2,
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-missing-close-reason"}
})).status,400,"closing requires an explicit reason");
assert.equal((await salesV2("GetDeal",viewer,{dealId})).body.deal?.version,3n,
  "rejected synthetic stage and missing reason cannot change the open deal");
const closeSale={dealId,outcome:2,reasonCode:"contract-signed",
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-close-main"}};
const closedSale=await salesV2("CloseDeal",admin,closeSale);
assert.equal(closedSale.status,200,describeProtobuf(closedSale.body));
assert.equal(closedSale.body.deal?.status,2);
assert.equal(closedSale.body.deal?.stage?.stageId,"qualified",
  "closing must preserve the real current stage");
assert.equal(closedSale.body.deal?.version,4n);
assert.equal((await salesV2("UpdateDeal",admin,{
  dealId,name:{operation:{case:"set",value:"Forbidden post-close edit"}},
  precondition:{expectedVersion:4n,idempotencyKey:"connect-sales-update-closed"}
})).status,409,"closed Deal must be immutable");
assert.equal((await salesV2("CloseDeal",admin,{
  dealId,outcome:2,reasonCode:"contract-signed",
  precondition:{expectedVersion:4n,idempotencyKey:"connect-sales-double-close"}
})).status,409,"double-close must be rejected");
assert.ok(closedSale.body.deal?.closedAt);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'"))[0].n,taskCountAfter,
  "Won closure must not create a second follow-up Task");
assert.equal((await salesV2("CloseDeal",admin,closeSale))
  .body.receipt?.replayed,true);
assert.equal((await salesV2("GetPipeline",viewer,{pipelineId:"pipeline.enterprise"}))
  .body.pipeline?.version,2n);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records "+
  "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0].n,2,
  "two original one-batch Sales->Activities delivery receipts");
const dealHistory=await salesV2("ListDealEvents",viewer,{dealId,page:{pageSize:25}});
assert.equal(dealHistory.status,200,describeProtobuf(dealHistory.body));
assert.deepEqual(dealHistory.body.events.map(e=>e.dealVersion),[4n,3n,2n,1n]);
assert.deepEqual(dealHistory.body.events.map(e=>e.kind),[4,3,2,1],
  "Append-only lifecycle history must be ordered and typed");
assert.equal(dealHistory.body.events[0].reasonCode,"contract-signed");
assert.equal(dealHistory.body.events[1].linkedTaskId?.length>0,true);
assert.equal(dealHistory.body.events[0].linkedTaskId,"",
  "Close does not invent a follow-up Task");
assert.equal(dealHistory.body.events[0].actor,"native-a@example.test");
assert.equal(dealHistory.body.events[0].requestId,closedSale.headers.get("x-request-id"));
assert.equal((await salesV2("ListDealEvents",viewer,{dealId,
  page:{pageSize:2}})).body.pageInfo?.nextPageToken,"3");
assert.deepEqual((await salesV2("ListDealEvents",viewer,{dealId,
  page:{pageSize:2,pageToken:"3"}})).body.events.map(e=>e.dealVersion),[2n,1n]);
assert.equal((await salesV2("ListDealEvents",viewer,{dealId,
  page:{pageSize:2,pageToken:"not-an-integer"}})).status,400);
assert.equal((await salesV2("ListDealEvents",viewer,{
  dealId:"missing-"+randomUUID(),page:{pageSize:1}})).status,404);

const dealOther="sales-aaa-other-"+randomUUID();
const dealLast="sales-zzz-ours-"+randomUUID();
for(const [index,nextId,pipe] of [
  [0,dealOther,"pipeline.other"],[1,dealLast,"pipeline.enterprise"]
]){
  const created=await salesV2("CreateDeal",admin,{
    dealId:nextId,name:"Sales page "+index,accountId,
    stage:{pipelineId:pipe,stageId:"lead",ordinal:1},
    probabilityBasisPoints:1000,
    precondition:{idempotencyKey:index===0?"r471-ci-ack-lost":"connect-sales-paging-"+index}});
  assert.equal(created.status,200,describeProtobuf(created.body));
  if(index===0){
    assert.equal(created.body.receipt.replayed,true);
    assert.equal(created.body.receipt.originalRequestId,created.headers.get("x-request-id"));
    assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?",
      "tenant-a",nextId))[0].n,1);
    console.log(JSON.stringify({event:"crm.native.s1-ack-reconciled",result:"PASS",receipt:true,single_audit:true}));
  }
}
assert.equal((await salesV2("ListDeals",viewer,{page:{pageSize:101}})).status,400);
assert.equal((await salesV2("ListDeals",viewer,{status:0})).status,400);
assert.equal((await salesV2("ListDeals",tenantB,{page:{pageSize:1}}))
  .body.deals.length,0);
const wonList=await collectPages(salesV2,"ListDeals","deals",{status:2});
assert.deepEqual(wonList.map(x=>x.dealId),[dealId]);
const filteredSales=await collectPages(salesV2,"ListDeals","deals",{
  pipelineId:"pipeline.enterprise"});
assert.deepEqual(filteredSales.map(x=>x.dealId),[dealId,dealLast]);
assert.equal((await collectPages(salesV2,"ListDeals","deals",{
  pipelineId:"pipeline.other"})).length,1);
// M2: Name or ID lookup must be tenant-bound and executed BEFORE LIMIT/cursor.
// It is not a separate read engine and never mutates audit/receipts.
const beforeDealLookupAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
const searchSales=await collectPages(salesV2,"ListDeals","deals",{
  search:"Sales page",page:{pageSize:1}});
assert.deepEqual(searchSales.map(x=>x.dealId),[dealOther,dealLast],
  "both matching Deals across pipelines must survive pageSize=1 and keyset cursor");
assert.deepEqual((await collectPages(salesV2,"ListDeals","deals",{
  search:"  Sales page 1  ",pipelineId:"pipeline.enterprise",page:{pageSize:1}}))
  .map(x=>x.dealId),[dealLast],
  "server trims search and composes pipeline before LIMIT");
assert.deepEqual((await collectPages(salesV2,"ListDeals","deals",{
  search:"Updated Connect Deal",status:2,accountId,page:{pageSize:1}}))
  .map(x=>x.dealId),[dealId],
  "search intersects exact status and Account filters");
assert.deepEqual((await collectPages(salesV2,"ListDeals","deals",{
  search:dealOther,page:{pageSize:1}})).map(x=>x.dealId),[dealOther],
  "exact ID fragments also find the owner-authorized Deal");
for(const literal of ["%","_","'","Несуществующая сделка"]){
  assert.equal((await salesV2("ListDeals",viewer,{search:literal,page:{pageSize:1}}))
    .body.deals.length,0,"special search characters must be literal, not SQL wildcards");
}
assert.equal((await salesV2("ListDeals",viewer,{search:"X".repeat(201)})).status,400);
assert.equal((await salesV2("ListDeals",viewer,{search:"X".repeat(200)})).status,200);
assert.equal((await salesV2("ListDeals",tenantB,{search:"Sales page"})).body.deals.length,0);
assert.equal((await salesV2("ListDeals",null,{search:"Sales page"})).status,401);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
  beforeDealLookupAudit,"Deal search and its negative cases are read-only");
console.log(JSON.stringify({event:"crm.native-m2-deal-name-search",result:"PASS",
  typed:true,server_before_limit:true,literal:true,tenant:true,readonly:true}));
// A non-UI test pipeline has more than one board page. The server count must
// be 27, not the 25 records rendered in a stage column.
for(let index=0;index<26;index++){
  const created=await salesV2("CreateDeal",admin,{
    dealId:"r7-overflow-"+String(index).padStart(2,"0"),name:"Pagination Deal "+index,
    accountId,stage:{pipelineId:"pipeline.other",stageId:"lead",ordinal:1},
    probabilityBasisPoints:1000,
    precondition:{idempotencyKey:"r7-stage-pagination-"+index}
  });
  assert.equal(created.status,200,describeProtobuf(created.body));
}
const overflow=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:25}});
assert.equal(overflow.status,200,describeProtobuf(overflow.body));
assert.equal(overflow.body.totalCount,27n,"board count must include all 27, not the page");
assert.equal(overflow.body.deals.length,25);
const ownerAccountName=(await accountV2("GetAccount",viewer,{accountId})).body.account.name;
assert.deepEqual(Object.entries(overflow.body.accountNames),[[accountId,ownerAccountName]],
  "25 cards sharing an Account carry one canonical name");
const salesNamedPage=await salesV2("ListDeals",viewer,{accountId,page:{pageSize:25}});
assert.equal(salesNamedPage.status,200);
assert.deepEqual(Object.entries(salesNamedPage.body.accountNames),[[accountId,ownerAccountName]]);
assert.deepEqual((await salesV2("ListDeals",admin,{accountId,page:{pageSize:25}})).body.accountNames,
  salesNamedPage.body.accountNames);
assert.deepEqual(Object.keys((await salesV2("ListDeals",tenantB,{accountId})).body.accountNames),[]);
assert.deepEqual(overflow.body.currencyTotals,[],
  "missing optional Money is not zero currency or an invented total");
assert.ok(overflow.body.pageInfo?.nextPageToken);
const overflowLast=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",
  page:{pageSize:25,pageToken:overflow.body.pageInfo.nextPageToken}});
assert.equal(overflowLast.status,200);
assert.equal(overflowLast.body.deals.length,2);
assert.equal(overflowLast.body.totalCount,27n);
assert.deepEqual(overflowLast.body.accountNames,overflow.body.accountNames);
assert.deepEqual(overflowLast.body.currencyTotals,overflow.body.currencyTotals);
const stageCount=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.enterprise",stageId:"lead",page:{pageSize:1}});
assert.equal(stageCount.status,200,describeProtobuf(stageCount.body));
assert.equal(stageCount.body.totalCount,1n);
assert.equal(stageCount.body.deals.length,1);
assert.equal(stageCount.body.deals[0]?.dealId,dealLast);
assert.deepEqual(stageCount.body.currencyTotals,[]);
const qualifiedTotals=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.enterprise",stageId:"qualified",page:{pageSize:1}});
assert.equal(qualifiedTotals.status,200,describeProtobuf(qualifiedTotals.body));
assert.deepEqual(qualifiedTotals.body.currencyTotals.map(t=>[
  t.currencyCode,t.totalMinorUnits,t.amountDealCount
]),[["PLN","98765",1n]],
  "closed money is included unless the caller selects a status filter");
assert.deepEqual((await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.enterprise",stageId:"qualified",status:1
})).body.currencyTotals,[],
  "status filter is applied to BOTH stage counts and summaries");

// Five monetary Deals are added AFTER the 27-row pagination fixture:
// 2 USD amounts exceed int64 when aggregated, while individually valid.
// USD, PLN, JPY and KWD must never be added across currencies.
const currencyCases=[
  ["USD",9223372036854775807n],
  ["USD",1n],
  ["PLN",2345n],
  ["JPY",345n],
  ["KWD",4567n],
];
for(const [index,[currencyCode,minorUnits]] of currencyCases.entries()){
  const made=await salesV2("CreateDeal",admin,{
    dealId:"r7-totals-"+String(index).padStart(2,"0"),
    name:"Currency Deal "+index,accountId,
    stage:{pipelineId:"pipeline.other",stageId:"lead",ordinal:999},
    amount:{minorUnits,currencyCode},probabilityBasisPoints:2000,
    ...(index===0?{expectedCloseDate:{year:2020,month:1,day:1}}:
      index===1?{expectedCloseDate:{year:2030,month:1,day:1}}:{}),
    precondition:{idempotencyKey:"r7-stage-totals-"+index}
  });
  assert.equal(made.status,200,describeProtobuf(made.body));
}
const totalView=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:1}});
assert.equal(totalView.status,200,describeProtobuf(totalView.body));
assert.equal(totalView.body.totalCount,32n);
assert.equal(totalView.body.deals.length,1);
assert.ok(totalView.body.pageInfo?.nextPageToken);
assert.deepEqual(totalView.body.currencyTotals.map(t=>[
  t.currencyCode,t.totalMinorUnits,t.amountDealCount
]),[
  ["JPY","345",1n],["KWD","4567",1n],["PLN","2345",1n],
  ["USD","9223372036854775808",2n]
], "full-stage exact totals must not overflow int64 or mix currencies");
const initialMetric=await salesV2("GetPipelineMetrics",viewer,{
  pipelineId:"pipeline.other"});
assert.equal(initialMetric.status,200,describeProtobuf(initialMetric.body));
assert.equal(initialMetric.body.totalDealCount,32n);
assert.deepEqual(initialMetric.body.statuses.map(x=>[x.stageId,x.status,x.dealCount]),
  [["lead",1,32n]],"metrics must count all Deals beyond one 25-row page");
assert.deepEqual(initialMetric.body.currencies.map(x=>[
  x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
]),[
  ["JPY","345","690000",1n],
  ["KWD","4567","9134000",1n],
  ["PLN","2345","4690000",1n],
  ["USD","9223372036854775808","18446744073709551616000",2n]
], "exact 128-bit sums, exact weighted numerator and currency separation");
// Dates are evaluated in UTC against one server snapshot, not by current
// browser page or client timezone. The original 27 Deals have no expected date.
const operational=initialMetric.body.operational;
assert.equal(operational.length,1);
assert.equal(operational[0].stageId,"lead");
assert.equal(operational[0].status,1);
assert.equal(operational[0].overdueExpectedCloseCount,1n);
assert.equal(operational[0].dueNext30DaysCount,0n);
assert.equal(operational[0].dueLaterCount,1n);
assert.equal(operational[0].noExpectedCloseCount,30n);
assert.equal(operational[0].knownStageAgeCount,32n);
assert.ok(BigInt(operational[0].totalStageAgeSeconds)>=0n);
assert.equal(operational[0].overdueLinkedTaskCount,0n);
assert.ok(initialMetric.body.asOfUnixSeconds>0n);
const metricsTenantDenied=await salesV2("GetPipelineMetrics",tenantB,{
  pipelineId:"pipeline.other"});
assert.equal(metricsTenantDenied.status,404,"other tenant cannot read foreign Pipeline totals");
const portfolioAudit=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n;
const portfolioRequest={pipelineId:"pipeline.other"};
const portfolio=(await salesV2("GetSalesPortfolio",viewer,portfolioRequest));
assert.equal(portfolio.status,200,describeProtobuf(portfolio.body));
assert.equal(portfolio.body.counts.pipelineCount,1n);
assert.equal(portfolio.body.counts.totalDealCount,32n);
assert.equal(portfolio.body.counts.openCount,32n);
assert.equal(portfolio.body.counts.noExpectedCloseCount,30n);
assert.equal(portfolio.body.counts.overdueExpectedCloseCount,1n);
assert.deepEqual(portfolio.body.pipelines[0].counts,portfolio.body.counts);
assert.equal(portfolio.body.pipelines[0].stages[0].dealCount,32n);
assert.equal(portfolio.body.pipelines[0].stages[0].stageId,"lead");
assert.ok(portfolio.body.pipelines[0].stages[0].averageStageAgeDays>=0n);
assert.deepEqual(portfolio.body.forecastCurrencies.map(x=>[
  x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
]),initialMetric.body.currencies.map(x=>[
  x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
]));
const portfolioAdmin=(await salesV2("GetSalesPortfolio",admin,portfolioRequest)).body;
assert.deepEqual(portfolioAdmin.counts,portfolio.body.counts);
assert.deepEqual(portfolioAdmin.forecastCurrencies,portfolio.body.forecastCurrencies);
assert.equal((await salesV2("GetSalesPortfolio",tenantB,portfolioRequest)).status,404);
assert.equal((await salesV2("GetSalesPortfolio",viewer,{...portfolioRequest,comparePrevious:true})).status,400);
const comparison=await salesV2("GetSalesPortfolio",viewer,{...portfolioRequest,comparePrevious:true,
  analysisPeriod:{startDate:{year:2026,month:3,day:1},endDate:{year:2026,month:3,day:2}}});
assert.equal(comparison.status,200,describeProtobuf(comparison.body));
assert.deepEqual(comparison.body.previousPeriod.startDate,{...comparison.body.previousPeriod.startDate,year:2026,month:2,day:27});
assert.deepEqual(comparison.body.previousPeriod.endDate,{...comparison.body.previousPeriod.endDate,year:2026,month:2,day:28});
assert.equal(comparison.body.previousClosedOutcomes.denominator,0n);
assert.equal(comparison.body.previousClosedOutcomes.winRateBasisPoints,undefined);
assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n,portfolioAudit);
console.log(JSON.stringify({event:"crm.native-sales-portfolio",result:"PASS",exact_i128:true,server_period:true,viewer_parity:true,tenant_ab:true,no_read_audit:true}));
assert.equal((await salesV2("GetPipelineMetrics",viewer,{
  pipelineId:"pipeline.other",status:2})).body.totalDealCount,0n);
assert.equal((await salesV2("GetPipelineMetrics",viewer,{
  pipelineId:"pipeline.other",accountId:randomUUID()})).body.totalDealCount,0n);
assert.equal((await salesV2("GetPipelineMetrics",viewer,{
  pipelineId:"pipeline.other",status:0})).status,400);
assert.equal((await salesV2("GetPipelineMetrics",viewer,{
  pipelineId:"pipeline.missing"})).status,404);
const totalNext=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",
  page:{pageSize:1,pageToken:totalView.body.pageInfo.nextPageToken}});
assert.equal(totalNext.status,200);
assert.equal(totalNext.body.deals.length,1);
assert.equal(totalNext.body.totalCount,32n);
assert.deepEqual(totalNext.body.currencyTotals,totalView.body.currencyTotals,
  "summary must not depend on independently paginated Deal cards");
assert.equal((await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",status:2
})).body.totalCount,0n);
assert.deepEqual((await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",status:2
})).body.currencyTotals,[]);
const missingAccount=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",accountId:randomUUID()});
assert.equal(missingAccount.status,200);
assert.equal(missingAccount.body.totalCount,0n);
assert.deepEqual(Object.keys(missingAccount.body.accountNames),[]);
assert.deepEqual(missingAccount.body.currencyTotals,[]);
const updatedCurrency=await salesV2("UpdateDeal",admin,{
  dealId:"r7-totals-01",
  amount:{operation:{case:"set",value:{minorUnits:2n,currencyCode:"USD"}}},
  precondition:{expectedVersion:1n,idempotencyKey:"r7-stage-totals-update"}
});
assert.equal(updatedCurrency.status,200,describeProtobuf(updatedCurrency.body));
assert.equal((await salesV2("UpdateDeal",admin,{
  dealId:"r7-totals-01",
  amount:{operation:{case:"set",value:{minorUnits:3n,currencyCode:"USD"}}},
  precondition:{expectedVersion:1n,idempotencyKey:"r7-stage-totals-stale"}
})).status,409);
const afterCurrencyUpdate=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:1}});
assert.equal(afterCurrencyUpdate.body.totalCount,32n);
assert.equal(afterCurrencyUpdate.body.currencyTotals
  .find(t=>t.currencyCode==="USD")?.totalMinorUnits,"9223372036854775809",
  "failed CAS cannot affect exact full-stage sum");
assert.equal((await salesV2("ListStageDeals",tenantB,{
  pipelineId:"pipeline.other",stageId:"lead"
})).status,404, "no cross-tenant summaries");
console.log(JSON.stringify({event:"crm.native-r71-board-currency-totals",result:"PASS",
  full_stage_count:32,visible_page:1,distinct_currencies:4,
  amount_exceeds_int64:true,missing_amount:true,
  status_account_filters:true,tenant_isolation:true,stale_cas:true}));

// Deterministic 100 and 1000 Deal load probes against the SAME workerd +
// persisted native D1, with all reads flowing through the original Rust
// ListStageDeals. This one-time isolated load fixture clones an existing valid
// Deal's serialized domain state; it does not become another app mutation path.
// Remove its records, receipts and audit immediately after the read-only probes.
{
  const sourceId="r7-totals-02";
  const [source]=(await sql("SELECT state,account_id,version,actor FROM crm_sales_deal_records "+
    "WHERE tenant='tenant-a' AND id='"+sourceId+"'"));
  assert.ok(source?.state.includes('"'+sourceId+'"'),
    "load fixture must clone a valid domain-serialized Deal");
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records "+
    "WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%'"))[0].n,0);
  const insertSql="INSERT INTO crm_sales_deal_records "+
    "(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor,last_action) "+
    "VALUES(?,?,?,?,?,?,?,?,?,?) RETURNING id";
  let inserted=0;
  const seedTo=async n=>{
    while(inserted<n){
      const ids=[],statements=[];
      for(let i=inserted;i<Math.min(n,inserted+100);i++){
        const id="r7-scale-"+String(i).padStart(4,"0");
        const state=source.state.replaceAll('"'+sourceId+'"','"'+id+'"');
        assert.ok(state!==source.state);
        ids.push(id);
        statements.push(statement(insertSql,"tenant-a",id,source.account_id,state,source.version,
          "r7-scale-key-"+i,"load-fixture",randomUUID(),source.actor,"sales_deals.create"));
      }
      const results=await batch(statements);
      assert.deepEqual(results.map(result=>result.results[0]?.id),ids);
      inserted+=ids.length;
    }
  };
  const benchmark=async n=>{
    await seedTo(n);
    const began=performance.now();
    const first=await salesV2("ListStageDeals",viewer,{
      pipelineId:"pipeline.other",stageId:"lead",page:{pageSize:25}});
    const latencyMs=Math.round(performance.now()-began);
    assert.equal(first.status,200,describeProtobuf(first.body));
    assert.equal(first.body.totalCount,BigInt(n+32));
    assert.equal(first.body.deals.length,25);
    assert.ok(first.body.pageInfo?.nextPageToken);
    assert.deepEqual(first.body.currencyTotals.map(t=>[
      t.currencyCode,t.totalMinorUnits,t.amountDealCount
    ]),[
      ["JPY","345",1n],["KWD","4567",1n],
      ["PLN",String(BigInt(n+1)*2345n),BigInt(n+1)],
      ["USD","9223372036854775809",2n]
    ],"summary must cover all "+n+" added Deals, not the 25-card page");
    const second=await salesV2("ListStageDeals",viewer,{
      pipelineId:"pipeline.other",stageId:"lead",
      page:{pageSize:25,pageToken:first.body.pageInfo.nextPageToken}});
    assert.equal(second.status,200);
    assert.equal(second.body.deals.length,25);
    assert.equal(second.body.totalCount,BigInt(n+32));
    assert.deepEqual(second.body.currencyTotals,first.body.currencyTotals);
    assert.ok(latencyMs<15000,
      "bounded indexed native D1 summary exceeded 15 s at "+n+" Deals: "+latencyMs+" ms");
    // SQL readback keeps money as text, including sums beyond signed int64.
    const filter={pipelineId:"pipeline.other",status:1,accountId};
    const metricStart=performance.now();
    const metrics=await salesV2("GetPipelineMetrics",viewer,filter);
    const metricMs=Math.round(performance.now()-metricStart);
    assert.equal(metrics.status,200,describeProtobuf(metrics.body));
    const facts=await sql("SELECT json_extract(state,'$.amount.currency') AS currency,"+
      "json_extract(state,'$.amount.minor_units') AS units,"+
      "json_extract(state,'$.probability_basis_points') AS bps FROM crm_sales_deal_records "+
      "WHERE tenant=? AND json_extract(state,'$.stage.pipeline_id')=? "+
      "AND json_extract(state,'$.status')='open' AND account_id=?",
      "tenant-a","pipeline.other",accountId);
    assert.equal(facts.length,n+32);
    assert.equal(metrics.body.totalDealCount,BigInt(facts.length));
    const expected=new Map();
    for(const row of facts){
      if(row.currency===null)continue;
      const v=expected.get(row.currency)??[0n,0n,0n];
      v[0]+=BigInt(row.units);v[1]+=BigInt(row.units)*BigInt(row.bps);v[2]++;
      expected.set(row.currency,v);
    }
    assert.deepEqual(metrics.body.currencies.map(x=>[
      x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
    ]),[...expected].sort(([a],[b])=>a.localeCompare(b)).map(([c,v])=>[c,String(v[0]),String(v[1]),v[2]]));
    assert.deepEqual(metrics.body.statuses.map(x=>[x.stageId,x.status,x.dealCount]),
      [["lead",1,BigInt(n+32)]]);
    assert.equal(metrics.body.operational[0].knownStageAgeCount,BigInt(n+32));
    assert.equal(metrics.body.operational[0].noExpectedCloseCount,BigInt(n+30));
    assert.ok(metricMs<15000,"native pipeline metrics exceeded 15 s at "+n+" Deals: "+metricMs+" ms");
    console.log(JSON.stringify({event:"crm.native-r73d-metrics-scale",result:"PASS",
      added_deals:n,matched_deals:facts.length,latency_ms:metricMs,exact_sql_comparison:true,
      nested_filters:true,exact_i128:true}));
    if(n===1000){
      const portfolio=await salesV2("GetSalesPortfolio",viewer,{pipelineId:"pipeline.other"});
      assert.equal(portfolio.status,200,describeProtobuf(portfolio.body));
      const historyYear=new Date().getUTCFullYear();
      const historyScope={pipelineId:"pipeline.other",timeZone:"UTC",pageSize:17,
        period:{startDate:{year:historyYear-1,month:1,day:1},endDate:{year:historyYear,month:12,day:31}}};
      const history=await salesV2("GetSalesHistory",viewer,historyScope);
      assert.equal(history.status,200,describeProtobuf(history.body));
      assert.equal(history.body.current.totalDealCount,1032n);
      assert.equal(history.body.current.deals.length,17);
      assert.equal(history.body.current.stateAtCutoff.totalCount+history.body.current.unknownHistoryCount,1032n);
      const historyNext=await salesV2("GetSalesHistory",viewer,{...historyScope,
        snapshot:history.body.snapshot,detail:{afterDealId:history.body.current.nextDealId}});
      assert.equal(historyNext.status,200,describeProtobuf(historyNext.body));
      assert.equal(historyNext.body.detail.deals.length,17);
      assert.ok(historyNext.body.detail.deals[0].dealId>history.body.current.nextDealId);
      console.log(JSON.stringify({event:"crm.native-history-scale",result:"PASS",
        deals:1032,page_size:17,full_summary:true,fenced_detail:true}));
      assert.equal(portfolio.body.counts.totalDealCount,1032n);
      assert.deepEqual(portfolio.body.forecastCurrencies.map(x=>[
        x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
      ]),metrics.body.forecastCurrencies.map(x=>[
        x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
      ]));
      // Both ends of the old paged scan change in a single fixture transaction.
      // Every committed state has the same total and weighted amount.
      const ids=["r7-scale-0000","r7-scale-0999"];
      // Atomic D1 batch competes with signed RPCs via the same binding.
      const updateSql="UPDATE crm_sales_deal_records SET "+
        "state=json_set(state,'$.amount.minor_units',?,'$.version',version+1),version=version+1,"+
        "idem_key=?,idem_digest='load-fixture',request_id=?,last_action='sales_deals.update' "+
        "WHERE tenant='tenant-a' AND id=? RETURNING id";
      const readBalances=async()=>(await sql("SELECT json_type(state,'$.amount.minor_units') AS kind,"+
        "json_extract(state,'$.amount.minor_units') AS units "+
        "FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id IN (?,?) ORDER BY id",
        ...ids)).map(row=>[row.kind,row.units]);
      const baseline=metrics.body.currencies;
      const forecastBaseline=metrics.body.forecastCurrencies;
      assert.deepEqual(await readBalances(),[["text","2345"],["text","2345"]]);
      for(let attempt=0;attempt<12;attempt++){
        const amounts=attempt%2===0?["2346","2344"]:["2345","2345"];
        const statements=ids.map((id,i)=>statement(updateSql,amounts[i],
          "r7-scale-key-swap-"+attempt+"-"+i,randomUUID(),id));
        let read,write;
        if(attempt%2===0){
          read=salesV2("GetPipelineMetrics",viewer,filter);
          write=batch(statements);
        }else{
          write=batch(statements);
          read=salesV2("GetPipelineMetrics",viewer,filter);
        }
        const [snapshot,changed]=await Promise.all([read,write]);
        assert.deepEqual(changed.map(result=>result.results[0]?.id),ids);
        assert.deepEqual(await readBalances(),amounts.map(value=>["text",value]));
        if(snapshot.status!==200)
          console.error(JSON.stringify({event:"crm.native-r73d-concurrent-failure",
            attempt,status:snapshot.status,request_id:snapshot.headers.get("x-request-id")}));
        assert.equal(snapshot.status,200,describeProtobuf(snapshot.body));
        assert.equal(snapshot.body.totalDealCount,1032n);
        assert.deepEqual(snapshot.body.currencies,baseline,
          "a concurrent atomic balance swap must not create a mixed money snapshot");
        assert.deepEqual(snapshot.body.forecastCurrencies,forecastBaseline,
          "forecast must share the same consistent snapshot");
        const whole=await salesV2("GetSalesPortfolio",viewer,{pipelineId:"pipeline.other"});
        assert.equal(whole.status,200,describeProtobuf(whole.body));
        assert.deepEqual(whole.body.forecastCurrencies.map(x=>[
          x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
        ]),forecastBaseline.map(x=>[
          x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount
        ]));
      }
      const after=await salesV2("GetPipelineMetrics",viewer,filter);
      assert.equal(after.status,200,describeProtobuf(after.body));
      assert.deepEqual(after.body.currencies,baseline);
      console.log(JSON.stringify({event:"crm.native-r73d-concurrent-metrics",result:"PASS",
        committed_atomic_swaps:12,concurrent_reads:12,final_read:true,exact_totals:true,d1_binding:true}));
      // Corrupt a disposable record outside the visible page. Both analytics
      // and Kanban must reject it through the original Rust persistence owner.
      // D1 permits only a catalog rank rebase without a version increment.
      // Use a disposable versioned fixture mutation, with unique receipt and
      // request identity, to exercise malformed persistence without bypassing
      // the real D1 guards or audit/history triggers.
      const invalidId=ids[1];
      await assert.rejects(sql("UPDATE crm_sales_deal_records SET "+
        "state=json_set(state,'$.amount.minor_units',2345) WHERE tenant='tenant-a' AND id=?",
        invalidId),/SALES_CATALOG_RANK_ONLY_REQUIRED/);
      let faultSequence=0;
      const invalid=async(field,value,id)=>{
        const [before]=await sql("SELECT version FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id=?",id);
        const key="r7-scale-key-fault-"+(++faultSequence);
        const rows=await sql("UPDATE crm_sales_deal_records SET "+
          "state=json_set(state,?,json(?),'$.version',version+1),version=version+1,"+
          "idem_key=?,idem_digest='load-fixture',request_id=?,last_action='sales_deals.update' "+
          "WHERE tenant='tenant-a' AND id=? AND version=? RETURNING id,version",
          field,value,key,randomUUID(),id,before.version);
        assert.deepEqual(rows,[{id,version:before.version+1}]);
        const [receipt]=await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts WHERE tenant='tenant-a' AND key=?",key);
        assert.equal(receipt.n,1);
      };
      for(const [field,bad,good] of [
        ["$.amount.minor_units","2345",'"2345"'],
        ["$.amount.minor_units","2345.0",'"2345"'],
        ["$.probability_basis_points",'"2000"',"2000"],
        ["$.probability_basis_points","2000.5","2000"],
        ["$.probability_source",'"unknown"','"manual"'],
        ["$.forecast_category",'"unknown"','"pipeline"']
      ]){
        try{
          await invalid(field,bad,invalidId);
          const brokenMetrics=await salesV2("GetPipelineMetrics",viewer,filter);
          assert.equal(brokenMetrics.status,500);
          assert.equal(brokenMetrics.body.message,"Sales D1 metrics read failed");
          const brokenBoard=await salesV2("ListStageDeals",viewer,{...filter,stageId:"lead",
            page:{pageSize:1}});
          assert.equal(brokenBoard.status,500);
          assert.equal(brokenBoard.body.message,"Sales D1 stage list failed");
        }finally{
          await invalid(field,good,invalidId);
        }
      }
      const restored=await salesV2("GetPipelineMetrics",viewer,filter);
      assert.equal(restored.status,200);
      assert.deepEqual(restored.body.currencies,baseline);
      // The original persisted Money domain spans i128, while this version
      // of the individual public Deal wire amount is i64. Test off-page facts.
      const largeUnits=9223372036854775808n;
      try{
        await invalid("$.amount.minor_units",JSON.stringify(String(largeUnits)),invalidId);
        const large=await salesV2("GetPipelineMetrics",viewer,filter);
        const largeBoard=await salesV2("ListStageDeals",viewer,{...filter,stageId:"lead",page:{pageSize:1}});
        assert.equal(large.status,200,describeProtobuf(large.body));
        assert.equal(largeBoard.status,200,describeProtobuf(largeBoard.body));
        const oldPln=baseline.find(x=>x.currencyCode==="PLN");
        const newPln=large.body.currencies.find(x=>x.currencyCode==="PLN");
        const delta=largeUnits-2345n;
        assert.equal(newPln.totalMinorUnits,String(BigInt(oldPln.totalMinorUnits)+delta));
        assert.equal(newPln.weightedMinorUnitsTimesBasisPoints,
          String(BigInt(oldPln.weightedMinorUnitsTimesBasisPoints)+delta*2000n));
        assert.equal(largeBoard.body.currencyTotals.find(x=>x.currencyCode==="PLN").totalMinorUnits,
          newPln.totalMinorUnits);
      }finally{
        await invalid("$.amount.minor_units",'"2345"',invalidId);
      }
      // Exercise ordinary signed Connect mutations concurrently with reads.
      // Each capture must match one entire committed state, not a hybrid.
      const current=await salesV2("GetDeal",viewer,{dealId:invalidId});
      assert.equal(current.status,200);
      const payload={dealId:invalidId,amount:{operation:{case:"set",value:{
        minorUnits:2346n,currencyCode:"PLN"}}},precondition:{
        expectedVersion:current.body.deal.version,idempotencyKey:"r7-scale-key-rpc-update"}};
      const [capture,boardCapture,changed]=await Promise.all([
        salesV2("GetPipelineMetrics",viewer,filter),
        salesV2("ListStageDeals",viewer,{...filter,stageId:"lead",page:{pageSize:1}}),
        salesV2("UpdateDeal",admin,payload)]);
      assert.equal(capture.status,200,describeProtobuf(capture.body));
      assert.equal(boardCapture.status,200,describeProtobuf(boardCapture.body));
      assert.equal(changed.status,200,describeProtobuf(changed.body));
      const committed=await salesV2("GetPipelineMetrics",viewer,filter);
      assert.equal(committed.status,200);
      const beforePln=baseline.find(x=>x.currencyCode==="PLN");
      const afterPln=committed.body.currencies.find(x=>x.currencyCode==="PLN");
      assert.equal(afterPln.totalMinorUnits,String(BigInt(beforePln.totalMinorUnits)+1n));
      assert.equal(afterPln.weightedMinorUnitsTimesBasisPoints,
        String(BigInt(beforePln.weightedMinorUnitsTimesBasisPoints)+2000n));
      const numericSignature=x=>JSON.stringify(x.map(m=>[m.currencyCode,m.totalMinorUnits,
        m.weightedMinorUnitsTimesBasisPoints,String(m.amountDealCount)]));
      assert.ok([numericSignature(baseline),numericSignature(committed.body.currencies)]
        .includes(numericSignature(capture.body.currencies)));
      const boardPln=boardCapture.body.currencyTotals.find(x=>x.currencyCode==="PLN");
      assert.ok([beforePln.totalMinorUnits,afterPln.totalMinorUnits].includes(boardPln.totalMinorUnits));
      const revert=await salesV2("UpdateDeal",admin,{...payload,
        amount:{operation:{case:"set",value:{minorUnits:2345n,currencyCode:"PLN"}}},
        precondition:{expectedVersion:changed.body.deal.version,idempotencyKey:"r7-scale-key-rpc-revert"}});
      assert.equal(revert.status,200,describeProtobuf(revert.body));
      const final=await salesV2("GetPipelineMetrics",viewer,filter);
      assert.equal(final.status,200);
      assert.deepEqual(final.body.currencies,baseline);
      console.log(JSON.stringify({event:"crm.native-r73d-strict-persistence",result:"PASS",
        invalid_money_types:2,invalid_probability_types:2,invalid_forecast_metadata_types:2,board_and_metrics:true,
        individual_persisted_i128:true,signed_concurrent_update:true,restored:true}));
    }
    console.log(JSON.stringify({event:"crm.native-r71-kanban-scale",result:"PASS",
      added_deals:n,stage_total_count:n+32,rendered_per_page:25,
      full_stage_currencies:4,latency_ms:latencyMs,cursor_stable:true,
      original_rust_owner:true,exact_i128:true}));
  };
  try {
    // EXPLAIN both actual native Rust statements, not approximations.
    const rust=readFileSync(join(cwd,"src","sales_deals.rs"),"utf8");
    const stageQuery=rust.match(/"SELECT id,state FROM crm_sales_deal_records WHERE \{condition\} ORDER BY id"/)?.[0];
    assert.ok(stageQuery,"canonical stage snapshot SELECT must be present");
    const stageSql=stageQuery.slice(1,-1).replace("{condition}",
      "tenant=? AND json_extract(state,'$.stage.pipeline_id')=? "+
      "AND json_extract(state,'$.stage.stage_id')=? AND json_extract(state,'$.status')=? AND account_id=?");
    const plan=await sql("EXPLAIN QUERY PLAN "+stageSql,
      "tenant-a","pipeline.other","lead","open",accountId);
    assert.ok(plan.some(row=>row.detail.includes("idx_crm_sales_board_stage")),
      "the 1000+ row snapshot must use the existing stage/ID index");
    // Compose the actual Rust format string from its shared raw SQL constants.
    // Preserve the same indexed plan checks after extracting the common fields.
    const clock=rust.match(/const METRIC_CLOCK: &str = r#"([\s\S]*?)"#;/)?.[1];
    const fields=rust.match(/const METRIC_FIELDS: &str = r#"([\s\S]*?)"#;/)?.[1];
    const query=rust.match(/"WITH \{METRIC_CLOCK\} SELECT \{METRIC_FIELDS\} FROM snapshot_clock c CROSS JOIN crm_sales_deal_records d WHERE \{condition\}"/)?.[0];
    assert.ok(clock&&fields&&query,"canonical metrics SELECT must be present");
    const metricsSql=query.slice(1,-1).replace("{METRIC_CLOCK}",clock).replace("{METRIC_FIELDS}",fields).replace("{condition}",
      "d.tenant=? AND json_extract(d.state,'$.stage.pipeline_id')=? "+
      "AND json_extract(d.state,'$.status')=? AND d.account_id=?");
    const metricsPlan=await sql("EXPLAIN QUERY PLAN "+metricsSql,
      "tenant-a","pipeline.other","open",accountId);
    assert.ok(metricsPlan.some(row=>row.detail.includes("idx_crm_sales_board_stage")));
    assert.ok(metricsPlan.some(row=>row.detail.includes("crm_sales_deal_events")));
    assert.ok(metricsPlan.some(row=>row.detail.includes("idx_crm_sales_link_deal")));
    const portfolioQuery=rust.match(/r#"(WITH \{METRIC_CLOCK\},[\s\S]*?FROM snapshot_clock c CROSS JOIN selected_deals d)"#/)?.[1];
    const portfolioCondition=rust.match(/"(d\.tenant=\? AND json_extract\(d\.state,'\$\.stage\.pipeline_id'\) IN \(SELECT id FROM eligible WHERE \? OR json_extract\(state,'\$\.archived'\)=0\))"/)?.[1];
    assert.ok(portfolioQuery&&portfolioCondition,"canonical portfolio snapshot SELECT must be present");
    const portfolioSql=portfolioQuery.replace("{METRIC_CLOCK}",clock).replace("{METRIC_FIELDS}",fields).replace("{condition}",portfolioCondition);
    const portfolioPlan=await sql("EXPLAIN QUERY PLAN "+portfolioSql,
      "tenant-a","pipeline.other","pipeline.other","tenant-a",0);
    assert.ok(portfolioPlan.some(row=>row.detail.includes("crm_sales_pipeline_records")),describeProtobuf(portfolioPlan));
    assert.ok(portfolioPlan.some(row=>row.detail.includes("crm_sales_deal_events")),describeProtobuf(portfolioPlan));
    assert.ok(portfolioPlan.some(row=>row.detail.includes("idx_crm_sales_link_deal")),describeProtobuf(portfolioPlan));
    // The new forecast capture has a different keyset access pattern. Verify
    // both exact query variants without weakening the original plan assertions.
    const captureRust=readFileSync(join(cwd,"src","sales_performance.rs"),"utf8");
    const capturePage=captureRust.match(/SELECT d\.tenant,d\.id,d\.state,d\.account_id FROM crm_sales_deal_records d WHERE d\.tenant=\?1[\s\S]*?ORDER BY d\.id LIMIT \?4/)?.[0];
    assert.ok(capturePage,"canonical forecast keyset SELECT must be present");
    for(const [filter,index,params] of [
      ["","idx_crm_forecast_open_deal",["tenant-a","", "0",256]],
      [" AND json_extract(d.state,'$.stage.pipeline_id')=?5","idx_crm_forecast_open_pipeline",["tenant-a","","0",256,"pipeline.other"]]
    ]){
      const captureSql="WITH fence AS MATERIALIZED (SELECT COALESCE((SELECT revision FROM crm_sales_forecast_revisions WHERE tenant=?1),0) revision) "+
        capturePage.replace("{pipeline_filter}",filter);
      const capturePlan=await sql("EXPLAIN QUERY PLAN "+captureSql,...params);
      assert.ok(capturePlan.some(row=>row.detail.includes("SEARCH d USING INDEX "+index)&&row.detail.includes("id>?")),
        describeProtobuf(capturePlan));
      assert.ok(!capturePlan.some(row=>row.detail.includes("TEMP B-TREE FOR ORDER BY")),describeProtobuf(capturePlan));
    }
    await benchmark(100);
    await benchmark(1000);
  }finally{
    // Only disposable scale fixtures; preserve the same atomic cleanup.
    await batch([
      statement("DELETE FROM crm_sales_deal_events WHERE tenant='tenant-a' AND deal_id LIKE 'r7-scale-%'"),
      statement("DELETE FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%'"),
      statement("DELETE FROM crm_sales_deal_receipts WHERE tenant='tenant-a' AND key LIKE 'r7-scale-key-%'"),
      statement("DELETE FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id LIKE 'r7-scale-%'"),
    ]);
    assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records "+
      "WHERE tenant='tenant-a' AND id LIKE 'r7-scale-%'"))[0].n,0);
  }
}
assert.equal((await salesV2("ListStageDeals",tenantB,{
  pipelineId:"pipeline.enterprise",stageId:"lead"
})).status,404,"tenant B cannot read tenant A pipeline");
assert.equal((await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.enterprise",stageId:"not-a-stage"
})).status,404,"unmanaged stage is not a valid board column");
assert.equal((await salesV2("ListDeals",viewer,{
  page:{pageSize:1,pageToken:dealLast},pipelineId:"pipeline.enterprise"})
  ).body.deals.length,0);
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
  "WHERE tenant='tenant-a' AND key IN ('connect-sales-create-main',"+
  "'connect-sales-update-main','connect-sales-advance-main','connect-sales-close-main')"
  ))[0].n,4,"original Deal receipt owners and D1 triggers remain singular");
assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'"))[0].n,4,
  "create + update + two stage transitions each write exactly one audit");

  assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
    "WHERE tenant_id='tenant-a' AND subject_id='"+dealId+"' ORDER BY rowid")),[
    {action:"sales_deals.create",actor:"native-a@example.test",
      request_id:salesCreated.headers.get("x-request-id")},
    {action:"sales_deals.update",actor:"service:ci-fixture.access",
      request_id:saleUpdated.headers.get("x-request-id")},
    {action:"sales_deals.advance",actor:"native-a@example.test",
      request_id:advancedSale.headers.get("x-request-id")},
    {action:"sales_deals.close",actor:"native-a@example.test",
      request_id:closedSale.headers.get("x-request-id")}
  ],"Sales create/update/stage actor and action survive cross-actor replay");
  const linked=(await sql("SELECT delivery_id,task_id,disposition,CASE WHEN task_id IS NULL THEN 1 ELSE 0 END AS no_task,actor,request_id FROM crm_sales_link_delivery_records "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"' ORDER BY deal_version"));
  assert.equal(linked.length,2);
  // One signed server request atomically writes Deal, linked Task (if any),
  // and linked-delivery audits. Distinguish their actions AND subject IDs.
  for(const [i,row] of linked.entries()){
    assert.equal(row.actor,"native-a@example.test");
    assert.equal(row.request_id,[advancedSale,closedSale][i].headers.get("x-request-id"));
    assert.equal(row.disposition,i===0?"applied":"ignored");
    assert.equal(row.no_task,i===0?0:1,
      "Ignored delivery must not create a Task; validate SQL NULL, not serialized string");
    const expected=[
      {action:i===0?"sales_deals.advance":"sales_deals.close",subject_id:dealId,
        actor:"native-a@example.test",request_id:row.request_id},
      {action:i===0?"sales_activities_link.applied":"sales_activities_link.ignored",
        subject_id:row.delivery_id,actor:"native-a@example.test",
        request_id:row.request_id},
      ...(row.disposition==="applied"?[{action:"activity_tasks.create",subject_id:row.task_id,
        actor:"native-a@example.test",request_id:row.request_id}]:[])
    ].sort((a,b)=>a.action.localeCompare(b.action));
    const actual=(await sql("SELECT action,subject_id,actor,request_id FROM crm_audit "+
      "WHERE tenant_id='tenant-a' AND request_id='"+row.request_id+"' ORDER BY action"));
    assert.deepEqual(actual,expected,
      "One AdvanceStage request must commit attributed Deal + Task + delivery audit together");
  }
  // M1-08 product check: not a legacy generic typed-resource filter.
  // Customers can have multiple Deals and manual Account tasks. The concrete
  // Deal page should show ONLY Tasks whose original domain state refers to
  // this specific sales.deal, with exact tenant+Account/page filtering.
  const appliedTask=linked.find(row=>row.disposition==="applied")?.task_id;
  assert.ok(appliedTask,"linked Sales→Task must retain the Task ID");
  const linkedTaskRead=await taskV2("GetTask",viewer,{taskId:appliedTask});
  assert.equal(linkedTaskRead.status,200,describeProtobuf(linkedTaskRead.body));
  assert.equal(linkedTaskRead.body.task?.relatedDealId,dealId);
  assert.equal(linkedTaskRead.body.task?.accountId,accountId);
  assert.equal((await taskV2("GetTask",viewer,{taskId:taskMid})).body.task?.relatedDealId,"",
    "ordinary Account task must not claim a Deal relation");
  const scopedDeals=await collectPages(taskV2,"ListTasks","tasks",{dealId,accountId,page:{pageSize:1}});
  assert.deepEqual(scopedDeals.map(task=>task.taskId),[appliedTask],
    "Deal task view must not mix in manual Account tasks or another Deal");
  assert.equal(scopedDeals[0].relatedDealId,dealId);
  assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{dealId,accountId,status:2}))
    .map(task=>task.taskId),[],"status filter applies before pagination");
  assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{dealId,accountId:accountNextId}))
    .map(task=>task.taskId),[],"Account/Deal cross-scope is an empty intersection");
  assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{dealId:dealLast}))
    .map(task=>task.taskId),[],"unlinked Deal cannot see another Deal's Tasks");
  assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{dealId:"nonexistent-valid-deal"}))
    .map(task=>task.taskId),[],"unknown Deal does not broaden the result");
  assert.equal((await taskV2("ListTasks",tenantB,{dealId,page:{pageSize:1}})).body.tasks.length,0);
  assert.equal((await taskV2("ListTasks",admin,{dealId:"invalid/deal"})).status,400);
  assert.equal((await taskV2("ListTasks",null,{dealId})).status,401);
  const scopedAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
  await taskV2("ListTasks",viewer,{dealId});
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,scopedAudit,
    "Deal task navigation must be read-only");
  console.log(JSON.stringify({event:"crm.native-m1-deal-task-navigation",result:"PASS",
    deal_link_from_original_task:true,scope_before_cursor:true,
    manual_account_task_excluded:true,other_deal_excluded:true,
    signed_tenant_filter:true,no_read_audit:true}));
  // M1-08 B: a salesperson can add an ordinary manual Task to a specific
  // existing Deal. The original Task owner validates the Sales owner and
  // commits the normal receipt/audit atomically, never using a second writer.
  const manualDealTaskId="task-manual-deal-"+randomUUID();
  const manualDealInput={taskId:manualDealTaskId,accountId,dealId,
    subject:"Manual deal follow up",priority:2,
    precondition:{idempotencyKey:"task-manual-deal-create-001"}};
  const manualBefore=await sql("SELECT "+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits");
  assert.equal((await taskV2("CreateTask",viewer,manualDealInput)).status,403);
  assert.equal((await taskV2("CreateTask",tenantB,manualDealInput)).status,400);
  assert.equal((await taskV2("CreateTask",admin,{...manualDealInput,dealId:"invalid/deal",
    precondition:{idempotencyKey:"manual-deal-invalid-id"}})).status,400);
  assert.equal((await taskV2("CreateTask",admin,{...manualDealInput,dealId:"missing-deal",
    precondition:{idempotencyKey:"manual-deal-missing"}})).status,400);
  assert.equal((await taskV2("CreateTask",admin,{...manualDealInput,accountId:accountNextId,
    precondition:{idempotencyKey:"manual-deal-wrong-account"}})).status,400);
  assert.deepEqual(await sql("SELECT "+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"),manualBefore,
    "Rejected Deal links must never write task, receipt or audit");
  const manualCreated=await taskV2("CreateTask",admin,manualDealInput);
  assert.equal(manualCreated.status,200,describeProtobuf(manualCreated.body));
  assert.equal(manualCreated.body.task?.relatedDealId,dealId);
  assert.equal(manualCreated.body.task?.accountId,accountId);
  assert.equal(manualCreated.body.receipt?.replayed,false);
  const manualReplay=await taskV2("CreateTask",knownService,manualDealInput);
  assert.equal(manualReplay.status,200,describeProtobuf(manualReplay.body));
  assert.equal(manualReplay.body.receipt?.replayed,true);
  assert.equal((await taskV2("CreateTask",admin,{...manualDealInput,dealId:dealLast})).status,409);
  assert.equal((await taskV2("GetTask",viewer,{taskId:manualDealTaskId})).body.task?.relatedDealId,dealId);
  const manualScoped=await collectPages(taskV2,"ListTasks","tasks",{dealId,accountId,page:{pageSize:1}});
  assert.deepEqual(manualScoped.map(task=>task.taskId).sort(),[appliedTask,manualDealTaskId].sort());
  assert.ok(!manualScoped.some(task=>task.taskId===taskMid),
    "A general Account Task is not a Task of the Deal");
  const manualAfter=await sql("SELECT "+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_activity_task_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits");
  assert.equal(manualAfter[0].tasks,manualBefore[0].tasks+1);
  assert.equal(manualAfter[0].receipts,manualBefore[0].receipts+1);
  assert.equal(manualAfter[0].audits,manualBefore[0].audits+1);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id='"+manualDealTaskId+"' AND action='activity_tasks.create'"))[0].n,1);
  console.log(JSON.stringify({event:"crm.native-m1-manual-deal-task",result:"PASS",
    original_task_owner:true,deal_and_account_crosscheck:true,negative_no_write:true,
    idempotent_replay:true,exact_scoped_list:true}));
  const salesActorSnapshot=async()=>(await sql("SELECT "+
    "(SELECT state FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id='"+dealLast+"') AS deal,"+
    "(SELECT COUNT(*) FROM crm_activity_task_records) AS tasks,"+
    "(SELECT COUNT(*) FROM crm_sales_link_delivery_records) AS deliveries,"+
    "(SELECT COUNT(*) FROM crm_sales_deal_receipts) AS receipts,"+
    "(SELECT COUNT(*) FROM crm_audit) AS audits"));
  const beforeSalesFault=(await salesActorSnapshot());
  (await sql("CREATE TRIGGER fixture_fail_sales_link_actor BEFORE INSERT ON crm_audit "+
    "WHEN NEW.action='sales_activities_link.applied' "+
    "BEGIN SELECT RAISE(ABORT,'fixture linked audit'); END;"));
  try{
    const failed=await salesV2("AdvanceStage",admin,{dealId:dealLast,
      targetStage:stageQualified,targetStatus:1,
      precondition:{expectedVersion:1n,idempotencyKey:"connect-sales-linked-actor-fail"}});
    assert.equal(failed.status,503);
    assert.equal(failed.body.code,"unavailable");
    assert.equal(failed.body.message,"SALES_COMMIT_UNCONFIRMED");
    const outcomeDetail=decodeDomainErrorDetail(failed.body.details.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
    assert.equal(outcomeDetail.commitOutcome,2);
    assert.equal(outcomeDetail.requestId,failed.headers.get("x-request-id"));
    assert.deepEqual((await salesActorSnapshot()),beforeSalesFault,
      "Late link audit failure rolls back Deal, new Task, receipt, delivery and audit");
  }finally{await sql("DROP TRIGGER fixture_fail_sales_link_actor");}
  // Reconfigure a populated pipeline: stable stage IDs, atomic rank rebase.
  const oldRank=(await sql("SELECT id,version,state FROM crm_sales_deal_records WHERE tenant='tenant-a' "+
    "AND id IN ('"+dealId+"','"+dealLast+"') ORDER BY id"));
  const oldDealAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id IN ('"+dealId+"','"+dealLast+"')"))[0].n;
  const oldDealReceipts=(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0].n;
  const reorder={pipelineId:"pipeline.enterprise",name:"Occupied stage reorder",
    stages:[pipelineSeed.stages[1],pipelineSeed.stages[0],pipelineSeed.stages[2]],
    archived:false,
    precondition:{expectedVersion:2n,idempotencyKey:"connect-r7-reorder-used-stage"}};
  // A late D1 failure after the catalog CAS must roll back BOTH the
  // Pipeline audit/receipt and every Deal rank projection.
  const failedRebaseSnapshot=async()=>(await sql("SELECT (SELECT state FROM crm_sales_pipeline_records "+
    "WHERE tenant='tenant-a' AND id='pipeline.enterprise') AS catalog, "+
    "(SELECT state FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id='"+dealId+"') AS closed, "+
    "(SELECT state FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id='"+dealLast+"') AS opened, "+
    "(SELECT COUNT(*) FROM crm_sales_pipeline_receipts WHERE tenant='tenant-a') AS receipts, "+
    "(SELECT COUNT(*) FROM crm_audit WHERE tenant_id='tenant-a') AS audits"));
  const beforeFailedRebase=(await failedRebaseSnapshot());
  (await sql("CREATE TRIGGER fixture_deny_rank_rebase BEFORE UPDATE ON crm_sales_deal_records "+
    "WHEN NEW.id='"+dealLast+"' AND NEW.version=OLD.version "+
    "BEGIN SELECT RAISE(ABORT,'fixture rank rollback'); END;"));
  try {
    const denied=await salesV2("UpdatePipeline",admin,{
      ...reorder,precondition:{expectedVersion:2n,idempotencyKey:"r7-rebase-rollback"}
    });
    assertUnconfirmed(denied,"SALES_PIPELINE_WRITE_UNCONFIRMED");
    assert.deepEqual((await failedRebaseSnapshot()),beforeFailedRebase,
      "Late D1 failure must roll back catalog, both Deals, receipts and audit");
  }finally{await sql("DROP TRIGGER fixture_deny_rank_rebase");}
  const reordered=await salesV2("UpdatePipeline",admin,reorder);
  assert.equal(reordered.status,200,describeProtobuf(reordered.body));
  assert.equal(reordered.body.pipeline?.version,3n);
  assert.deepEqual(reordered.body.pipeline?.stages.map(s=>s.stageId),["qualified","lead","won"]);
  assert.deepEqual(reordered.body.pipeline?.stages.map(s=>s.ordinal),[1,2,3]);
  assert.equal((await salesV2("UpdatePipeline",admin,reorder)).body.receipt?.replayed,true);
  const rankAfter=(await sql("SELECT id,version,state FROM crm_sales_deal_records WHERE tenant='tenant-a' "+
    "AND id IN ('"+dealId+"','"+dealLast+"') ORDER BY id"));
  assert.deepEqual(rankAfter.map(x=>x.version),oldRank.map(x=>x.version),
    "Rank-only metadata must not change Deal versions");
  assert.equal(JSON.parse(rankAfter.find(x=>x.id===dealId).state).stage.ordinal,1);
  assert.equal(JSON.parse(rankAfter.find(x=>x.id===dealLast).state).stage.ordinal,2);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id IN ('"+dealId+"','"+dealLast+"')"))[0].n,oldDealAudit,
    "No fictional Deal audit for catalog reorder");
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0].n,oldDealReceipts,
    "No fictional Deal receipt for catalog reorder");
  assert.equal((await salesV2("UpdatePipeline",admin,{
    ...reorder,stages:[pipelineSeed.stages[1],pipelineSeed.stages[2]],
    precondition:{expectedVersion:3n,idempotencyKey:"r7-deny-delete-occupied"}
  })).status,409,"Referenced stage deletion must fail");
  assert.equal((await salesV2("UpdatePipeline",admin,{
    ...reorder,archived:true,
    precondition:{expectedVersion:3n,idempotencyKey:"r7-deny-archive-open"}
  })).status,409,"Open Deals must forbid pipeline archive");
  assert.equal((await salesV2("GetPipeline",viewer,{pipelineId:"pipeline.enterprise"}))
    .body.pipeline?.version,3n,"Failed catalog writes must roll back");
  const freshInput={
    dealId:"r7-catalog-rank-"+randomUUID(),name:"Owner ordered",accountId,
    stage:{pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:999},
    probabilityBasisPoints:1000,
    precondition:{idempotencyKey:"r471-ci-receipt-read-fail"}
  };
  const unreadableCommit=await salesV2("CreateDeal",admin,freshInput);
  assert.equal(unreadableCommit.status,503);
  const unreadableDetail=decodeDomainErrorDetail(unreadableCommit.body.details.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
  assert.equal(unreadableDetail.commitOutcome,2);
  assert.equal(unreadableDetail.requestId,unreadableCommit.headers.get("x-request-id"));
  assert.equal((await sql("SELECT version FROM crm_sales_deal_records WHERE tenant=? AND id=?",
    "tenant-a",freshInput.dealId))[0].version,1);
  const fresh=await salesV2("CreateDeal",admin,freshInput);
  assert.equal(fresh.status,200,describeProtobuf(fresh.body));
  assert.equal(fresh.body.receipt.replayed,true);
  assert.equal(fresh.body.receipt.originalRequestId,unreadableCommit.headers.get("x-request-id"));
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?",
    "tenant-a",freshInput.dealId))[0].n,1);
  console.log(JSON.stringify({event:"crm.native.s1-receipt-read-failure",result:"PASS",
    committed_before_failed_read:true,unconfirmed:true,same_intent_replay:true,single_audit:true}));
  assert.equal(fresh.body.deal?.stage?.ordinal,1);
  // An ordinary UpdateDeal wins version 2 without a stage link.
  // Simulate the losing CAS's remaining batch against the actual D1 binding.
  const winnerId=fresh.body.deal.dealId;
  const winner=await salesV2("UpdateDeal",admin,{
    dealId:winnerId,name:{operation:{case:"set",value:"CAS winner"}},
    precondition:{expectedVersion:1n,idempotencyKey:"r471-cas-winner"}
  });
  assert.equal(winner.status,200,describeProtobuf(winner.body));
  assert.equal(winner.body.deal.version,2n);
  const casWinnerReceiptsBefore=await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts");
  const casWinnerAuditsBefore=await sql("SELECT COUNT(*) AS n FROM crm_audit");
  await assert.rejects(()=>batch([
    statement("UPDATE crm_sales_deal_records SET request_id=? WHERE tenant=? AND id=? AND version=?",
      "r471-losing-request","tenant-a",winnerId,1),
    statement("INSERT INTO crm_sales_link_delivery_records(tenant,delivery_id,event_id,deal_id,deal_version,disposition,state_json,request_id,actor) "+
      "VALUES(?,?,?,?,?,'ignored','{}',?,?)","tenant-a","r471-losing-delivery","r471-losing-event",
      winnerId,2,"r471-losing-request","native-a@example.test")
  ]),/SALES_LINK_SOURCE_COMMIT_MISMATCH/);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records WHERE tenant=? AND deal_id=?",
    "tenant-a",winnerId))[0].n,0);
  assert.deepEqual(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts"),casWinnerReceiptsBefore);
  assert.deepEqual(await sql("SELECT COUNT(*) AS n FROM crm_audit"),casWinnerAuditsBefore);
  assert.deepEqual(await sql("SELECT original_request_id FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
    "tenant-a","r471-cas-winner"),[{original_request_id:winner.headers.get("x-request-id")}]);
  console.log(JSON.stringify({event:"crm.native-r7-occupied-stage-reorder",result:"PASS",
    stable_ids:true,open_closed_rebased:true,versions_preserved:true,
    audit_receipt_single_owner:true,cas:true,invalid_deletion_denied:true,
    catalog_assigned_ordinal:true}));
  // A finished Deal is reopened only through the explicit original owner.
  const reopenSale={dealId,reasonCode:"customer-revisited",
    precondition:{expectedVersion:4n,idempotencyKey:"connect-sales-reopen-main"}};
  assert.equal((await salesV2("ReopenDeal",viewer,reopenSale)).status,403);
  assert.equal((await salesV2("ReopenDeal",admin,{
    ...reopenSale,precondition:{expectedVersion:3n,idempotencyKey:"r72-reopen-stale"}
  })).status,409);
  const reopenedSale=await salesV2("ReopenDeal",admin,reopenSale);
  assert.equal(reopenedSale.status,200,describeProtobuf(reopenedSale.body));
  assert.equal(reopenedSale.body.deal?.status,1);
  assert.equal(reopenedSale.body.deal?.version,5n);
  assert.equal(reopenedSale.body.deal?.closedAt,undefined);
  assert.equal(reopenedSale.body.deal?.stage?.stageId,"qualified");
  assert.equal((await salesV2("ReopenDeal",admin,reopenSale)).body.receipt?.replayed,true);
  assert.equal((await salesV2("ReopenDeal",admin,{
    dealId,reasonCode:"attempt-again",
    precondition:{expectedVersion:5n,idempotencyKey:"r72-reopen-already-open"}
  })).status,409);
  const afterReopen=await salesV2("ListDealEvents",viewer,{dealId,page:{pageSize:25}});
  assert.deepEqual(afterReopen.body.events.map(e=>e.dealVersion),[5n,4n,3n,2n,1n]);
  assert.equal(afterReopen.body.events[0].kind,5);
  assert.equal(afterReopen.body.events[0].reasonCode,"customer-revisited");
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0].n,2,
    "reopen must not invent a stage-change delivery");
  const reclosed=await salesV2("CloseDeal",admin,{
    dealId,outcome:3,reasonCode:"customer-cancelled",
    precondition:{expectedVersion:5n,idempotencyKey:"r72-close-again"}
  });
  assert.equal(reclosed.status,200);
  assert.equal(reclosed.body.deal?.status,3);
  assert.equal(reclosed.body.deal?.version,6n);
  assert.equal((await salesV2("CloseDeal",admin,closeSale)).body.receipt?.replayed,true,
    "first CloseDeal must replay unchanged after stage reorder and reopen");
  const reopenAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id='"+dealId+"'"))[0].n;
  const reopenReceiptBefore=(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0].n;
  const reopenEventsBefore=(await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_events "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0].n;
  (await sql("CREATE TRIGGER fixture_fail_reopen_audit BEFORE INSERT ON crm_audit "+
    "WHEN NEW.action='sales_deals.reopen' "+
    "BEGIN SELECT RAISE(ABORT,'fixture reopen audit'); END;"));
  try{
    const failed=await salesV2("ReopenDeal",admin,{dealId,reasonCode:"revisit",
      precondition:{expectedVersion:6n,idempotencyKey:"r72-failed-reopen"}});
    assert.equal(failed.status,503);
    assert.equal(failed.body.message,"SALES_COMMIT_UNCONFIRMED");
  }finally{await sql("DROP TRIGGER fixture_fail_reopen_audit");}
  assert.equal((await salesV2("GetDeal",viewer,{dealId})).body.deal?.version,6n);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
    "AND subject_id='"+dealId+"'"))[0].n,reopenAuditBefore);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
    "WHERE tenant='tenant-a'"))[0].n,reopenReceiptBefore);
  assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_events "+
    "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'"))[0].n,reopenEventsBefore);
  console.log(JSON.stringify({event:"crm.native-r72-deal-lifecycle",result:"PASS",
    separate_close_reopen:true,closed_readonly:true,actor_audit:true,
    append_only_history:true,cursor:true,linked_task:true,
    replay_after_reclose:true,stale_cas:true,rollback:true}));
  console.log(JSON.stringify({event:"crm.native-r2a-sales-linked-actor-audit",result:"PASS",
    all_sales_actions:true,linked_task_delivery:true,late_batch_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-sales-v2",result:"PASS",
  rpc_methods:["CreatePipeline","UpdatePipeline","GetPipeline","ListPipelines","ListStageDeals",
    "CreateDeal","UpdateDeal","AdvanceStage","CloseDeal","ReopenDeal",
    "ListDealEvents","GetDeal","ListDeals"],
  generated_binary:true,original_owner:"crm.sales",signed_access:true,tenant_ab:true,
  viewer_read:true,viewer_write_denied:true,exact_money:true,
  before_limit_filters:true,cas:true,idempotent_receipts:true,
  atomic_sales_activities:true,no_extra_audit:true,published_active:true}));

 console.log(JSON.stringify({event:"crm.native-g3-party-v3",
   result:"PASS",generated_binary:true,owner:"crm.parties",
   rpc_methods:["CreateParty","UpdateParty","GetParty","ListParties"],
   signed_access:true,tenant_ab:true,viewer_mutation_denied:true,
   original_d1_cas:true,d1_receipts:true,d1_audit:true,
   published_active:true}));

 assert.equal((await call("/crm.future.v9.FutureService/NotRegistered",viewer,"POST")).status,501);

 // Privacy intake: execute original domain transitions through binary Connect.
 const privacy=signedReadOnly(privacyV2Path,binaryPrivacyV2,decodePrivacyV2);
 const caseId="privacy-case-"+randomUUID();
 const draftInput={caseId,kind:1,precondition:{idempotencyKey:"privacy-create-main"}};
 assert.equal((await privacy("CreatePrivacyCase",admin,{...draftInput,kind:0})).body.code,"invalid_argument");
 assert.equal((await privacy("CreatePrivacyCase",admin,{...draftInput,kind:999})).body.code,"invalid_argument");
 assert.equal((await privacy("CreatePrivacyCase",admin,{...draftInput,
   precondition:{idempotencyKey:"privacy-create-invalid",expectedVersion:1n}})).body.code,"invalid_argument");
 const draft=await privacy("CreatePrivacyCase",admin,draftInput,{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test",
   "x-request-id":"forged-privacy-request"});
 assert.equal(draft.status,200,describeProtobuf(draft.body));
 assert.equal(draft.body.privacyCase?.caseId,caseId);
 assert.equal(draft.body.privacyCase?.status,1);
 assert.equal(draft.body.privacyCase?.version,1n);
 assert.equal(draft.body.privacyCase?.subjectVerified,false);
 assert.equal(draft.body.receipt?.requestId,draft.headers.get("x-request-id"));
 assert.equal(draft.body.receipt?.replayed,false);
 const originalAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'"))[0].n;
 assert.equal(originalAudit,1);
 assert.equal((await privacy("CreatePrivacyCase",admin,draftInput)).body.receipt?.replayed,true);
 assert.equal((await privacy("CreatePrivacyCase",admin,{...draftInput,kind:4})).body.code,"aborted");
 assert.equal((await privacy("GetPrivacyCase",viewer,{caseId})).body.privacyCase?.version,1n);
 assert.equal((await privacy("GetPrivacyCase",tenantB,{caseId})).body.code,"not_found");
 assert.equal((await privacy("GetPrivacyCase",viewer,{caseId:"bad/id"})).body.code,"invalid_argument");
 const submit={caseId,precondition:{expectedVersion:1n,idempotencyKey:"privacy-submit-main"}};
 assert.equal((await privacy("SubmitPrivacyCase",tenantB,submit)).body.code,"not_found");
 assert.equal((await privacy("SubmitPrivacyCase",admin,{...submit,
   precondition:{expectedVersion:9n,idempotencyKey:"privacy-submit-stale"}})).body.code,"aborted");
 const submitted=await privacy("SubmitPrivacyCase",knownService,submit);
 assert.equal(submitted.status,200,describeProtobuf(submitted.body));
 assert.equal(submitted.body.privacyCase?.status,2);
 assert.equal(submitted.body.receipt?.resourceVersion,2n);
 assert.equal((await privacy("SubmitPrivacyCase",admin,submit)).body.receipt?.replayed,true);
 assert.equal((await privacy("SubmitPrivacyCase",admin,{...submit,
   precondition:{expectedVersion:2n,idempotencyKey:"privacy-submit-again"}})).body.code,"failed_precondition");
 const cancel={caseId,precondition:{expectedVersion:2n,idempotencyKey:"privacy-cancel-main"}};
 assert.equal((await privacy("CancelPrivacyCase",tenantB,cancel)).body.code,"not_found");
 const cancelled=await privacy("CancelPrivacyCase",admin,cancel);
 assert.equal(cancelled.status,200,describeProtobuf(cancelled.body));
 assert.equal(cancelled.body.privacyCase?.status,15);
 assert.equal(cancelled.body.receipt?.resourceVersion,3n);
 assert.equal((await privacy("CancelPrivacyCase",admin,cancel)).body.receipt?.replayed,true);
 // Replaying the earlier submit returns its original receipt, not latest state.
 const submitReplay=await privacy("SubmitPrivacyCase",admin,submit);
 assert.equal(submitReplay.body.receipt?.replayed,true);
 assert.equal(submitReplay.body.privacyCase?.status,2);
 assert.equal((await privacy("GetPrivacyCase",viewer,{caseId})).body.privacyCase?.status,15);
 assert.equal((await sql("SELECT version FROM crm_privacy_case_records WHERE tenant='tenant-a' AND id='"+caseId+"'"))[0].version,3);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts WHERE tenant='tenant-a'"))[0].n,3);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'"))[0].n,3);
 assert.deepEqual((await sql("SELECT action,subject_id,request_id,actor FROM crm_audit WHERE "+
   "tenant_id='tenant-a' AND subject_id='"+caseId+"' ORDER BY rowid")),[
   {action:"customer_privacy.create",subject_id:caseId,
     request_id:draft.headers.get("x-request-id"),actor:"native-a@example.test"},
   {action:"customer_privacy.submit",subject_id:caseId,
     request_id:submitted.headers.get("x-request-id"),actor:"service:ci-fixture.access"},
   {action:"customer_privacy.cancel",subject_id:caseId,
     request_id:cancelled.headers.get("x-request-id"),actor:"native-a@example.test"},
 ],"Committed actions must preserve their own verified Access actor and request");
 assert.deepEqual((await sql("SELECT created_by,actor FROM crm_privacy_case_records "+
   "WHERE tenant='tenant-a' AND id='"+caseId+"'")),[
   {created_by:"native-a@example.test",actor:"native-a@example.test"},
 ],"Creator is immutable and the last transition actor is separate");
 const failingTransitionId="privacy-audit-transition-"+randomUUID();
 assert.equal((await privacy("CreatePrivacyCase",admin,{
   caseId:failingTransitionId,kind:1,
   precondition:{idempotencyKey:"privacy-audit-ready-transition"}})).status,200);
 const privacySnapshot=async()=>(await sql("SELECT "+
   "(SELECT json_group_array(json_array(tenant,id,state,version,created_by,actor)) "+
   "FROM crm_privacy_case_records) AS states,"+
   "(SELECT COUNT(*) FROM crm_privacy_case_receipts) AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit WHERE tenant_id='tenant-a') AS audits"));
 const privacyBeforeDenials=(await privacySnapshot());
 for(const token of [null,viewer,unknownService]){
   assert.ok((await privacy("CreatePrivacyCase",token,draftInput)).status>=400);
   assert.ok((await privacy("SubmitPrivacyCase",token,submit)).status>=400);
   assert.ok((await privacy("CancelPrivacyCase",token,cancel)).status>=400);
 }
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await privacy("SubmitPrivacyCase",knownService,submit)).status,401,
   "Inactive service cannot replay a committed Privacy write");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await privacy("SubmitPrivacyCase",knownService,submit)).status,403,
   "Authorization must precede idempotent replay");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 assert.equal((await privacy("SubmitPrivacyCase",tenantB,submit)).body.code,"not_found");
 assert.equal((await privacy("SubmitPrivacyCase",admin,{
   ...submit,precondition:{expectedVersion:1n,idempotencyKey:"privacy-audit-denied-terminal"}
 })).body.code,"failed_precondition",
   "A cancelled case must reject a second Submit before version conflict");
 assert.deepEqual((await privacySnapshot()),privacyBeforeDenials,
   "Denied, cross-tenant and stale Privacy writes cannot alter state, receipts or audit");
 // Reject audit INSERT after the owner wrote state and receipt.
 // No failure injection is compiled into the production Worker.
 (await sql("CREATE TRIGGER fixture_fail_privacy_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('privacy-audit-failed-create','"+failingTransitionId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;"));
 try{
   assertUnconfirmed(await privacy("CreatePrivacyCase",admin,{
     caseId:"privacy-audit-failed-create",kind:1,
     precondition:{idempotencyKey:"privacy-audit-failed-create-key"}}),"PRIVACY_CASE_WRITE_UNCONFIRMED");
   assertUnconfirmed(await privacy("SubmitPrivacyCase",knownService,{
     caseId:failingTransitionId,
     precondition:{expectedVersion:1n,idempotencyKey:"privacy-audit-failed-submit-key"}
   }),"PRIVACY_CASE_WRITE_UNCONFIRMED");
   assert.deepEqual((await privacySnapshot()),privacyBeforeDenials,
     "Audit failure must roll back Privacy state, receipt and audit together");
 }finally{await sql("DROP TRIGGER fixture_fail_privacy_audit");}
 console.log(JSON.stringify({event:"crm.native-r2a-privacy-actor-audit",result:"PASS",
   human_create:true,service_submit:true,human_cancel:true,cross_actor_replay:true,
   authorization_before_replay:true,forged_headers_ignored:true,tenant_ab:true,
   stale_no_write:true,sql_failure_rollback:true}));
 for(const suffix of ["aaa","zzz"]){
   const result=await privacy("CreatePrivacyCase",admin,{caseId:"privacy-"+suffix,kind:4,
     precondition:{idempotencyKey:"privacy-page-"+suffix}});
   assert.equal(result.status,200,describeProtobuf(result.body));
 }
 assert.equal((await privacy("ListPrivacyCases",viewer,{page:{pageSize:101}})).body.code,"invalid_argument");
 const allCases=await collectPages(privacy,"ListPrivacyCases","cases");
 assert.equal(allCases.length,4);
 assert.deepEqual(allCases.map(x=>x.caseId),
   ["privacy-aaa",caseId,"privacy-zzz",failingTransitionId].sort());
 assert.equal((await privacy("ListPrivacyCases",tenantB,{page:{pageSize:1}})).body.cases.length,0);
 assert.equal((await privacy("ListPrivacyCases",viewer,{page:{pageSize:1,pageToken:"privacy-zzz"}})).body.cases.length,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'"))[0].n,3,
   "read and denied requests must not write audit for the case");
 console.log(JSON.stringify({event:"crm.native-privacy-v2",result:"PASS",
   original_owner:"crm.customer-privacy",tenant_ab:true,cas:true,replay:true,
   page_bounds:true,d1_audit:true,subject_verification:"NOT_CONNECTED"}));

 // R4a human self-verification: signed Access email + independently verified
 // original ContactPoint + Person owner; never trust service or dev-agent proof.
 const r4Contact="privacy-own-contact-"+randomUUID();
 const r4Created=await contactV2("CreateContactPoint",admin,{
   contactPointId:r4Contact,partyId:pagingPersonId,kind:1,
   value:"native-a@example.test",preferred:false,
   precondition:{idempotencyKey:"r4-privacy-contact-create-"+r4Contact}});
 assert.equal(r4Created.status,200,describeProtobuf(r4Created.body));
 assert.equal((await contactV2("VerifyOwnEmailContactPoint",admin,{
   contactPointId:r4Contact,
   precondition:{expectedVersion:1n,idempotencyKey:"r4-privacy-contact-verify-"+r4Contact}
 })).status,200);
 const ownCaseId="privacy-own-"+randomUUID();
 const ownCreate=await privacy("CreatePrivacyCase",admin,{
   caseId:ownCaseId,kind:2,
   precondition:{idempotencyKey:"r4-privacy-create-"+ownCaseId}});
 assert.equal(ownCreate.status,200,describeProtobuf(ownCreate.body));
 const ownSubmit=await privacy("SubmitPrivacyCase",admin,{
   caseId:ownCaseId,
   precondition:{expectedVersion:1n,idempotencyKey:"r4-privacy-submit-"+ownCaseId}});
 assert.equal(ownSubmit.status,200,describeProtobuf(ownSubmit.body));
 const ownProof={
   caseId:ownCaseId,partyId:pagingPersonId,contactPointId:r4Contact,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-verify-"+ownCaseId}
 };
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",knownService,ownProof))
   .body.code,"permission_denied","Service cannot impersonate human");
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",viewer,ownProof))
   .body.code,"permission_denied","Another signed email cannot verify a subject");
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",tenantB,ownProof))
   .body.code,"permission_denied","Tenant B cannot verify tenant A's ContactPoint");
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",admin,{
   ...ownProof,partyId:id,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-wrong-party"}
 })).body.code,"permission_denied","An Organization cannot be a verified Person");
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",admin,{
   ...ownProof,contactPointId:r3Contact,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-wrong-contact"}
 })).body.code,"permission_denied","A verified email on another Party is not ownership");
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",admin,{
   ...ownProof,contactPointId:"bad/id"
 })).body.code,"invalid_argument");
 assert.equal((await privacy("GetPrivacyCase",viewer,{caseId:ownCaseId}))
   .body.privacyCase?.subjectVerified,false);
 // A current-state scope read is unavailable before verification.
 assert.equal((await privacy("PreviewOwnPrivacyScope",admin,{caseId:ownCaseId}))
   .body.code,"failed_precondition");
 assert.equal((await privacy("PreviewOwnPrivacyScope",admin,{caseId:"bad/id"}))
   .body.code,"invalid_argument");
 const ownVerified=await privacy("VerifyOwnPrivacyCaseSubject",admin,ownProof,{
   "x-crm-tenant":"tenant-b","x-crm-actor":"forged@example.test",
   "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(ownVerified.status,200,describeProtobuf(ownVerified.body));
 assert.equal(ownVerified.body.privacyCase?.status,3);
 assert.equal(ownVerified.body.privacyCase?.subjectVerified,true);
 assert.equal(ownVerified.body.privacyCase?.version,3n);
 assert.equal(ownVerified.body.receipt?.resourceVersion,3n);
 assert.equal(ownVerified.body.receipt?.replayed,false);
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",admin,ownProof))
   .body.receipt?.replayed,true,"Exact intent replay uses original snapshot");
 assert.equal((await privacy("GetPrivacyCase",viewer,{caseId:ownCaseId}))
   .body.privacyCase?.subjectVerified,true);
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",admin,{
   ...ownProof,precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-verify-stale"}
 })).body.code,"failed_precondition");
 // R4a phase 2: the exact original canonical registry contains nine owners.
 // Only Parties and ContactPoints have current read-through contributions.
 // No durable ScopeSnapshot can be finalized from these two.
 const previewAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit "+
   "WHERE tenant_id='tenant-a' AND subject_id='"+ownCaseId+"'"))[0].n;
 assert.equal((await privacy("PreviewOwnPrivacyScope",viewer,{caseId:ownCaseId}))
   .body.code,"permission_denied","Another human has no access to own-scope preview");
 assert.equal((await privacy("PreviewOwnPrivacyScope",knownService,{caseId:ownCaseId}))
   .body.code,"permission_denied","Service identity cannot impersonate the human");
 assert.equal((await privacy("PreviewOwnPrivacyScope",tenantB,{caseId:ownCaseId}))
   .body.code,"not_found","Tenant B must not see tenant A's scope");
 const preview=await privacy("PreviewOwnPrivacyScope",admin,{caseId:ownCaseId},{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test"});
 assert.equal(preview.status,200,describeProtobuf(preview.body));
 assert.equal(preview.body.complete,false,"Never claim a nine-owner discovery snapshot");
 assert.equal(preview.body.caseId,ownCaseId);
 assert.equal(preview.body.caseVersion,3n);
 assert.equal(preview.body.canonicalPartyId,pagingPersonId);
 assert.equal(preview.body.registryDigest?.length,32);
 assert.deepEqual(preview.body.owners.map(o=>o.ownerModuleId),
   ["crm.contact-points","crm.parties"],"Use the canonical owner registry order");
 assert.equal(preview.body.owners.length,2);
 assert.equal(preview.body.missingOwnerModuleIds.length,7);
 assert.ok(preview.body.missingOwnerModuleIds.includes("crm.consents"));
 assert.ok(preview.body.missingOwnerModuleIds.includes("crm.identity-resolution"));
 assert.ok(preview.body.owners.every(o=>o.contributionDigest?.length===32));
 assert.ok(preview.body.resources.some(x=>x.resourceId===pagingPersonId
   && x.ownerModuleId==="crm.parties"&&x.resourceVersion===1n));
 assert.ok(preview.body.resources.some(x=>x.resourceId===r4Contact
   && x.ownerModuleId==="crm.contact-points"&&x.resourceVersion===2n));
 assert.equal(preview.body.resources.length,
   preview.body.owners.reduce((sum,owner)=>sum+owner.resourceCount,0));
 assert.deepEqual((await privacy("PreviewOwnPrivacyScope",admin,{caseId:ownCaseId})).body,
   preview.body,"Deterministic owner contributions when D1 state is unchanged");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit "+
   "WHERE tenant_id='tenant-a' AND subject_id='"+ownCaseId+"'"))[0].n,
   previewAuditBefore,"Read-only preview must never create an audit write");
 const r4Cancelled=await privacy("CancelPrivacyCase",admin,{
   caseId:ownCaseId,
   precondition:{expectedVersion:3n,idempotencyKey:"r4-privacy-cancel-after-verify"}
 });
 assert.equal(r4Cancelled.status,200,describeProtobuf(r4Cancelled.body));
 assert.equal(r4Cancelled.body.privacyCase?.status,15);
 assert.equal((await privacy("PreviewOwnPrivacyScope",admin,{caseId:ownCaseId}))
   .body.code,"failed_precondition","Cancelled case cannot drive export preview");
 assert.equal(r4Cancelled.body.privacyCase?.subjectVerified,true,
   "Cancelling an authenticated case retains its original verified binding");
 assert.deepEqual((await sql("SELECT action,actor,request_id FROM crm_audit "+
   "WHERE tenant_id='tenant-a' AND subject_id='"+ownCaseId+"' ORDER BY rowid")).map(row=>row.action),
   ["customer_privacy.create","customer_privacy.submit",
     "customer_privacy.subject.verify_own","customer_privacy.cancel"]);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts "+
   "WHERE tenant='tenant-a' AND key='r4-privacy-verify-"+ownCaseId+"'"))[0].n,1);

 // Audit insertion failure must roll back the new state, receipt and actor
 // together, through the SAME D1 trigger as all other Privacy transitions.
 const failProofId="privacy-own-failure-"+randomUUID();
 assert.equal((await privacy("CreatePrivacyCase",admin,{
   caseId:failProofId,kind:1,
   precondition:{idempotencyKey:"r4-privacy-fail-create-"+failProofId}})).status,200);
 assert.equal((await privacy("SubmitPrivacyCase",admin,{
   caseId:failProofId,
   precondition:{expectedVersion:1n,idempotencyKey:"r4-privacy-fail-submit-"+failProofId}
 })).status,200);
 const failVerification={
   caseId:failProofId,partyId:pagingPersonId,contactPointId:r4Contact,
   precondition:{expectedVersion:2n,idempotencyKey:"r4-privacy-fail-verify-"+failProofId}};
 await sql("CREATE TRIGGER fixture_r4_privacy_audit_fail BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+failProofId+"' AND NEW.action='customer_privacy.subject.verify_own' "+
   "BEGIN SELECT RAISE(ABORT,'test audit failure'); END");
 try {
   assertUnconfirmed(await privacy("VerifyOwnPrivacyCaseSubject",admin,failVerification),"PRIVACY_CASE_WRITE_UNCONFIRMED");
   assert.equal((await privacy("GetPrivacyCase",admin,{caseId:failProofId}))
     .body.privacyCase?.subjectVerified,false);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts "+
     "WHERE tenant='tenant-a' AND key='r4-privacy-fail-verify-"+failProofId+"'"))[0].n,0);
 }finally{await sql("DROP TRIGGER fixture_r4_privacy_audit_fail");}
 assert.equal((await privacy("VerifyOwnPrivacyCaseSubject",admin,failVerification)).status,200,
   "Retry after atomic rollback must commit exactly once");
 console.log(JSON.stringify({event:"crm.native-r4a-privacy-scope-preview",result:"PASS",
   scope_complete:false,covered_owners:2,missing_owners:7,
   original_owner_registry:true,owner_digests:true,tenant_isolation:true,
   unauthorized_denied:true,read_only_no_audit:true,not_export_eligible:true}));
 console.log(JSON.stringify({event:"crm.native-r4a-privacy-signed-subject",result:"PASS",
   owner:"crm.customer-privacy",trusted_contact:"access-email",human_self_only:true,
   subject_verified:true,service_denied:true,tenant_isolation:true,
   forged_headers_ignored:true,d1_cas:true,receipt_replay:true,
   old_legacy_runtime:false,audit_failure_atomic:true}));

 // Immutable configuration only: no provider I/O or Party mutation.
 const enrichment=signedReadOnly(enrichmentV2Path,binaryEnrichmentV2,decodeEnrichmentV2);
 const profileInput={
   providerKey:"company_registry",adapterKind:"registry_http_v1",adapterContractVersion:"1.0.0",
   purposeCodes:["customer_profile_enrichment","due_diligence"],
   licenseId:"Registry commercial data licence v3",permittedUseClass:"customer_master_review",
   residencyRegion:"eu",retentionDays:30,rawPayloadPolicy:1,
   credentialHandleAliases:["registry_primary"],
   effectiveAt:{seconds:1n,nanos:123000000},expiresAt:{seconds:2n},
   precondition:{idempotencyKey:"enrichment-profile-main"}
 };
 const enrichmentAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const partyCountBefore=(await sql("SELECT COUNT(*) AS n FROM crm_party_records"))[0].n;
 for(const bad of [
   {rawPayloadPolicy:0},{rawPayloadPolicy:999},{effectiveAt:undefined},
   {effectiveAt:{seconds:1n,nanos:1}},{effectiveAt:{seconds:-1n}},
   {effectiveAt:{seconds:253402300800n}},{effectiveAt:{seconds:1n,nanos:1000000000}},
   {retentionDays:3651},{expiresAt:{seconds:0n}},
   {precondition:{idempotencyKey:"enrichment-invalid-version",expectedVersion:1n}}
 ]){
   const result=await enrichment("PublishProviderProfile",admin,{...profileInput,...bad});
   assert.equal(result.body.code,"invalid_argument",describeProtobuf(result.body));
 }
 const profileResult=await enrichment("PublishProviderProfile",admin,profileInput,{
   "x-crm-actor":"forged@example.test","x-crm-tenant":"tenant-b",
   "cf-access-authenticated-user-email":"forged@example.test",
   "x-request-id":"forged-enrichment-request"
 });
 assert.equal(profileResult.status,200,describeProtobuf(profileResult.body));
 const profileId=profileResult.body.profile.profileVersionId;
 assert.ok(profileId);
 assert.equal(profileResult.body.profile.effectiveAt.nanos,123000000);
 assert.equal(profileResult.body.receipt.resourceVersion,1n);
 assert.equal(profileResult.body.receipt.requestId,profileResult.headers.get("x-request-id"));
 assert.equal(profileResult.body.receipt.replayed,false);
 assert.equal((await enrichment("PublishProviderProfile",knownService,profileInput)).body.receipt.replayed,true);
 assert.equal((await enrichment("PublishProviderProfile",admin,{...profileInput,retentionDays:31})).body.code,"aborted");
 assert.equal((await enrichment("GetProviderProfile",viewer,{profileVersionId:profileId})).body.profile.profileVersionId,profileId);
 assert.equal((await enrichment("GetProviderProfile",tenantB,{profileVersionId:profileId})).body.code,"not_found");
 const enrichmentAuditBeforeProfileReceiptReads=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const profileReceiptRead=await enrichment("GetProviderProfile",viewer,{
   profileVersionId:"",receiptIdempotencyKey:profileInput.precondition.idempotencyKey});
 assert.equal(profileReceiptRead.status,200,describeProtobuf(profileReceiptRead.body));
 assert.equal(profileReceiptRead.body.profile.profileVersionId,profileId);
 assert.equal(profileReceiptRead.body.confirmedMutation?.originalRequestId,profileResult.headers.get("x-request-id"));
 assert.equal(profileReceiptRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal(profileReceiptRead.body.confirmedMutation?.requestId,profileReceiptRead.headers.get("x-request-id"));
 assert.equal((await enrichment("GetProviderProfile",tenantB,{
   profileVersionId:"",receiptIdempotencyKey:profileInput.precondition.idempotencyKey})).body.code,"not_found");
 assert.equal((await enrichment("GetProviderProfile",viewer,{profileVersionId:""})).body.code,"invalid_argument");
 assert.equal((await enrichment("GetProviderProfile",viewer,{
   profileVersionId:"",receiptIdempotencyKey:"short"})).body.code,"invalid_argument");
 assert.equal((await enrichment("GetProviderProfile",viewer,{
   profileVersionId:"",receiptIdempotencyKey:"enrichment-profile-missing"})).body.code,"not_found");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,enrichmentAuditBeforeProfileReceiptReads,
   "Profile receipt lookup is read-only");
 const mappingInput={
   mappingKey:"party_display_name",providerProfileVersionId:profileId,
   providerResponseFieldPath:"organization.legal_name",maximumSuggestionsPerResponse:1,
   confidenceRequired:true,precondition:{idempotencyKey:"enrichment-mapping-main"}
 };
 assert.equal((await enrichment("PublishMapping",tenantB,mappingInput)).body.code,"not_found");
 assert.equal((await enrichment("PublishMapping",admin,{...mappingInput,maximumSuggestionsPerResponse:0})).body.code,"invalid_argument");
 const mappingResult=await enrichment("PublishMapping",knownService,mappingInput);
 assert.equal(mappingResult.status,200,describeProtobuf(mappingResult.body));
 const mappingId=mappingResult.body.mapping.mappingVersionId;
 assert.equal(mappingResult.body.mapping.providerProfileVersionId,profileId);
 assert.equal(mappingResult.body.receipt.replayed,false);
 assert.equal((await enrichment("PublishMapping",admin,mappingInput)).body.receipt.replayed,true);
 assert.equal((await enrichment("PublishMapping",admin,{...mappingInput,confidenceRequired:false})).body.code,"aborted");
 assert.equal((await enrichment("PublishMapping",admin,{...mappingInput,
   precondition:{idempotencyKey:"enrichment-profile-main"}})).body.code,"aborted");
 assert.equal((await enrichment("GetMapping",viewer,{mappingVersionId:mappingId})).body.mapping.mappingVersionId,mappingId);
 assert.equal((await enrichment("GetMapping",tenantB,{mappingVersionId:mappingId})).body.code,"not_found");
 const enrichmentAuditBeforeMappingReceiptReads=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const mappingReceiptRead=await enrichment("GetMapping",viewer,{
   mappingVersionId:"",receiptIdempotencyKey:mappingInput.precondition.idempotencyKey});
 assert.equal(mappingReceiptRead.status,200,describeProtobuf(mappingReceiptRead.body));
 assert.equal(mappingReceiptRead.body.mapping.mappingVersionId,mappingId);
 assert.equal(mappingReceiptRead.body.confirmedMutation?.originalRequestId,mappingResult.headers.get("x-request-id"));
 assert.equal(mappingReceiptRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal(mappingReceiptRead.body.confirmedMutation?.requestId,mappingReceiptRead.headers.get("x-request-id"));
 assert.equal((await enrichment("GetMapping",tenantB,{
   mappingVersionId:"",receiptIdempotencyKey:mappingInput.precondition.idempotencyKey})).body.code,"not_found");
 assert.equal((await enrichment("GetMapping",viewer,{
   mappingVersionId:"",receiptIdempotencyKey:"enrichment-profile-main"})).body.code,"not_found");
 assert.equal((await enrichment("GetMapping",viewer,{
   mappingVersionId:mappingId,receiptIdempotencyKey:"enrichment-profile-main"
 })).body.confirmedMutation,undefined,"a profile receipt cannot confirm a mapping");
 assert.equal((await enrichment("GetProviderProfile",viewer,{
   profileVersionId:"",receiptIdempotencyKey:mappingInput.precondition.idempotencyKey
 })).body.code,"not_found","a mapping receipt cannot confirm a provider profile");
 assert.equal((await enrichment("GetMapping",viewer,{mappingVersionId:""})).body.code,"invalid_argument");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,enrichmentAuditBeforeMappingReceiptReads,
   "Mapping receipt lookup is read-only");
 await sql("ALTER TABLE crm_enrichment_receipts RENAME TO fixture_unavailable_enrichment_receipts");
 try {
   assert.equal((await enrichment("GetProviderProfile",viewer,{
     profileVersionId:"",receiptIdempotencyKey:profileInput.precondition.idempotencyKey})).status,503,
     "an unavailable receipt source must not confirm a profile write");
   assert.equal((await enrichment("GetProviderProfile",viewer,{profileVersionId:profileId})).status,200,
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
   assert.ok((await enrichment("PublishProviderProfile",token,profileInput)).status>=400);
   assert.ok((await enrichment("PublishMapping",token,mappingInput)).status>=400);
 }
 await sql("UPDATE crm_users SET active=0 WHERE email='service:ci-fixture.access'");
 assert.equal((await enrichment("PublishMapping",knownService,mappingInput)).status,401,
   "Inactive service cannot replay a committed Enrichment mapping");
 await sql("UPDATE crm_users SET active=1,role='viewer' WHERE email='service:ci-fixture.access'");
 assert.equal((await enrichment("PublishMapping",knownService,mappingInput)).status,403,
   "Permission must precede replay of immutable mapping");
 await sql("UPDATE crm_users SET role='admin' WHERE email='service:ci-fixture.access'");
 const tenantAEnrichment=async()=>(await sql("SELECT "+
   "(SELECT COUNT(*) FROM crm_enrichment_profiles WHERE tenant='tenant-a') AS profiles,"+
   "(SELECT COUNT(*) FROM crm_enrichment_mappings WHERE tenant='tenant-a') AS mappings,"+
   "(SELECT COUNT(*) FROM crm_enrichment_receipts WHERE tenant='tenant-a') AS receipts,"+
   "(SELECT COUNT(*) FROM crm_audit WHERE tenant_id='tenant-a') AS audits"));
 const tenantABeforeOtherTenant=(await tenantAEnrichment());
 assert.equal((await enrichment("PublishProviderProfile",tenantB,profileInput)).status,200,
   "Separate tenant may publish its own independent immutable definition");
 assert.equal((await enrichment("GetMapping",tenantB,{mappingVersionId:mappingId})).body.code,"not_found");
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
   assertUnconfirmed(await enrichment("PublishProviderProfile",admin,{
     ...profileInput,retentionDays:37,
     precondition:{idempotencyKey:"enrichment-profile-audit-failure"}
   }),"ENRICHMENT_WRITE_UNCONFIRMED");
   assertUnconfirmed(await enrichment("PublishMapping",knownService,{
     ...mappingInput,providerResponseFieldPath:"organization.alternate_legal_name",
     precondition:{idempotencyKey:"enrichment-mapping-audit-failure"}
   }),"ENRICHMENT_WRITE_UNCONFIRMED");
   assert.deepEqual((await enrichmentSnapshot()),beforeAuditFailure,
     "Audit failure must atomically roll back published state and receipt");
 }finally{await sql("DROP TRIGGER fixture_fail_enrichment_audit");}
 console.log(JSON.stringify({event:"crm.native-r2a-enrichment-actor-audit",result:"PASS",
   human_profile:true,service_mapping:true,cross_actor_replay:true,
   forged_headers_ignored:true,authorization_before_replay:true,
   tenant_ab:true,sql_failure_rollback:true,immutable_created_by:true}));
 for(const index of [1,2]){
   const published=await enrichment("PublishProviderProfile",admin,{
     ...profileInput,retentionDays:30+index,
     precondition:{idempotencyKey:"enrichment-profile-page-"+index}});
   assert.equal(published.status,200,describeProtobuf(published.body));
   const mapped=await enrichment("PublishMapping",admin,{
     ...mappingInput,providerProfileVersionId:published.body.profile.profileVersionId,
     precondition:{idempotencyKey:"enrichment-mapping-page-"+index}});
   assert.equal(mapped.status,200,describeProtobuf(mapped.body));
 }
 const profiles=await collectPages(enrichment,"ListProviderProfiles","profiles");
 const mappings=await collectPages(enrichment,"ListMappings","mappings");
 assert.equal(profiles.length,3);assert.equal(mappings.length,3);
 assert.equal(new Set(profiles.map(x=>x.profileVersionId)).size,3);
 assert.equal(new Set(mappings.map(x=>x.mappingVersionId)).size,3);
 for(const [method,field] of [["ListProviderProfiles","profiles"],["ListMappings","mappings"]]){
   assert.equal((await enrichment(method,viewer,{page:{pageSize:101}})).body.code,"invalid_argument");
   const isolated=await enrichment(method,tenantB,{page:{pageSize:1}});
   assert.equal(isolated.status,200,describeProtobuf(isolated.body));
   assert.equal(isolated.body[field].length,method==="ListProviderProfiles"?1:0,
     "Tenant B sees only its own deliberate profile fixture, never A's mappings");
   if(method==="ListProviderProfiles"){
     assert.equal(isolated.body.profiles[0].profileVersionId,profileId,
       "Content-addressed version ID may match across independently scoped tenants");
   }
   assert.equal((await enrichment(method,viewer,{page:{pageToken:"bad/id"}})).body.code,"invalid_argument");
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_enrichment_receipts WHERE tenant='tenant-a'"))[0].n,6);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_enrichment_profiles WHERE tenant='tenant-a'"))[0].n,3);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_enrichment_mappings WHERE tenant='tenant-a'"))[0].n,3);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,enrichmentAuditBefore+7,
   "six tenant-a publishes and one isolated tenant-b publish write audit; replay and denials never do");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records"))[0].n,partyCountBefore);
 console.log(JSON.stringify({event:"crm.native-enrichment-v2",result:"PASS",
   original_owner:"crm.customer-enrichment",immutable:true,tenant_ab:true,replay:true,
   page_bounds:true,d1_audit:true,timestamp_precision:true,provider_execution:"NOT_CONNECTED"}));


 // Account-rooted composition uses the original owner values and never writes.
 const overviewCall=signedReadOnly(overviewV2Path,binaryOverviewV2,decodeOverviewV2);
 const overviewAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 assert.equal((await overviewCall("GetAccountOverview",tenantB,{accountId})).body.code,"not_found");
 assert.equal((await overviewCall("GetAccountOverview",viewer,{accountId:"bad/id"})).body.code,"invalid_argument");
 assert.equal((await overviewCall("GetAccountOverview",viewer,{accountId:"missing-account"})).body.code,"not_found");
 const overviewResult=await overviewCall("GetAccountOverview",viewer,{accountId});
 assert.equal(overviewResult.status,200,describeProtobuf(overviewResult.body));
 const card=overviewResult.body.overview;
 assert.equal(card.account.accountId,accountId);
 assert.equal(card.primaryParty.partyId,id);
 assert.equal(card.perCollectionLimit,100);
 assert.equal(card.truncated.contactPoints,false);
 assert.equal(card.truncated.relationships,false);
 assert.equal(card.truncated.deals,false);
 assert.equal(card.truncated.tasks,false);
 assert.deepEqual(card.primaryParty,(await v3("GetParty",viewer,{partyId:id})).body.party);
 assert.deepEqual(card.contactPoints,await collectPages(contactV2,"ListContactPoints","contactPoints",{partyId:id}));
 assert.deepEqual(card.relationships,await collectPages(relationV2,"ListPartyRelationships","relationships",{partyId:id}));
 assert.deepEqual(card.deals,(await collectPages(salesV2,"ListDeals","deals")).filter(item=>item.accountId===accountId));
 assert.deepEqual(card.tasks,await collectPages(taskV2,"ListTasks","tasks",{accountId}));
 assert.ok(card.contactPoints.some(item=>item.contactPointId===contactId));
 assert.ok(card.relationships.some(item=>item.relationshipId===rMid));
 assert.ok(card.deals.some(item=>item.dealId===dealId));
 assert.ok(card.tasks.some(item=>item.taskId===taskMid));
 assert.ok(card.tasks.every(item=>item.taskId!==taskOther));
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,overviewAudit);
 // Sales-created tasks retain their original Deal reference, and their D1 Account.
 const linkedTaskId=(await sql("SELECT task_id FROM crm_sales_link_delivery_records WHERE tenant='tenant-a' AND deal_id='"+
   dealId+"' AND task_id IS NOT NULL ORDER BY task_id LIMIT 1"))[0]?.task_id;
 assert.ok(linkedTaskId,"Sales fixture must create a linked Task");
 const linkedTask=await taskV2("GetTask",viewer,{taskId:linkedTaskId});
 assert.equal(linkedTask.status,200,describeProtobuf(linkedTask.body));
 assert.equal(linkedTask.body.task.accountId,accountId);
 assert.ok(card.tasks.some(task=>task.taskId===linkedTaskId));
 assert.equal((await taskV2("GetTask",tenantB,{taskId:linkedTaskId})).status,404);
 const completeLinked=await taskV2("CompleteTask",admin,{taskId:linkedTaskId,
   precondition:{idempotencyKey:"overview-linked-task-complete",expectedVersion:linkedTask.body.task.version}});
 assert.equal(completeLinked.status,200,describeProtobuf(completeLinked.body));
 assert.equal(completeLinked.body.task.accountId,accountId);
 assert.equal(completeLinked.body.task.status,2);
 // Exercise the composition bound with real original ContactPoint mutations.
 for(let index=0;index<101-card.contactPoints.length;index++){
   const created=await contactV2("CreateContactPoint",admin,{
     contactPointId:"overview-boundary-"+String(index).padStart(3,"0"),partyId:id,kind:1,
     value:"overview-"+index+"@example.org",
     precondition:{idempotencyKey:"overview-contact-"+String(index).padStart(3,"0")}});
   assert.equal(created.status,200,describeProtobuf(created.body));
 }
 const boundedAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const boundedCard=(await overviewCall("GetAccountOverview",viewer,{accountId})).body.overview;
 assert.equal(boundedCard.contactPoints.length,100);
 assert.equal(boundedCard.truncated.contactPoints,true);
 assert.equal(boundedCard.truncated.tasks,false);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,boundedAudit);
 console.log(JSON.stringify({event:"crm.native-account-overview-v2",result:"PASS",
   owner:"crm.customer360",account_rooted:true,original_owners:true,
   tenant_ab:true,read_only:true,bounded:true}));
 // CSV operations use the original parser/Party owner, explicit admin permissions,
 // atomic D1 writes and a per-operation receipt in that same database.
 const csv=signedReadOnly(csvV2Path,binaryCsvV2,decodeCsvV2);
 const csvBytes=new TextEncoder().encode(
   'party_id,kind,display_name,resource_version\r\nexternal-one,person,CSV One,1\r\nexternal-two,organization,CSV Two,1\r\n');
 const importAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const importPreview=await csv("PreviewPartyImport",admin,{csv:csvBytes});
 assert.equal(importPreview.status,200,describeProtobuf(importPreview.body));
 assert.equal(importPreview.body.count,2);
 assert.equal(importPreview.body.previouslyImported,0);
 assert.equal(importPreview.body.csvSha256Hex,createHash("sha256").update(csvBytes).digest("hex"));
 assert.equal(new Set(importPreview.body.targetPartyIds).size,2);
 assert.ok(importPreview.body.targetPartyIds.every(id=>!id.startsWith("external-")));
 const otherPreview=await csv("PreviewPartyImport",tenantB,{csv:csvBytes});
 assert.equal(otherPreview.status,200,describeProtobuf(otherPreview.body));
 assert.ok(otherPreview.body.targetPartyIds.every(id=>!importPreview.body.targetPartyIds.includes(id)));
 const applyInput={csv:csvBytes,expectedSha256Hex:importPreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-import-operation-main"}};
 assert.equal((await csv("ApplyPartyImport",admin,{...applyInput,expectedSha256Hex:"0".repeat(64)})).body.code,"aborted");
 assert.equal((await csv("ApplyPartyImport",admin,{...applyInput,
   precondition:{idempotencyKey:"csv-import-invalid-version",expectedVersion:1n}})).body.code,"invalid_argument");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,importAuditBefore,"previews and invalid input are read-only");
 const applied=await csv("ApplyPartyImport",admin,applyInput);
 assert.equal(applied.status,200,describeProtobuf(applied.body));
 assert.equal(applied.body.createdCount,2);assert.equal(applied.body.replayedCount,0);
 assert.equal(applied.body.receipt.replayed,false);
 assert.equal(applied.body.receipt.requestId,applied.headers.get("x-request-id"));
 assert.equal(applied.body.receipt.resourceVersion,1n);
 const importReplay=await csv("ApplyPartyImport",knownService,applyInput);
 assert.equal(importReplay.body.createdCount,0);assert.equal(importReplay.body.replayedCount,2);
 assert.equal(importReplay.body.receipt.replayed,true);

 const csvReceiptAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const csvReceiptInput={csv:new Uint8Array(),receiptIdempotencyKey:applyInput.precondition.idempotencyKey};
 const csvReceiptRead=await csv("PreviewPartyImport",admin,csvReceiptInput);
 assert.equal(csvReceiptRead.status,200,describeProtobuf(csvReceiptRead.body));
 assert.equal(csvReceiptRead.body.csvSha256Hex,applyInput.expectedSha256Hex);
 assert.equal(csvReceiptRead.body.count,2);
 assert.equal(csvReceiptRead.body.previouslyImported,2);
 assert.deepEqual(csvReceiptRead.body.targetPartyIds,[]);
 assert.equal(csvReceiptRead.body.confirmedMutation?.originalRequestId,applied.headers.get("x-request-id"));
 assert.equal(csvReceiptRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal(csvReceiptRead.body.confirmedMutation?.requestId,csvReceiptRead.headers.get("x-request-id"));
 assert.equal((await csv("PreviewPartyImport",tenantB,csvReceiptInput)).body.code,"not_found");
 assert.equal((await csv("PreviewPartyImport",viewer,csvReceiptInput)).status,403,
   "CSV receipt lookup must preserve the existing admin-only permission");
 assert.equal((await csv("PreviewPartyImport",admin,{csv:new Uint8Array()})).body.code,"invalid_argument");
 assert.equal((await csv("PreviewPartyImport",admin,{
   csv:new Uint8Array(),receiptIdempotencyKey:"short"})).body.code,"invalid_argument");
 assert.equal((await csv("PreviewPartyImport",admin,{
   csv:new Uint8Array(),receiptIdempotencyKey:"csv-import-missing-key"})).body.code,"not_found");
 assert.equal((await csv("PreviewPartyImport",admin,{
   csv:csvBytes,receiptIdempotencyKey:applyInput.precondition.idempotencyKey})).body.code,"invalid_argument");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,csvReceiptAuditBefore,
   "CSV receipt lookup must not append audit");
 await sql("ALTER TABLE crm_party_import_receipts RENAME TO fixture_unavailable_party_import_receipts");
 try {
   assert.equal((await csv("PreviewPartyImport",admin,csvReceiptInput)).status,503,
     "an unavailable batch receipt must remain unconfirmed");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_party_import_receipts RENAME TO crm_party_import_receipts");
 }
 assert.equal((await csv("ApplyPartyImport",admin,{...applyInput,
   precondition:{idempotencyKey:"csv-import-another-operation"}})).body.replayedCount,2);
 assert.equal((await csv("PreviewPartyImport",admin,{csv:csvBytes})).body.previouslyImported,2);
 for(const target of importPreview.body.targetPartyIds){
   assert.equal((await v3("GetParty",viewer,{partyId:target})).status,200);
   assert.equal((await v3("GetParty",tenantB,{partyId:target})).body.code,"not_found");
   assert.deepEqual((await sql("SELECT action,tenant_id,actor,request_id FROM crm_audit WHERE subject_id='"+target+"'")),[
     {action:"parties.create",tenant_id:"tenant-a",actor:"native-a@example.test",request_id:applied.headers.get("x-request-id")},
   ],"Import replay by another actor must retain the original attribution");
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,importAuditBefore+2,"replays never duplicate Party audit");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE tenant='tenant-a'"))[0].n,2);
 const changedBytes=new TextEncoder().encode(new TextDecoder().decode(csvBytes).replace("CSV One","CSV Changed"));
 assert.equal((await csv("ApplyPartyImport",admin,{...applyInput,csv:changedBytes,
   expectedSha256Hex:createHash("sha256").update(changedBytes).digest("hex")})).body.code,"aborted");
 // Structural file failures remain fail-closed Connect errors.
 for(const invalid of [
   new Uint8Array(),new Uint8Array(16385),new TextEncoder().encode("invalid,csv"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\n"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\n"+
     Array.from({length:21},(_,i)=>"source-"+i+",person,Name "+i+",1\r\n").join(""))
 ]){
   assert.equal((await csv("PreviewPartyImport",admin,{csv:invalid})).body.code,
     "invalid_argument","Malformed header/size/count must never return a commit-ready preview");
 }
 // Row semantic errors become typed, NON-COMMITTABLE previews, not generic 400s.
 const semanticCases=[
   ["x,invalid,Name,1",[[1,"kind","IMPORT_KIND_INVALID"]]],
   ["x,person,Name,0",[[1,"resource_version","IMPORT_SOURCE_VERSION_INVALID"]]],
   ["x,person,One,1\r\nx,person,Two,1",[[2,"party_id","IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID"]]]
 ];
 for(const [rows,expectedIssues] of semanticCases){
   const invalid=new TextEncoder().encode(
     "party_id,kind,display_name,resource_version\r\n"+rows+"\r\n");
   const response=await csv("PreviewPartyImport",admin,{csv:invalid});
   assert.equal(response.status,200,describeProtobuf(response.body));
   assert.deepEqual(response.body.targetPartyIds,[]);
   assert.deepEqual(response.body.rowIssues.map(x=>[x.rowPosition,x.field,x.code]),
     expectedIssues);
   assert.equal((await csv("ApplyPartyImport",admin,{
     csv:invalid,expectedSha256Hex:response.body.csvSha256Hex,
     precondition:{idempotencyKey:"csv-reject-semantic-"+createHash("sha256").update(invalid).digest("hex").slice(0,16)}
   })).status,400,"A typed invalid preview must still be rejected by the owner");
 }
 // The same original CSV parser reports ALL row/field diagnostics, not the
 // first unrelated error code. No target or audit can be created by preview.
 const invalidRowsCsv=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\n"+
   "duplicate-one,invalid,First,0\r\n"+
   "duplicate-one,person,Second,1\r\n"+
   "empty-name,person,,1\r\n"+
   "valid-last,organization,Valid,1\r\n");
 const invalidAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const invalidPartyBefore=(await sql("SELECT COUNT(*) AS n FROM crm_party_records"))[0].n;
 const invalidReceiptBefore=(await sql("SELECT COUNT(*) AS n FROM crm_party_import_receipts"))[0].n;
 const invalidPreview=await csv("PreviewPartyImport",admin,{csv:invalidRowsCsv});
 assert.equal(invalidPreview.status,200,describeProtobuf(invalidPreview.body));
 assert.equal(invalidPreview.body.count,4);
 assert.equal(invalidPreview.body.csvSha256Hex,createHash("sha256").update(invalidRowsCsv).digest("hex"));
 assert.deepEqual(invalidPreview.body.targetPartyIds,[],"No ID from invalid preview is actionable");
 assert.deepEqual(invalidPreview.body.rowIssues.map(issue=>[
   issue.rowPosition,issue.field,issue.code
 ]),[
   [1,"resource_version","IMPORT_SOURCE_VERSION_INVALID"],
   [1,"kind","IMPORT_KIND_INVALID"],
   [2,"party_id","IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID"],
   [3,"display_name","IMPORT_PARTY_INVALID"]
 ]);
 const invalidApply=await csv("ApplyPartyImport",admin,{
   csv:invalidRowsCsv,expectedSha256Hex:invalidPreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-import-invalid-rows-denied"}
 });
 assert.equal(invalidApply.status,400,"Apply must revalidate even an advertised digest");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,invalidAuditBefore);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records"))[0].n,invalidPartyBefore);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_import_receipts"))[0].n,invalidReceiptBefore);
 console.log(JSON.stringify({event:"crm.native-n01-csv-row-issues",result:"PASS",
   per_row:true,multiple_errors:true,field_codes:true,sha256:true,apply_denied:true,
   no_partial_write:true}));

 // A conflict on the second target must leave the first target absent.
 const conflictBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nconflict-one,person,First,1\r\nconflict-two,person,Second,1\r\n");
 const conflictPreview=await csv("PreviewPartyImport",admin,{csv:conflictBytes});
 assert.equal(conflictPreview.status,200,describeProtobuf(conflictPreview.body));
 const [firstTarget,secondTarget]=conflictPreview.body.targetPartyIds;
 assert.equal((await v3("CreateParty",admin,{partyId:secondTarget,kind:1,displayName:"Existing target",
   precondition:{idempotencyKey:"csv-target-conflict-create"}})).status,200);
 const conflictRepreview=await csv("PreviewPartyImport",admin,{csv:conflictBytes});
 assert.equal(conflictRepreview.status,200);
 assert.deepEqual(conflictRepreview.body.targetPartyIds,[]);
 assert.deepEqual(conflictRepreview.body.rowIssues.map(issue=>[
   issue.rowPosition,issue.field,issue.code
 ]),[[2,"party_id","IMPORT_TARGET_PARTY_CONFLICT"]]);
 const failedImport=await csv("ApplyPartyImport",admin,{csv:conflictBytes,
   expectedSha256Hex:conflictPreview.body.csvSha256Hex,precondition:{idempotencyKey:"csv-conflict-operation"}});
 assert.equal(failedImport.body.code,"aborted");
 assert.equal((await v3("GetParty",viewer,{partyId:firstTarget})).body.code,"not_found");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE key='csv-conflict-operation'"))[0].n,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,importAuditBefore+3);
 // Unlike the preflight target conflict above, this fails inside the second SQL
 // audit INSERT, after the first row has executed in the same D1 batch.
 const rollbackBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nrollback-one,person,First,1\r\nrollback-two,person,Second,1\r\n");
 const rollbackPreview=await csv("PreviewPartyImport",admin,{csv:rollbackBytes});
 assert.equal(rollbackPreview.status,200);
 const rollbackInput={csv:rollbackBytes,expectedSha256Hex:rollbackPreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-audit-rollback"}};
 const beforeRollback=(await partySnapshot());
 for(const token of [null,viewer,unknownService])assert.ok((await csv("ApplyPartyImport",token,rollbackInput)).status>=400);
 (await sql("CREATE TRIGGER fixture_fail_import_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+rollbackPreview.body.targetPartyIds[1]+"' "+
   "BEGIN SELECT RAISE(ABORT,'fixture late import audit failure'); END;"));
 try{
   assertUnconfirmed(await csv("ApplyPartyImport",knownService,rollbackInput),"IMPORT_WRITE_UNCONFIRMED");
   assert.deepEqual((await partySnapshot()),beforeRollback,"Late batch failure must remove every state, receipt and audit");
 }finally{await sql("DROP TRIGGER fixture_fail_import_audit");}
 const retryImport=await csv("ApplyPartyImport",knownService,rollbackInput);
 assert.equal(retryImport.status,200,describeProtobuf(retryImport.body));
 assert.equal(retryImport.body.createdCount,2,"Failed batch must not reserve its idempotency key");
 for(const target of rollbackPreview.body.targetPartyIds){
   assert.deepEqual((await sql("SELECT actor,request_id FROM crm_audit WHERE subject_id='"+target+"'")),[
     {actor:"service:ci-fixture.access",request_id:retryImport.headers.get("x-request-id")},
   ]);
 }
 // Product-first simple source: the ORIGINAL CSV ReaderBuilder, optional
 // source identity and position-derived ImportRow, not a second parser/owner.
 const simpleCsv=new TextEncoder().encode(
   'kind,display_name\r\nperson,"Simple, CSV Person"\r\norganization,"Simple ""Quoted"" Org"\r\n');
 const simplePreview=await csv("PreviewPartyImport",admin,{csv:simpleCsv});
 assert.equal(simplePreview.status,200,describeProtobuf(simplePreview.body));
 assert.equal(simplePreview.body.count,2);
 assert.equal(simplePreview.body.previouslyImported,0);
 assert.equal(simplePreview.body.rowIssues.length,0);
 assert.equal(new Set(simplePreview.body.targetPartyIds).size,2);
 assert.ok(simplePreview.body.targetPartyIds.every(id=>!id.includes("external-one")));
 const simpleTenantB=await csv("PreviewPartyImport",tenantB,{csv:simpleCsv});
 assert.equal(simpleTenantB.status,200);
 assert.ok(simpleTenantB.body.targetPartyIds.every(id=>!simplePreview.body.targetPartyIds.includes(id)));
 assert.equal((await csv("PreviewPartyImport",viewer,{csv:simpleCsv})).status,403);
 const brokenSimple=new TextEncoder().encode(
   "kind,display_name\r\nperson,Valid Name\r\norganization,\r\n");
 const invalidSimple=await csv("PreviewPartyImport",admin,{csv:brokenSimple});
 assert.equal(invalidSimple.status,200,describeProtobuf(invalidSimple.body));
 assert.deepEqual(invalidSimple.body.targetPartyIds,[]);
 assert.deepEqual(invalidSimple.body.rowIssues.map(x=>[x.rowPosition,x.field,x.code]),[
   [2,"display_name","IMPORT_PARTY_INVALID"]]);
 const simpleBeforeAudit=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const simpleBeforeParties=(await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"))[0].n;
 assert.equal((await csv("ApplyPartyImport",admin,{
   csv:brokenSimple,expectedSha256Hex:invalidSimple.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-simple-invalid-denied"}
 })).status,400);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,simpleBeforeAudit);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"))[0].n,simpleBeforeParties);
 const simpleApply={csv:simpleCsv,expectedSha256Hex:simplePreview.body.csvSha256Hex,
   precondition:{idempotencyKey:"csv-simple-import-original-owner"}};
 const simpleSaved=await csv("ApplyPartyImport",admin,simpleApply);
 assert.equal(simpleSaved.status,200,describeProtobuf(simpleSaved.body));
 assert.equal(simpleSaved.body.createdCount,2);
 assert.equal((await csv("ApplyPartyImport",knownService,simpleApply)).body.replayedCount,2);
 assert.equal((await csv("PreviewPartyImport",admin,{csv:simpleCsv})).body.previouslyImported,2);
 for(const target of simplePreview.body.targetPartyIds){
   const found=await v3("GetParty",viewer,{partyId:target});
   assert.equal(found.status,200);
   assert.ok(["Simple, CSV Person",'Simple "Quoted" Org'].includes(found.body.party.displayName));
   assert.equal((await v3("GetParty",tenantB,{partyId:target})).status,404);
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE tenant='tenant-a'"))[0].n,simpleBeforeParties+2);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,simpleBeforeAudit+2);
 console.log(JSON.stringify({event:"crm.native-simple-csv-original-owner",result:"PASS",
   native_csv:true,headers_two_or_four:true,tenant:true,original_position_id:true,
   row_errors:true,atomic:true,replay:true,quoted_values:true,audit:true}));
 assert.equal((await csv("PreviewPartyExport",admin,{kind:0})).body.code,"invalid_argument");
 assert.equal((await csv("PreviewPartyExport",admin,{kind:999})).body.code,"invalid_argument");
 const exportBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const exported=await csv("PreviewPartyExport",admin,{});
 assert.equal(exported.status,200,describeProtobuf(exported.body));
 assert.equal(exported.body.maximumTenantParties,100);
 assert.deepEqual(exported.body.fields,["party_id","kind","display_name","resource_version"]);
 const downloaded=exported.body.csv;
 assert.ok(downloaded instanceof Uint8Array);
 // Original party-export-csv/v1 uses LF records; do not alter canonical bytes.
 assert.match(new TextDecoder().decode(downloaded),/^party_id,kind,display_name,resource_version\n/);
 assert.equal(createHash("sha256").update(downloaded).digest("hex"),exported.body.csvSha256Hex);
 assert.equal((await csv("PreviewPartyExport",null,{})).status,401);
 assert.equal((await csv("PreviewPartyExport",viewer,{})).status,403);
 // Retired business HTTP route must not reappear alongside the typed RPC.
 assert.equal((await rpcFetch("/api/customer-data-operations/parties.csv",{
   headers:{"cf-access-jwt-assertion":admin}})).status,404);
 const filteredExport=await csv("PreviewPartyExport",admin,{kind:1});
 assert.equal(filteredExport.status,200,describeProtobuf(filteredExport.body));
 assert.ok(filteredExport.body.count<exported.body.count);
 assert.equal(createHash("sha256").update(filteredExport.body.csv).digest("hex"),filteredExport.body.csvSha256Hex);
 assert.equal((await csv("PreviewPartyExport",tenantB,{})).body.count,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,exportBefore);
 // Explicit boundary fixture in tenant B: the 101st row must prevent partial export,
 // even when a filter would reduce the returned data below the limit.
 const boundaryRows=Array.from({length:101},(_,i)=>{
   const state=JSON.stringify({party_id:"csv-boundary-"+i,kind:"person",display_name:"Boundary "+i,
     created_at_unix_nanos:10,updated_at_unix_nanos:10,version:1});
   return "('tenant-b','csv-boundary-"+i+"','"+state+"',1,'csv-boundary-key-"+i+"','fixture','fixture','native-b@example.test','Boundary "+i+"')";
 }).join(",");
 await sql("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) VALUES "+boundaryRows);
 assert.equal((await csv("PreviewPartyExport",tenantB,{kind:2})).body.code,"failed_precondition");
 console.log(JSON.stringify({event:"crm.native-party-csv-v2",result:"PASS",admin_read:true,
   original_owner:"crm.customer-data-operations",party_owner:"crm.parties",
   tenant_ab:true,atomic:true,replay:true,audit:true,verified_actor:true,sql_failure_rollback:true,csv_bytes_sha256:true,bounded:true}));
 // M1-10: signed workerd + generated Protobuf prove the actual open-field
 // decision for each primary customer owner. Read the WHOLE returned business
 // object, not a hand-maintained list of fields that could miss future additions.
 // This is the same native suite/policy/D1 fixture, not an extra auth runner.
 const m1AuditBeforeReads=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 const m1FullReads=[
   ["Party",v3,"GetParty",{partyId:id},"party"],
   ["Account",accountV2,"GetAccount",{accountId},"account"],
   ["ContactPoint",contactV2,"GetContactPoint",{contactPointId:contactId},"contactPoint"],
   ["Relationship",relationV2,"GetPartyRelationship",{relationshipId:rMid},"relationship"],
   ["Deal",salesV2,"GetDeal",{dealId},"deal"],
   ["Task",taskV2,"GetTask",{taskId:linkedTaskId},"task"],
   ["Customer360",overviewCall,"GetAccountOverview",{accountId},"overview"]
 ];
 const m1ReadValues=new Map();
 for(const [label,call,method,payload,field] of m1FullReads){
   const fromAdmin=await call(method,admin,payload);
   const fromViewer=await call(method,viewer,payload);
   assert.equal(fromAdmin.status,200,label+" admin Get");
   assert.equal(fromViewer.status,200,label+" viewer Get");
   assert.ok(fromAdmin.body[field],label+" must contain an authorized business record");
   assert.deepEqual(fromViewer.body[field],fromAdmin.body[field],
     label+" must reveal every existing Protobuf business field to viewer, not just IDs");
   m1ReadValues.set(label,fromViewer.body[field]);
   assert.equal((await call(method,tenantB,payload)).status,404,
     label+" must not reveal a cross-tenant record");
 }
 const m1ListReads=[
   ["Party",v3,"ListParties",{page:{pageSize:5}},"parties"],
   ["Account",accountV2,"ListAccounts",{page:{pageSize:5}},"accounts"],
   ["ContactPoint",contactV2,"ListContactPoints",
     {partyId:id,page:{pageSize:5}},"contactPoints"],
   ["Relationship",relationV2,"ListPartyRelationships",
     {partyId:id,page:{pageSize:5}},"relationships"],
   ["Deal",salesV2,"ListDeals",{accountId,page:{pageSize:5}},"deals"],
   ["Task",taskV2,"ListTasks",{accountId,page:{pageSize:5}},"tasks"]
 ];
 for(const [label,call,method,payload,field] of m1ListReads){
   const fromAdmin=await call(method,admin,payload);
   const fromViewer=await call(method,viewer,payload);
   assert.equal(fromAdmin.status,200,label+" admin List");
   assert.equal(fromViewer.status,200,label+" viewer List");
   assert.ok(fromAdmin.body[field].length>0,label+" fixture must exercise populated list");
   if(label==="Deal"){
     // Caller-specific permissions are intentionally not shared business facts.
     // Compare every other response field, including future business fields.
     const {dealCapabilities:viewerCapabilities,...viewerBusiness}=fromViewer.body;
     const {dealCapabilities:adminCapabilities,...adminBusiness}=fromAdmin.body;
     assert.deepEqual(viewerBusiness,adminBusiness,label+" List business facts and pagination");
     assert.deepEqual(viewerCapabilities.map(x=>x.dealId),fromViewer.body.deals.map(x=>x.dealId));
     assert.deepEqual(adminCapabilities.map(x=>x.dealId),fromAdmin.body.deals.map(x=>x.dealId));
     assert.ok(adminCapabilities.every(x=>x.capabilities.manageOpportunityTeam));
     assert.ok(viewerCapabilities.every(x=>!x.capabilities.manageOpportunityTeam));
   }else{
     assert.deepEqual(fromViewer.body,fromAdmin.body,
       label+" List must preserve all business fields, pagination and filters for viewer");
   }
 }
 // The same related records must form one coherent Party -> Account -> Deal ->
 // Sales event -> original linked Task -> Customer360 chain, not isolated RPC 200s.
 const m1Party=m1ReadValues.get("Party"),m1Account=m1ReadValues.get("Account");
 const m1Deal=m1ReadValues.get("Deal"),m1Task=m1ReadValues.get("Task");
 const m1Overview=m1ReadValues.get("Customer360");
 assert.equal(m1Party.partyId,id);
 assert.ok(m1Account.parties.some(row=>row.partyId===id&&row.role===1));
 assert.equal(m1Deal.accountId,m1Account.accountId);
 assert.equal(m1Deal.amount?.minorUnits,98765n);
 assert.equal(m1Deal.amount?.currencyCode,"PLN");
 assert.equal(m1Task.accountId,m1Account.accountId);
 const m1Events=await salesV2("ListDealEvents",viewer,{dealId,page:{pageSize:25}});
 assert.equal(m1Events.status,200);
 assert.ok(m1Events.body.events.some(event=>event.linkedTaskId===linkedTaskId),
   "Original Sales stage event must carry the linked Activities Task ID");
 assert.equal(m1Overview.account.accountId,m1Account.accountId);
 assert.equal(m1Overview.primaryParty.partyId,m1Party.partyId);
 assert.ok(m1Overview.deals.some(row=>row.dealId===m1Deal.dealId));
 assert.ok(m1Overview.tasks.some(row=>row.taskId===linkedTaskId));
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   m1AuditBeforeReads,"M1 complete business reads and denied tenant lookups must not write audit");
 console.log(JSON.stringify({event:"crm.native-m1-read-parity-linked-flow",result:"PASS",
   signed_admin_and_viewer:true,full_wire_fields_equal:true,tenant_b_not_found:true,
   get_owners:m1FullReads.length,list_owners:m1ListReads.length,
   party_account_deal_sales_event_task_customer360:true,read_only:true}));
 // SK-05: the SAME binary Connect + signed worker + original Activities D1
 // validates completed calls/meetings/notes independently of Sales stage audit.
 const interactionV2=async(method,token,payload)=>{
   const response=await rpcFetch(interactionV2Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
       ...(token?{"cf-access-jwt-assertion":token}:{})},
     body:binaryInteractionV2(method,payload)});
   return {status:response.status,headers:response.headers,
     body:response.status<400
       ?decodeInteractionV2(method,new Uint8Array(await response.arrayBuffer()))
       :await response.json()};
 };
 const eventLater="sk05-later-"+randomUUID();
 const eventEarlier="sk05-earlier-"+randomUUID();
 const eventNote="sk05-note-"+randomUUID();
 const occurrence=(seconds)=>({seconds:BigInt(seconds),nanos:123456789});
 const interactionInput=(interactionId,kind,seconds,deal="")=>({
   interactionId,accountId,dealId:deal,kind,
   text:"SK05 validated interaction "+interactionId,
   occurredAt:occurrence(seconds),
   precondition:{idempotencyKey:"sk05-"+interactionId}
 });
 const laterInput=interactionInput(eventLater,1,1780000000,dealId);
 const earlierInput=interactionInput(eventEarlier,2,1770000000,dealId);
 const noteInput=interactionInput(eventNote,3,1775000000);
 const interactionAuditBefore=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='activities.interaction.recorded'"))[0].n;
 assert.equal((await interactionV2("CreateInteraction",null,laterInput)).status,401);
 assert.equal((await interactionV2("CreateInteraction",viewer,laterInput)).status,403);
 assert.equal((await interactionV2("CreateInteraction",tenantB,laterInput)).status,404,
   "cross-tenant Account must not accept events");
 assert.equal((await interactionV2("CreateInteraction",admin,{
   ...laterInput,accountId:accountNextId,
   precondition:{idempotencyKey:"sk05-mismatch-"+eventLater}})).status,400,
   "Deal and Account must have identical tenant-scoped parent");
 assert.equal((await interactionV2("CreateInteraction",admin,{
   ...laterInput,dealId:"sk05-nonexistent-deal",
   precondition:{idempotencyKey:"sk05-absent-"+eventLater}})).status,404);
 assert.equal((await interactionV2("CreateInteraction",admin,{
   ...laterInput,kind:0,
   precondition:{idempotencyKey:"sk05-unknown-kind-"+eventLater}})).status,400);
 const recordedLater=await interactionV2("CreateInteraction",admin,laterInput);
 assert.equal(recordedLater.status,200,describeProtobuf(recordedLater.body));
 assert.equal(recordedLater.body.interaction?.author,"native-a@example.test");
 assert.equal(recordedLater.body.interaction?.dealId,dealId);
 assert.equal(recordedLater.body.interaction?.accountId,accountId);
 assert.equal(recordedLater.body.interaction?.kind,1);
 assert.equal(recordedLater.body.interaction?.occurredAt?.seconds,1780000000n);
 assert.equal(recordedLater.body.receipt?.resourceVersion,1n);
 const recordedEarlier=await interactionV2("CreateInteraction",admin,earlierInput);
 const recordedNote=await interactionV2("CreateInteraction",admin,noteInput);
 for(const result of [recordedEarlier,recordedNote]){
   assert.equal(result.status,200,describeProtobuf(result.body));
   assert.equal(result.body.receipt?.resourceVersion,1n);
 }
 assert.equal(recordedNote.body.interaction?.kind,3);
 assert.equal(recordedNote.body.interaction?.dealId,"");
 const replayed=await interactionV2("CreateInteraction",admin,laterInput);
 assert.equal(replayed.status,200,describeProtobuf(replayed.body));
 assert.equal(replayed.body.receipt?.replayed,true);
 assert.equal(replayed.body.receipt?.originalRequestId,recordedLater.receipt?.requestId
   ??recordedLater.headers.get("x-request-id"));
 assert.equal((await interactionV2("CreateInteraction",admin,{
   ...laterInput,text:"Modified on replay"
 })).status,409);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE tenant=? AND id=?",
   "tenant-a",eventLater))[0].n,1);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interaction_receipts WHERE tenant='tenant-a'"))[0].n,3);
 // The original Interaction INSERT, receipt, audit and Activities-owned
 // immutable automation source event are one physical D1 commit. Replays,
 // forged/denied requests and other tenants never emit a second event.
 const sourceEvents=await sql(
   "SELECT event_id,event_type,source_id,source_version,account_id,deal_id,source_actor,origin_request_id,origin_intent_digest,recorded_at_key "+
   "FROM crm_activity_automation_source_events WHERE tenant=? AND source_id IN (?,?,?) ORDER BY source_id",
   "tenant-a",eventLater,eventEarlier,eventNote);
 assert.equal(sourceEvents.length,3);
 for(const row of sourceEvents){
   assert.equal(row.event_type,"crm.activities.interaction_created.v1");
   assert.equal(row.event_id,"crm.activities.e1:"+row.origin_intent_digest);
   assert.ok(row.event_id.length<=180,"source EventId must match SDK bound");
   assert.equal(row.source_version,1);
   assert.equal(row.account_id,accountId);
   assert.equal(row.source_actor,"native-a@example.test");
   assert.match(row.origin_request_id,/^[0-9a-f-]{36}$/);
   assert.match(row.origin_intent_digest,/^[a-f0-9]{64}$/);
   assert.match(row.recorded_at_key,/^[0-9]{20}$/);
   assert.equal(row.deal_id,row.source_id===eventNote?null:dealId);
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE tenant=?","tenant-b"))[0].n,0);
 // Wrangler's documented native harness dispatches the compiled scheduled
 // entrypoint of THIS SAME Rust Worker. Neither cron nor a rule is enabled
 // in production: the boundary is read-only until complete run/lease acceptance.
 const scheduledAuditBefore=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n;
 const logStart=testHarness.getLogs().length;
 for(let attempt=0;attempt<2;attempt++){
   const result=await worker.scheduled({
     cron:"*/5 * * * *",scheduledTime:new Date("2026-11-24T00:05:00.000Z")
   });
   assert.equal(result.outcome,"ok","native scheduled Worker must dispatch without an exception");
   assert.equal(result.noRetry,false);
 }
 assert.ok(testHarness.getLogs().slice(logStart).some(entry=>
   String(entry.message).includes("CRM_AUTOMATION_SCHEDULED_DISPATCH_DISABLED")),
   "actual Rust scheduled callback must log its disabled-dispatch boundary");
 assert.equal((await sql(
   "SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE tenant=?","tenant-a"))[0].n,3,
   "two cron invocations cannot consume or duplicate source events");
 assert.equal((await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=?","tenant-a"))[0].n,
   scheduledAuditBefore,"scheduled compatibility probe must not write audit");
 console.log(JSON.stringify({event:"crm.native-sk08a-scheduled-boundary",result:"PASS",
   native_workerd_handler:true,repeat_count:2,no_cross_tenant_read:true,
   no_task_creation:true,no_new_audit:true,production_cron_disabled:true}));


 assert.equal((await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND action='activities.interaction.recorded'"))[0].n,
   interactionAuditBefore+3,"replay and rejected writes must not create another audit");
 const getAdmin=await interactionV2("GetInteraction",admin,{
   interactionId:eventLater,receiptIdempotencyKey:laterInput.precondition.idempotencyKey});
 const getViewer=await interactionV2("GetInteraction",viewer,{
   interactionId:eventLater,receiptIdempotencyKey:laterInput.precondition.idempotencyKey});
 assert.equal(getAdmin.status,200);
 assert.equal(getViewer.status,200);
 assert.equal(getViewer.body.interaction?.author,"native-a@example.test");
 assert.deepEqual(getAdmin.body.interaction,getViewer.body.interaction);
 assert.equal(getAdmin.body.confirmedMutation?.originalRequestId,
   recordedLater.headers.get("x-request-id"));
 assert.equal(getAdmin.body.confirmedMutation?.resourceVersion,1n);
 assert.equal((await interactionV2("GetInteraction",viewer,{
   interactionId:eventEarlier,receiptIdempotencyKey:laterInput.precondition.idempotencyKey
 })).body.confirmedMutation,undefined,"other record receipt must not be disclosed");
 assert.equal((await interactionV2("GetInteraction",tenantB,{
   interactionId:eventLater})).status,404);
 const firstPage=await interactionV2("ListInteractions",viewer,{accountId,page:{pageSize:2}});
 assert.equal(firstPage.status,200);
 assert.deepEqual(firstPage.body.interactions.map(row=>row.interactionId),[eventLater,eventNote],
   "late backdated event must sort by fact time, not recorded/audit time");
 assert.ok(firstPage.body.pageInfo?.nextPageToken);
 const secondPage=await interactionV2("ListInteractions",viewer,{
   accountId,page:{pageSize:2,pageToken:firstPage.body.pageInfo.nextPageToken}
 });
 assert.deepEqual(secondPage.body.interactions.map(row=>row.interactionId),[eventEarlier]);
 assert.equal(secondPage.body.pageInfo?.nextPageToken??"","");
 const listAdmin=await interactionV2("ListInteractions",admin,{accountId,page:{pageSize:2}});
 assert.deepEqual(listAdmin.body,firstPage.body,
   "viewer sees all accessible original business fields and chronology");
 const dealPage=await interactionV2("ListInteractions",viewer,{accountId,dealId,page:{pageSize:10}});
 assert.deepEqual(dealPage.body.interactions.map(row=>row.interactionId),
   [eventLater,eventEarlier],"Deal history cannot show Account-only note");
 assert.deepEqual((await interactionV2("ListInteractions",tenantB,{
   accountId,page:{pageSize:10}})).body.interactions,[]);
 assert.deepEqual((await interactionV2("ListInteractions",viewer,{
   accountId:accountNextId,dealId,page:{pageSize:10}})).body.interactions,[]);
 assert.equal((await interactionV2("ListInteractions",viewer,{
   accountId,page:{pageSize:2,pageToken:"invalid"}
 })).status,400);
 const interactionAuditBeforeReads=(await sql(
   "SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 await interactionV2("GetInteraction",viewer,{interactionId:eventNote});
 await interactionV2("ListInteractions",viewer,{accountId,page:{pageSize:5}});
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   interactionAuditBeforeReads,"read-only history must not write audit");
 const failedId="sk05-failed-"+randomUUID();
 const failedInput=interactionInput(failedId,3,1773000000);
 await sql("CREATE TRIGGER fixture_fail_sk05_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id='"+failedId+"' BEGIN SELECT RAISE(ABORT,'fixture audit'); END;");
 try{
   assertUnconfirmed(await interactionV2("CreateInteraction",admin,failedInput),
     "ACTIVITIES_INTERACTION_WRITE_UNCONFIRMED");
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE id=?",
     failedId))[0].n,0);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interaction_receipts WHERE key=?",
     failedInput.precondition.idempotencyKey))[0].n,0);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE source_id=?",
     failedId))[0].n,0);
 }finally{await sql("DROP TRIGGER fixture_fail_sk05_audit");}
 const recovered=await interactionV2("CreateInteraction",admin,failedInput);
 assert.equal(recovered.status,200);
 assert.equal(recovered.body.receipt?.replayed,false);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE subject_id=?",failedId))[0].n,1);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE source_id=?",failedId))[0].n,1);
 // The original RecordId maximum must never overflow EventId. The
 // Activity owner writes an opaque bounded event ID from the signed intent
 // digest, not a potentially 180-character user-supplied source ID.
 const longestInteractionId="i".repeat(180);
 const longKey="sk08a-long-"+randomUUID();
 const longestResult=await interactionV2("CreateInteraction",admin,{
   ...noteInput,interactionId:longestInteractionId,
   precondition:{idempotencyKey:longKey}
 });
 assert.equal(longestResult.status,200,describeProtobuf(longestResult.body));
 const longSource=(await sql(
   "SELECT event_id,origin_intent_digest FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
   "tenant-a",longestInteractionId));
 assert.equal(longSource.length,1);
 assert.equal(longSource[0].event_id,"crm.activities.e1:"+longSource[0].origin_intent_digest);
 assert.ok(longSource[0].event_id.length<=180);
 // This boundary-length fixture is intentionally not part of the SK-05
 // Account chronology; remove only its exact four linked D1 records.
 // All real Interaction/read/replay/rollback evidence above remains intact.
 await batch([
   statement("DELETE FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
     "tenant-a",longestInteractionId),
   statement("DELETE FROM crm_activity_interaction_receipts WHERE tenant=? AND key=?",
     "tenant-a",longKey),
   statement("DELETE FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action=?",
     "tenant-a",longestInteractionId,"activities.interaction.recorded"),
   statement("DELETE FROM crm_activity_interactions WHERE tenant=? AND id=?",
     "tenant-a",longestInteractionId)
 ]);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_interactions WHERE tenant=? AND id=?",
   "tenant-a",longestInteractionId))[0].n,0,"long-ID fixture must not alter the next scenario");
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
   "tenant-a",longestInteractionId))[0].n,0);

 console.log(JSON.stringify({event:"crm.native-sk08a-interaction-source",result:"PASS",atomic:true,
   original_activities_owner:true,exact_origin_request:true,tenant_ab:true,
   replay_single_event:true,failed_audit_rollback:true,bounded_event_id:true,
   automation_execution_disabled:true}));
 console.log(JSON.stringify({event:"crm.native-sk05-interaction",result:"PASS",
   source_sha:sourceSha,original_owner:"crm.activities",binary_connect:true,tenant_ab:true,
   viewer_fields_equal:true,viewer_write_denied:true,account_deal_fk:true,
   chronological_keyset:true,idempotent_replay:true,
   immutable_receipt_and_audit:true,failed_audit_rollback:true}));


 // SK-06: one original Task owner filters due instants before LIMIT, using a
 // different (due-nanos, id) keyset. Existing ID-sorted Task views are unchanged.
 const sk06Seeds=[
   ["sk06-task-z-"+randomUUID(),"SK06 Workday earliest",1800100000n],
   ["sk06-task-a-"+randomUUID(),"SK06 Workday same A",1800200000n],
   ["sk06-task-c-"+randomUUID(),"SK06 Workday same C",1800200000n],
   ["sk06-task-undated-"+randomUUID(),"SK06 Workday undated",null],
   ["sk06-task-done-"+randomUUID(),"SK06 Workday completed",1800150000n]
 ];
 for(const [taskId,subject,due] of sk06Seeds){
   const made=await taskV2("CreateTask",admin,{
     taskId,subject,accountId:accountNextId,priority:2,
     ...(due===null?{}:{dueAt:{seconds:due,nanos:0}}),
     precondition:{idempotencyKey:"sk06-task-create-"+taskId}
   });
   assert.equal(made.status,200,describeProtobuf(made.body));
 }
 const [sk06Early,sk06SameA,sk06SameC,sk06Undated,sk06Done]=sk06Seeds.map(x=>x[0]);
 const sk06Completed=await taskV2("CompleteTask",admin,{taskId:sk06Done,
   precondition:{expectedVersion:1n,idempotencyKey:"sk06-task-complete-"+sk06Done}});
 assert.equal(sk06Completed.status,200,describeProtobuf(sk06Completed.body));
 const sk06ReadAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 const sk06DueFilter={status:1,sortByDue:true,search:"SK06 Workday ",
   dueFrom:{seconds:1800100000n,nanos:0},
   dueBefore:{seconds:1800300000n,nanos:0},page:{pageSize:1}};
 const sk06DueTasks=await collectPages(taskV2,"ListTasks","tasks",sk06DueFilter);
 assert.deepEqual(sk06DueTasks.map(task=>task.taskId),
   [sk06Early,sk06SameA,sk06SameC],
   "Task due keyset must order by exact nanoseconds then ID before each LIMIT");
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{
   status:1,withoutDueOnly:true,search:"SK06 Workday ",page:{pageSize:1}
 })).map(task=>task.taskId),[sk06Undated]);
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{
   status:1,sortByDue:true,
   dueFrom:{seconds:1800200000n,nanos:0},
   dueBefore:{seconds:1800200001n,nanos:0},search:"SK06 Workday ",page:{pageSize:1}
 })).map(task=>task.taskId),[sk06SameA,sk06SameC]);
 assert.equal((await taskV2("ListTasks",tenantB,sk06DueFilter)).body.tasks.length,0);
 const sk06Viewer=await taskV2("ListTasks",viewer,{...sk06DueFilter,page:{pageSize:2}});
 const sk06Admin=await taskV2("ListTasks",admin,{...sk06DueFilter,page:{pageSize:2}});
 assert.equal(sk06Viewer.status,200);
 assert.deepEqual(sk06Viewer.body,sk06Admin.body,"viewer must see same authorized Task fields");
 for(const bad of [
   {...sk06DueFilter,page:{pageSize:2,pageToken:"invalid"}},
   {...sk06DueFilter,dueFrom:{seconds:1800300000n,nanos:0}},
   {...sk06DueFilter,sortByDue:false,withoutDueOnly:true}
 ]){
   assert.equal((await taskV2("ListTasks",viewer,bad)).status,400,
     "SK-06 invalid cursor/range/undated mix must fail closed");
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   sk06ReadAuditBefore,"SK-06 Task read filtering must never append audit");

 // The SAME Interaction List RPC supports an explicitly authorized
 // tenant-wide recent view; Account/Deal history remains scoped as before.
 const sk06OtherInteraction="sk06-recent-"+randomUUID();
 const sk06RecentInput={...interactionInput(sk06OtherInteraction,1,1790000000),
   accountId:accountNextId};
 const sk06Created=await interactionV2("CreateInteraction",admin,sk06RecentInput);
 assert.equal(sk06Created.status,200,describeProtobuf(sk06Created.body));
 const sk06RecentAuditBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 const sk06Recent=await interactionV2("ListInteractions",viewer,{
   recentAcrossAccounts:true,page:{pageSize:2}
 });
 assert.equal(sk06Recent.status,200);
 assert.deepEqual(sk06Recent.body.interactions.map(event=>event.interactionId),
   [sk06OtherInteraction,eventLater],
   "SK-06 newest interactions span multiple Accounts before keyset LIMIT");
 assert.ok(sk06Recent.body.accountNames[accountNextId]);
 assert.ok(sk06Recent.body.accountNames[accountId]);
 assert.ok(sk06Recent.body.pageInfo?.nextPageToken);
 const sk06NextRecent=await interactionV2("ListInteractions",viewer,{
   recentAcrossAccounts:true,page:{pageSize:2,
     pageToken:sk06Recent.body.pageInfo.nextPageToken}
 });
 assert.deepEqual(sk06NextRecent.body.interactions.map(event=>event.interactionId),
   [eventNote,failedId],"a recovered SK-05 Interaction is a real fact, not a missing event");
 assert.ok(sk06NextRecent.body.pageInfo?.nextPageToken);
 const sk06ThirdRecent=await interactionV2("ListInteractions",viewer,{
   recentAcrossAccounts:true,page:{pageSize:2,
     pageToken:sk06NextRecent.body.pageInfo.nextPageToken}
 });
 assert.deepEqual(sk06ThirdRecent.body.interactions.map(event=>event.interactionId),
   [eventEarlier],"tenant-wide keyset must return every immutable event");
 assert.equal(sk06ThirdRecent.body.pageInfo?.nextPageToken??"","");
 assert.deepEqual((await interactionV2("ListInteractions",admin,{
   recentAcrossAccounts:true,page:{pageSize:2}
 })).body,sk06Recent.body);
 assert.deepEqual((await interactionV2("ListInteractions",tenantB,{
   recentAcrossAccounts:true,page:{pageSize:3}
 })).body.interactions,[]);
 assert.equal((await interactionV2("ListInteractions",viewer,{
   recentAcrossAccounts:true,accountId,page:{pageSize:2}
 })).status,400);
 assert.equal((await interactionV2("ListInteractions",viewer,{
   recentAcrossAccounts:true,dealId,page:{pageSize:2}
 })).status,400);
 assert.equal((await interactionV2("ListInteractions",viewer,{
   recentAcrossAccounts:false,page:{pageSize:2}
 })).status,400);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   sk06RecentAuditBefore,"recent original Interaction reads are audit-free");
 console.log(JSON.stringify({event:"crm.native-sk06-workday",result:"PASS",source_sha:sourceSha,
   owner:"crm.activities",typed_binary:true,due_range_before_limit:true,
   due_nanosecond_keyset:true,undated:true,tenant_ab:true,viewer_full_fields:true,
   recent_across_accounts:true,account_names:true,invalid_scope_denied:true,
   no_read_audit:true}));

 // SK-07.B: signed human membership, CAS role/deactivation and original
 // TaskOwner reassignment. No identity headers or service->human impersonation.
 const beforeMemberRead=(await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n;
 const myMember=await memberV2("GetMyMembership",admin,{});
 assert.equal(myMember.status,200,describeProtobuf(myMember.body));
 assert.equal(myMember.body.member?.email,"native-a@example.test");
 assert.equal(myMember.body.member?.role,2);
 assert.equal((await memberV2("GetMyMembership",viewer,{})).body.member?.role,1);
 assert.equal((await memberV2("GetMyMembership",knownService,{})).body.member?.email,
   "service:ci-fixture.access","service actor remains a SERVICE");
 assert.equal((await memberV2("GetMyMembership",tenantB,{})).body.member?.email,"native-b@example.test");
 const initialRoster=await memberV2("ListMembers",viewer,{page:{pageSize:1}});
 assert.equal(initialRoster.status,200);
 assert.equal(initialRoster.body.members.length,1);
 assert.ok(initialRoster.body.pageInfo?.nextPageToken);
 assert.equal((await memberV2("GetMember",tenantB,{email:"native-a@example.test"})).status,404);
 assert.equal((await memberV2("GetMember",viewer,{email:"native-a@example.test"})).status,200);
 assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id='tenant-a'"))[0].n,
   beforeMemberRead,"Signed membership reads must not append audit");

 const newStaff="native-staff@example.test";
 const staffCreate={email:newStaff,role:1,
   precondition:{idempotencyKey:"sk07-member-create-staff"}};
 assert.equal((await memberV2("ProvisionMember",viewer,staffCreate)).status,403);
 assert.equal((await memberV2("ProvisionMember",tenantB,{
   ...staffCreate,precondition:{idempotencyKey:"sk07-member-tenant-b"}
 })).status,200,"tenant B may independently invite an email ONLY if globally unused");
 const staffWrongTenant=(await memberV2("ProvisionMember",admin,staffCreate));
 assert.equal(staffWrongTenant.status,409,"an email enrolled by tenant B cannot be reparented");
 // A second independent human is provisioned inside tenant A, not minted as JWT.
 const assignee="native-assignee@example.test";
 const staffInput={email:assignee,role:1,
   precondition:{idempotencyKey:"sk07-member-create-assignee"}};
 const staffCreated=await memberV2("ProvisionMember",admin,staffInput);
 assert.equal(staffCreated.status,200,describeProtobuf(staffCreated.body));
 assert.equal(staffCreated.body.member?.email,assignee);
 assert.equal(staffCreated.body.member?.active,true);
 assert.equal(staffCreated.body.member?.version,1n);
 assert.equal((await memberV2("ProvisionMember",admin,staffInput)).body.receipt?.replayed,true);
 assert.equal((await memberV2("ProvisionMember",admin,{...staffInput,role:2})).status,409);
 const memberProof=await memberV2("GetMember",viewer,{email:assignee,
   receiptIdempotencyKey:staffInput.precondition.idempotencyKey});
 assert.equal(memberProof.body.confirmedMutation?.originalRequestId,
   staffCreated.body.receipt?.requestId);
 assert.equal(memberProof.body.confirmedMutation?.resourceVersion,1n);
 assert.equal((await memberV2("GetMember",tenantB,{email:assignee})).status,404);
 assert.equal((await memberV2("ProvisionMember",admin,{
   email:"service:forged.access",role:2,
   precondition:{idempotencyKey:"sk07-member-forged-service"}
 })).status,400);

 // Signed invited HUMAN login really joins the same tenant through Access.
 const invitedJwt=jwt(assignee);
 const invitedMe=await memberV2("GetMyMembership",invitedJwt,{});
 assert.equal(invitedMe.status,200);
 assert.equal(invitedMe.body.member?.email,assignee);
 assert.equal((await memberV2("ProvisionMember",invitedJwt,staffCreate)).status,403);
 const changeMember={email:assignee,role:2,
   precondition:{idempotencyKey:"sk07-member-promote",expectedVersion:1n}};
 const promoted=await memberV2("UpdateMember",admin,changeMember);
 assert.equal(promoted.status,200,describeProtobuf(promoted.body));
 assert.equal(promoted.body.member?.version,2n);
 assert.equal(promoted.body.member?.role,2);
 assert.equal((await memberV2("UpdateMember",admin,changeMember)).body.receipt?.replayed,true);
 assert.equal((await memberV2("UpdateMember",admin,{
   ...changeMember,precondition:{idempotencyKey:"sk07-member-stale",expectedVersion:1n}
 })).status,409);
 assert.equal((await memberV2("UpdateMember",invitedJwt,{
   email:assignee,active:false,
   precondition:{idempotencyKey:"sk07-member-self",expectedVersion:2n}
 })).status,403,"no self lockout");
 assert.equal((await memberV2("UpdateMember",admin,{
   email:"native-b@example.test",active:false,
   precondition:{idempotencyKey:"sk07-member-foreign",expectedVersion:1n}
 })).status,409,"cross tenant membership mutation is forbidden");

 // One original Task owner: assign to a signed human, query BEFORE LIMIT,
 // reassign by original Rust domain Patch<TaskOwner>, CAS and receipts.
 const delegatedTask="sk07-task-"+randomUUID();
 const delegatedCreate={taskId:delegatedTask,accountId,subject:"SK07 assigned Task",
   priority:2,assigneeEmail:assignee,
   precondition:{idempotencyKey:"sk07-task-create-"+delegatedTask}};
 const delegated=await taskV2("CreateTask",admin,delegatedCreate);
 assert.equal(delegated.status,200,describeProtobuf(delegated.body));
 assert.equal(delegated.body.task?.assigneeIdentity,assignee);
 const delegatedList=await taskV2("ListTasks",viewer,{
   assigneeEmail:assignee,page:{pageSize:100}
 });
 assert.equal(delegatedList.status,200);
 assert.ok(delegatedList.body.tasks.some(t=>t.taskId===delegatedTask));
 assert.equal((await taskV2("ListTasks",invitedJwt,{assignedToMe:true,
   page:{pageSize:100}})).body.tasks.some(t=>t.taskId===delegatedTask),true);
 assert.equal((await taskV2("ListTasks",viewer,{assignedToMe:true,
   assigneeEmail:assignee})).status,400);
 assert.equal((await taskV2("ListTasks",tenantB,{assigneeEmail:assignee})).body.tasks.length,0);
 assert.equal((await taskV2("CreateTask",admin,{
   ...delegatedCreate,taskId:"sk07-task-foreign",
   assigneeEmail:"native-b@example.test",
   precondition:{idempotencyKey:"sk07-task-foreign"}
 })).status,400);
 const workload=await taskV2("ListTaskWorkload",viewer,{});
 assert.equal(workload.status,200);
 assert.ok(workload.body.buckets.some(b=>b.assigneeIdentity===assignee&&b.openCount>0n));
 assert.ok(workload.body.asOfUnixSeconds>0n);
 assert.deepEqual((await taskV2("ListTaskWorkload",admin,{})).body.buckets,workload.body.buckets);
 assert.equal((await taskV2("ListTaskWorkload",tenantB,{})).body.buckets
   .some(b=>b.assigneeIdentity===assignee),false);
 const personalBefore={taskId:delegatedTask,
   precondition:{expectedVersion:1n,idempotencyKey:"sk07-not-my-task"}};
 assert.equal((await taskV2("CompleteTask",viewer,personalBefore)).status,403);
 assert.equal((await taskV2("ScheduleReminder",viewer,{
   ...personalBefore,reminderAt:{seconds:1860000000n,nanos:0}
 })).status,403);
 const delegationChange={taskId:delegatedTask,assigneeEmail:"native-viewer@example.test",
   precondition:{idempotencyKey:"sk07-task-reassign",expectedVersion:1n}};
 const reassign=await taskV2("UpdateTask",admin,delegationChange);
 assert.equal(reassign.status,200,describeProtobuf(reassign.body));
 assert.equal(reassign.body.task?.assigneeIdentity,"native-viewer@example.test");
 assert.equal(reassign.body.task?.version,2n);
 assert.equal((await taskV2("UpdateTask",admin,delegationChange)).body.receipt?.replayed,true);
 assert.equal((await taskV2("UpdateTask",admin,{
   ...delegationChange,precondition:{idempotencyKey:"sk07-task-stale",expectedVersion:1n}
 })).status,409);
 const delegatedProof=await taskV2("GetTask",viewer,{
   taskId:delegatedTask,receiptIdempotencyKey:"sk07-task-reassign"
 });
 assert.equal(delegatedProof.body.confirmedMutation?.originalRequestId,
   reassign.body.receipt?.requestId);
 assert.equal((await taskV2("ListTasks",viewer,{assignedToMe:true,
   page:{pageSize:100}})).body.tasks.some(t=>t.taskId===delegatedTask),true);
 assert.equal((await taskV2("ListTasks",invitedJwt,{assignedToMe:true,
   page:{pageSize:100}})).body.tasks.some(t=>t.taskId===delegatedTask),false);

 // Personal writes use the signed viewer, not administrator impersonation.
 const personalReminder={taskId:delegatedTask,reminderAt:{seconds:1860000000n,nanos:0},
   precondition:{expectedVersion:2n,idempotencyKey:"sk07-own-reminder"}};
 const reminderOwn=await taskV2("ScheduleReminder",viewer,personalReminder);
 assert.equal(reminderOwn.status,200,describeProtobuf(reminderOwn.body));
 assert.equal(reminderOwn.body.task?.version,3n);
 assert.equal((await taskV2("ScheduleReminder",viewer,personalReminder)).body.receipt?.replayed,true);
 const personalComplete={taskId:delegatedTask,
   precondition:{expectedVersion:3n,idempotencyKey:"sk07-own-complete"}};
 assert.equal((await taskV2("CompleteTask",tenantB,personalComplete)).status,404);
 assert.equal((await taskV2("UpdateTask",viewer,{
   ...delegationChange,precondition:{expectedVersion:3n,idempotencyKey:"sk07-no-self-delegate"}
 })).status,403);
 const ownDone=await taskV2("CompleteTask",viewer,personalComplete);
 assert.equal(ownDone.status,200,describeProtobuf(ownDone.body));
 assert.equal(ownDone.body.task?.status,2);
 assert.equal(ownDone.body.task?.version,4n);
 assert.equal((await taskV2("CompleteTask",viewer,personalComplete)).body.receipt?.replayed,true);
 assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=? AND actor=?",
   "tenant-a",delegatedTask,"native-viewer@example.test"))[0].n,2);

 // Assignment and revocation contend in the same D1. A successful assignment
 // must appear before revocation; a rejected command must not write/audit.
 const raceCreateId="sk07-race-create-"+randomUUID();
 const raceUpdateId="sk07-race-update-"+randomUUID();
 const raceBase=await taskV2("CreateTask",admin,{
   taskId:raceUpdateId,accountId,subject:"SK07 race reassignment base",priority:2,
   precondition:{idempotencyKey:"sk07-race-base-"+raceUpdateId}
 });
 assert.equal(raceBase.status,200,describeProtobuf(raceBase.body));
 const raceCreate={...delegatedCreate,taskId:raceCreateId,
   precondition:{idempotencyKey:"sk07-race-assign-"+raceCreateId}};
 const raceUpdate={taskId:raceUpdateId,assigneeEmail:assignee,
   precondition:{expectedVersion:1n,idempotencyKey:"sk07-race-assign-"+raceUpdateId}};
 const [createRace,updateRace,deactivate]=await Promise.all([
   taskV2("CreateTask",admin,raceCreate),
   taskV2("UpdateTask",admin,raceUpdate),
   memberV2("UpdateMember",admin,{email:assignee,active:false,
     precondition:{idempotencyKey:"sk07-member-deactivate",expectedVersion:2n}})
 ]);
 assert.equal(deactivate.status,200,describeProtobuf(deactivate.body));
 const revokedAt=(await sql("SELECT rowid AS seq FROM crm_audit WHERE tenant_id=? AND request_id=?",
   "tenant-a",deactivate.body.receipt.requestId))[0].seq;
 for(const [response,taskId,action,method,payload] of [
   [createRace,raceCreateId,"activity_tasks.create","CreateTask",raceCreate],
   [updateRace,raceUpdateId,"activity_tasks.update","UpdateTask",raceUpdate]
 ]){
   const audit=await sql("SELECT rowid AS seq FROM crm_audit WHERE tenant_id=? AND subject_id=? AND action=?",
     "tenant-a",taskId,action);
   if(response.status===200){
     assert.equal(audit.length,1);
     assert.ok(audit[0].seq<revokedAt,"assignment committed after member was revoked");
     assert.equal((await taskV2(method,admin,payload)).body.receipt?.replayed,true);
   }else{
     assert.ok([400,409].includes(response.status),describeProtobuf(response.body));
     assert.equal(audit.length,0,"rejected concurrent assignment wrote audit");
   }
 }
 assert.equal(deactivate.body.member?.active,false);
 assert.equal((await memberV2("GetMyMembership",invitedJwt,{})).status,401);
 assert.equal((await taskV2("CreateTask",admin,{
   ...delegatedCreate,taskId:"sk07-task-inactive",assigneeEmail:assignee,
   precondition:{idempotencyKey:"sk07-task-inactive"}
 })).status,400);
 assert.equal((await memberV2("ListMembers",viewer,{includeInactive:true,
   page:{pageSize:100}})).body.members.some(m=>m.email===assignee&&!m.active),true);
 const workflowAudit=await sql("SELECT action FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND (subject_id=? OR subject_id=?) ORDER BY rowid",assignee,delegatedTask);
 assert.deepEqual(workflowAudit.map(r=>r.action),[
   "access.member.provision","access.member.update","activity_tasks.create",
   "activity_tasks.update","activity_tasks.reminder_scheduled","activity_tasks.complete","access.member.update"
 ]);
 console.log(JSON.stringify({event:"crm.native-sk07b-workforce",result:"PASS",
   signed_human:true,service_distinct:true,tenant_ab:true,viewer_read_all:true,
   admin_delegation_only:true,personal_task_write:true,atomic_assignment_revocation:true,
   member_receipts:true,cas:true,replay:true,revocation:true,
   task_assignee_original_domain:true,mine_signed:true,workload_before_paging:true}));

 // SK-07.C shared Task pool and original Access team aggregate.
 const teamId="sk07-team-"+randomUUID();
 const teamMember="native-viewer@example.test";
 const teamInput={teamId,name:"Native shared team "+teamId,description:"QA",
   roster:{memberEmails:[teamMember,"native-a@example.test"]},
   precondition:{idempotencyKey:"sk07-team-create-"+teamId}};
 assert.equal((await memberV2("CreateTeam",viewer,teamInput)).status,403);
 const teamCreated=await memberV2("CreateTeam",admin,teamInput);
 assert.equal(teamCreated.status,200,describeProtobuf(teamCreated.body));
 assert.equal((await memberV2("CreateTeam",admin,teamInput)).body.receipt?.replayed,true);
 const teamRead=await memberV2("GetTeam",viewer,{teamId,receiptIdempotencyKey:teamInput.precondition.idempotencyKey});
 assert.equal(teamRead.body.confirmedMutation?.resourceVersion,1n);
 assert.equal((await memberV2("GetTeam",tenantB,{teamId})).status,404);
 assert.ok((await memberV2("ListTeams",viewer,{page:{pageSize:100}})).body.teams.some(t=>t.teamId===teamId));
 const poolId="sk07-pool-"+randomUUID();
 const poolOwner={owner:{case:"teamId",value:teamId}};
 const poolCreate={taskId:poolId,accountId,subject:"Native team pool",priority:2,owner:poolOwner,
   precondition:{idempotencyKey:"sk07-pool-create-"+poolId}};
 const pool=await taskV2("CreateTask",admin,poolCreate);
 assert.equal(pool.status,200,describeProtobuf(pool.body));
 assert.equal(pool.body.task?.ownerTeamId,teamId);
 assert.deepEqual(pool.body.task?.owner?.owner,poolOwner.owner);
 // A pre-existing v2 request maps to precisely the same original digest.
 const {owner:canonicalPoolOwner,...legacyPoolCreate}=poolCreate;
 assert.equal(canonicalPoolOwner.owner.value,teamId);
 assert.equal((await taskV2("CreateTask",admin,{...legacyPoolCreate,ownerTeamId:teamId})).body.receipt?.replayed,true);
 assert.equal((await taskV2("CreateTask",admin,{...poolCreate,ownerTeamId:teamId})).status,400);
 assert.equal((await taskV2("CreateTask",admin,{...poolCreate,owner:{}})).status,400);
 assert.equal((await taskV2("ListTasks",viewer,{ownerFilter:poolOwner,assignedToMe:true})).status,400);
 assert.equal((await taskV2("ListTasks",viewer,{ownerFilter:poolOwner,ownerTeamId:teamId})).status,400);
 const poolBucket=(await taskV2("ListTaskWorkload",viewer,{})).body.buckets.find(b=>b.owner?.owner.case==="teamId"&&b.owner.owner.value===teamId);
 assert.equal(poolBucket?.openCount,1n);
 const teamArchive={teamId,active:false,precondition:{expectedVersion:1n,idempotencyKey:"sk07-team-archive-"+teamId}};
 assert.equal((await memberV2("UpdateTeam",admin,teamArchive)).status,409);
 const teamList=await taskV2("ListTasks",viewer,{ownerFilter:poolOwner,page:{pageSize:1}});
 assert.equal(teamList.body.tasks[0]?.taskId,poolId);
 assert.equal((await taskV2("ListTasks",tenantB,{ownerFilter:poolOwner})).body.tasks.length,0);
 const claim={taskId:poolId,precondition:{expectedVersion:1n,idempotencyKey:"sk07-pool-claim-"+poolId}};
 assert.equal((await taskV2("ClaimTask",knownService,claim)).status,403);
 assert.equal((await taskV2("ClaimTask",tenantB,claim)).status,404);
 const claimOther={...claim,precondition:{...claim.precondition,idempotencyKey:"sk07-other-claim-"+poolId}};
 const claimResults=await Promise.all([taskV2("ClaimTask",viewer,claim),taskV2("ClaimTask",admin,claimOther)]);
 assert.deepEqual(claimResults.map(r=>r.status).sort(),[200,409]);
 const claimWinner=claimResults[0].status===200?viewer:admin;
 const wonInput=claimResults[0].status===200?claim:claimOther;
 const claimed=claimResults.find(r=>r.status===200);
 assert.equal(claimed.body.task?.version,2n);
 assert.equal(claimed.body.task?.owner?.owner.case,"assigneeIdentity");
 assert.equal(claimed.body.task?.owner?.owner.value,claimWinner===viewer?teamMember:"native-a@example.test");
 assert.equal((await taskV2("ClaimTask",claimWinner,wonInput)).body.receipt?.replayed,true);
 assert.equal((await taskV2("ListTasks",viewer,{ownerTeamId:teamId})).body.tasks.length,0);
 assert.equal((await taskV2("CompleteTask",claimWinner,{taskId:poolId,
   precondition:{expectedVersion:2n,idempotencyKey:"sk07-pool-complete-"+poolId}})).status,200);
 assert.equal((await memberV2("UpdateTeam",admin,teamArchive)).status,200);
 assert.equal((await taskV2("CreateTask",admin,{...poolCreate,taskId:"sk07-inactive-pool-"+poolId,
   precondition:{idempotencyKey:"sk07-inactive-pool-"+poolId}})).status,400);
 assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",poolId))[0].n,3);
 console.log(JSON.stringify({event:"crm.native-sk07c-team-pool",result:"PASS",atomic_claim:true,tenant_ab:true,archive_guard:true,receipts:true}));

 // SK-07.C3: roster revocation and assignment/archival are actual atomic
 // owner races, not browser timing assumptions. All identities remain signed.
 {
   const id="sk07-claim-race-"+randomUUID();
   const create={teamId:id,name:"Claim revocation "+id,roster:{memberEmails:["native-a@example.test"]},
     precondition:{idempotencyKey:"create-"+id}};
   assert.equal((await memberV2("CreateTeam",admin,create)).status,200);
   const owner={owner:{case:"teamId",value:id}};
   const taskId="sk07-race-task-"+randomUUID();
   const input={taskId,accountId,subject:"Claim revocation QA",priority:2,owner,
     precondition:{idempotencyKey:"create-"+taskId}};
   assert.equal((await taskV2("CreateTask",admin,input)).status,200);
   const attempt={taskId,precondition:{expectedVersion:1n,idempotencyKey:"claim-"+taskId}};
   assert.equal((await taskV2("ClaimTask",viewer,attempt)).status,403);
   const add={teamId:id,roster:{memberEmails:["native-a@example.test",teamMember]},
     precondition:{expectedVersion:1n,idempotencyKey:"add-"+id}};
   assert.equal((await memberV2("UpdateTeam",admin,add)).status,200);
   const remove={teamId:id,roster:{memberEmails:["native-a@example.test"]},
     precondition:{expectedVersion:2n,idempotencyKey:"remove-"+id}};
   const [claim,removed]=await Promise.all([
     taskV2("ClaimTask",viewer,attempt),memberV2("UpdateTeam",admin,remove)
   ]);
   assert.equal(removed.status,200,describeProtobuf(removed.body));
   assert.ok([200,403,409].includes(claim.status),describeProtobuf(claim.body));
   const current=await taskV2("GetTask",admin,{taskId});
   assert.equal(current.status,200);
   if(claim.status===200){
     assert.equal(current.body.task?.owner?.owner.value,teamMember);
     assert.equal(current.body.task?.version,2n);
   }else{
     assert.equal(current.body.task?.owner?.owner.value,id);
     assert.equal(current.body.task?.version,1n);
     assert.equal((await taskV2("ClaimTask",viewer,{...attempt,precondition:{...attempt.precondition,idempotencyKey:"denied-"+taskId}})).status,403);
     assert.equal((await taskV2("ClaimTask",admin,{...attempt,precondition:{...attempt.precondition,idempotencyKey:"admin-"+taskId}})).status,200);
   }
   assert.equal((await taskV2("CompleteTask",claim.status===200?viewer:admin,{taskId,
     precondition:{expectedVersion:2n,idempotencyKey:"complete-"+taskId}})).status,200);
   assert.equal((await sql("SELECT COUNT(*) n FROM crm_audit WHERE tenant_id=? AND subject_id=?","tenant-a",taskId))[0].n,3);
   assert.equal((await memberV2("UpdateTeam",admin,{teamId:id,active:false,
     precondition:{expectedVersion:3n,idempotencyKey:"archive-"+id}})).status,200);

   const raceTeam="sk07-assignment-race-"+randomUUID();
   const raceCreate={teamId:raceTeam,name:"Assignment archive "+raceTeam,roster:{memberEmails:["native-a@example.test"]},
     precondition:{idempotencyKey:"create-"+raceTeam}};
   assert.equal((await memberV2("CreateTeam",admin,raceCreate)).status,200);
   const raceTask="sk07-assignment-task-"+randomUUID();
   const raceInput={taskId:raceTask,accountId,subject:"Assignment archive QA",priority:2,
     owner:{owner:{case:"teamId",value:raceTeam}},precondition:{idempotencyKey:"create-"+raceTask}};
   const archive={teamId:raceTeam,active:false,precondition:{expectedVersion:1n,idempotencyKey:"archive-"+raceTeam}};
   const [assigned,archived]=await Promise.all([taskV2("CreateTask",admin,raceInput),memberV2("UpdateTeam",admin,archive)]);
   assert.equal(Number(assigned.status===200)+Number(archived.status===200),1,
     "assignment and archive cannot both commit");
   if(assigned.status===200){
     assert.equal(archived.status,409);
     assert.equal((await taskV2("ClaimTask",admin,{taskId:raceTask,precondition:{expectedVersion:1n,idempotencyKey:"claim-"+raceTask}})).status,200);
     assert.equal((await taskV2("CompleteTask",admin,{taskId:raceTask,precondition:{expectedVersion:2n,idempotencyKey:"complete-"+raceTask}})).status,200);
     assert.equal((await memberV2("UpdateTeam",admin,archive)).status,200);
   }else{
     assert.ok([400,409].includes(assigned.status),describeProtobuf(assigned.body));
     assert.equal((await taskV2("GetTask",admin,{taskId:raceTask})).status,404);
   }
   const team=await memberV2("GetTeam",viewer,{teamId:raceTeam});
   assert.equal(team.body.team?.active,false);
   console.log(JSON.stringify({event:"crm.native-sk07c3-owner-oneof",result:"PASS",shared_owner:true,
     legacy_digest:true,mixed_rejected:true,nonmember_denied:true,claim_remove_race:true,assignment_archive_race:true}));
 }
 await typedBrowser({base,admin,viewer,tenantB,sql,sourceSha,adminIdentity:"native-a@example.test"});
 const salesAuditBeforeFilter=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 assert.deepEqual((await collectPages(salesV2,"ListDeals","deals",{accountId})).map(d=>d.dealId),
   (await collectPages(salesV2,"ListDeals","deals")).filter(d=>d.accountId===accountId).map(d=>d.dealId));
 assert.deepEqual((await salesV2("ListDeals",tenantB,{accountId})).body.deals,[]);
 assert.deepEqual((await salesV2("ListDeals",viewer,{accountId:"missing-account"})).body.deals,[]);
 assert.equal((await salesV2("ListDeals",viewer,{accountId:"bad/id"})).status,400);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,salesAuditBeforeFilter);
 // Search/pagination needs >100 nonmatching rows, not 202 repeated RPC
 // creation tests. Create one canonical Party+Account through the original
 // typed Rust owners, then copy only their persisted state in isolated D1.
 // No parallel business codec, schema or extra test framework is introduced.
 const searchFixtureStarted=Date.now();
 const paddingParty="zz-search-padding-000",paddingAccount=paddingParty;
 assert.equal((await v3("CreateParty",admin,{partyId:paddingParty,kind:1,
   displayName:"Search padding 000",
   precondition:{idempotencyKey:"search-party-padding-000"}})).status,200);
 assert.equal((await accountV2("CreateAccount",admin,{accountId:paddingAccount,
   name:"Search padding 000",primaryPartyId:id,
   precondition:{idempotencyKey:"search-account-padding-000"}})).status,200);
 // D1 triggers retain their normal receipt/audit ownership. Only the synthetic
 // search fixture uses cloning; all tested business mutations still use typed RPC.
 for(const [table,idField,labelField,receiptPrefix,requestPrefix] of [
   ["crm_party_records","party_id","display_name","search-party-padding","8001"],
   ["crm_account_records","account_id","name","search-account-padding","8002"]
 ]){
   const account=table==="crm_account_records";
   const source=account?"source.primary_party,":"";
   const columns=account?"tenant,id,primary_party,state,version,idem_key,idem_digest,request_id,actor,search_name":
     "tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name";
   await sql("WITH RECURSIVE pad(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM pad WHERE n<100) "+
     "INSERT INTO "+table+"("+columns+") "+
     "SELECT source.tenant,'zz-search-padding-'||printf('%03d',pad.n),"+source+
     "json_set(source.state,'$."+idField+"','zz-search-padding-'||printf('%03d',pad.n),"+
       "'$."+labelField+"','Search padding '||printf('%03d',pad.n)),source.version,"+
     "'"+receiptPrefix+"-'||printf('%03d',pad.n),'fixture-only',"+
     "printf('00000000-0000-4000-"+requestPrefix+"-%012d',pad.n),source.actor,"+
     "'Search padding '||printf('%03d',pad.n) "+
     "FROM pad CROSS JOIN "+table+" AS source "+
     "WHERE source.tenant='tenant-a' AND source.id=?",
     account?paddingAccount:paddingParty);
   const [proof]=await sql("SELECT COUNT(*) AS total,COUNT(DISTINCT id) AS distinct_ids,"+
     "SUM(CASE WHEN json_valid(state)=1 AND "+
     "json_extract(state,'$."+idField+"')=id AND search_name=json_extract(state,'$."+labelField+"') "+
     "THEN 0 ELSE 1 END) AS bad "+
     "FROM "+table+" WHERE tenant='tenant-a' AND id LIKE 'zz-search-padding-%'");
   assert.deepEqual(proof,{total:101,distinct_ids:101,bad:0},
     "Search padding must preserve owner-produced typed state in "+table);
 }
 console.log(JSON.stringify({event:"crm.native-search-padding-fixture",result:"PASS",
   typed_owner_creates:2,cloned_fixture_rows:200,elapsed_ms:Date.now()-searchFixtureStarted}));
 const searchIds=["zzz-search-first","zzz-search-gap","zzz-search-last"];
 for(const [index,target] of searchIds.entries()){
   const name=index===1?"Nonmatching gap":"Поиск 100%_ O'Neil "+index;
   assert.equal((await v3("CreateParty",admin,{partyId:target,kind:2,displayName:name,
     precondition:{idempotencyKey:"search-party-"+index}})).status,200);
   assert.equal((await accountV2("CreateAccount",admin,{accountId:target,name,primaryPartyId:id,
     precondition:{idempotencyKey:"search-account-"+index}})).status,200);
 }
 const nfcPartyId="zzz-nfc-party",nfcAccountId="zzz-nfc-account";
 const originalNfcName="Cafe\u0301 И\u0306ошкар-Ола";
 const canonicalNfcName="Café Йошкар-Ола";
 const nfcParty=await v3("CreateParty",admin,{partyId:nfcPartyId,kind:2,displayName:originalNfcName,
   precondition:{idempotencyKey:"nfc-party-create"}});
 assert.equal(nfcParty.status,200,describeProtobuf(nfcParty.body));
 const nfcAccount=await accountV2("CreateAccount",admin,{accountId:nfcAccountId,name:canonicalNfcName,
   primaryPartyId:nfcPartyId,precondition:{idempotencyKey:"nfc-account-create"}});
 assert.equal(nfcAccount.status,200,describeProtobuf(nfcAccount.body));
 for(const needle of [originalNfcName,canonicalNfcName]){
   const parties=await collectPages(v3,"ListParties","parties",{search:needle});
   assert.equal(parties.find(row=>row.partyId===nfcPartyId)?.displayName,originalNfcName);
   const accounts=await collectPages(accountV2,"ListAccounts","accounts",{search:needle});
   assert.equal(accounts.find(row=>row.accountId===nfcAccountId)?.name,canonicalNfcName);
 }
 assert.deepEqual(await sql("SELECT search_name FROM crm_party_records WHERE id='zzz-nfc-party'"),
   [{search_name:canonicalNfcName}]);
 assert.deepEqual(await sql("SELECT search_name FROM crm_account_records WHERE id='zzz-nfc-account'"),
   [{search_name:canonicalNfcName}]);
 const nfcUpdateName="Renamed Cafe\u0301";
 const nfcUpdateCanonical="Renamed Café";
 const updatedNfcParty=await v3("UpdateParty",admin,{partyId:nfcPartyId,displayName:nfcUpdateName,
   precondition:{idempotencyKey:"nfc-party-update",expectedVersion:1n}});
 assert.equal(updatedNfcParty.status,200,describeProtobuf(updatedNfcParty.body));
 const updatedNfcAccount=await accountV2("UpdateAccount",admin,{accountId:nfcAccountId,name:nfcUpdateName,status:1,
   precondition:{idempotencyKey:"nfc-account-update",expectedVersion:1n}});
 assert.equal(updatedNfcAccount.status,200,describeProtobuf(updatedNfcAccount.body));
 assert.equal((await collectPages(v3,"ListParties","parties",{search:canonicalNfcName})).length,0);
 assert.equal((await collectPages(accountV2,"ListAccounts","accounts",{search:canonicalNfcName})).length,0);
 assert.equal((await collectPages(v3,"ListParties","parties",{search:nfcUpdateCanonical}))[0]?.displayName,nfcUpdateName);
 assert.equal((await collectPages(accountV2,"ListAccounts","accounts",{search:nfcUpdateCanonical}))[0]?.name,nfcUpdateName);
 const nfcCsvBytes=new TextEncoder().encode("party_id,kind,display_name,resource_version\r\nnfc-import,person,Imported Cafe\u0301,1\r\n");
 const nfcPreview=await csv("PreviewPartyImport",admin,{csv:nfcCsvBytes});
 assert.equal(nfcPreview.status,200,describeProtobuf(nfcPreview.body));
 const nfcApplied=await csv("ApplyPartyImport",admin,{csv:nfcCsvBytes,
   expectedSha256Hex:nfcPreview.body.csvSha256Hex,precondition:{idempotencyKey:"nfc-import-operation"}});
 assert.equal(nfcApplied.status,200,describeProtobuf(nfcApplied.body));
 const importedNfc=await collectPages(v3,"ListParties","parties",{search:"Imported Café"});
 assert.equal(importedNfc.length,1);
 assert.equal(importedNfc[0].partyId,nfcPreview.body.targetPartyIds[0]);
 assert.equal(importedNfc[0].displayName,"Imported Cafe\u0301");
 const searchSnapshot=async()=>({party:(await partySnapshot()),
   accounts:(await sql("SELECT id,state,version FROM crm_account_records ORDER BY tenant,id")),
   accountReceipts:(await sql("SELECT COUNT(*) AS n FROM crm_account_receipts"))});
 const searchBefore=(await searchSnapshot());
 for(const [rpcCall,method,field,idField,filter,opposite] of [
   [v3,"ListParties","parties","partyId",{kind:2},{kind:1}],
   [accountV2,"ListAccounts","accounts","accountId",{status:1},{status:2}],
 ]){
   const firstPage=await rpcCall(method,viewer,{page:{pageSize:100}});
   assert.equal(firstPage.status,200);
   assert.ok(!firstPage.body[field].some(row=>searchIds.includes(row[idField])));
   const found=await collectPages(rpcCall,method,field,{search:"  Поиск 100%_ O'Neil  ",...filter});
   assert.deepEqual(found.map(row=>row[idField]),[searchIds[0],searchIds[2]],"Search must filter before LIMIT and retain every matching cursor");
   assert.deepEqual((await rpcCall(method,viewer,{search:"100%_",...opposite})).body[field],[]);
   assert.deepEqual((await rpcCall(method,tenantB,{search:"Поиск 100%_"})).body[field],[]);
   assert.deepEqual((await rpcCall(method,viewer,{search:"поиск 100%_"})).body[field],[],"Literal search is case-sensitive");
   assert.deepEqual((await rpcCall(method,viewer,{search:"%' OR 1=1 --"})).body[field],[]);
   assert.deepEqual((await rpcCall(method,viewer,{search:searchIds[2]})).body[field].map(row=>row[idField]),[searchIds[2]]);
   assert.deepEqual((await rpcCall(method,viewer,{search:"   ",page:{pageSize:100}})).body,firstPage.body);
   assert.equal((await rpcCall(method,viewer,{search:"я".repeat(200)})).status,200);
   const tooLong=await rpcCall(method,viewer,{search:"я".repeat(201)});
   assert.equal(tooLong.status,400);assert.equal(tooLong.body.code,"invalid_argument");
   assert.equal((await rpcCall(method,null,{search:"Поиск"})).status,401);
 }
 assert.deepEqual((await searchSnapshot()),searchBefore,"Search cannot mutate state, receipts or audit");
 console.log(JSON.stringify({event:"crm.native-party-account-search",result:"PASS",
   before_limit:true,unicode_literal:true,tenant_ab:true,read_only:true,existing_rpcs:true}));
 // Actual React search and both pickers find a target beyond 100 existing rows.
 const searchBrowser=await chromium.launch({headless:true});
 try{
   const context=await searchBrowser.newContext({extraHTTPHeaders:{"cf-access-jwt-assertion":admin}});
   const page=await context.newPage();
   await installUiDiagnostics(page);
   const searchRequests=[];
   const panelTransitions=[];
   const enterSearch=async(label,text,method)=>{
     const input=page.getByLabel(label,{exact:true});
     await input.fill(text);
     const pending=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
     await input.press("Enter");
     const response=await pending,headers=await response.allHeaders();
     assert.equal(response.status(),200);assert.equal(headers["x-crm-source-sha"],sourceSha);
     searchRequests.push({method,status:response.status(),request_id:headers["x-request-id"]});
   };
   for(const [hash,title,label,method] of [
     ["parties","Люди и организации","Поиск участников","ListParties"],
     ["accounts","Клиенты","Поиск клиентов","ListAccounts"],
   ]){
     await page.goto(base+"/#"+hash);
     await page.getByRole("heading",{name:title,exact:true}).waitFor();
     const nextPage=page.waitForResponse(r=>r.url().endsWith("/"+method)&&r.request().method()==="POST");
     await page.getByRole("button",{name:"Следующая страница",exact:true}).click();
     assert.equal((await nextPage).status(),200);
     await enterSearch(label,searchIds[2],method);
     await page.getByRole("heading",{name:"Поиск 100%_ O'Neil 2",exact:true}).waitFor();
     await enterSearch(label,"zz-search-padding-000",method);
     await page.getByRole("heading",{name:"Search padding 000",exact:true}).waitFor();
   }
   // Searchable pickers are inside the shared create panel, not the list.
   // Use the real UI, preserving the same native-Rust List RPC assertions.
   for(const [hash,createAction,label,searchLabel,method] of [
     ["accounts","Создать клиента","Основной участник","Поиск: Основной участник","ListParties"],
     ["tasks","Новая задача","Клиент","Поиск: Клиент","ListAccounts"],
   ]){
     await page.goto(base+"/#"+hash);
     await page.getByRole("button",{name:createAction,exact:true}).click();
     const panel=page.getByRole("dialog");
     await panel.waitFor();
     const panelId=await panel.getAttribute("aria-labelledby");
     assert.ok(panelId,"The native picker must retain a stable React useId identity");
     const select=panel.getByLabel(label,{exact:true});
     await enterSearch(searchLabel,searchIds[2],method);
     await select.selectOption(searchIds[2]);
     assert.equal(await select.inputValue(),searchIds[2]);
     await enterSearch(searchLabel,"missing-search-target",method);
     assert.equal(await select.inputValue(),"","Changed search must discard the old form selection");
     await enterSearch(searchLabel,searchIds[0],method);
     await select.selectOption(searchIds[0]);
     assert.equal(await select.inputValue(),searchIds[0]);
     // Finish the form before navigating to the next screen. The shared
     // AccountCreator correctly protects dirty drafts; an unhandled hash
     // transition must NOT bypass that guard or masquerade as a missing Task UI.
     if(hash==="accounts"){
       // Rejecting dirty-form closure must preserve BOTH the React intent
       // and the native dialog, rather than silently losing an unsaved draft.
       page.once("dialog",dialog=>void dialog.dismiss());
       await panel.getByRole("button",{name:"Отмена",exact:true}).click();
       const rejected=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"dirty-account-close-rejected",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const rejectedPanel=rejected.panels.find(p=>p.title==="Новый клиент");
       assert.equal(rejectedPanel?.dirty,"true","CreateAccount draft guard must be active");
       assert.equal(rejected.confirmations.at(-1)?.accepted,false,
         "Closing dirty Account after rejecting confirm must not discard draft");
       panelTransitions.push({title:"Новый клиент",result:"REJECTED",desired:rejectedPanel?.desired,
         nativeOpen:rejectedPanel?.nativeOpen,dirty:rejectedPanel?.dirty});
       // Escape must also enter the SAME dirty guard before the native dialog
       // closes. Reject first, then accept: no second navigation/close owner.
       page.once("dialog",dialog=>void dialog.dismiss());
       await page.keyboard.press("Escape");
       const escapeRejected=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"dirty-account-escape-rejected",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const escapePanel=escapeRejected.panels.find(p=>p.title==="Новый клиент");
       assert.equal(escapePanel?.closeSource,"escape");
       assert.equal(escapeRejected.confirmations.at(-1)?.accepted,false);
       assert.equal(escapePanel?.nativeOpen,true,"native Escape cannot bypass rejected dirty close");
       panelTransitions.push({title:"Новый клиент",result:"ESCAPE_REJECTED",
         desired:escapePanel?.desired,nativeOpen:escapePanel?.nativeOpen});
       // Regression for Chromium native cancel after repeated rejections:
       // the second Escape must also be prevented before the browser closes
       // the HTMLDialogElement (React desired and native open stay in sync).
       page.once("dialog",dialog=>void dialog.dismiss());
       await page.keyboard.press("Escape");
       const secondRejected=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"dirty-account-second-escape-rejected",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const secondPanel=secondRejected.panels.find(p=>p.panelId===panelId);
       assert.equal(secondRejected.confirmations.at(-1)?.accepted,false);
       assert.equal(secondPanel?.desired,"open");
       assert.equal(secondPanel?.nativeOpen,true);
       assert.equal(secondPanel?.closeSource,"escape");
       panelTransitions.push({title:"Новый клиент",result:"SECOND_ESCAPE_REJECTED",
         desired:secondPanel?.desired,nativeOpen:secondPanel?.nativeOpen});
       page.once("dialog",dialog=>void dialog.accept());
       await page.keyboard.press("Escape");
     }else{
       // A native <select> can consume Escape. Use the explicit close button
       // and inspect React intent and HTMLDialogElement.open independently.
       await panel.getByRole("button",{name:"Закрыть панель",exact:true}).click();
     }
     const title=hash==="accounts"?"Новый клиент":"Новая задача";
     const closed=await assertUiPanel({page,panel,panelId,title,
       expected:"closed",scenario:hash+"-picker-closed",
       evidenceDirectory:join(cwd,"../e2e-evidence-native")});
     const observed=closed.panels.find(p=>p.title===title);
     panelTransitions.push({title,result:"CLOSED",desired:observed?.desired,
       nativeOpen:observed?.nativeOpen,dirty:observed?.dirty,
       closeSource:observed?.closeSource});
     if(hash==="accounts"){
       // A dirty draft rejected twice and later discarded must never leave a
       // closed native dialog behind a stale React open=true on the next click.
       await page.getByRole("button",{name:createAction,exact:true}).click();
       const reopened=await assertUiPanel({page,panel,panelId,title:"Новый клиент",
         expected:"open",scenario:"account-reopen-after-escape",
         evidenceDirectory:join(cwd,"../e2e-evidence-native")});
       const reopenedPanel=reopened.panels.find(p=>p.panelId===panelId);
       assert.equal(reopenedPanel?.dirty,"false");
       assert.equal(await panel.getByLabel("Название клиента",{exact:true}).inputValue(),"");
       panelTransitions.push({title,result:"REOPENED",desired:reopenedPanel?.desired,
         nativeOpen:reopenedPanel?.nativeOpen});
       await panel.getByRole("button",{name:"Отмена",exact:true}).click();
       await assertUiPanel({page,panel,panelId,title:"Новый клиент",expected:"closed",
         scenario:"account-reopened-cancel",evidenceDirectory:join(cwd,"../e2e-evidence-native")});
     }
   }
   await page.setViewportSize({width:390,height:844});
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1));
   assert.deepEqual((await searchSnapshot()),searchBefore,"Search and Enter inside pickers must never submit the enclosing mutation form");
   writeFileSync(join(cwd,"../e2e-evidence-native/search-browser.json"),JSON.stringify({result:"PASS",source_sha:sourceSha,
     scenarios:["search after first unfiltered page","Party picker beyond 100 rows","Account picker beyond 100 rows",
       "selection reset","keyboard Enter does not submit mutation","mobile layout","read-only",
       "native dialog and React state matched on close","dirty draft close rejection retained dialog",
       "second rejected Escape preserves native modal before accepted close",
       "Account modal reopens cleanly after confirmed native Escape"],
     panel_transitions:panelTransitions,requests:searchRequests},null,2)+"\n");
 }finally{await searchBrowser.close();}
 // R7.3 forecast uses the same original owner, CAS, D1 binding and
 // read snapshot; this isolated tenant-a pipeline is created after the
 // existing regression scenarios and is disposed with the native database.
 {
   const pipelineId="pipeline.forecast-native";
   const stages=[
     {stageId:"lead",name:"Lead",probabilityBasisPoints:1000},
     {stageId:"qualified",name:"Qualified",probabilityBasisPoints:4500},
     {stageId:"proposal",name:"Proposal",probabilityBasisPoints:8000}
   ];
   const pre=(suffix,version)=>({
     idempotencyKey:"r73-forecast-"+suffix,...(version!==undefined?{expectedVersion:version}:{})
   });
   const pipeline=await salesV2("CreatePipeline",admin,{
     pipelineId,name:"Forecast native",stages,precondition:pre("pipeline")
   });
   assert.equal(pipeline.status,200,describeProtobuf(pipeline.body));
   const ids=[randomUUID(),randomUUID(),randomUUID()];
   const create=(id,category,units,currency,probability)=>({
     dealId:id,name:"Forecast "+id,accountId,stage:{pipelineId,stageId:"lead",ordinal:1},
     forecastCategory:category,amount:{minorUnits:units,currencyCode:currency},
     ...(probability===undefined?{}:{probabilityBasisPoints:probability}),
     precondition:pre("create-"+id)
   });
   const firstInput={...create(ids[0],3,10001n,"EUR"),expectedCloseDate:{year:2027,month:2,day:28}};
   const first=await salesV2("CreateDeal",admin,firstInput);
   const second=await salesV2("CreateDeal",admin,{
     ...create(ids[1],2,2500n,"USD",0),expectedCloseDate:{year:2027,month:3,day:1}
   });
   const third=await salesV2("CreateDeal",admin,create(ids[2],4,985n,"EUR",5000));
   for(const result of [first,second,third])assert.equal(result.status,200,describeProtobuf(result.body));
   assert.equal(first.body.deal.probabilityBasisPoints,1000);
   assert.equal(first.body.deal.probabilitySource,2);
   assert.equal(first.body.deal.forecastCategory,3);
   assert.equal(second.body.deal.probabilityBasisPoints,0);
   assert.equal(second.body.deal.probabilitySource,1);
   assert.equal(third.body.deal.forecastCategory,4);
   const replay=await salesV2("CreateDeal",admin,firstInput);
   assert.equal(replay.status,200);
   assert.equal(replay.body.receipt.replayed,true);
   assert.equal(replay.body.deal.probabilitySource,2);
   const badCategory=await salesV2("UpdateDeal",admin,{
     dealId:ids[0],forecastCategory:0,precondition:pre("invalid-category",1n)
   });
   assert.equal(badCategory.status,400);
   const badSource=await salesV2("UpdateDeal",admin,{
     dealId:ids[0],useStageProbability:true,probabilityBasisPoints:5,precondition:pre("invalid-source",1n)
   });
   assert.ok([400,422].includes(badSource.status),describeProtobuf(badSource.body));
   const move=async(id,targetStage,version,suffix)=>salesV2("AdvanceStage",admin,{
     dealId:id,targetStage:{pipelineId,stageId:targetStage,ordinal:2},targetStatus:1,precondition:pre(suffix,version)
   });
   const moved=await move(ids[0],"qualified",1n,"stage-move");
   assert.equal(moved.status,200,describeProtobuf(moved.body));
   assert.equal(moved.body.deal.probabilityBasisPoints,4500);
   assert.equal(moved.body.deal.probabilitySource,2);
   const movedManual=await move(ids[1],"qualified",1n,"manual-move");
   assert.equal(movedManual.status,200,describeProtobuf(movedManual.body));
   assert.equal(movedManual.body.deal.probabilityBasisPoints,0);
   assert.equal(movedManual.body.deal.probabilitySource,1);
   const reset=await salesV2("UpdateDeal",admin,{
     dealId:ids[1],useStageProbability:true,forecastCategory:3,
     precondition:pre("reset",movedManual.body.deal.version)
   });
   assert.equal(reset.status,200,describeProtobuf(reset.body));
   assert.equal(reset.body.deal.probabilityBasisPoints,4500);
   assert.equal(reset.body.deal.probabilitySource,2);
   assert.equal(reset.body.deal.forecastCategory,3);
   const stale=await salesV2("UpdateDeal",admin,{
     dealId:ids[1],forecastCategory:1,precondition:pre("stale",1n)
   });
   assert.equal(stale.status,409);
   const metric=async(status)=>salesV2("GetPipelineMetrics",viewer,{
     pipelineId,accountId,...(status===undefined?{}:{status})
   });
   const summary=await metric();
   assert.equal(summary.status,200,describeProtobuf(summary.body));
   assert.deepEqual(summary.body.forecastCategories.map(x=>[x.category,x.dealCount,x.stageDefaultCount,x.manualCount]),
     [[3,2n,2n,0n],[4,1n,0n,1n]]);
   assert.deepEqual(summary.body.forecastCurrencies.map(x=>[x.category,x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints,x.amountDealCount]),
     [[3,"EUR","10001","45004500",1n],[3,"USD","2500","11250000",1n],[4,"EUR","985","4925000",1n]]);
   const windowMetric=async(analysisPeriod)=>salesV2("GetPipelineMetrics",viewer,{
     pipelineId,accountId,analysisPeriod
   });
   const day={year:2027,month:2,day:28};
   const window=await windowMetric({startDate:day,endDate:day});
   assert.equal(window.status,200,describeProtobuf(window.body));
   assert.equal(window.body.totalDealCount,3n,"window must not filter board inventory");
   assert.deepEqual(window.body.forecastCurrencies.map(x=>[x.category,x.currencyCode,x.totalMinorUnits,x.weightedMinorUnitsTimesBasisPoints]),
     [[3,"EUR","10001","45004500"]]);
   assert.deepEqual(window.body.undatedForecastCurrencies.map(x=>[x.category,x.currencyCode,x.totalMinorUnits]),
     [[4,"EUR","985"]]);
   assert.equal(window.body.closedOutcomes.denominator,0n);
   assert.equal(window.body.closedOutcomes.winRateBasisPoints,undefined);
   assert.equal((await windowMetric({startDate:{year:2027,month:3,day:1},endDate:day})).status,400);
   assert.equal((await windowMetric({startDate:{year:2027,month:2,day:29},endDate:day})).status,400);
   assert.equal((await windowMetric({startDate:day})).status,400);
   const closed=await salesV2("CloseDeal",admin,{
     dealId:ids[0],outcome:2,reasonCode:"forecast-accepted",precondition:pre("close",moved.body.deal.version)
   });
   assert.equal(closed.status,200,describeProtobuf(closed.body));
   assert.equal((await metric(2)).body.forecastCategories.length,0);
   const openSummary=await metric();
   assert.equal(openSummary.body.closedOutcomes.wonCount,1n);
   assert.equal(openSummary.body.closedOutcomes.lostCount,0n);
   assert.equal(openSummary.body.closedOutcomes.denominator,1n);
   assert.equal(openSummary.body.closedOutcomes.winRateBasisPoints,10000);
   assert.equal((await metric(2)).body.closedOutcomes,undefined);
   const closeDay=new Date(Number(closed.body.deal.closedAt.seconds)*1000);
   const utcDay={year:closeDay.getUTCFullYear(),month:closeDay.getUTCMonth()+1,day:closeDay.getUTCDate()};
   assert.equal((await windowMetric({startDate:utcDay,endDate:utcDay})).body.closedOutcomes.wonCount,1n);
   assert.equal((await windowMetric({startDate:{year:2000,month:1,day:1},endDate:{year:2000,month:1,day:1}})).body.closedOutcomes.denominator,0n);
   assert.equal(openSummary.body.totalDealCount,3n);
   assert.equal(openSummary.body.forecastCategories.find(x=>x.category===3).dealCount,1n);
   assert.equal(openSummary.body.forecastCurrencies.some(x=>x.category===3&&x.currencyCode==="EUR"),false);
   const lost=await salesV2("CloseDeal",admin,{
     dealId:ids[2],outcome:3,reasonCode:"forecast-lost",precondition:pre("close-lost",third.body.deal.version)
   });
   assert.equal(lost.status,200,describeProtobuf(lost.body));
   const bothClosed=(await metric()).body.closedOutcomes;
   assert.deepEqual([bothClosed.wonCount,bothClosed.lostCount,bothClosed.denominator,bothClosed.winRateBasisPoints],
     [1n,1n,2n,5000]);
   const lostReopened=await salesV2("ReopenDeal",admin,{
     dealId:ids[2],reasonCode:"forecast-lost-reopened",precondition:pre("reopen-lost",lost.body.deal.version)
   });
   assert.equal(lostReopened.status,200,describeProtobuf(lostReopened.body));
   const reopened=await salesV2("ReopenDeal",admin,{
     dealId:ids[0],reasonCode:"forecast-reopened",precondition:pre("reopen",closed.body.deal.version)
   });
   assert.equal(reopened.status,200,describeProtobuf(reopened.body));
   assert.equal(reopened.body.deal.forecastCategory,3);
   assert.equal(reopened.body.deal.probabilityBasisPoints,4500);
   assert.equal((await metric()).body.closedOutcomes.denominator,0n);
   const persisted=await sql("SELECT json_extract(state,'$.probability_source') AS source,json_extract(state,'$.forecast_category') AS category FROM crm_sales_deal_records WHERE tenant='tenant-a' AND id=?",ids[0]);
   assert.deepEqual(persisted,[{source:"stage_default",category:"commit"}]);
   assert.deepEqual((await metric()).body.forecastCurrencies,summary.body.forecastCurrencies);
   // Catalog configuration changes must not silently edit existing Deals.
   const updatedCatalog=await salesV2("UpdatePipeline",admin,{
     pipelineId,name:"Forecast native",stages:stages.map(stage=>stage.stageId==="qualified"?
       {...stage,probabilityBasisPoints:6000}:stage),archived:false,
     precondition:pre("catalog-edit",pipeline.body.pipeline.version)
   });
   assert.equal(updatedCatalog.status,200,describeProtobuf(updatedCatalog.body));
   const retained=await salesV2("GetDeal",viewer,{dealId:ids[0]});
   assert.equal(retained.status,200);
   assert.equal(retained.body.deal.probabilityBasisPoints,4500);
   const refresh=await salesV2("UpdateDeal",admin,{
     dealId:ids[0],useStageProbability:true,
     precondition:pre("refresh-default",retained.body.deal.version)
   });
   assert.equal(refresh.status,200,describeProtobuf(refresh.body));
   assert.equal(refresh.body.deal.probabilityBasisPoints,6000);
   assert.equal(refresh.body.deal.probabilitySource,2);
   assert.equal((await metric()).body.forecastCurrencies.find(x=>x.category===3&&x.currencyCode==="EUR").weightedMinorUnitsTimesBasisPoints,"60006000");
   assert.equal((await salesV2("GetPipelineMetrics",tenantB,{pipelineId})).status,404);
   console.log(JSON.stringify({event:"crm.native-r73-forecast",result:"PASS",stage_default:true,
     manual_zero:true,move:true,reset:true,category:true,currency:true,close_reopen:true,cas:true,tenant:true}));
 }
 // #471 S5a: five existing original owners expose a READ-only keyed receipt
 // that proves exactly the original signed commit, not the latest row version.
 const s5aReceipts=[
   {owner:"party",call:v3,method:"GetParty",idField:"partyId",id,key:creation.precondition.idempotencyKey,
     original:created.headers.get("x-request-id"),table:"crm_party_receipts"},
   {owner:"account",call:accountV2,method:"GetAccount",idField:"accountId",id:accountId,
     key:accountInput.precondition.idempotencyKey,original:accountCreated.headers.get("x-request-id"),
     table:"crm_account_receipts"},
   {owner:"contact",call:contactV2,method:"GetContactPoint",idField:"contactPointId",id:contactId,
     key:contactInput.precondition.idempotencyKey,original:contactCreated.headers.get("x-request-id"),
     table:"crm_contact_point_receipts"},
   {owner:"relationship",call:relationV2,method:"GetPartyRelationship",idField:"relationshipId",id:rMid,
     key:midRelation.precondition.idempotencyKey,original:midCreated.headers.get("x-request-id"),
     table:"crm_party_relationship_receipts"},
   {owner:"task",call:taskV2,method:"GetTask",idField:"taskId",id:taskMid,
     key:taskMidInput.precondition.idempotencyKey,original:taskCreated.headers.get("x-request-id"),
     table:"crm_activity_task_receipts"}
 ];
 for(const entry of s5aReceipts) {
   const input={[entry.idField]:entry.id,receiptIdempotencyKey:entry.key};
   const read=await entry.call(entry.method,viewer,input);
   assert.equal(read.status,200,entry.owner+" "+describeProtobuf(read.body));
   assert.equal(read.body.confirmedMutation?.originalRequestId,entry.original,
     "original signed D1 receipt provenance must survive subsequent owner updates");
   assert.equal(read.body.confirmedMutation?.resourceVersion,1n,
     "read-only recovery must use the receipt snapshot version, not mutable latest state");
   assert.equal(read.body.confirmedMutation?.requestId,read.headers.get("x-request-id"));
   const saved=await sql("SELECT original_request_id FROM "+entry.table+" WHERE tenant=? AND key=?",
     "tenant-a",entry.key);
   assert.deepEqual(saved,[{original_request_id:entry.original}]);
   assert.equal((await entry.call(entry.method,tenantB,input)).status,404,
     "tenant isolation precedes receipt lookup");
   assert.equal((await entry.call(entry.method,null,input)).status,401);
   assert.equal((await entry.call(entry.method,viewer,
     {...input,receiptIdempotencyKey:"short"})).status,400);
   const missing=await entry.call(entry.method,viewer,
     {...input,receiptIdempotencyKey:"s5a-unknown-key-"+entry.owner});
   assert.equal(missing.status,200);
   assert.equal(missing.body.confirmedMutation,undefined);
   // A same-tenant unrelated resource cannot inherit somebody else's key.
   const other=entry.owner==="party"?"binary-rpc-proof-01":
     entry.owner==="account"?accountNextId:
     entry.owner==="contact"?contactLastId:
     entry.owner==="relationship"?rLast:taskLast;
   const unrelated=await entry.call(entry.method,viewer,
     {[entry.idField]:other,receiptIdempotencyKey:entry.key});
   assert.equal(unrelated.status,200,entry.owner+" other record");
   assert.equal(unrelated.body.confirmedMutation,undefined);
   // Legacy receipts without known origin must not be inferred from row order.
   await sql("UPDATE "+entry.table+" SET original_request_id=NULL WHERE tenant=? AND key=?",
     "tenant-a",entry.key);
   try {
     const historical=await entry.call(entry.method,viewer,input);
     assert.equal(historical.status,200);
     assert.equal(historical.body.confirmedMutation,undefined,
       "historical NULL provenance is not evidence of a committed mutation");
   } finally {
     await sql("UPDATE "+entry.table+" SET original_request_id=? WHERE tenant=? AND key=?",
       entry.original,"tenant-a",entry.key);
   }
   const after=await entry.call(entry.method,viewer,input);
   assert.equal(after.body.confirmedMutation?.originalRequestId,entry.original);
   assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE request_id=?",
     read.headers.get("x-request-id")))[0].n,0,"receipt checks never append audit");
 }
 // Genuine D1 read failure must remain UNCONFIRMED; ordinary GetParty without
 // a recovery key still works and the client must not claim "no commit".
 await sql("ALTER TABLE crm_party_receipts RENAME TO fixture_unavailable_party_receipts");
 try {
   const denied=await v3("GetParty",viewer,{
     partyId:id,receiptIdempotencyKey:creation.precondition.idempotencyKey});
   assert.equal(denied.status,503,describeProtobuf(denied.body));
   assert.equal((await v3("GetParty",viewer,{partyId:id})).status,200);
 } finally {
   await sql("ALTER TABLE fixture_unavailable_party_receipts RENAME TO crm_party_receipts");
 }
 assert.equal((await v3("GetParty",viewer,{
   partyId:id,receiptIdempotencyKey:creation.precondition.idempotencyKey
 })).body.confirmedMutation?.originalRequestId,created.headers.get("x-request-id"));
 console.log(JSON.stringify({event:"crm.native-s5a-crud-commit-provenance",result:"PASS",
   owners:s5aReceipts.map(x=>x.owner),original_request_ids:true,read_only:true,
   latest_version_independent:true,legacy_null_fail_closed:true,
   tenant_isolation:true,wrong_resource_rejected:true}));
 // #471 S2: prove SQL JSON paths and native persisted Rust state agree
 // for all active owner records created through generated binary Connect.
 // These are *SQL usage contracts*, not a new JSON business-schema registry.
 const nativeSqlStateContracts=[
   {table:"crm_party_records",id:"party_id",fields:["kind","display_name"],version:true},
   {table:"crm_account_records",id:"account_id",fields:["name","status"],version:true},
   {table:"crm_contact_point_records",id:"contact_point_id",fields:["kind","normalized_value","status"],version:true},
   {table:"crm_party_relationship_records",id:"party_relationship_id",fields:["status"],version:true},
   {table:"crm_activity_task_records",id:"task_id",fields:["status","priority"],version:true},
   {table:"crm_sales_deal_records",id:"deal_id",
     fields:["status","stage.pipeline_id","stage.stage_id"],version:true},
   {table:"crm_sales_pipeline_records",id:"pipeline_id",fields:["name"],version:false},
   {table:"crm_sales_saved_view_records",id:"view_id",fields:["name"],version:false}
 ];
 let sqlStateRows=0;
 for(const contract of nativeSqlStateContracts){
   const tests=[
     "json_valid(state) IS NOT 1",
     "json_extract(state,'$."+contract.id+"') IS NOT id",
     ...contract.fields.map(path=>"json_type(state,'$."+path+"') IS NOT 'text'"),
     ...(contract.version?["json_extract(state,'$.version') IS NOT version"]:[])
   ];
   if(contract.table==="crm_sales_pipeline_records")
     tests.push("json_type(state,'$.stages') IS NOT 'array'");
   const [result]=await sql("SELECT COUNT(*) AS n,SUM(CASE WHEN "+tests.join(" OR ")+
     " THEN 1 ELSE 0 END) AS broken FROM "+contract.table+" WHERE tenant=?", "tenant-a");
   assert.ok(result.n>0,contract.table+" must be covered by signed owner fixtures");
   assert.equal(result.broken,0,contract.table+" SQL paths drifted from the Rust owner codec");
   sqlStateRows+=result.n;
 }
 // The Privacy owner intentionally persists profiled canonical JSON rather
 // than raw Serde, and SQL must never lose or silently bypass the profile.
 const [privacySqlState]=await sql("SELECT COUNT(*) AS n,SUM(CASE WHEN "+
   "json_extract(state,'$.canonicalization_profile') IS NOT 'crm.cjson/v1' "+
   "THEN 1 ELSE 0 END) AS broken FROM crm_privacy_case_records WHERE tenant=?", "tenant-a");
 assert.ok(privacySqlState.n>0,"Privacy signed owner scenario must exercise canonical snapshots");
 assert.equal(privacySqlState.broken,0,"Privacy canonicalization profile must survive D1 writes");
 sqlStateRows+=privacySqlState.n;
 console.log(JSON.stringify({event:"crm.native-s2-owner-sql-persistence-contract",result:"PASS",
   proof:"signed generated Protobuf RPC -> original Rust owner -> real workerd D1 SQL paths",
   owners:nativeSqlStateContracts.length+1,checked_rows:sqlStateRows,
   privacy_canonical_profile:true}));
 // The exact production receipt query (not a mocked storage error) must
 // distinguish a proven absent key from an inaccessible receipt table.
 // Use only the existing signed native-ci read-only probe and local D1 fixture.
 // Preparation failure occurs before a command can reach D1 writes;
 // unlike a failed lookup of the original receipt, it is a typed rejection.
 const preparationInput={...saleCreate,dealId:"r471-prepare-"+randomUUID(),
   precondition:{idempotencyKey:"r471-preparation-failure"}};
 await sql("ALTER TABLE crm_sales_pipeline_records RENAME TO fixture_unavailable_deal_catalog");
 try {
   const preparationFailed=await salesV2("CreateDeal",admin,preparationInput);
   assert.equal(preparationFailed.status,503,describeProtobuf(preparationFailed.body));
   const detail=decodeDomainErrorDetail(preparationFailed.body.details.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
   assert.equal(detail.domainCode,"SALES_WRITE_PREPARATION_FAILED");
   assert.equal(detail.commitOutcome,1);
   assert.equal(detail.requestId,preparationFailed.headers.get("x-request-id"));
 } finally {
   await sql("ALTER TABLE fixture_unavailable_deal_catalog RENAME TO crm_sales_pipeline_records");
 }
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_records WHERE tenant=? AND id=?",
   "tenant-a",preparationInput.dealId))[0].n,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
   "tenant-a",preparationInput.precondition.idempotencyKey))[0].n,0);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id=? AND subject_id=?",
   "tenant-a",preparationInput.dealId))[0].n,0);
 const receiptProbe=await call("/api/_ci/original-domain-check",admin);
 assert.equal(receiptProbe.status,200);
 assert.equal(receiptProbe.body.receipt_read_classification,"PROVEN_NO_CHANGE");
 await sql("ALTER TABLE crm_sales_deal_receipts RENAME TO fixture_unavailable_deal_receipts");
 try {
   const unavailable=await call("/api/_ci/original-domain-check",admin);
   assert.equal(unavailable.status,200);
   const unknownReplay=await salesV2("CreateDeal",admin,saleCreate);
   assert.equal(unknownReplay.status,503);
   const unknownDetail=decodeDomainErrorDetail(unknownReplay.body.details.find(d=>d.type==="crm.core.v2.DomainErrorDetail"));
   assert.equal(unknownDetail.domainCode,"SALES_COMMIT_UNCONFIRMED");
   assert.equal(unknownDetail.commitOutcome,2);
   assert.equal(unavailable.body.receipt_read_classification,"RECEIPT_READ_FAILED",
     "An inaccessible receipt source must not become a proven CAS conflict");
 } finally {
   await sql("ALTER TABLE fixture_unavailable_deal_receipts RENAME TO crm_sales_deal_receipts");
 }
 const recoveredProbe=await call("/api/_ci/original-domain-check",admin);
 assert.equal(recoveredProbe.status,200);
 assert.equal(recoveredProbe.body.receipt_read_classification,"PROVEN_NO_CHANGE");
 console.log(JSON.stringify({event:"crm.native-s1-reconciliation-read",result:"PASS",
   actual_d1_read_failure:true,restore:true,production_receipt_query:true}));
 // Write evidence only after every scenario assertion above has completed.
 // A successful call is transport exercise coverage, not proof of all possible
 // business states. Domain/CAS/tenant assertions remain independently explicit.
 // Retry the exact previously unconfirmed intents concurrently. Both callers must
 // see one committed version with the same original request ID and audit row.
 for(const [call,method,input,subject] of [
   [v3,"CreateParty",{partyId:"party-audit-failure",kind:1,displayName:"Rollback",
     precondition:{idempotencyKey:"party-audit-failed-create"}},"party-audit-failure"],
   [privacy,"CreatePrivacyCase",{caseId:"privacy-audit-failed-create",kind:1,
     precondition:{idempotencyKey:"privacy-audit-failed-create-key"}},"privacy-audit-failed-create"],
   [accountV2,"CreateAccount",accountRetryInput,accountRetryInput.accountId],
   [contactV2,"CreateContactPoint",contactRetryInput,contactRetryInput.contactPointId],
 ]){
   const replies=await Promise.all([call(method,admin,input),call(method,admin,input)]);
   for(const reply of replies)assert.equal(reply.status,200,describeProtobuf(reply.body));
   const receipts=replies.map(reply=>reply.body.receipt);
   assert.equal(receipts.filter(receipt=>!receipt.replayed).length,1);
   const original=receipts.find(receipt=>!receipt.replayed).requestId;
   for(const receipt of receipts){
     assert.equal(receipt.originalRequestId,original);
     assert.equal(receipt.resourceVersion,1n);
   }
   assert.deepEqual(await sql("SELECT request_id FROM crm_audit WHERE tenant_id=? AND subject_id=?",
     "tenant-a",subject),[{request_id:original}]);
 }
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
 assert.deepEqual([...writesWithReceipt].sort(),[...mutationPaths].sort(),
   "every write method must exercise a committed response with a typed receipt");
 assert.deepEqual([...successfulCalls].sort(),[...rpc.registered_methods].sort(),
   "every compiled RPC must have a successful executed binary call");
 writeFileSync(process.env.CRM_RPC_RUNTIME_PROOF,JSON.stringify({
   source_sha:sourceSha,result:"PASS",features:["native-ci"],rpc,
   committed_rpc_calls:[...writesWithReceipt].sort(),
   successful_rpc_calls:[...successfulCalls].sort(),
   unauthenticated_denials:unauthenticatedDenials.sort(),
   viewer_denials:viewerDenials.sort(),
   fixture_access:"D1 binding",runtime:"Wrangler createTestHarness/workerd",
   jwks_requests:jwksRequests,authentication:"production JWKS path",
  scenarios:"existing signed workerd assertions completed"
 },null,2)+"\n");
 console.log(JSON.stringify({event:"crm.signed-connect-acceptance",
   result:"PASS",admin:true,viewer:true,unauthenticated_denied:true,
   legacy_mutations:0,verified_auth:true}));
}catch(error){
 testHarness.debug();
 throw error;
}finally{
 try{await testHarness.close();}finally{mock.restoreAll();}
}

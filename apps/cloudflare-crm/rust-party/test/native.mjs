// Full generated binary Connect contract and actual React against signed workerd/D1.
import {execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,randomUUID,createHash} from "node:crypto";
import {mkdtempSync,rmSync,writeFileSync,readFileSync} from "node:fs";
import {Miniflare,Log,LogLevel} from "miniflare";
import {unstable_getMiniflareWorkerOptions} from "wrangler";
import {tmpdir} from "node:os";
import {dirname,join,resolve} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import {chromium} from "playwright";
import {typedBrowser} from "./typed-browser.mjs";
import {installUiDiagnostics,assertUiPanel} from "./ui-diagnostics.mjs";
import {binaryPartyV3,decodePartyV3,partyV3Path,
  binaryAccountV2,decodeAccountV2,accountV2Path,
  binaryContactV2,decodeContactV2,contactV2Path,
  binaryRelationshipV2,decodeRelationshipV2,relationshipV2Path,
  binaryTaskV2,decodeTaskV2,taskV2Path,
  binaryQualityV2,decodeQualityV2,qualityV2Path,
  binaryIdentityV2,decodeIdentityV2,identityV2Path,
  binaryConsentV2,decodeConsentV2,consentV2Path,
  binarySalesV2,decodeSalesV2,salesV2Path,
  binaryPrivacyV2,decodePrivacyV2,privacyV2Path,
  binaryEnrichmentV2,decodeEnrichmentV2,enrichmentV2Path,
  binaryCsvV2,decodeCsvV2,csvV2Path,
  binaryOverviewV2,decodeOverviewV2,overviewV2Path,decodeDomainErrorDetail} from "../../../../scripts/crm-connect-binary-fixture.mjs";
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const data=mkdtempSync(join(tmpdir(),"crm-native-connect-"));
let base,runtime,fixtureDb;
const successfulCalls=new Set();
// The CLI migrations finish before the runtime starts. Every live fixture
// read/write and the actual Rust Worker use one native D1 binding.
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
 if(path.startsWith("/crm.")&&response.ok)successfulCalls.add(path);
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
   assert.equal(response.headers.get("x-crm-error-code"),error.code,
     "RPC diagnostics must preserve the actual Connect error code");
 }
 return response;
};
const sourceSha=execFileSync("git",["rev-parse","HEAD"],{cwd,encoding:"utf8"}).trim();
assert.equal(process.env.CRM_SOURCE_SHA,sourceSha,"runtime evidence must identify tested checkout");
assert.ok(process.env.CRM_RPC_RUNTIME_PROOF,"CI runtime output path required");
const run=(args)=>execFileSync("npx",["wrangler",...args],{cwd,encoding:"utf8"});
// SQL fixture access goes through the same native D1 binding as CRM.
const partySnapshot=async()=>(await sql("SELECT "+
  "(SELECT json_group_array(json_array(tenant,id,state,version,actor)) FROM crm_party_records) AS states,"+
  "(SELECT COUNT(*) FROM crm_party_receipts) AS receipts,"+
  "(SELECT COUNT(*) FROM crm_party_import_receipts) AS imports,"+
  "(SELECT COUNT(*) FROM crm_audit) AS audits"));
const {privateKey,publicKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const kid="local-signed-rsa-fixture";
const jwks=JSON.stringify({keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]});
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
try{
 run(["d1","migrations","apply","crm-rust-party-local","--local","--persist-to",data]);
 run(["d1","execute","crm-rust-party-local","--local","--persist-to",data,
   "--command","INSERT INTO crm_tenants VALUES('tenant-a','A'),('tenant-b','B');"+
   "INSERT INTO crm_users(email,tenant_id,role,active) VALUES"+
   "('native-a@example.test','tenant-a','admin',1),"+
   "('native-viewer@example.test','tenant-a','viewer',1),"+
   "('native-b@example.test','tenant-b','admin',1),"+
   "('service:ci-fixture.access','tenant-a','admin',1);"]);
 // This is a fixture row created in isolated D1 before the Worker starts,
 // not a second implementation of CRM's Party persistence.
 const state=JSON.stringify({party_id:"binary-rpc-proof-01",kind:"organization",
   display_name:"Original Rust Party",created_at_unix_nanos:10,
   updated_at_unix_nanos:10,version:1});
 run(["d1","execute","crm-rust-party-local","--local","--persist-to",data,
   "--command","INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id,actor,search_name) "+
     "VALUES('tenant-a','binary-rpc-proof-01','"+state.replaceAll("'","''")+
     "',1,'fixture-key-01','fixture-digest-01',NULL,'native-a@example.test','Original Rust Party');"]);
 // Wrangler's applied-migration ledger must prevent a second reset.
 run(["d1","migrations","apply","crm-rust-party-local","--local","--persist-to",data]);
 // Use the pinned Wrangler config translator: no second routing or asset
 // configuration, no testing Worker, no direct access to storage files.
 const configPath=join(cwd,"wrangler.jsonc");
 const config=JSON.parse(readFileSync(configPath,"utf8"));
 const {workerOptions,externalWorkers}=unstable_getMiniflareWorkerOptions(configPath);
 assert.deepEqual(externalWorkers,[],"one CRM Worker; no external fixture Worker");
 assert.equal(config.d1_databases.length,1);
 assert.equal(config.d1_databases[0].binding,"DB");
 assert.equal(config.d1_databases[0].database_id,"00000000-0000-4000-8000-000000000001");
 assert.equal(config.vars.CRM_CI_LOCAL_ONLY,"yes");
 const scriptPath=resolve(cwd,config.main);
 runtime=new Miniflare({
   host:"127.0.0.1",port:0,defaultPersistRoot:join(data,"v3"),
   log:new Log(LogLevel.INFO),
   workers:[{
     ...workerOptions,name:config.name,scriptPath,modules:true,modulesRoot:dirname(scriptPath),
     // workers-rs emits ESM with .js names; Miniflare defaults .js to CommonJS.
     modulesRules:[{type:"ESModule",include:["**/*.js","**/*.mjs"]},...workerOptions.modulesRules],
     bindings:{...workerOptions.bindings,CRM_NATIVE_CI_JWKS:jwks,GIT_SHA:sourceSha},
   }],
 });
 base=(await runtime.ready).origin;
 fixtureDb=await runtime.getD1Database("DB",config.name);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_tenants"))[0].n,2,
   "runtime must bind the exact Wrangler-migrated isolated test database");
 console.log(JSON.stringify({event:"crm.native-ci.d1-runtime",source_sha:sourceSha,
   runtime:"Miniflare/workerd",fixture_access:"D1 binding",worker_count:1,d1_count:1}));
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
 const version=await rpcFetch("/api/version",{headers:{"cf-access-jwt-assertion":admin}});
 assert.equal(version.status,200);
 assert.equal(version.headers.get("x-crm-source-sha"),sourceSha,"Worker must receive the tested checkout SHA");
 assert.equal((await version.json()).source,sourceSha);
 const compiled=await call("/api/_ci/original-domain-check",admin);
 assert.equal(compiled.status,200,JSON.stringify(compiled.body));
 assert.equal(compiled.body.result,"PASS");
 const rpc=compiled.body.rpc;
 assert.equal(rpc.compiled_policy_check,true);
 assert.ok(rpc.registered_methods.length>0);
 assert.equal(new Set(rpc.registered_methods).size,rpc.registered_methods.length);
 assert.equal(new Map(rpc.policies).size,rpc.policies.length,"duplicate compiled policy");
 assert.deepEqual(rpc.policies.map(([path])=>path).sort(),[...rpc.registered_methods].sort());
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

 for(const path of ["/api/parties","/api/accounts"]){
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
 assert.equal((await sql("SELECT COUNT(*) AS n FROM d1_migrations WHERE name='0002_hard_cutover.sql'"))[0].n,1);
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
   assert.equal((await v3("CreateParty",admin,{partyId:"party-audit-failure",kind:1,displayName:"Rollback",
     precondition:{idempotencyKey:"party-audit-failed-create"}})).status,409);
   assert.equal((await v3("UpdateParty",admin,{...updatedInput,displayName:"Must roll back",
     precondition:{expectedVersion:2n,idempotencyKey:"party-audit-failed-update"}})).status,409);
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
 (await sql("CREATE TRIGGER fixture_fail_account_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('account-audit-rollback','"+accountId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture account audit failure'); END;"));
 try{
   assert.equal((await accountV2("CreateAccount",admin,{
     accountId:"account-audit-rollback",name:"Must roll back",primaryPartyId:id,
     precondition:{idempotencyKey:"account-audit-failed-create"}})).status,409);
   assert.equal((await accountV2("UpdateAccount",admin,{
     ...accountUpdate,name:"Must roll back",
     precondition:{expectedVersion:2n,idempotencyKey:"account-audit-failed-update"}})).status,409);
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
 (await sql("CREATE TRIGGER fixture_fail_contact_audit BEFORE INSERT ON crm_audit "+
   "WHEN NEW.subject_id IN ('contact-audit-rollback','"+contactId+"') "+
   "BEGIN SELECT RAISE(ABORT,'fixture contact audit failure'); END;"));
 try{
   assert.equal((await contactV2("CreateContactPoint",admin,{
     contactPointId:"contact-audit-rollback",partyId:id,kind:1,
     value:"rollback@example.org",preferred:false,
     precondition:{idempotencyKey:"contact-audit-failed-create"}})).status,409);
   assert.equal((await contactV2("UpdateContactPoint",admin,{
     ...contactUpdate,value:"rollback@example.org",
     precondition:{expectedVersion:2n,idempotencyKey:"contact-audit-failed-update"}})).status,409);
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
    assert.equal((await relationV2("UpdatePartyRelationship",admin,{relationshipId:rMid,status:1,
      precondition:{expectedVersion:4n,idempotencyKey:"candidate-rel-actor-failed"}})).status,409);
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
    assert.equal((await taskV2("UpdateTask",admin,{taskId:taskOther,
      subject:patchSubject("Must not persist"),
      precondition:{expectedVersion:3n,idempotencyKey:"candidate-task-actor-failed"}})).status,409);
    assert.deepEqual((await taskActorSnapshot()),beforeTaskFault);
  }finally{await sql("DROP TRIGGER fixture_fail_task_actor");}
  console.log(JSON.stringify({event:"crm.native-r2a-task-actor-audit",result:"PASS",
    update_complete_actor:true,no_op:true,failed_audit_rollback:true}));
console.log(JSON.stringify({event:"crm.native-g3-activities-tasks-v2",
   result:"PASS",rpc_methods:["CreateTask","UpdateTask","CompleteTask","GetTask","ListTasks"],
   generated_binary:true,original_owner:"crm.activities",signed_access:true,
   tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,account_fk:true,d1_cas:true,
   no_op_without_write:true,replay:true,d1_receipts:true,d1_audit:true,
   published_active:true}));


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
   assert.equal((await consentV2("GrantVerifiedEmailConsent",admin,{
     ...r3Grant,authorizationId:failingGrantId,
     precondition:{idempotencyKey:failingKey}})).status,409);
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
assert.equal((await salesV2("CreatePipeline",admin,pipelineSeed)).body.receipt?.replayed,true);
assert.equal((await salesV2("CreatePipeline",admin,{...pipelineSeed,name:"modified"})).status,409);
assert.equal((await salesV2("GetPipeline",tenantB,{pipelineId:"pipeline.enterprise"})).status,404);
assert.equal((await salesV2("GetPipeline",viewer,{pipelineId:"pipeline.enterprise"})).status,200);
const pipelineOther=await salesV2("CreatePipeline",admin,{
  pipelineId:"pipeline.other",name:"Other",stages:[pipelineSeed.stages[0]],
  precondition:{idempotencyKey:"connect-r7-pipeline-create-2"}});
assert.equal(pipelineOther.status,200,describeProtobuf(pipelineOther.body));
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
  assert.notEqual(failed.status,200,describeProtobuf(failed.body));
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
assert.deepEqual(overflow.body.currencyTotals,[],
  "missing optional Money is not zero currency or an invented total");
assert.ok(overflow.body.pageInfo?.nextPageToken);
const overflowLast=await salesV2("ListStageDeals",viewer,{
  pipelineId:"pipeline.other",stageId:"lead",
  page:{pageSize:25,pageToken:overflow.body.pageInfo.nextPageToken}});
assert.equal(overflowLast.status,200);
assert.equal(overflowLast.body.deals.length,2);
assert.equal(overflowLast.body.totalCount,27n);
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
    const query=rust.match(/r#"WITH snapshot_clock AS MATERIALIZED \([\s\S]*?WHERE \{condition\}"#/)?.[0];
    assert.ok(query,"canonical metrics SELECT must be present");
    const metricsSql=query.slice(3,-2).replace("{condition}",
      "d.tenant=? AND json_extract(d.state,'$.stage.pipeline_id')=? "+
      "AND json_extract(d.state,'$.status')=? AND d.account_id=?");
    const metricsPlan=await sql("EXPLAIN QUERY PLAN "+metricsSql,
      "tenant-a","pipeline.other","open",accountId);
    assert.ok(metricsPlan.some(row=>row.detail.includes("idx_crm_sales_board_stage")));
    assert.ok(metricsPlan.some(row=>row.detail.includes("crm_sales_deal_events")));
    assert.ok(metricsPlan.some(row=>row.detail.includes("idx_crm_sales_link_deal")));
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
    assert.equal(denied.status,409,describeProtobuf(denied.body));
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
   assert.equal((await privacy("CreatePrivacyCase",admin,{
     caseId:"privacy-audit-failed-create",kind:1,
     precondition:{idempotencyKey:"privacy-audit-failed-create-key"}})).body.code,"aborted");
   assert.equal((await privacy("SubmitPrivacyCase",knownService,{
     caseId:failingTransitionId,
     precondition:{expectedVersion:1n,idempotencyKey:"privacy-audit-failed-submit-key"}
   })).body.code,"aborted");
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
   assert.equal((await enrichment("PublishProviderProfile",admin,{
     ...profileInput,retentionDays:37,
     precondition:{idempotencyKey:"enrichment-profile-audit-failure"}
   })).body.code,"aborted");
   assert.equal((await enrichment("PublishMapping",knownService,{
     ...mappingInput,providerResponseFieldPath:"organization.alternate_legal_name",
     precondition:{idempotencyKey:"enrichment-mapping-audit-failure"}
   })).body.code,"aborted");
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
 for(const invalid of [
   new Uint8Array(),new Uint8Array(16385),new TextEncoder().encode("invalid,csv"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\n"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\nx,invalid,Name,1\r\n"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\nx,person,Name,0\r\n"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\nx,person,One,1\r\nx,person,Two,1\r\n"),
   new TextEncoder().encode("party_id,kind,display_name,resource_version\r\n"+
     Array.from({length:21},(_,i)=>"source-"+i+",person,Name "+i+",1\r\n").join(""))
 ]){
   assert.equal((await csv("PreviewPartyImport",admin,{csv:invalid})).body.code,"invalid_argument");
 }
 // A conflict on the second target must leave the first target absent.
 const conflictBytes=new TextEncoder().encode(
   "party_id,kind,display_name,resource_version\r\nconflict-one,person,First,1\r\nconflict-two,person,Second,1\r\n");
 const conflictPreview=await csv("PreviewPartyImport",admin,{csv:conflictBytes});
 assert.equal(conflictPreview.status,200,describeProtobuf(conflictPreview.body));
 const [firstTarget,secondTarget]=conflictPreview.body.targetPartyIds;
 assert.equal((await v3("CreateParty",admin,{partyId:secondTarget,kind:1,displayName:"Existing target",
   precondition:{idempotencyKey:"csv-target-conflict-create"}})).status,200);
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
   assert.equal((await csv("ApplyPartyImport",knownService,rollbackInput)).status,409);
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
 assert.equal((await csv("PreviewPartyExport",admin,{kind:0})).body.code,"invalid_argument");
 assert.equal((await csv("PreviewPartyExport",admin,{kind:999})).body.code,"invalid_argument");
 const exportBefore=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 const exported=await csv("PreviewPartyExport",admin,{});
 assert.equal(exported.status,200,describeProtobuf(exported.body));
 assert.equal(exported.body.maximumTenantParties,100);
 assert.deepEqual(exported.body.fields,["party_id","kind","display_name","resource_version"]);
 const download=await rpcFetch("/api/customer-data-operations/parties.csv",{
   headers:{"cf-access-jwt-assertion":admin}});
 assert.equal(download.status,200);
 assert.match(download.headers.get("content-type"),/^text\/csv/);
 const downloaded=new Uint8Array(await download.arrayBuffer());
 assert.equal(createHash("sha256").update(downloaded).digest("hex"),exported.body.csvSha256Hex);
 assert.equal(download.headers.get("x-crm-export-count"),String(exported.body.count));
 assert.equal(download.headers.get("cache-control"),"private, no-store");
 assert.equal((await rpcFetch("/api/customer-data-operations/parties.csv")).status,401);
 assert.equal((await rpcFetch("/api/customer-data-operations/parties.csv",{
   headers:{"cf-access-jwt-assertion":viewer}})).status,403);
 const filteredExport=await csv("PreviewPartyExport",admin,{kind:1});
 assert.equal(filteredExport.status,200,describeProtobuf(filteredExport.body));
 assert.ok(filteredExport.body.count<exported.body.count);
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
 assert.equal((await rpcFetch("/api/customer-data-operations/parties.csv?kind=organization",{
   headers:{"cf-access-jwt-assertion":tenantB}})).status,409);
 console.log(JSON.stringify({event:"crm.native-party-csv-v2",result:"PASS",admin_read:true,
   original_owner:"crm.customer-data-operations",party_owner:"crm.parties",
   tenant_ab:true,atomic:true,replay:true,audit:true,verified_actor:true,sql_failure_rollback:true,csv_bytes_sha256:true,bounded:true}));
 await typedBrowser({base,admin,viewer,tenantB,sql,sourceSha,adminIdentity:"native-a@example.test"});
 const salesAuditBeforeFilter=(await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n;
 assert.deepEqual((await collectPages(salesV2,"ListDeals","deals",{accountId})).map(d=>d.dealId),
   (await collectPages(salesV2,"ListDeals","deals")).filter(d=>d.accountId===accountId).map(d=>d.dealId));
 assert.deepEqual((await salesV2("ListDeals",tenantB,{accountId})).body.deals,[]);
 assert.deepEqual((await salesV2("ListDeals",viewer,{accountId:"missing-account"})).body.deals,[]);
 assert.equal((await salesV2("ListDeals",viewer,{accountId:"bad/id"})).status,400);
 assert.equal((await sql("SELECT COUNT(*) AS n FROM crm_audit"))[0].n,salesAuditBeforeFilter);
 // Search is part of the existing List RPCs. Put matching rows after an entire
 // unfiltered page; literal punctuation and Unicode must not become SQL patterns.
 for(let index=0;index<101;index++){
   const suffix=String(index).padStart(3,"0");
   assert.equal((await v3("CreateParty",admin,{partyId:"zz-search-padding-"+suffix,kind:1,
     displayName:"Search padding "+suffix,precondition:{idempotencyKey:"search-party-padding-"+suffix}})).status,200);
   assert.equal((await accountV2("CreateAccount",admin,{accountId:"zz-search-padding-"+suffix,
     name:"Search padding "+suffix,primaryPartyId:id,
     precondition:{idempotencyKey:"search-account-padding-"+suffix}})).status,200);
 }
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
 assert.deepEqual([...successfulCalls].sort(),[...rpc.registered_methods].sort(),
   "every compiled RPC must have a successful executed binary call");
 writeFileSync(process.env.CRM_RPC_RUNTIME_PROOF,JSON.stringify({
   source_sha:sourceSha,result:"PASS",features:["native-ci"],rpc,
   successful_rpc_calls:[...successfulCalls].sort(),
   unauthenticated_denials:unauthenticatedDenials.sort(),
   viewer_denials:viewerDenials.sort(),
   fixture_access:"D1 binding",runtime:"Miniflare/workerd",
  scenarios:"existing signed workerd assertions completed"
 },null,2)+"\n");
 console.log(JSON.stringify({event:"crm.signed-connect-acceptance",
   result:"PASS",admin:true,viewer:true,unauthenticated_denied:true,
   legacy_mutations:0,verified_auth:true}));
}finally{
 if(runtime)await runtime.dispose();
 rmSync(data,{recursive:true,force:true});
}

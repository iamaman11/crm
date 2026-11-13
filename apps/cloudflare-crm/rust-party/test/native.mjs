// Full generated binary Connect contract and actual React against signed workerd/D1.
import {spawn,execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,randomUUID,createHash} from "node:crypto";
import {mkdtempSync,rmSync,writeFileSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import {typedBrowser} from "./typed-browser.mjs";
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
  binaryOverviewV2,decodeOverviewV2,overviewV2Path} from "../../../../scripts/crm-connect-binary-fixture.mjs";
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const data=mkdtempSync(join(tmpdir(),"crm-native-connect-"));
const base="http://127.0.0.1:18789";
const successfulCalls=new Set();
const rpcFetch=async(path,options)=>{
 const response=await globalThis.fetch(base+path,options);
 if(path.startsWith("/crm.")&&response.ok)successfulCalls.add(path);
 if(path.startsWith("/crm.")&&!response.ok){
   const error=await response.clone().json();
   assert.equal(response.headers.get("x-crm-error-code"),error.code,
     "RPC diagnostics must preserve the actual Connect error code");
 }
 return response;
};
const sourceSha=execFileSync("git",["rev-parse","HEAD"],{cwd,encoding:"utf8"}).trim();
assert.equal(process.env.CRM_SOURCE_SHA,sourceSha,"runtime evidence must identify tested checkout");
assert.ok(process.env.CRM_RPC_RUNTIME_PROOF,"CI runtime output path required");
const run=(args)=>execFileSync("npx",["wrangler",...args],{cwd,encoding:"utf8"});
const {privateKey,publicKey}=generateKeyPairSync("rsa",{modulusLength:2048});
const kid="local-signed-rsa-fixture";
const jwks=JSON.stringify({keys:[{...publicKey.export({format:"jwk"}),kid,alg:"RS256",use:"sig"}]});
const iss="https://royal-limit-ea43.cloudflareaccess.com";
const aud="e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const now=Math.floor(Date.now()/1000);
function jwt(email){
 const parts=[{alg:"RS256",kid,typ:"JWT"},{
   iss,aud,exp:now+3600,iat:now-10,sub:email,email
 }].map(x=>Buffer.from(JSON.stringify(x)).toString("base64url"));
 const raw=parts.join(".");
 return raw+"."+createSign("RSA-SHA256").update(raw).end().sign(privateKey).toString("base64url");
}
let child;
try{
 run(["d1","migrations","apply","crm-rust-party-local","--local","--persist-to",data]);
 run(["d1","execute","crm-rust-party-local","--local","--persist-to",data,
   "--command","INSERT INTO crm_tenants VALUES('tenant-a','A'),('tenant-b','B');"+
   "INSERT INTO crm_users(email,tenant_id,role,active) VALUES"+
   "('native-a@example.test','tenant-a','admin',1),"+
   "('native-viewer@example.test','tenant-a','viewer',1),"+
   "('native-b@example.test','tenant-b','admin',1);"]);
 // This is a fixture row created in isolated D1 before the Worker starts,
 // not a second implementation of CRM's Party persistence.
 const state=JSON.stringify({party_id:"binary-rpc-proof-01",kind:"organization",
   display_name:"Original Rust Party",created_at_unix_nanos:10,
   updated_at_unix_nanos:10,version:1});
 run(["d1","execute","crm-rust-party-local","--local","--persist-to",data,
   "--command","INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) "+
     "VALUES('tenant-a','binary-rpc-proof-01','"+state.replaceAll("'","''")+
     "',1,'fixture-key-01','fixture-digest-01',NULL);"]);
 // Wrangler's applied-migration ledger must prevent a second reset.
 run(["d1","migrations","apply","crm-rust-party-local","--local","--persist-to",data]);
 child=spawn("npx",["wrangler","dev","--local","--port","18789","--persist-to",data,
  "--var","CRM_NATIVE_CI_JWKS:"+jwks,"--var","GIT_SHA:"+sourceSha],
  {cwd,detached:true,stdio:["ignore","ignore","pipe"]});
 let stderr="";
 child.stderr.on("data",b=>stderr=(stderr+b.toString()).slice(-2500));
 let ready=false;
 for(let i=0;i<120;i++){
   if(child.exitCode!==null)throw Error("native-ci exited "+stderr);
   try{if((await rpcFetch("/api/parties")).status===401){ready=true;break;}}catch{}
   await new Promise(resolve=>setTimeout(resolve,250));
 }
 assert(ready,"native-ci not ready: "+stderr);
 async function call(path,token,method="GET"){
   const response=await rpcFetch(path,{method,
     headers:{...(token?{"cf-access-jwt-assertion":token}:{}),"content-type":"application/json"},
     ...(method==="POST"?{body:"{}"}:{})});
   return {status:response.status,body:await response.json()};
 }
 assert.equal((await call("/api/parties",null)).status,401);
 const admin=jwt("native-a@example.test"),viewer=jwt("native-viewer@example.test");
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
 const v3=async(method,token,payload)=>{
   const response=await rpcFetch(partyV3Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
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
   for(let attempt=0;attempt<16;attempt++){
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
   throw Error("filtered pagination never reached its last page");
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
 const created=await v3("CreateParty",admin,creation);
 assert.equal(created.status,200,describeProtobuf(created.body));
 assert.equal(created.body.party?.partyId,id);
 assert.equal(created.body.party?.displayName,"Connect Native Rust");
 assert.equal(created.body.receipt?.resourceVersion,1n);
 assert.equal(created.body.receipt?.replayed,false);
 assert.equal(created.body.receipt?.requestId,created.headers.get("x-request-id"));
 const replay=await v3("CreateParty",admin,creation);
 assert.equal(replay.status,200);
 assert.equal(replay.body.receipt?.replayed,true);
 assert.equal((await v3("CreateParty",admin,{...creation,
   displayName:"different replay"})).status,409);
 assert.equal((await v3("GetParty",tenantB,{partyId:id})).status,404);
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
 const updated=await v3("UpdateParty",admin,updatedInput);
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
 const sql=(query)=>JSON.parse(run(["d1","execute","crm-rust-party-local","--local",
   "--persist-to",data,"--json","--command",query]))[0].results;
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_records WHERE id='binary-rpc-proof-01'")[0].n,1,
   "Repeated migrations apply must preserve existing business data");
 assert.equal(sql("SELECT COUNT(*) AS n FROM d1_migrations WHERE name='0002_hard_cutover.sql'")[0].n,1);
 assert.equal(sql("SELECT version FROM crm_party_records WHERE id='"+id+"'")[0].version,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v3-create-001','candidate-v3-update-001')")[0].n,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+id+"'")[0].n,2);
 assert.equal((await v3("GetParty",admin,{partyId:id})).body.party?.version,2n);

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
 const accountV2=async(method,token,payload)=>{
   const response=await rpcFetch(accountV2Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
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
 const accountCreated=await accountV2("CreateAccount",admin,accountInput);
 assert.equal(accountCreated.status,200,describeProtobuf(accountCreated.body));
 assert.equal(accountCreated.body.account?.accountId,accountId);
 assert.equal(accountCreated.body.account?.name,"Connected Account");
 assert.equal(accountCreated.body.account?.parties?.[0]?.partyId,id);
 assert.equal(accountCreated.body.account?.parties?.[0]?.role,1);
 assert.equal(accountCreated.body.receipt?.resourceVersion,1n);
 assert.equal(accountCreated.body.receipt?.replayed,false);
 assert.equal(accountCreated.body.receipt?.requestId,accountCreated.headers.get("x-request-id"));
 const accountReplay=await accountV2("CreateAccount",admin,accountInput);
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
 const accountUpdated=await accountV2("UpdateAccount",admin,accountUpdate);
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
 assert.equal(sql("SELECT version FROM crm_account_records WHERE tenant='tenant-a' "+
   "AND id='"+accountId+"'")[0].version,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_account_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v2-account-create-001','candidate-v2-account-update-001')")[0].n,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+accountId+"' AND action LIKE 'accounts.%'")[0].n,2);
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
 const contactV2=async(method,token,payload)=>{
   const response=await rpcFetch(contactV2Path(method),{method:"POST",
     headers:{"content-type":"application/proto","connect-protocol-version":"1",
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
 const contactCreated=await contactV2("CreateContactPoint",admin,contactInput);
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
 const contactReplay=await contactV2("CreateContactPoint",admin,contactInput);
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
 const contactUpdated=await contactV2("UpdateContactPoint",admin,contactUpdate);
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
 assert.equal(sql("SELECT version FROM crm_contact_point_records WHERE tenant='tenant-a' "+
   "AND id='"+contactId+"'")[0].version,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_contact_point_receipts WHERE tenant='tenant-a' "+
   "AND key IN ('candidate-v2-contact-create-001','candidate-v2-contact-update-001')")[0].n,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"' AND action LIKE 'contact_points.%'")[0].n,2);
 // Filtered ContactPoint list uses the EXISTING D1 party_id column.
 // Nonmatches sort before our two matches, exposing the old LIMIT bug.
 const contactLastId="contact-zzz-ours-"+randomUUID();
 for(const [index,pointToCreate] of [
   ["contact-aaa-other-"+randomUUID(),pagingPersonId],
   ["contact-aab-other-"+randomUUID(),pagingPersonId],
   [contactLastId,id]
 ].entries()){
   const made=await contactV2("CreateContactPoint",admin,{
     contactPointId:pointToCreate[0],partyId:pointToCreate[1],kind:1,
     value:"page-"+index+"@example.org",preferred:false,
     precondition:{idempotencyKey:"page-contact-create-"+index}});
   assert.equal(made.status,200,describeProtobuf(made.body));
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
 const beforeReplay=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"'")[0].n;
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
 assert.equal(sql("SELECT version FROM crm_contact_point_records WHERE tenant='tenant-a' "+
   "AND id='"+contactId+"'")[0].version,4);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+contactId+"'")[0].n,beforeReplay);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_contact_point_receipts "+
   "WHERE tenant='tenant-a' AND key IN ("+
   "'candidate-v2-contact-keep-001','candidate-v2-contact-later-001')")[0].n,2);
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
 const updatedRel=await relationV2("UpdatePartyRelationship",admin,relUpdate);
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
 const relAuditBefore=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"+
   " AND subject_id='"+rMid+"'")[0].n;
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
 assert.equal(sql("SELECT version FROM crm_party_relationship_records WHERE tenant='tenant-a'"+
   " AND id='"+rMid+"'")[0].version,4);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'"+
   " AND subject_id='"+rMid+"'")[0].n,relAuditBefore);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_relationship_receipts WHERE tenant='tenant-a'"+
   " AND key IN ('candidate-rel-create-mid','candidate-rel-update-01',"+
   " 'candidate-rel-keep-02','candidate-rel-later-03')")[0].n,4);
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
 const taskAuditBeforeFilters=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'")[0].n;
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'")[0].n,taskAuditBeforeFilters,
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
 const taskRow=()=>sql("SELECT state,version FROM crm_activity_task_records WHERE tenant='tenant-a' AND id='"+taskOther+"'")[0];
 const taskAudits=()=>sql("SELECT action,request_id FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+taskOther+"' ORDER BY rowid");
 const beforeTask=taskRow(),beforeTaskAudit=taskAudits();
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
 assert.deepEqual(taskRow(),beforeTask,"late rejection must not persist an earlier field change");
 assert.deepEqual(taskAudits(),beforeTaskAudit,"rejection must not create audit");
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a' AND key='"+updateKey+"'")[0].n,0);
 const updateTask={...baseUpdate,subject:patchSubject("Edited original Task"),
   description:{operation:{case:"set",value:"Edited through Connect"}},
   priority:3,dueAt:{operation:{case:"clear",value:true}}};
 const updatedTask=await taskV2("UpdateTask",admin,updateTask);
 assert.equal(updatedTask.status,200,describeProtobuf(updatedTask.body));
 assert.equal(updatedTask.body.task.subject,"Edited original Task");
 assert.equal(updatedTask.body.task.description,"Edited through Connect");
 assert.equal(updatedTask.body.task.priority,3);
 assert.equal(updatedTask.body.task.dueAt,undefined);
 assert.equal(updatedTask.body.task.version,2n);
 assert.equal(updatedTask.body.receipt.resourceVersion,2n);
 assert.equal(updatedTask.body.receipt.requestId,updatedTask.headers.get("x-request-id"));
 assert.deepEqual(taskAudits().map(x=>x.action),["activity_tasks.create","activity_tasks.update"]);
 assert.equal(taskAudits()[1].request_id,updatedTask.headers.get("x-request-id"));
 assert.deepEqual((await collectPages(taskV2,"ListTasks","tasks",{priority:3,accountId:accountNextId})).map(x=>x.taskId),[taskOther]);
 assert.equal((await collectPages(taskV2,"ListTasks","tasks",{priority:4,accountId:accountNextId})).length,0);
 const replayTask=await taskV2("UpdateTask",admin,updateTask);
 assert.equal(replayTask.status,200);
 assert.equal(replayTask.body.receipt.replayed,true);
 assert.equal(taskAudits().length,2,"replay cannot duplicate audit");
 assert.equal((await taskV2("UpdateTask",admin,{...updateTask,subject:patchSubject("Same key, different payload")})).status,409);
 assert.equal((await taskV2("UpdateTask",admin,{...updateTask,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-update-stale"}})).status,409);
 assert.equal(taskAudits().length,2,"stale version must not create audit");
 const secondTaskEdit=await taskV2("UpdateTask",admin,{taskId:taskOther,
   description:{operation:{case:"clear",value:true}},priority:2,
   dueAt:{operation:{case:"set",value:{seconds:1870000000n,nanos:987654321}}},
   precondition:{expectedVersion:2n,idempotencyKey:"candidate-task-update-second"}});
 assert.equal(secondTaskEdit.status,200,describeProtobuf(secondTaskEdit.body));
 assert.equal(secondTaskEdit.body.task.description,"");
 assert.equal(secondTaskEdit.body.task.dueAt.nanos,987654321);
 assert.equal(secondTaskEdit.body.task.version,3n);
 assert.deepEqual(taskAudits().map(x=>x.action),["activity_tasks.create","activity_tasks.update","activity_tasks.update"]);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts WHERE tenant='tenant-a' AND key IN ('candidate-task-create-other','candidate-task-update-other','candidate-task-update-second')")[0].n,3);
 const taskComplete={taskId:taskMid,
   precondition:{expectedVersion:1n,idempotencyKey:"candidate-task-complete-mid"}};
 assert.equal((await taskV2("CompleteTask",viewer,taskComplete)).status,403);
 assert.equal((await taskV2("CompleteTask",admin,{
   ...taskComplete,precondition:{expectedVersion:0n,
     idempotencyKey:"candidate-task-invalid-version"}})).status,400);
 const completed=await taskV2("CompleteTask",admin,taskComplete);
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
 const beforeNoop=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+taskMid+"'")[0].n;
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
 assert.equal(sql("SELECT version FROM crm_activity_task_records WHERE tenant='tenant-a' "+
   "AND id='"+taskMid+"'")[0].version,2);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+taskMid+"'")[0].n,beforeNoop);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_activity_task_receipts "+
   "WHERE tenant='tenant-a' AND key IN ('candidate-task-create-mid',"+
   "'candidate-task-complete-mid')")[0].n,2);
 console.log(JSON.stringify({event:"crm.native-g3-activities-tasks-v2",
   result:"PASS",rpc_methods:["CreateTask","UpdateTask","CompleteTask","GetTask","ListTasks"],
   generated_binary:true,original_owner:"crm.activities",signed_access:true,
   tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,account_fk:true,d1_cas:true,
   no_op_without_write:true,replay:true,d1_receipts:true,d1_audit:true,
   published_active:true}));


 // Original read-only DataQuality and Identity Resolution, signed binary
 // Connect from the exact generated descriptor, with tenant A/B and viewer.
 const signedReadOnly=(path,encode,decode)=>async(method,token,payload)=>{
   const r=await rpcFetch(path(method),{method:"POST",headers:{
     "content-type":"application/proto","connect-protocol-version":"1",
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
   "AND subject_id='"+shortQualityId+"'")[0].n,1,
   "quality read must not append audit or finding");
 console.log(JSON.stringify({event:"crm.native-g3-data-quality-v2",
   result:"PASS",rpc_methods:["EvaluatePartyName"],generated_binary:true,
   original_owner:"crm.data-quality",tenant_ab:true,viewer_read:true,
   no_duplicate_rules:true,no_writes:true,published_active:true}));

 const beforeCandidates=await identityV2("FindSharedEmailCandidates",viewer,{page:{pageSize:1}});
 assert.equal(beforeCandidates.status,200,describeProtobuf(beforeCandidates.body));
 assert.equal(beforeCandidates.body.candidates.length,0);
 const auditBefore=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'")[0].n;
 for(const [index,partyId] of [pagingPersonId,"binary-rpc-proof-01"].entries()){
   const contact=await contactV2("CreateContactPoint",admin,{
     contactPointId:"shared-email-"+index+"-"+randomUUID(),
     partyId,kind:1,value:"later-timestamps@example.org",preferred:false,
     precondition:{idempotencyKey:"shared-email-create-"+index}});
   assert.equal(contact.status,200,describeProtobuf(contact.body));
   assert.equal(contact.body.contactPoint?.verified,false);
 }
 const auditAfter=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'")[0].n;
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a'")[0].n,
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
assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+denyInput.authorizationId+"'")[0].n,1,
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

// Original crm-sales v2: generated binary Connect, original Deal aggregate,
// one tenant D1, exact money, CAS, receipt replay, and atomic stage->Activity.
const salesV2=signedReadOnly(salesV2Path,binarySalesV2,decodeSalesV2);
const dealId="sales-mid-"+randomUUID();
const stageLead={pipelineId:"pipeline.enterprise",stageId:"lead",ordinal:1};
const stageQualified={pipelineId:"pipeline.enterprise",stageId:"qualified",ordinal:2};
const saleCreate={dealId,name:"Connect Deal",accountId,
  stage:stageLead,amount:{minorUnits:12500n,currencyCode:"USD"},
  probabilityBasisPoints:2500,
  precondition:{idempotencyKey:"connect-sales-create-main"}};
assert.equal((await salesV2("CreateDeal",null,saleCreate)).status,401);
assert.equal((await salesV2("CreateDeal",viewer,saleCreate)).status,403);
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
  ...saleCreate,precondition:{idempotencyKey:"connect-sales-create-invalid-version",
    expectedVersion:1n}})).status,400);
const salesCreated=await salesV2("CreateDeal",admin,saleCreate);
assert.equal(salesCreated.status,200,describeProtobuf(salesCreated.body));
assert.equal(salesCreated.body.deal?.dealId,dealId);
assert.equal(salesCreated.body.deal?.name,"Connect Deal");
assert.equal(salesCreated.body.deal?.amount?.minorUnits,12500n);
assert.equal(salesCreated.body.deal?.amount?.currencyCode,"USD");
assert.equal(salesCreated.body.deal?.status,1);
assert.equal(salesCreated.body.receipt?.resourceVersion,1n);
assert.equal(salesCreated.body.receipt?.replayed,false);
assert.equal(salesCreated.body.receipt?.requestId,salesCreated.headers.get("x-request-id"));
const saleReplayed=await salesV2("CreateDeal",admin,saleCreate);
assert.equal(saleReplayed.status,200);
assert.equal(saleReplayed.body.receipt?.replayed,true);
assert.equal((await salesV2("CreateDeal",admin,{
  ...saleCreate,name:"changed idempotency payload"})).status,409);
assert.equal((await salesV2("GetDeal",tenantB,{dealId})).status,404);
assert.equal((await salesV2("GetDeal",viewer,{dealId})).body.deal?.dealId,dealId);
assert.equal((await salesV2("GetDeal",viewer,{dealId:"bad/invalid"})).status,400);
const updateSale={dealId,
  name:{operation:{case:"set",value:"Updated Connect Deal"}},
  amount:{operation:{case:"set",value:{minorUnits:98765n,currencyCode:"PLN"}}},
  probabilityBasisPoints:6750,
  precondition:{expectedVersion:1n,idempotencyKey:"connect-sales-update-main"}};
assert.equal((await salesV2("UpdateDeal",viewer,updateSale)).status,403);
assert.equal((await salesV2("UpdateDeal",admin,{
  ...updateSale,precondition:{expectedVersion:100n,
    idempotencyKey:"connect-sales-stale-version"}})).status,409);
const saleUpdated=await salesV2("UpdateDeal",admin,updateSale);
assert.equal(saleUpdated.status,200,describeProtobuf(saleUpdated.body));
assert.equal(saleUpdated.body.deal?.name,"Updated Connect Deal");
assert.equal(saleUpdated.body.deal?.amount?.minorUnits,98765n);
assert.equal(saleUpdated.body.deal?.amount?.currencyCode,"PLN");
assert.equal(saleUpdated.body.deal?.probabilityBasisPoints,6750);
assert.equal(saleUpdated.body.receipt?.resourceVersion,2n);
assert.equal(saleUpdated.body.receipt?.replayed,false);
const saleUpdateReplayed=await salesV2("UpdateDeal",admin,updateSale);
assert.equal(saleUpdateReplayed.status,200);
assert.equal(saleUpdateReplayed.body.receipt?.replayed,true);
assert.equal((await salesV2("UpdateDeal",admin,{
  ...updateSale,probabilityBasisPoints:100})).status,409);
const advanceSale={dealId,targetStage:stageQualified,targetStatus:1,
  precondition:{expectedVersion:2n,idempotencyKey:"connect-sales-advance-main"}};
assert.equal((await salesV2("AdvanceStage",viewer,advanceSale)).status,403);
assert.equal((await salesV2("AdvanceStage",admin,{
  ...advanceSale,targetStage:stageLead,
  precondition:{expectedVersion:2n,idempotencyKey:"connect-sales-regress"}})).status,400);
const taskCountBefore=sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'")[0].n;
const advancedSale=await salesV2("AdvanceStage",admin,advanceSale);
assert.equal(advancedSale.status,200,describeProtobuf(advancedSale.body));
assert.equal(advancedSale.body.deal?.stage?.stageId,"qualified");
assert.equal(advancedSale.body.deal?.version,3n);
assert.equal(advancedSale.body.receipt?.resourceVersion,3n);
assert.equal(advancedSale.body.receipt?.replayed,false);
const taskCountAfter=sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'")[0].n;
assert.equal(taskCountAfter,taskCountBefore+1,
  "original Sales stage event must create one Activities Task atomically");
const advanceReplayed=await salesV2("AdvanceStage",admin,advanceSale);
assert.equal(advanceReplayed.status,200);
assert.equal(advanceReplayed.body.receipt?.replayed,true);
assert.equal(sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'")[0].n,taskCountAfter);
const closeSale={dealId,targetStage:{pipelineId:"pipeline.enterprise",
  stageId:"won",ordinal:3},targetStatus:2,closeReasonCode:"contract-signed",
  precondition:{expectedVersion:3n,idempotencyKey:"connect-sales-close-main"}};
const closedSale=await salesV2("AdvanceStage",admin,closeSale);
assert.equal(closedSale.status,200,describeProtobuf(closedSale.body));
assert.equal(closedSale.body.deal?.status,2);
assert.equal(closedSale.body.deal?.version,4n);
assert.ok(closedSale.body.deal?.closedAt);
assert.equal(sql("SELECT COUNT(*) AS n FROM crm_activity_task_records "+
  "WHERE tenant='tenant-a'")[0].n,taskCountAfter,
  "Won closure must not create a second follow-up Task");
assert.equal((await salesV2("AdvanceStage",admin,closeSale))
  .body.receipt?.replayed,true);
assert.equal(sql("SELECT COUNT(*) AS n FROM crm_sales_link_delivery_records "+
  "WHERE tenant='tenant-a' AND deal_id='"+dealId+"'")[0].n,2,
  "two original one-batch Sales->Activities delivery receipts");
const dealOther="sales-aaa-other-"+randomUUID();
const dealLast="sales-zzz-ours-"+randomUUID();
for(const [index,nextId,pipe] of [
  [0,dealOther,"pipeline.other"],[1,dealLast,"pipeline.enterprise"]
]){
  const created=await salesV2("CreateDeal",admin,{
    dealId:nextId,name:"Sales page "+index,accountId,
    stage:{pipelineId:pipe,stageId:"lead",ordinal:1},
    probabilityBasisPoints:1000,
    precondition:{idempotencyKey:"connect-sales-paging-"+index}});
  assert.equal(created.status,200,describeProtobuf(created.body));
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
assert.equal((await salesV2("ListDeals",viewer,{
  page:{pageSize:1,pageToken:dealLast},pipelineId:"pipeline.enterprise"})
  ).body.deals.length,0);
assert.equal(sql("SELECT COUNT(*) AS n FROM crm_sales_deal_receipts "+
  "WHERE tenant='tenant-a' AND key IN ('connect-sales-create-main',"+
  "'connect-sales-update-main','connect-sales-advance-main','connect-sales-close-main')"
  )[0].n,4,"original Deal receipt owners and D1 triggers remain singular");
assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' "+
  "AND subject_id='"+dealId+"'")[0].n,4,
  "create + update + two stage transitions each write exactly one audit");
console.log(JSON.stringify({event:"crm.native-g3-sales-v2",result:"PASS",
  rpc_methods:["CreateDeal","UpdateDeal","AdvanceStage","GetDeal","ListDeals"],
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
 const draft=await privacy("CreatePrivacyCase",admin,draftInput);
 assert.equal(draft.status,200,describeProtobuf(draft.body));
 assert.equal(draft.body.privacyCase?.caseId,caseId);
 assert.equal(draft.body.privacyCase?.status,1);
 assert.equal(draft.body.privacyCase?.version,1n);
 assert.equal(draft.body.privacyCase?.subjectVerified,false);
 assert.equal(draft.body.receipt?.requestId,draft.headers.get("x-request-id"));
 assert.equal(draft.body.receipt?.replayed,false);
 const originalAudit=sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'")[0].n;
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
 const submitted=await privacy("SubmitPrivacyCase",admin,submit);
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
 assert.equal(sql("SELECT version FROM crm_privacy_case_records WHERE tenant='tenant-a' AND id='"+caseId+"'")[0].version,3);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_privacy_case_receipts WHERE tenant='tenant-a'")[0].n,3);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'")[0].n,3);
 for(const suffix of ["aaa","zzz"]){
   const result=await privacy("CreatePrivacyCase",admin,{caseId:"privacy-"+suffix,kind:4,
     precondition:{idempotencyKey:"privacy-page-"+suffix}});
   assert.equal(result.status,200,describeProtobuf(result.body));
 }
 assert.equal((await privacy("ListPrivacyCases",viewer,{page:{pageSize:101}})).body.code,"invalid_argument");
 const allCases=await collectPages(privacy,"ListPrivacyCases","cases");
 assert.equal(allCases.length,3);
 assert.deepEqual(allCases.map(x=>x.caseId),["privacy-aaa",caseId,"privacy-zzz"].sort());
 assert.equal((await privacy("ListPrivacyCases",tenantB,{page:{pageSize:1}})).body.cases.length,0);
 assert.equal((await privacy("ListPrivacyCases",viewer,{page:{pageSize:1,pageToken:"privacy-zzz"}})).body.cases.length,0);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit WHERE tenant_id='tenant-a' AND subject_id='"+caseId+"'")[0].n,3,
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
 const enrichmentAuditBefore=sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n;
 const partyCountBefore=sql("SELECT COUNT(*) AS n FROM crm_party_records")[0].n;
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
 const profileResult=await enrichment("PublishProviderProfile",admin,profileInput);
 assert.equal(profileResult.status,200,describeProtobuf(profileResult.body));
 const profileId=profileResult.body.profile.profileVersionId;
 assert.ok(profileId);
 assert.equal(profileResult.body.profile.effectiveAt.nanos,123000000);
 assert.equal(profileResult.body.receipt.resourceVersion,1n);
 assert.equal(profileResult.body.receipt.requestId,profileResult.headers.get("x-request-id"));
 assert.equal(profileResult.body.receipt.replayed,false);
 assert.equal((await enrichment("PublishProviderProfile",admin,profileInput)).body.receipt.replayed,true);
 assert.equal((await enrichment("PublishProviderProfile",admin,{...profileInput,retentionDays:31})).body.code,"aborted");
 assert.equal((await enrichment("GetProviderProfile",viewer,{profileVersionId:profileId})).body.profile.profileVersionId,profileId);
 assert.equal((await enrichment("GetProviderProfile",tenantB,{profileVersionId:profileId})).body.code,"not_found");
 const mappingInput={
   mappingKey:"party_display_name",providerProfileVersionId:profileId,
   providerResponseFieldPath:"organization.legal_name",maximumSuggestionsPerResponse:1,
   confidenceRequired:true,precondition:{idempotencyKey:"enrichment-mapping-main"}
 };
 assert.equal((await enrichment("PublishMapping",tenantB,mappingInput)).body.code,"not_found");
 assert.equal((await enrichment("PublishMapping",admin,{...mappingInput,maximumSuggestionsPerResponse:0})).body.code,"invalid_argument");
 const mappingResult=await enrichment("PublishMapping",admin,mappingInput);
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
   assert.equal((await enrichment(method,tenantB,{page:{pageSize:1}})).body[field].length,0);
   assert.equal((await enrichment(method,viewer,{page:{pageToken:"bad/id"}})).body.code,"invalid_argument");
 }
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_enrichment_receipts WHERE tenant='tenant-a'")[0].n,6);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_enrichment_profiles WHERE tenant='tenant-a'")[0].n,3);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_enrichment_mappings WHERE tenant='tenant-a'")[0].n,3);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,enrichmentAuditBefore+6,
   "only six immutable publishes write audit; replay, read and denials never write");
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_records")[0].n,partyCountBefore);
 console.log(JSON.stringify({event:"crm.native-enrichment-v2",result:"PASS",
   original_owner:"crm.customer-enrichment",immutable:true,tenant_ab:true,replay:true,
   page_bounds:true,d1_audit:true,timestamp_precision:true,provider_execution:"NOT_CONNECTED"}));


 // Account-rooted composition uses the original owner values and never writes.
 const overviewCall=signedReadOnly(overviewV2Path,binaryOverviewV2,decodeOverviewV2);
 const overviewAudit=sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n;
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,overviewAudit);
 // Sales-created tasks retain their original Deal reference, and their D1 Account.
 const linkedTaskId=sql("SELECT task_id FROM crm_sales_link_delivery_records WHERE tenant='tenant-a' AND deal_id='"+
   dealId+"' AND task_id IS NOT NULL ORDER BY task_id LIMIT 1")[0]?.task_id;
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
 const boundedAudit=sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n;
 const boundedCard=(await overviewCall("GetAccountOverview",viewer,{accountId})).body.overview;
 assert.equal(boundedCard.contactPoints.length,100);
 assert.equal(boundedCard.truncated.contactPoints,true);
 assert.equal(boundedCard.truncated.tasks,false);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,boundedAudit);
 console.log(JSON.stringify({event:"crm.native-account-overview-v2",result:"PASS",
   owner:"crm.customer360",account_rooted:true,original_owners:true,
   tenant_ab:true,read_only:true,bounded:true}));
 // CSV operations use the original parser/Party owner, explicit admin permissions,
 // atomic D1 writes and a per-operation receipt in that same database.
 const csv=signedReadOnly(csvV2Path,binaryCsvV2,decodeCsvV2);
 const csvBytes=new TextEncoder().encode(
   'party_id,kind,display_name,resource_version\r\nexternal-one,person,CSV One,1\r\nexternal-two,organization,CSV Two,1\r\n');
 const importAuditBefore=sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n;
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,importAuditBefore,"previews and invalid input are read-only");
 const applied=await csv("ApplyPartyImport",admin,applyInput);
 assert.equal(applied.status,200,describeProtobuf(applied.body));
 assert.equal(applied.body.createdCount,2);assert.equal(applied.body.replayedCount,0);
 assert.equal(applied.body.receipt.replayed,false);
 assert.equal(applied.body.receipt.requestId,applied.headers.get("x-request-id"));
 assert.equal(applied.body.receipt.resourceVersion,1n);
 const importReplay=await csv("ApplyPartyImport",admin,applyInput);
 assert.equal(importReplay.body.createdCount,0);assert.equal(importReplay.body.replayedCount,2);
 assert.equal(importReplay.body.receipt.replayed,true);
 assert.equal((await csv("ApplyPartyImport",admin,{...applyInput,
   precondition:{idempotencyKey:"csv-import-another-operation"}})).body.replayedCount,2);
 assert.equal((await csv("PreviewPartyImport",admin,{csv:csvBytes})).body.previouslyImported,2);
 for(const target of importPreview.body.targetPartyIds){
   assert.equal((await v3("GetParty",viewer,{partyId:target})).status,200);
   assert.equal((await v3("GetParty",tenantB,{partyId:target})).body.code,"not_found");
 }
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,importAuditBefore+2,"replays never duplicate Party audit");
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE tenant='tenant-a'")[0].n,2);
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_party_import_receipts WHERE key='csv-conflict-operation'")[0].n,0);
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,importAuditBefore+3);
 assert.equal((await csv("PreviewPartyExport",admin,{kind:0})).body.code,"invalid_argument");
 assert.equal((await csv("PreviewPartyExport",admin,{kind:999})).body.code,"invalid_argument");
 const exportBefore=sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n;
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
 assert.equal(sql("SELECT COUNT(*) AS n FROM crm_audit")[0].n,exportBefore);
 // Explicit boundary fixture in tenant B: the 101st row must prevent partial export,
 // even when a filter would reduce the returned data below the limit.
 const boundaryRows=Array.from({length:101},(_,i)=>{
   const state=JSON.stringify({party_id:"csv-boundary-"+i,kind:"person",display_name:"Boundary "+i,
     created_at_unix_nanos:10,updated_at_unix_nanos:10,version:1});
   return "('tenant-b','csv-boundary-"+i+"','"+state+"',1,'csv-boundary-key-"+i+"','fixture','fixture')";
 }).join(",");
 sql("INSERT INTO crm_party_records(tenant,id,state,version,idem_key,idem_digest,request_id) VALUES "+boundaryRows);
 assert.equal((await csv("PreviewPartyExport",tenantB,{kind:2})).body.code,"failed_precondition");
 assert.equal((await rpcFetch("/api/customer-data-operations/parties.csv?kind=organization",{
   headers:{"cf-access-jwt-assertion":tenantB}})).status,409);
 console.log(JSON.stringify({event:"crm.native-party-csv-v2",result:"PASS",admin_read:true,
   original_owner:"crm.customer-data-operations",party_owner:"crm.parties",
   tenant_ab:true,atomic:true,replay:true,audit:true,csv_bytes_sha256:true,bounded:true}));
 await typedBrowser({base,admin,viewer,tenantB,sql,sourceSha});
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
   scenarios:"existing signed workerd assertions completed"
 },null,2)+"\n");
 console.log(JSON.stringify({event:"crm.signed-connect-acceptance",
   result:"PASS",admin:true,viewer:true,unauthenticated_denied:true,
   legacy_mutations:0,verified_auth:true}));
}finally{
 if(child){
   try{process.kill(-child.pid,"SIGTERM");}catch(e){if(e.code!=="ESRCH")throw e;}
   if(child.exitCode===null)await Promise.race([new Promise(resolve=>child.once("exit",resolve)),
     new Promise(resolve=>setTimeout(resolve,1400))]);
   try{process.kill(-child.pid,"SIGKILL");}catch(e){if(e.code!=="ESRCH")throw e;}
   child.stderr.destroy();child.unref();
 }
 rmSync(data,{recursive:true,force:true});
}

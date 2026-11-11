// Isolated native-ci RSA-signed Access proof of intentional contract-only shutdown.
import {spawn,execFileSync} from "node:child_process";
import {generateKeyPairSync,createSign,randomUUID} from "node:crypto";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import assert from "node:assert/strict";
import {binaryPartyGet,decodePartyGet,partyGetPath,
  binaryPartyV3,decodePartyV3,partyV3Path,
  binaryAccountV2,decodeAccountV2,accountV2Path,
  binaryContactV2,decodeContactV2,contactV2Path,
  binaryRelationshipV2,decodeRelationshipV2,relationshipV2Path} from "../../../../scripts/crm-connect-binary-fixture.mjs";
const cwd=dirname(dirname(fileURLToPath(import.meta.url)));
const data=mkdtempSync(join(tmpdir(),"crm-native-contract-only-"));
const base="http://127.0.0.1:18789";
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
 run(["d1","execute","crm-rust-party-local","--local",
   "--file","../migrations/0001_initial.sql","--persist-to",data]);
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
 child=spawn("npx",["wrangler","dev","--local","--port","18789","--persist-to",data,
  "--var","CRM_CI_LOCAL_ONLY:yes","--var","CRM_NATIVE_CI_JWKS:"+jwks],
  {cwd,detached:true,stdio:["ignore","ignore","pipe"]});
 let stderr="";
 child.stderr.on("data",b=>stderr=(stderr+b.toString()).slice(-2500));
 let ready=false;
 for(let i=0;i<120;i++){
   if(child.exitCode!==null)throw Error("native-ci exited "+stderr);
   try{if((await fetch(base+"/api/parties")).status===401){ready=true;break;}}catch{}
   await new Promise(resolve=>setTimeout(resolve,250));
 }
 assert(ready,"native-ci not ready: "+stderr);
 async function call(path,token,method="GET"){
   const response=await fetch(base+path,{method,
     headers:{...(token?{"cf-access-jwt-assertion":token}:{}),"content-type":"application/json"},
     ...(method==="POST"?{body:"{}"}:{})});
   return {status:response.status,body:await response.json()};
 }
 assert.equal((await call("/api/parties",null)).status,401);
 const admin=jwt("native-a@example.test"),viewer=jwt("native-viewer@example.test");
 for(const [path,method] of [["/api/parties","GET"],["/api/parties","POST"],
     ["/api/accounts","GET"],["/crm.sales.v1.DealService/GetDeal","POST"],
     ["/crm.future.v9.FutureService/NotRegistered","POST"]]){
   const r=await call(path,admin,method);
   assert.equal(r.status,410,path);
   assert.equal(r.body.code,"LEGACY_TRANSPORT_DISABLED");
 }
 assert.equal((await call("/api/parties",viewer)).status,410);
 // Exercise the *actual* generated binary request/response, signed JWT,
 // tenant A/B read isolation, and read-only D1 persistence in workerd.
 assert.equal(partyGetPath,"/crm.parties.v2.PartyService/GetParty");
 const binaryCall=async(token,id)=>{
   const r=await fetch(base+partyGetPath,{method:"POST",headers:{
     "cf-access-jwt-assertion":token,"content-type":"application/proto",
     "connect-protocol-version":"1"},body:binaryPartyGet(id)});
   return r;
 };
 for(const token of [admin,viewer]){
   const r=await binaryCall(token,"binary-rpc-proof-01");
   assert.equal(r.status,200,"valid signed tenant A must read via binary Connect");
   assert.match(r.headers.get("x-request-id")||"",/^[0-9a-f-]{36}$/i);
   const item=decodePartyGet(new Uint8Array(await r.arrayBuffer())).party;
   assert.equal(item?.partyId,"binary-rpc-proof-01");
   assert.equal(item?.displayName,"Original Rust Party");
   assert.equal(item?.version,1n);
 }
 const tenantB=jwt("native-b@example.test");
 const denied=await binaryCall(tenantB,"binary-rpc-proof-01");
 assert.equal(denied.status,404,"signed tenant B must never read tenant A");
 assert.equal((await denied.json()).code,"not_found");
 const malformed=await binaryCall(admin,"");
 assert.equal(malformed.status,400,"invalid binary RPC must be rejected by Rust");
 assert.equal((await malformed.json()).code,"invalid_argument");
 const db=JSON.parse(run(["d1","execute","crm-rust-party-local","--local",
   "--persist-to",data,"--json","--command",
   "SELECT COUNT(*) AS n FROM crm_party_records;"]));
 assert.equal(db[0]?.results?.[0]?.n,1,"read-only RPC must not mutate original D1");

 // G3 candidate, active only in signed native-ci workerd. Every request and
 // response is generated from the SAME Buf descriptor as the React client.
 // The production contract-only Worker must continue returning 410 for v3.
 const describeProtobuf=value=>JSON.stringify(value,
   (_key,item)=>typeof item==="bigint"?item.toString():item);
 const v3=async(method,token,payload)=>{
   const response=await fetch(base+partyV3Path(method),{method:"POST",
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
   const response=await fetch(base+accountV2Path(method),{method:"POST",
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
 console.log(JSON.stringify({event:"crm.native-g3-account-v2-candidate",result:"PASS",
   generated_binary:true,owner:"crm.customer-accounts",
   rpc_methods:["CreateAccount","UpdateAccount","GetAccount","ListAccounts"],
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   d1_cas:true,d1_receipts:true,d1_audit:true,published_active:false}));


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
   const response=await fetch(base+contactV2Path(method),{method:"POST",
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

 console.log(JSON.stringify({event:"crm.native-g3-contact-points-v2-candidate",result:"PASS",
   generated_binary:true,owner:"crm.contact-points",
   rpc_methods:["CreateContactPoint","UpdateContactPoint","GetContactPoint","ListContactPoints"],
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   no_browser_verification:true,d1_cas:true,d1_receipts:true,d1_audit:true,
   published_active:false}));

 // G3 Relationships v2: original Rust domain owner and one D1, binary
 // Protobuf descriptor, signed JWT + tenant A/B + viewer rights.
 const relationV2=async(method,token,payload)=>{
   const response=await fetch(base+relationshipV2Path(method),{method:"POST",
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
 console.log(JSON.stringify({event:"crm.native-g3-relationship-v2-candidate",
   result:"PASS",rpc_methods:["CreatePartyRelationship","UpdatePartyRelationship",
     "GetPartyRelationship","ListPartyRelationships"],
   generated_binary:true,original_owner:"crm.party-relationships",
   signed_access:true,tenant_ab:true,viewer_write_denied:true,
   filtered_pagination:true,temporal_patch:true,replay_after_interleaving:true,
   no_extra_audit:true,published_active:false}));

 console.log(JSON.stringify({event:"crm.native-g3-party-v3-candidate",
   result:"PASS",generated_binary:true,owner:"crm.parties",
   rpc_methods:["CreateParty","UpdateParty","GetParty","ListParties"],
   signed_access:true,tenant_ab:true,viewer_mutation_denied:true,
   original_d1_cas:true,d1_receipts:true,d1_audit:true,
   published_active:false}));

 assert.equal((await call("/crm.future.v9.FutureService/NotRegistered",viewer,"POST")).status,410);
 console.log(JSON.stringify({event:"crm.contract-only-signed-access",
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

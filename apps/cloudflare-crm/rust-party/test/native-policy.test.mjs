import assert from "node:assert/strict";
import {test} from "node:test";
import {writeFileSync} from "node:fs";
import {withNativeHarness} from "./native.mjs";

test("compiled policy and unknown routes on isolated D1",async context=>{
 const evidence=await withNativeHarness(context,async({sourceSha,partySnapshot,rpc,rpcFetch,viewer,admin,call})=>{
 const beforeDenials=await partySnapshot();
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
 assert.deepEqual(await partySnapshot(),beforeDenials,"denied routes must not mutate the fixture");
 return {source_sha:sourceSha,result:"PASS",features:["native-ci"],rpc,
   unauthenticated_denials:unauthenticatedDenials.sort(),viewer_denials:viewerDenials.sort()};
 });
 writeFileSync(process.env.CRM_RPC_RUNTIME_PROOF+".policy",JSON.stringify(evidence,null,2)+"\n");
});

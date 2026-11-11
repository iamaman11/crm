// The final API surface comes from the compiled Buf FileDescriptorSet.
// No hand-maintained method count, feature catalog, implementation flag or
// invented RPC is allowed to count as executable evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createRequire} from "node:module";
const clientRequire=createRequire(new URL("../packages/client/package.json",import.meta.url));
const {fromBinary}=clientRequire("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=clientRequire("@bufbuild/protobuf/wkt");
const descriptor=process.env.CRM_PROTO_DESCRIPTOR_FILE;
if(!descriptor)throw Error("compiled Buf FileDescriptorSet required");
const compiled=fromBinary(FileDescriptorSetSchema,readFileSync(descriptor));
const finalFiles=compiled.file.filter(f=>
 /^crm\/[^/]+\/v(?:2|3)\/[^/]+\.proto$/.test(f.name) &&
 f.package!=="crm.parties.v2" && f.service.length);
if(!finalFiles.length)throw Error("no versioned cutover services discovered");
const allServices=finalFiles.flatMap(f=>f.service.map(service=>({file:f,service})));
const fullName=(pkg,name)=>"."+pkg+"."+name;
const findMessage=(file,name)=>{
 const found=file.messageType.find(message=>fullName(file.package,message.name)===name);
 assert.ok(found,"missing canonical typed "+name);
 return found;
};
const namedField=(message,name)=>{
 const found=message.field.find(f=>f.name===name);
 assert.ok(found,"missing "+message.name+"."+name);
 return found;
};
const isMutation=name=>/^(?:Create|Update|Advance|Complete|Suppress|Publish|Submit|Cancel|Apply)/.test(name);
const isList=name=>/^(?:List|FindShared)/.test(name);
test("all future Connect RPCs have canonical generated request/response types",()=>{
 for(const {file,service} of allServices){
  assert.match(file.package,/^crm\.[a-z0-9_]+\.(v2|v3)$/);
  assert.ok(service.name.endsWith("Service"),service.name);
  for(const method of service.method){
   assert.equal(method.inputType,fullName(file.package,method.name+"Request"));
   assert.equal(method.outputType,fullName(file.package,method.name+"Response"));
   const req=findMessage(file,method.inputType);
   const res=findMessage(file,method.outputType);
   const forbidden=req.field.filter(x=>/^(tenant_id|actor_id|role_id|is_admin|allow_skip|allow_regression)$/.test(x.name));
   assert.deepEqual(forbidden.map(x=>x.name),[],file.name+" "+method.name+" trusts client identity/policy");
   for(const field of [...req.field,...res.field])
    assert.ok(field.type!==1 && field.type!==2,"floating-point wire field "+file.name+" "+field.name);
  }
 }
});
test("all actual mutating RPC contracts require CAS/idempotency and receipts",()=>{
 let total=0;
 for(const {file,service} of allServices)for(const method of service.method){
  if(!isMutation(method.name))continue;
  total++;
  const request=findMessage(file,method.inputType);
  const response=findMessage(file,method.outputType);
  assert.equal(namedField(request,"precondition").typeName,
   ".crm.core.v2.MutationPrecondition",method.name);
  assert.equal(namedField(response,"receipt").typeName,
   ".crm.core.v2.MutationReceipt",method.name);
 }
 assert.ok(total>0,"no mutation proof");
});
test("all list RPCs require bounded page request and explicit continuation",()=>{
 let total=0;
 for(const {file,service} of allServices)for(const method of service.method){
  if(!isList(method.name))continue;
  total++;
  const request=findMessage(file,method.inputType);
  const response=findMessage(file,method.outputType);
  assert.equal(namedField(request,"page").typeName,".crm.core.v2.PageRequest");
  assert.equal(namedField(response,"page_info").typeName,".crm.core.v2.PageInfo");
 }
 assert.ok(total>0,"no paginated list proof");
});
test("future domain enums reject accidental default business values",()=>{
 let total=0;
 for(const file of finalFiles)for(const value of file.enumType){
  total++;
  assert.ok(value.value.length>0,file.name+" empty enum");
  assert.equal(value.value[0].number,0,file.name+" first enum number");
  assert.ok(value.value[0].name.endsWith("_UNSPECIFIED"),
   file.name+" default must be UNSPECIFIED");
 }
 assert.ok(total>0);
});
test("no fake grant, merge, provider dispatch or privacy execution appears in active proposals",()=>{
 const methods=allServices.flatMap(({file,service})=>
  service.method.map(method=>file.package+"."+service.name+"."+method.name));
 for(const forbidden of [/GrantConsent/i,/MergeParty/i,/VerifyPrivacyCaseSubject/i,
  /ExecuteErasure/i,/DispatchEnrichmentRequest/i,/CreateSuggestion/i])
  assert.equal(methods.filter(x=>forbidden.test(x)).length,0,forbidden.toString());
});

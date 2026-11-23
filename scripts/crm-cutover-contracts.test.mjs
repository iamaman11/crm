// The final API surface comes from the compiled Buf FileDescriptorSet.
// No hand-maintained method count, feature catalog, implementation flag or
// invented RPC is allowed to count as executable evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createRequire} from "node:module";
const clientRequire=createRequire(new URL("../packages/client/package.json",import.meta.url));
const {fromBinary,toBinary,create,createFileRegistry,ScalarType}=clientRequire("@bufbuild/protobuf");
import {descriptorMethods,targetMethods} from "./crm-rpc-descriptors.mjs";
const {FileDescriptorSetSchema,MethodOptions_IdempotencyLevel}=clientRequire("@bufbuild/protobuf/wkt");
const descriptor=process.env.CRM_PROTO_DESCRIPTOR_FILE;
if(!descriptor)throw Error("compiled Buf FileDescriptorSet required");
const compiled=fromBinary(FileDescriptorSetSchema,readFileSync(descriptor));
// Reuse the compiled-descriptor scope from the executed router/policy gate.
function activeFiles(files) {
 const methods=descriptorMethods(toBinary(FileDescriptorSetSchema,create(FileDescriptorSetSchema,{file:files})));
 const required=new Set(targetMethods(methods));
 return files.filter(f=>f.service.some(service=>service.method.some(method=>
   required.has("/"+f.package+"."+service.name+"/"+method.name))));
}
const finalFiles=activeFiles(compiled.file);
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
const {NO_SIDE_EFFECTS,IDEMPOTENT}=MethodOptions_IdempotencyLevel;
function assertWireContracts(files, descriptors) {
 const registry=createFileRegistry(create(FileDescriptorSetSchema,{file:descriptors}));
 // The same message may be both input and output. Only inputs forbid identity fields.
 const seenInputs=new Set(),seenOutputs=new Set();
 function visit(message,isInput){
  const seen=isInput?seenInputs:seenOutputs;
  if(seen.has(message.typeName))return;
  seen.add(message.typeName);
  for(const field of message.fields){
   const name=message.typeName+"."+field.name;
   if(isInput)assert.doesNotMatch(field.name,/^(tenant_id|actor_id|role_id|is_admin|allow_skip|allow_regression)$/,
    "request trusts client identity/policy: "+name);
   assert.ok(field.scalar!==ScalarType.DOUBLE && field.scalar!==ScalarType.FLOAT,
    "floating-point wire field "+name);
   // Protobuf-ES resolves imported types, list elements and map values here.
   if(field.message)visit(field.message,isInput);
  }
 }
 for(const file of files){
  assert.match(file.package,/^crm\.[a-z0-9_]+\.v(?:[2-9]|[1-9][0-9]+)$/);
  for(const service of file.service){
   const compiledService=registry.getService(file.package+"."+service.name);
   assert.ok(compiledService,"missing compiled service "+service.name);
   for(const method of compiledService.methods){
    visit(method.input,true);
    visit(method.output,false);
   }
  }
 }
}
test("active CRM wire contracts reject client identity fields and floating-point values",()=>{
 assertWireContracts(finalFiles,compiled.file);
});
test("an imported nested message must obey request and response field rules",()=>{
 for(const [message,name,type,expected] of [
  ["MutationPrecondition","actor_id",9,/request trusts client identity.*MutationPrecondition.actor_id/],
  ["MutationReceipt","inexact",2,/floating-point wire field .*MutationReceipt.inexact/],
 ]){
  const files=structuredClone(compiled.file);
  const core=files.find(file=>file.package==="crm.core.v2");
  core.messageType.find(item=>item.name===message).field.push({
   name,number:1000,type,label:1
  });
  assert.throws(()=>assertWireContracts(finalFiles,files),expected);
 }
});
test("every RPC declares side-effect semantics using standard Protobuf options",()=>{
 for(const {file,service} of allServices)for(const method of service.method){
  const level=method.options?.idempotencyLevel;
  assert.ok(level===NO_SIDE_EFFECTS||level===IDEMPOTENT,"explicit semantics required: "+method.name);
  if(level===NO_SIDE_EFFECTS){
   assert.ok(!findMessage(file,method.inputType).field.some(f=>f.typeName===".crm.core.v2.MutationPrecondition"),
     "read-only RPC must not require mutation precondition: "+method.name);
   assert.ok(!findMessage(file,method.outputType).field.some(f=>f.typeName===".crm.core.v2.MutationReceipt"),
     "read-only RPC must not return a mutation receipt: "+method.name);
  }
 }
});
function assertMutationContracts(files) {
 let total=0;
 for(const file of files)for(const service of file.service)for(const method of service.method){
  if(method.options?.idempotencyLevel!==IDEMPOTENT)continue;
  total++;
  const request=findMessage(file,method.inputType);
  const response=findMessage(file,method.outputType);
  assert.equal(namedField(request,"precondition").typeName,
   ".crm.core.v2.MutationPrecondition",method.name);
  assert.equal(namedField(response,"receipt").typeName,
   ".crm.core.v2.MutationReceipt",method.name);
 }
 assert.ok(total>0,"no mutation proof");
}
test("all actual mutating RPC contracts require CAS/idempotency and receipts",()=>assertMutationContracts(finalFiles));
test("paginated RPCs pair typed bounds with explicit continuation",()=>{
 let total=0;
 for(const {file,service} of allServices)for(const method of service.method){
  const request=findMessage(file,method.inputType);
  const response=findMessage(file,method.outputType);
  const paginated=request.field.some(f=>f.typeName===".crm.core.v2.PageRequest");
  const continuation=response.field.some(f=>f.typeName===".crm.core.v2.PageInfo");
  assert.equal(paginated,continuation,"page request/continuation must be paired: "+method.name);
  if(!paginated)continue;
  total++;
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
// Names cannot prove that a function is fake. Runtime coverage separately requires
// exact manifest ownership, compiled registration/policy and successful execution.
test("future v4 and v10 use the same active scope and reject invalid contracts",()=>{
 const task=compiled.file.find(f=>f.name==="crm/activities/v2/task.proto");
 assert.ok(task,"current Task descriptor required");
 for(const version of [4,10]){
  const next=structuredClone(task);
  next.name="crm/activities/v"+version+"/task.proto";
  next.package="crm.activities.v"+version;
  for(const message of next.messageType)for(const field of message.field){
   if(field.typeName?.startsWith(".crm.activities.v2."))
    field.typeName=field.typeName.replace(".crm.activities.v2.", "."+next.package+".");
  }
  for(const service of next.service)for(const method of service.method){
   method.inputType=method.inputType.replace(".crm.activities.v2.",".crm.activities.v"+version+".");
   method.outputType=method.outputType.replace(".crm.activities.v2.",".crm.activities.v"+version+".");
  }
  const selected=activeFiles([...compiled.file,next]);
  assert.ok(selected.some(f=>f.name===next.name),"future version omitted: v"+version);
  assertWireContracts([next],[...compiled.file,next]);
  assertMutationContracts([next]);
  const malformed=structuredClone(next);
  const message=malformed.messageType.find(m=>m.name==="CreateTaskRequest");
  assert.ok(message);
  message.field=message.field.filter(f=>f.name!=="precondition");
  assert.throws(()=>assertMutationContracts([malformed]),/missing CreateTaskRequest.precondition/);
  const unsafe=structuredClone(next);
  unsafe.messageType.find(m=>m.name==="CreateTaskRequest").field.push({
   name:"unsafe_float",number:1000,type:1,label:1,jsonName:"unsafeFloat"
  });
  assert.throws(()=>assertWireContracts([unsafe],[...compiled.file,unsafe]),/floating-point wire field/);
 }
});

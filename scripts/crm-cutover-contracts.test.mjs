// The final API surface comes from the compiled Buf FileDescriptorSet.
// No hand-maintained method count, feature catalog, implementation flag or
// invented RPC is allowed to count as executable evidence.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createRequire} from "node:module";
const clientRequire=createRequire(new URL("../packages/client/package.json",import.meta.url));
const {fromBinary,toBinary}=clientRequire("@bufbuild/protobuf");
import {descriptorMethods,targetMethods} from "./crm-rpc-descriptors.mjs";
const {FileDescriptorSetSchema,MethodOptions_IdempotencyLevel}=clientRequire("@bufbuild/protobuf/wkt");
const descriptor=process.env.CRM_PROTO_DESCRIPTOR_FILE;
if(!descriptor)throw Error("compiled Buf FileDescriptorSet required");
const compiled=fromBinary(FileDescriptorSetSchema,readFileSync(descriptor));
// Reuse the compiled-descriptor scope from the executed router/policy gate.
function activeFiles(files) {
 const methods=descriptorMethods(toBinary(FileDescriptorSetSchema,{file:files}));
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
function assertWireContracts(files) {
 for(const file of files)for(const service of file.service){
  assert.match(file.package,/^crm\.[a-z0-9_]+\.v(?:[2-9]|[1-9][0-9]+)$/);
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
}
test("all future Connect RPCs have canonical generated request/response types",()=>assertWireContracts(finalFiles));
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
  for(const service of next.service)for(const method of service.method){
   method.inputType=method.inputType.replace(".crm.activities.v2.",".crm.activities.v"+version+".");
   method.outputType=method.outputType.replace(".crm.activities.v2.",".crm.activities.v"+version+".");
  }
  const selected=activeFiles([...compiled.file,next]);
  assert.ok(selected.some(f=>f.name===next.name),"future version omitted: v"+version);
  assertWireContracts([next]);
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
  assert.throws(()=>assertWireContracts([unsafe]),/floating-point wire field/);
 }
});

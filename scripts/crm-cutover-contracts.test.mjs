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
const {FileDescriptorSetSchema,AnySchema,FieldDescriptorProto_Label,FieldDescriptorProto_Type,MethodOptions_IdempotencyLevel}=clientRequire("@bufbuild/protobuf/wkt");
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
const namedField=(message,name)=>{
 const found=message.field.find(f=>f.name===name);
 assert.ok(found,"missing "+message.name+"."+name);
 return found;
};
const {NO_SIDE_EFFECTS,IDEMPOTENT}=MethodOptions_IdempotencyLevel;
// Resolve method fields through Protobuf-ES, including messages declared in imported files.
function canonicalField(message, name, type, required = false) {
 const typeName = "crm.core.v2." + type;
 const path = message.typeName + "." + name;
 const matches = message.fields.filter(field => field.name === name || field.message?.typeName === typeName);
 if (!matches.length) {
  assert.ok(!required, "missing " + path);
  return false;
 }
 assert.equal(matches.length, 1, "ambiguous " + path);
 const [field] = matches;
 assert.equal(field.name, name, "canonical name required for " + path);
 assert.equal(field.fieldKind, "message", "singular message required for " + path);
 assert.equal(field.message.typeName, typeName, "incorrect type for " + path);
 assert.ok(!field.oneof, "independent field required for " + path);
 return true;
}
function assertMethodContract(method) {
 const level = method.proto.options?.idempotencyLevel;
 assert.ok(level === NO_SIDE_EFFECTS || level === IDEMPOTENT, "explicit semantics required: " + method.name);
 const writes = level === IDEMPOTENT;
 assert.equal(canonicalField(method.input, "precondition", "MutationPrecondition", writes), writes,
  "read-only request has a mutation precondition: " + method.name);
 assert.equal(canonicalField(method.output, "receipt", "MutationReceipt", writes), writes,
  "read-only response has a mutation receipt: " + method.name);
 const paginated = canonicalField(method.input, "page", "PageRequest");
 assert.equal(canonicalField(method.output, "page_info", "PageInfo"), paginated,
  "page request/continuation must be paired: " + method.name);
 return {writes, paginated};
}
function assertWireContracts(files, descriptors) {
 const registry=createFileRegistry(create(FileDescriptorSetSchema,{file:descriptors}));
 // The same message may be both input and output. Only inputs forbid identity fields.
 const seenInputs=new Set(),seenOutputs=new Set();
 let mutations=0, paginatedMethods=0;
 function visit(message,isInput){
  const seen=isInput?seenInputs:seenOutputs;
  if(seen.has(message.typeName))return;
  seen.add(message.typeName);
  assert.ok(!["google.protobuf.Any","google.protobuf.Struct","google.protobuf.Value",
   "google.protobuf.ListValue"].includes(message.typeName),"untyped payload "+message.typeName);
  for(const field of message.fields){
   const name=message.typeName+"."+field.name;
   if(isInput)assert.doesNotMatch(field.name,/^(tenant_id|actor_id|request_id|role_id|is_admin|allow_skip|allow_regression)$/,
    "request trusts client identity/policy: "+name);
   assert.ok(field.scalar!==ScalarType.DOUBLE && field.scalar!==ScalarType.FLOAT,
    "floating-point wire field "+name);
   // Protobuf-ES resolves imported types, list elements and map values here.
   if(field.enum && field.enum.typeName.startsWith("crm.")){
    const first=field.enum.values[0];
    assert.ok(first?.number===0 && first.name.endsWith("_UNSPECIFIED"),
     field.enum.typeName+" default must be UNSPECIFIED");
   }
   if(field.message)visit(field.message,isInput);
  }
 }
 for(const file of files){
  assert.match(file.package,/^crm\.[a-z0-9_]+\.v(?:[2-9]|[1-9][0-9]+)$/);
  for(const service of file.service){
   const compiledService=registry.getService(file.package+"."+service.name);
   assert.ok(compiledService,"missing compiled service "+service.name);
   for(const method of compiledService.methods){
    assert.equal(method.methodKind,"unary",method.name+" must be unary");
    const {writes,paginated}=assertMethodContract(method);
    if(writes)mutations++;
    if(paginated)paginatedMethods++;
    visit(method.input,true);
    visit(method.output,false);
   }
  }
 }
 return {mutations,paginatedMethods};
}
test("compiled active CRM methods enforce typed reads, mutations, pagination and payloads",()=>{
 const {mutations,paginatedMethods}=assertWireContracts(finalFiles,compiled.file);
 assert.ok(mutations>0,"no mutation proof");
 assert.ok(paginatedMethods>0,"no paginated list proof");
});
test("an imported nested message must obey request and response field rules",()=>{
 for(const [message,name,type,expected] of [
  ["MutationPrecondition","request_id",9,/request trusts client identity.*MutationPrecondition.request_id/],
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
test("an imported nested domain enum cannot default to a business decision",()=>{
 const files=structuredClone(compiled.file);
 const core=files.find(f=>f.package==="crm.core.v2");
 const precondition=core.messageType.find(m=>m.name==="MutationPrecondition");
 precondition.enumType.push({name:"Decision",value:[{name:"DECISION_APPROVED",number:0}]});
 precondition.field.push({name:"decision",number:1000,type:FieldDescriptorProto_Type.ENUM,
  label:FieldDescriptorProto_Label.OPTIONAL,typeName:".crm.core.v2.MutationPrecondition.Decision"});
 assert.throws(()=>assertWireContracts(finalFiles,files),/Decision default must be UNSPECIFIED/);
});
test("imported RPC messages obey the same canonical field rules",()=>{
 const files=structuredClone(compiled.file);
 const task=files.find(file=>file.name==="crm/activities/v2/task.proto");
 const messages={...task,name:"crm/activities/v2/task_messages.proto",service:[]};
 task.dependency=[messages.name];
 task.messageType=[];
 task.enumType=[];
 files.splice(files.indexOf(task),0,messages);
 const check=()=>assertWireContracts([task],files);
 check();
 const request=messages.messageType.find(message=>message.name==="ListTasksRequest");
 const response=messages.messageType.find(message=>message.name==="ListTasksResponse");
 const page=namedField(request,"page"), info=namedField(response,"page_info");
 info.label=FieldDescriptorProto_Label.REPEATED;
 assert.throws(check,/singular message required.*ListTasksResponse.page_info/);
 info.label=FieldDescriptorProto_Label.OPTIONAL;
 response.oneofDecl.push({name:"choice"});
 info.oneofIndex=0;
 assert.throws(check,/independent field required.*ListTasksResponse.page_info/);
 delete info.oneofIndex;
 response.oneofDecl.pop();
 for(const field of [page,info]){
  field.type=FieldDescriptorProto_Type.STRING;
  delete field.typeName;
 }
 assert.throws(check,/singular message required.*ListTasksRequest.page/);
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
  const check=file=>assertWireContracts([file],[...compiled.file,file]);
  const malformed=structuredClone(next);
  const message=malformed.messageType.find(m=>m.name==="CreateTaskRequest");
  assert.ok(message);
  message.field=message.field.filter(f=>f.name!=="precondition");
  assert.throws(()=>check(malformed),/missing .*CreateTaskRequest.precondition/);
  const repeated=structuredClone(next);
  namedField(repeated.messageType.find(m=>m.name==="CreateTaskResponse"),"receipt").label=FieldDescriptorProto_Label.REPEATED;
  assert.throws(()=>check(repeated),/singular message required.*CreateTaskResponse.receipt/);
  const duplicate=structuredClone(next);
  const input=duplicate.messageType.find(m=>m.name==="CreateTaskRequest");
  input.field.push({...namedField(input,"precondition"),name:"other",jsonName:"other",number:1000});
  assert.throws(()=>check(duplicate),/ambiguous .*CreateTaskRequest.precondition/);
  const opaque=structuredClone(next);
  opaque.dependency.push(AnySchema.file.proto.name);
  opaque.messageType.find(m=>m.name==="CreateTaskRequest").field.push({
   name:"payload",number:1000,type:FieldDescriptorProto_Type.MESSAGE,
   typeName:".google.protobuf.Any",label:FieldDescriptorProto_Label.OPTIONAL
  });
  assert.throws(()=>assertWireContracts([opaque],[
   ...compiled.file.filter(f=>f.name!==AnySchema.file.proto.name),AnySchema.file.proto,opaque
  ]),/untyped payload google.protobuf.Any/);
  const unsafe=structuredClone(next);
  unsafe.messageType.find(m=>m.name==="CreateTaskRequest").field.push({
   name:"unsafe_float",number:1000,type:1,label:1,jsonName:"unsafeFloat"
  });
  assert.throws(()=>assertWireContracts([unsafe],[...compiled.file,unsafe]),/floating-point wire field/);
 }
});

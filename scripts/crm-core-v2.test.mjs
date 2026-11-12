// Validate the real compiled v2 contract, not the text of a .proto file.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createRequire} from "node:module";
const requireClient=createRequire(new URL("../packages/client/package.json",import.meta.url));
const {fromBinary}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=requireClient("@bufbuild/protobuf/wkt");
const file=process.env.CRM_PROTO_DESCRIPTOR_FILE;
if(!file)throw Error("compiled Buf descriptor required");
const set=fromBinary(FileDescriptorSetSchema,readFileSync(file));
const core=set.file.find(x=>x.package==="crm.core.v2");
if(!core)throw Error("compiled crm.core.v2 missing");
const message=name=>{
 const value=core.messageType.find(x=>x.name===name);
 assert.ok(value,"missing "+name);
 return value;
};
const field=(name,key)=>{
 const value=message(name).field.find(x=>x.name===key);
 assert.ok(value,"missing "+name+"."+key);
 return value;
};
test("exact money is signed int64, never double or legacy string",()=>{
 assert.equal(field("ExactMoney","minor_units").type,3);
 assert.equal(field("ExactMoney","minor_units").number,1);
 assert.equal(field("ExactMoney","currency_code").type,9);
});
test("patch preserves absent / set / clear",()=>{
 for(const name of ["StringPatch","MoneyPatch","TimestampPatch"]){
  assert.equal(message(name).oneofDecl.length,1);
  assert.equal(field(name,"set").oneofIndex,0);
  assert.equal(field(name,"clear").oneofIndex,0);
  assert.equal(field(name,"clear").type,8);
 }
});
test("optimistic version has explicit presence and exact precision",()=>{
 const value=field("MutationPrecondition","expected_version");
 assert.equal(value.type,3);
 assert.equal(value.proto3Optional,true);
 assert.equal(field("MutationPrecondition","idempotency_key").type,9);
});
test("receipt and errors retain server request correlation",()=>{
 assert.equal(field("MutationReceipt","request_id").type,9);
 assert.equal(field("MutationReceipt","resource_version").type,3);
 assert.equal(field("DomainErrorDetail","request_id").type,9);
 assert.equal(field("DomainErrorDetail","field_violations").label,3);
});

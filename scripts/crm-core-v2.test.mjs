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

test("the commit outcome and original request ID have stable generated wire semantics",()=>{
 const original=field("MutationReceipt","original_request_id");
 assert.equal(original.number,4);
 assert.equal(original.type,9);
 assert.equal(original.proto3Optional,true);
 const outcome=field("DomainErrorDetail","commit_outcome");
 assert.equal(outcome.number,4);
 assert.equal(outcome.type,14);
 assert.equal(outcome.typeName,".crm.core.v2.MutationCommitOutcome");
 const enumeration=core.enumType.find(value=>value.name==="MutationCommitOutcome");
 assert.ok(enumeration);
 assert.deepEqual(enumeration.value.map(value=>[value.name,value.number]),[
  ["MUTATION_COMMIT_OUTCOME_UNSPECIFIED",0],
  ["MUTATION_COMMIT_OUTCOME_REJECTED",1],
  ["MUTATION_COMMIT_OUTCOME_UNCONFIRMED",2],
 ]);
});

test("read-only commit evidence never claims another write or replay",()=>{
 assert.deepEqual(message("MutationCommitEvidence").field.map(x=>[x.name,x.number,x.type]),[
  ["request_id",1,9],["resource_version",2,3],["original_request_id",3,9]
 ]);
 const sales=set.file.find(x=>x.package==="crm.sales.v2");
 assert.ok(sales,"compiled Deal service required");
 const result=sales.messageType.find(x=>x.name==="GetDealResponse");
 assert.equal(result?.field.find(x=>x.name==="confirmed_mutation")?.typeName,
  ".crm.core.v2.MutationCommitEvidence");
});

test("currency reference precision has explicit zero presence",()=>{
 const code=field("CurrencyDefinition","currency_code");
 assert.equal(code.type,9);
 const exponent=field("CurrencyDefinition","minor_unit_exponent");
 assert.equal(exponent.type,13);
 assert.equal(exponent.proto3Optional,true);
 const sales=set.file.find(x=>x.package==="crm.sales.v2");
 const currencies=sales?.messageType.find(x=>x.name==="ListPipelinesResponse")?.field.find(x=>x.name==="currencies");
 assert.equal(currencies?.typeName,".crm.core.v2.CurrencyDefinition");
 assert.equal(currencies?.label,3);
});

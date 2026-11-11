import {describe,expect,it} from "vitest";
import {create,fromBinary,fromJsonString,toBinary,toJsonString} from "@bufbuild/protobuf";
import {
  ExactMoneySchema,MutationPreconditionSchema,StringPatchSchema,
} from "../gen/crm/core/v2/operation_pb";

// This is the ONLY proposed v2 numeric money representation; keep v1
// string amounts untouched for historical consumers until full cutover.
describe("Cloudflare CRM v2 Protobuf numeric/patch conformance",()=>{
 it("preserves signed 64-bit amounts beyond JS Number precision in binary and JSON",()=>{
  const values=[
   -(1n<<63n),-9007199254740993n,-1n,0n,
   9007199254740993n,(1n<<63n)-1n,
  ];
  for(const amount of values){
   const message=create(ExactMoneySchema,{minorUnits:amount,currencyCode:"USD"});
   const fromWire=fromBinary(ExactMoneySchema,toBinary(ExactMoneySchema,message));
   expect(fromWire.minorUnits).toBe(amount);
   expect(typeof fromWire.minorUnits).toBe("bigint");
   const json=toJsonString(ExactMoneySchema,message);
   const fromJson=fromJsonString(ExactMoneySchema,json);
   expect(fromJson.minorUnits).toBe(amount);
   if(amount!==0n)expect(JSON.parse(json).minorUnits).toBe(amount.toString());
  }
 });
 it("does not collapse absent, clear and set-empty into one mutation",()=>{
  const absent=create(StringPatchSchema,{});
  const setEmpty=create(StringPatchSchema,{operation:{case:"set",value:""}});
  const clear=create(StringPatchSchema,{operation:{case:"clear",value:true}});
  const decodedAbsent=fromBinary(StringPatchSchema,toBinary(StringPatchSchema,absent));
  const decodedSet=fromBinary(StringPatchSchema,toBinary(StringPatchSchema,setEmpty));
  const decodedClear=fromBinary(StringPatchSchema,toBinary(StringPatchSchema,clear));
  expect(decodedAbsent.operation.case).toBeUndefined();
  expect(decodedSet.operation).toEqual({case:"set",value:""});
  expect(decodedClear.operation).toEqual({case:"clear",value:true});
 });
 it("keeps explicit expected_version=0 distinct from an absent version",()=>{
  const absent=create(MutationPreconditionSchema,{idempotencyKey:"proof-key"});
  const zero=create(MutationPreconditionSchema,{expectedVersion:0n,idempotencyKey:"proof-key"});
  expect(fromBinary(MutationPreconditionSchema,toBinary(MutationPreconditionSchema,absent)).expectedVersion)
   .toBeUndefined();
  expect(fromBinary(MutationPreconditionSchema,toBinary(MutationPreconditionSchema,zero)).expectedVersion)
   .toBe(0n);
 });
});

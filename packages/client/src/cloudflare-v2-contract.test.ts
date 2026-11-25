import {describe,expect,it} from "vitest";
import {Code,ConnectError} from "@connectrpc/connect";
import {decodeCrmError} from "./cloudflare";
import {create,fromBinary,fromJsonString,toBinary,toJsonString} from "@bufbuild/protobuf";
import {
  ExactMoneySchema,MutationPreconditionSchema,StringPatchSchema,
  MutationReceiptSchema,DomainErrorDetailSchema,MutationCommitOutcome,
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

describe("S1 generated mutation outcomes and correlation",()=>{
 const meta={"x-request-id":"attempt-id","x-crm-runtime":"native-workers-rs"};
 it("keeps the original commit ID distinct from the current request, including legacy absence",()=>{
   const roundtrip=(input:{requestId:string;resourceVersion:bigint;replayed:boolean;originalRequestId?:string})=>
     fromBinary(MutationReceiptSchema,toBinary(MutationReceiptSchema,create(MutationReceiptSchema,input)));
   expect(roundtrip({requestId:"retry",resourceVersion:2n,replayed:true,
     originalRequestId:"commit"})).toMatchObject({
       requestId:"retry",originalRequestId:"commit",resourceVersion:2n,replayed:true});
   expect(roundtrip({requestId:"legacy",resourceVersion:2n,replayed:true}).originalRequestId).toBeUndefined();
 });
 it("does not turn an unconfirmed mutation into a version conflict",()=>{
   const detail=create(DomainErrorDetailSchema,{
     domainCode:"SALES_COMMIT_UNCONFIRMED",requestId:"attempt-id",
     commitOutcome:MutationCommitOutcome.UNCONFIRMED});
   const error=new ConnectError("write acknowledgement missing",Code.Aborted,meta,[{desc:DomainErrorDetailSchema,value:detail}]);
   expect(decodeCrmError(error)).toMatchObject({outcome:"unconfirmed",domainCode:undefined,
     requestId:"attempt-id"});
 });
 it("includes typed rejection and field paths without parsing message text",()=>{
   const detail=create(DomainErrorDetailSchema,{
     domainCode:"INVALID_DEAL",requestId:"attempt-id",
     commitOutcome:MutationCommitOutcome.REJECTED,
     fieldViolations:[{fieldPath:"amount.currency_code",description:"Unsupported currency"}]});
   const error=new ConnectError("invalid",Code.InvalidArgument,meta,[{desc:DomainErrorDetailSchema,value:detail}]);
   const failure=decodeCrmError(error);
   expect(failure).toMatchObject({outcome:"rejected",domainCode:undefined,requestId:"attempt-id"});
   expect(failure.fieldViolations[0]).toMatchObject({fieldPath:"Валюта",description:"Проверьте значение."});
   const wrongAttempt=new ConnectError("invalid",Code.Aborted,{...meta,"x-request-id":"other"},
     [{desc:DomainErrorDetailSchema,value:detail}]);
   expect(decodeCrmError(wrongAttempt).outcome).toBe("unconfirmed");
   const received=new ConnectError("invalid wire response",Code.InvalidArgument,meta);
   received.details=[{type:DomainErrorDetailSchema.typeName,value:toBinary(DomainErrorDetailSchema,detail)}];
   expect(decodeCrmError(received)).toMatchObject({
     outcome:"rejected",domainCode:undefined,requestId:"attempt-id",
     fieldViolations:[{fieldPath:"Валюта",description:"Проверьте значение."}]
   });
 });
 it("shows a safe request identifier and a localized known domain error",()=>{
   const id="12345678-1234-4234-8234-123456789abc";
   const detail=create(DomainErrorDetailSchema,{domainCode:"SALES_DEAL_VERSION_CONFLICT",requestId:id,
     commitOutcome:MutationCommitOutcome.REJECTED});
   const failure=decodeCrmError(new ConnectError("secret",Code.Aborted,{"x-request-id":id},
     [{desc:DomainErrorDetailSchema,value:detail}]));
   expect(failure.outcome).toBe("rejected");
   expect(failure.domainCode).toBe("SALES_DEAL_VERSION_CONFLICT");
   expect(failure.text).toContain(id);
   expect(failure.text).not.toContain("secret");
 });
 it("localizes the typed Deal name violation without exposing server text",()=>{
   const detail=create(DomainErrorDetailSchema,{
     domainCode:"SALES_DEAL_NAME_INVALID",requestId:"attempt-id",
     commitOutcome:MutationCommitOutcome.REJECTED,
     fieldViolations:[{fieldPath:"deal.name",description:"secret"}]});
   const failure=decodeCrmError(new ConnectError("secret",Code.InvalidArgument,meta,
     [{desc:DomainErrorDetailSchema,value:detail}]));
   expect(failure).toMatchObject({outcome:"rejected",domainCode:"SALES_DEAL_NAME_INVALID",
     fieldViolations:[{fieldPath:"Название сделки",description:"Проверьте значение."}]});
   expect(failure.text).toContain("Введите название сделки.");
   expect(JSON.stringify(failure)).not.toContain("secret");
 });
 it("does not expose untrusted messages or field details",()=>{
   const detail=create(DomainErrorDetailSchema,{domainCode:"PRIVATE_VALUE",requestId:"attempt-id",
     commitOutcome:MutationCommitOutcome.REJECTED,
     fieldViolations:[{fieldPath:"private value",description:"secret"},
       {fieldPath:"amount.currency_code",description:"secret"}]});
   const failure=decodeCrmError(new ConnectError("secret",Code.InvalidArgument,meta,
     [{desc:DomainErrorDetailSchema,value:detail}]));
   expect(JSON.stringify(failure)).not.toContain("secret");
   expect(JSON.stringify(failure)).not.toContain("PRIVATE_VALUE");
   expect(failure.fieldViolations).toHaveLength(1);
   expect(decodeCrmError(new Error("secret")).text).not.toContain("secret");
   const mismatch=decodeCrmError(new ConnectError("secret",Code.InvalidArgument,{"x-request-id":"other"},
     [{desc:DomainErrorDetailSchema,value:detail}]));
   expect(mismatch.fieldViolations).toEqual([]);
 });
 it("treats edge errors, lost responses and internal exceptions conservatively",()=>{
   expect(decodeCrmError(new ConnectError("HTTP 404",Code.NotFound)).outcome).toBe("unconfirmed");
   expect(decodeCrmError(new ConnectError("transport",Code.Unavailable,meta)).outcome).toBe("unconfirmed");
   expect(decodeCrmError(new Error("network")).outcome).toBe("unconfirmed");
   expect(decodeCrmError(new ConnectError("stale",Code.Aborted,meta)).outcome).toBe("unconfirmed");
 });
});

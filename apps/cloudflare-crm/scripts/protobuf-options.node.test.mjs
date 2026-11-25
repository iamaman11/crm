// Isolated feasibility probe, not a new production manifest or registry.
import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from "node:fs";
import {createRequire} from "node:module";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {fileURLToPath} from "node:url";
import test from "node:test";

const root=fileURLToPath(new URL("../../../",import.meta.url));
const requireClient=createRequire(new URL("../../../packages/client/package.json",import.meta.url));
const {fromBinary,createFileRegistry,hasOption,getOption}=requireClient("@bufbuild/protobuf");
const {FileDescriptorSetSchema}=requireClient("@bufbuild/protobuf/wkt");

test("standard Buf descriptors carry typed ownership options without a YAML schema",()=>{
  const dir=mkdtempSync(join(tmpdir(),"crm-protobuf-options-"));
  try{
    const path=join(dir,"pilot","v1","pilot.proto");
    mkdirSync(join(dir,"pilot","v1"),{recursive:true});
    writeFileSync(join(dir,"buf.yaml"),"version: v2\n");
    const source=`syntax = "proto3";

package pilot.v1;

import "google/protobuf/descriptor.proto";

message CapabilityBinding {
  string id = 1;
  string version = 2;
}

extend google.protobuf.ServiceOptions {
  string module_id = 51001;
}

extend google.protobuf.MethodOptions {
  CapabilityBinding capability = 51002;
}

message GetRecordRequest {}
message GetRecordResponse {}
message ListRecordsRequest {}
message ListRecordsResponse {}
message InvalidBindingRequest {}
message InvalidBindingResponse {}

service PilotService {
  option (module_id) = "crm.pilot";

  rpc GetRecord(GetRecordRequest) returns (GetRecordResponse) {
    option (capability) = {id: "pilot.record.get" version: "1.0.0"};
  }

  rpc ListRecords(ListRecordsRequest) returns (ListRecordsResponse) {}
  rpc InvalidBinding(InvalidBindingRequest) returns (InvalidBindingResponse) {
    option (capability) = {};
  }
}
`;
    writeFileSync(path,source);
    const image=join(dir,"pilot.binpb");
    const args=["exec","buf","build",dir,"--as-file-descriptor-set","-o",image];
    execFileSync("pnpm",args,{cwd:root,stdio:"pipe"});
    const registry=createFileRegistry(fromBinary(FileDescriptorSetSchema,readFileSync(image)));
    const service=registry.getService("pilot.v1.PilotService");
    const owner=registry.getExtension("pilot.v1.module_id");
    const capability=registry.getExtension("pilot.v1.capability");
    assert.ok(service&&owner&&capability,"compiled custom option descriptors");
    assert.ok(hasOption(service,owner),"explicit owner presence");
    assert.equal(getOption(service,owner),"crm.pilot");
    function binding(method){
      assert.ok(hasOption(method,capability),"explicit capability option required");
      const value=getOption(method,capability);
      assert.ok(typeof value.id==="string"&&value.id&&typeof value.version==="string"&&value.version,
        "nonempty capability coordinate required");
      return {path:"/"+service.typeName+"/"+method.name,module_id:getOption(service,owner),
        capability_id:value.id,capability_version:value.version,request:method.input.typeName,
        response:method.output.typeName};
    }
    const method=name=>{
      const result=service.methods.find(item=>item.name===name);
      assert.ok(result,"compiled method "+name);
      return result;
    };
    assert.deepEqual(binding(method("GetRecord")),{
      path:"/pilot.v1.PilotService/GetRecord",module_id:"crm.pilot",
      capability_id:"pilot.record.get",capability_version:"1.0.0",
      request:"pilot.v1.GetRecordRequest",response:"pilot.v1.GetRecordResponse"});
    assert.throws(()=>binding(method("ListRecords")),/explicit capability option required/);
    assert.throws(()=>binding(method("InvalidBinding")),/nonempty capability coordinate required/);
    const wrong=source.replace('option (module_id) = "crm.pilot";','option (module_id) = 17;');
    assert.notEqual(wrong,source);
    writeFileSync(path,wrong);
    assert.throws(()=>execFileSync("pnpm",args,{cwd:root,stdio:"pipe"}),
      "Buf compiler must reject a numeric value for a string option");
  }finally{rmSync(dir,{recursive:true,force:true});}
});

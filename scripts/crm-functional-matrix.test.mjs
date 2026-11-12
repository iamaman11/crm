// G0 mutation tests for real UI syntax, not a manually curated operation list.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {inspectUiFile,deriveMatrix} from "./crm-functional-matrix.mjs";

test("AST discovers forms, action buttons and REST method variants",()=>{
 const source='function View({load}){async function save(){return load("sales-deals/"+id,{method:edit?"PATCH":"POST"});}return <form onSubmit={save}><button type="submit">Save</button></form>}';
 const result=inspectUiFile(source,"View.jsx");
 assert.equal(result.requests.length,1);
 assert.deepEqual(result.requests[0].methods,["PATCH","POST"]);
 assert.equal(result.requests[0].static_prefix,"sales-deals/");
 assert.ok(result.controls.some(x=>x.element==="form"));
 assert.ok(result.controls.some(x=>x.element==="button"));
});
test("new UI action cannot silently disappear from generated inventory",()=>{
 const baseline=inspectUiFile("const x=load('parties');","A.jsx");
 const changed=inspectUiFile("const x=load('parties'); const y=load('new-feature',{method:'POST'});","A.jsx");
 assert.equal(changed.requests.length,baseline.requests.length+1);
 assert.equal(changed.requests[1].static_path,"new-feature");
 assert.deepEqual(changed.requests[1].methods,["POST"]);
});
test("missing request path is an error, not dropped coverage",()=>{
 assert.throws(()=>inspectUiFile("const x=load();","A.jsx"),
  /CRM_FUNCTIONAL_MATRIX_VIOLATION/);
});
test("an active RPC without linked canonical owner is rejected",()=>{
 const base={modules:[],active_rpc_paths:["/crm.test.v1.Service/Get"],phase:"contract-only"};
 assert.throws(()=>deriveMatrix({files:[],modules:base,descriptors:[],phase:base.phase,sha:"a"}),
  /active RPC not backed/);
});
test("real React source is analyzed without a fixed component count",()=>{
 const content=readFileSync("apps/cloudflare-crm/web/src/main.jsx","utf8");
 const result=inspectUiFile(content,"apps/cloudflare-crm/web/src/main.jsx");
 assert.ok(result.requests.length>=2);
 assert.ok(result.controls.some(x=>x.element==="button"));
});

test("new versioned contract requires its exact canonical binding",()=>{
 const owner={module_id:"crm.example",linked:true,
  rpc_bindings:[{path:"/crm.example.v2.ExampleService/CreateExample",
    request:"crm.example.v2.CreateExampleRequest",response:"crm.example.v2.CreateExampleResponse"}]};
 const proposed={file:"crm/example/v2/example.proto",
  path:"/crm.example.v2.ExampleService/CreateExample",
  input:"crm.example.v2.CreateExampleRequest",
  output:"crm.example.v2.CreateExampleResponse"};
 const report=deriveMatrix({files:[],modules:{modules:[owner],active_rpc_paths:[]},
  descriptors:[proposed],phase:"contract-only",sha:"a"});
 assert.equal(report.protobuf.proposed_cutover_contracts[0].module_id,"crm.example");
 assert.equal(report.protobuf.proposed_cutover_contracts[0].activation,
  "NOT_EXECUTED");
 assert.equal(report.summary.active_rpc_methods,0);
});
test("orphan new RPC proposal fails closed without an original Rust owner",()=>{
 assert.throws(()=>deriveMatrix({files:[],
  modules:{modules:[],active_rpc_paths:[]},
  descriptors:[{file:"crm/ghost/v2/ghost.proto",
   path:"/crm.ghost.v2.GhostService/GetGhost"}],
  phase:"contract-only",sha:"a"}),/lacks exact canonical binding/);
});

test("historical binding never grants ownership of a new version implicitly",()=>{
 const owner={module_id:"crm.example",linked:true,
  rpc_bindings:[{path:"/crm.example.v1.ExampleService/GetExample",
    request:"crm.example.v1.GetExampleRequest",response:"crm.example.v1.GetExampleResponse"}]};
 assert.throws(()=>deriveMatrix({files:[],modules:{modules:[owner],active_rpc_paths:[]},
  descriptors:[{file:"crm/example/v2/example.proto",path:"/crm.example.v2.ExampleService/GetExample",
   input:"crm.example.v2.GetExampleRequest",output:"crm.example.v2.GetExampleResponse"}],
  phase:"contract-only",sha:"a"}),/lacks exact canonical binding/);
});

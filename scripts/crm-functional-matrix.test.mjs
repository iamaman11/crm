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

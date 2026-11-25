import assert from "node:assert/strict";
import test from "node:test";
import {resolve} from "node:path";
import {ownerPlan, verifyContractGraphs} from "./test-rust-owners.mts";

const root = resolve("fixture-repository");
const worker = resolve(root, "apps/cloudflare-crm/rust-party");
const owner = resolve(root, "modules/example-owner");

function fixture() {
  const packages = [
    {id:"worker-id",name:"fixture-worker",manifest_path:resolve(worker,"Cargo.toml"),dependencies:[
      {name:"example-owner",rename:"alias",path:owner,kind:null,target:null,uses_default_features:false,features:["selected"]},
    ]},
    {id:"owner-id",name:"example-owner",manifest_path:resolve(owner,"Cargo.toml")},
    {id:"wire-id",name:"crm-proto-contracts"},
    {id:"generated-id",name:"crm-connect-contracts"},
    {id:"buffa-id",name:"buffa"},
  ];
  const nodes = [
    {id:"worker-id",dependencies:["owner-id"]},
    {id:"wire-id",features:["sales-activities-link"]},
    {id:"generated-id",features:[],dependencies:["buffa-id"]},
  ];
  return {packages,resolve:{root:"worker-id",nodes}};
}
function report() {
  return {modules:[
    {linked:true,crate_path:"modules/example-owner",crate_name:"example-owner"},
    {linked:true,crate_path:"apps/cloudflare-crm/rust-party",crate_name:"fixture-worker"},
    {linked:false,crate_path:"modules/historical-owner",crate_name:"historical-owner"},
  ]};
}

test("Cargo owner plan uses actual dependency identity and declared feature flags",()=>{
  const plan=ownerPlan(report(),fixture(),root,worker);
  assert.deepEqual([...plan], [
    ["crm-core-contracts",[]],["crm-module-sdk",[]],
    ["example-owner",["--no-default-features","--features","selected"]],
  ]);
});
test("Cargo owner plan fails closed on a missing resolved link, target-only edge or wrong package",()=>{
  const missing=fixture();
  missing.resolve.nodes[0].dependencies=[];
  assert.throws(()=>ownerPlan(report(),missing,root,worker),/resolved Worker owner/);
  const target=fixture();
  target.packages[0].dependencies[0].target="cfg(unix)";
  assert.throws(()=>ownerPlan(report(),target,root,worker),/declared normal Worker owner/);
  const wrong=fixture();
  wrong.packages[0].dependencies[0].name="wrong";
  assert.throws(()=>ownerPlan(report(),wrong,root,worker),/declared normal Worker owner/);
});
test("generated Connect dependency and persisted wire feature checks fail closed in either graph",()=>{
  assert.deepEqual(verifyContractGraphs(fixture(),fixture()),{features:[],dependencies:["buffa"]});
  const impure=fixture();
  impure.resolve.nodes[2].dependencies.push("wire-id");
  assert.throws(()=>verifyContractGraphs(fixture(),impure),/must not depend/);
  const features=fixture();
  features.resolve.nodes[2].features.push("unexpected");
  assert.throws(()=>verifyContractGraphs(fixture(),features),/must match/);
  const wire=fixture();
  wire.resolve.nodes[1].features.push("unexpected");
  assert.throws(()=>verifyContractGraphs(fixture(),wire),/original link contract feature/);
});

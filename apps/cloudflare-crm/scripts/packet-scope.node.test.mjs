import assert from "node:assert/strict";
import test from "node:test";
import {readFileSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {scopeClassifier,pathMatches,verifyPacketPaths,packetClassifier} from "./packet-scope.mts";

const root=fileURLToPath(new URL("../../../",import.meta.url));
const packet=JSON.parse(readFileSync(resolve(root,"repository-packet.json"),"utf8"));
const policy=JSON.parse(readFileSync(resolve(root,"affected-scope-policy.json"),"utf8"));
const metadata={packages:[
  {id:"core",name:"core",manifest_path:resolve(root,"crates/core/Cargo.toml")},
],workspace_members:["core"]};

test("scope classification uses actual Cargo owners, never a custom test subset",()=>{
  const classify=scopeClassifier(root,metadata,policy);
  assert.equal(classify("crates/core/src/lib.rs").known,true);
  assert.equal(classify("Cargo.toml").known,true);
  assert.equal(classify("unknown/new.bin").known,false);
  const broken=structuredClone(policy);
  broken.scopes[0].required_workflows=["Missing CI"];
  assert.throws(()=>scopeClassifier(root,metadata,broken),/requires missing required CI workflows/);
  const duplicate=structuredClone(policy);
  duplicate.scopes.push(duplicate.scopes[0]);
  assert.throws(()=>scopeClassifier(root,metadata,duplicate),/invalid scope categories/);
});
test("canonical packet ignores documentation and rejects forbidden/unclassified paths",()=>{
  verifyPacketPaths(packet,["AGENTS.md","apps/cloudflare-crm/web/src/App.tsx"],"origin/dev");
  assert.throws(()=>verifyPacketPaths(packet,["database/secret.sql"],"origin/dev"),/forbidden path/);
  assert.throws(()=>verifyPacketPaths(packet,["unknown/new.bin"],"origin/dev"),/outside packet/);
  assert.throws(()=>verifyPacketPaths(packet,[],"origin/main"),/unrecognized packet base/);
  const classify=packetClassifier(packet,"origin/dev");
  assert.equal(classify("docs/README.md"),"documentation");
  assert.equal(classify("modules/example/src/nested/lib.rs"),"allowed");
  assert.notEqual(classify("modules/example/nested/module.yaml"),"allowed");
});
test("native glob boundaries and actual governed scope representatives",()=>{
  assert(pathMatches("crates/crm-customer-enrichment-query-adapter/src/lib.rs","crates/crm-customer-enrichment-*/**"));
  const cases=[
    ["modules/crm-sales/module.yaml","contracts"],
    ["proto/crm/example/v1/example.proto","protobuf_api_compatibility"],
    ["apps/cloudflare-crm/migrations/0004.sql","d1_migrations"],
    ["apps/cloudflare-crm/rust-party/test/native.mjs","d1_runtime_acceptance"],
    ["package.json","cloudflare_product_plane"],
    ["apps/cloudflare-crm/web/src/app.tsx","frontend"],
    ["repository-packet.json","operations"],
  ];
  const classify=scopeClassifier(root,metadata,policy);
  for(const [path,id] of cases){
    const result=classify(path);
    assert(result.known,path);
    assert(result.scopes.includes(id),path);
  }
});

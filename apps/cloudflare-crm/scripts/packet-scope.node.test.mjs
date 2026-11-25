import assert from "node:assert/strict";
import test from "node:test";
import {readFileSync} from "node:fs";
import {resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {analyzeScope,pathMatches,verifyPacketPaths} from "./packet-scope.mts";

const root=fileURLToPath(new URL("../../../",import.meta.url));
const packet=JSON.parse(readFileSync(resolve(root,"repository-packet.json"),"utf8"));
const policy=JSON.parse(readFileSync(resolve(root,"affected-scope-policy.json"),"utf8"));
const metadata={packages:[
  {id:"core",name:"core",manifest_path:resolve(root,"crates/core/Cargo.toml"),dependencies:[]},
  {id:"owner",name:"owner",manifest_path:resolve(root,"modules/owner/Cargo.toml"),dependencies:[{name:"core"}]},
  {id:"app",name:"app",manifest_path:resolve(root,"crates/app/Cargo.toml"),dependencies:[{name:"owner"}]},
],workspace_members:["core","owner","app"]};

test("actual Cargo workspace reverse impact and broadening are deterministic",()=>{
  const report=analyzeScope(root,["crates/core/src/lib.rs"],metadata,policy);
  assert.deepEqual(report.direct_packages,["core"]);
  assert.deepEqual(report.affected_packages,["app","core","owner"]);
  assert.equal(report.broadened,false);
  const broad=analyzeScope(root,["Cargo.toml"],metadata,policy);
  assert.equal(broad.broadened,true);
  assert.deepEqual(broad.affected_packages,["app","core","owner"]);
});
test("canonical packet ignores only documentation and rejects forbidden/unclassified paths",()=>{
  verifyPacketPaths(packet,["AGENTS.md","apps/cloudflare-crm/web/src/App.tsx"],"origin/dev");
  assert.throws(()=>verifyPacketPaths(packet,["database/secret.sql"],"origin/dev"),/forbidden path/);
  assert.throws(()=>verifyPacketPaths(packet,["unknown/new.bin"],"origin/dev"),/outside packet/);
  assert.throws(()=>verifyPacketPaths(packet,[],"origin/main"),/unrecognized packet base/);
  assert.throws(()=>analyzeScope(root,["unknown/new.bin"],metadata,policy),/unknown affected scope/);
  const docs=analyzeScope(root,["docs/README.md"],metadata,policy);
  assert.equal(docs.selected_workflows.length,0);
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
  for(const [path,id] of cases){
    const report=analyzeScope(root,[path],metadata,policy);
    assert(report.selected_scopes.includes(id),path);
    assert.equal(report.selected_workflows.length,1);
  }
});

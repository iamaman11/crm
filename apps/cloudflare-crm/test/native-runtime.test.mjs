// Native-only developer contract; no duplicate JS backend or JS business rules.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync,existsSync} from "node:fs";
import {fileURLToPath} from "node:url";
import {resolve,dirname} from "node:path";
import {modules,implementationOwners,ownerFiles} from "../shared/catalog.mjs";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const read=path=>readFileSync(resolve(root,path),"utf8");
const config=JSON.parse(read("wrangler.jsonc"));

test("exactly one native workers-rs Cloudflare entrypoint, no JS backend",()=>{
  assert.equal(config.name,"crm-dev");
  assert.equal(config.main,"./rust-party/build/index.js");
  assert.deepEqual(config.assets.run_worker_first,["/api/*"]);
  assert.equal(config.d1_databases.length,1);
  assert.equal(config.d1_databases[0].binding,"DB");
  assert.equal(config.version_metadata.binding,"CF_VERSION_METADATA");
  assert.equal(config.observability.enabled,true);
  for(const old of ["worker/index.mjs","worker/unified.mjs",
    "features/customer360/query.mjs","rust-party/wrangler.unified.jsonc",
    "rust-party/test/unified.mjs","scripts/native-cloud-proof.mjs"]){
    assert.equal(existsSync(resolve(root,old)),false,"duplicate old backend remains: "+old);
  }
  assert.ok(read("rust-party/src/lib.rs").includes('mod access;'));
  assert.ok(read("rust-party/src/lib.rs").includes('mod pilot;'));
  assert.ok(read("rust-party/src/lib.rs").includes('access::actor(&req, &env)'));
  assert.ok(!read("rust-party/Cargo.toml").includes("integrated = []"),
    "import-only JS-compatible legacy Rust build must be retired");
});

test("React feature fields and native Rust pilot owners have exact parity",()=>{
  const rust=read("rust-party/src/pilot.rs");
  for(const [name,model] of Object.entries(modules)){
    assert.equal(implementationOwners[name],"pilot-core");
    assert.equal(ownerFiles["pilot-core"].api,"rust-party/src/pilot.rs");
    const pattern=new RegExp('"'+name+'"\\s*=> Some\\(Model \\{ table: "'+model.table+
      '", fields: &\\[([^\\]]+)\\]');
    const match=rust.match(pattern);
    assert.ok(match,"missing native Rust model "+name);
    assert.deepEqual(JSON.parse("["+match[1]+"]"),model.fields.map(([key])=>key));
  }
  assert.equal(ownerFiles["customer360"].api,"rust-party/src/pilot.rs");
  assert.equal(ownerFiles["rust-party"].api,"rust-party/src/lib.rs");
  assert.equal(ownerFiles["rust-party"].tests,"rust-party/test/native.mjs");
});

test("release refuses native UI write without D1 and exact Cloudflare Logs",()=>{
  const deploy=read("scripts/deploy.mjs"),e2e=read("e2e/live.mjs");
  assert.match(deploy,/native-functional-cycle\.json/);
  assert.match(deploy,/SELECT request_id,action,subject_id FROM crm_audit/);
  assert.match(deploy,/event\.\$workers\?\.scriptVersion\?\.id===liveVersion/);
  assert.match(deploy,/if\(pending\.length\)throw Error/);
  assert.match(e2e,/uiCoverage/);
  assert.match(e2e,/nativeResponses/);
  assert.match(e2e,/native-functional-cycle\.json/);
  assert.match(e2e,/customer360\.retry/);
  assert.match(e2e,/customers\.edit\.cancel/);
  assert.match(e2e,/deals\.board\.stage/);
});

test("live doctor is read-only and never claims CI results it cannot see",()=>{
  const rust=read("rust-party/src/lib.rs");
  assert.match(rust,/async fn native_doctor\(/);
  assert.match(rust,/tenant_reads/);
  assert.match(rust,/COUNT\(request_id\)/);
  assert.match(rust,/EXTERNAL_PROTECTED_CI_CHECK_REQUIRED/);
  assert.match(rust,/EXTERNAL_CLOUDFLARE_DEPLOYMENTS_CHECK_REQUIRED/);
  assert.ok(!rust.includes('return fetch("/api/doctor"'),"doctor cannot call a JS proxy");
});

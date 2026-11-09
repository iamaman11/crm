// Native-only developer contract. No duplicate JS or pilot Rust business code.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync,existsSync} from "node:fs";
import {fileURLToPath} from "node:url";
import {resolve,dirname} from "node:path";
import {modules,implementationOwners,ownerFiles} from "../shared/catalog.mjs";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const read=p=>readFileSync(resolve(root,p),"utf8");
const config=JSON.parse(read("wrangler.jsonc"));
test("one published native Worker with one D1 and no competing backend",()=>{
  assert.equal(config.name,"crm-dev");
  assert.equal(config.main,"./rust-party/build/index.js");
  assert.deepEqual(config.assets.run_worker_first,["/api/*"]);
  assert.equal(config.d1_databases.length,1);
  assert.equal(config.d1_databases[0].binding,"DB");
  assert.equal(config.version_metadata.binding,"CF_VERSION_METADATA");
  assert.equal(config.observability.enabled,true);
  for(const old of ["worker/index.mjs","worker/unified.mjs",
    "features/customer360/query.mjs","rust-party/wrangler.unified.jsonc",
    "rust-party/test/unified.mjs","scripts/native-cloud-proof.mjs",
    "rust-party/src/pilot.rs"]){
    assert.equal(existsSync(resolve(root,old)),false,"old backend remains: "+old);
  }
  const rust=read("rust-party/src/lib.rs");
  assert.match(rust,/mod access;/);
  assert.match(rust,/mod account_overview;/);
  assert.match(rust,/access::actor\(&req, &env\)/);
  assert.doesNotMatch(rust,/pilot::route|mod pilot;/);
});
test("each visible CRM feature names its original owner",()=>{
  const ui=read("web/src/main.jsx");
  const owners={
    accounts:["original-accounts","rust-party/src/accounts.rs","/api/accounts"],
    contacts:["original-contact-points","rust-party/src/contact_points.rs","/api/contact-points"],
    deals:["original-sales","rust-party/src/sales_deals.rs","/api/sales-deals"],
    tasks:["original-activities","rust-party/src/activity_tasks.rs","/api/activity-tasks"]
  };
  assert.deepEqual(Object.keys(modules),Object.keys(owners));
  for(const [name,[owner,file,route]] of Object.entries(owners)){
    assert.equal(implementationOwners[name],owner);
    assert.equal(ownerFiles[owner].api,file);
    assert.match(ui,new RegExp(route.replace("/api/","")));
    assert.ok(existsSync(resolve(root,file)));
  }
  const card=read("web/src/Customer360.jsx");
  assert.ok(card.includes('load("accounts/"+accountId+"/overview")'));
  assert.doesNotMatch(card,/customer360\//);
});
test("React has one Party-list owner and no swallowed Party refresh failures",()=>{
  const app=read("web/src/main.jsx"),party=read("web/src/Parties.jsx");
  assert.match(app,/<Parties items=\{items\.parties\} load=\{api\}/);
  assert.match(app,/api\("parties"\)/);
  assert.doesNotMatch(party,/useEffect|setItems\(|load\("parties"\)/);
  assert.doesNotMatch(party,/const \[items,/);
  assert.match(party,/await onSaved\(\);\s*setForm\(null\);\s*setNotice\(/);
  assert.match(party,/catch\(e\) \{setError\(e\.message\);\}/);
});
test("single release remains protected by D1, logs and browser evidence",()=>{
  const deploy=read("scripts/deploy.mjs"),e2e=read("e2e/live.mjs");
  assert.match(deploy,/native-functional-cycle\.json/);
  assert.match(deploy,/SELECT request_id,action,subject_id FROM crm_audit/);
  assert.match(deploy,/if\(pending\.length\)throw Error/);
  assert.match(e2e,/uiCoverage/);
  assert.match(e2e,/nativeResponses/);
  assert.match(e2e,/native-functional-cycle\.json/);
});
test("CI tests and compiles once in PR; main promotes identical tree, never recompiles",()=>{
  const workflow=read("../../.github/workflows/cloudflare-crm-spike.yml");
  const step=name=>{
    const start=workflow.indexOf("      - name: "+name+"\n");
    assert.notEqual(start,-1,"missing CI step "+name);
    const tail=workflow.slice(start);
    const end=tail.indexOf("\n      - name: ",1);
    return end===-1?tail:tail.slice(0,end);
  };
  for(const name of [
    "Verify all existing Rust business modules compile for WebAssembly",
    "Execute the original Rust domain rules from WASM in Node",
    "Build isolated native D1 test Worker (PR only)",
    "Test original Rust Party through workerd and isolated native D1",
    "Build exact native Rust Worker for review or release",
    "Compile native Rust Access gate using isolated RSA test keys",
    "Install Chromium for physical React to native Rust Worker proof",
    "Test native WebCrypto JWT and all React CRM screens under workerd",
    "Build React CRM",
    "Install pinned workers-rs build CLI"
  ]) {
    assert.match(step(name),/if: github.event_name == 'pull_request'/,
      "PR build/test step must not execute again on main: "+name);
  }
  assert.equal((workflow.match(/worker-build --release --features native\n/g)||[]).length,1);
  const seal=step("Seal PR-tested production bundle and checkout tree");
  assert.match(seal,/if: github.event_name == 'pull_request'/);
  assert.match(seal,/git rev-parse HEAD\^\{tree\}/);
  assert.match(seal,/sha256sum --check --strict/);
  const saved=step("Save tested PR bundle for content-identical main promotion");
  assert.match(saved,/if: github.event_name == 'pull_request'/);
  assert.match(saved,/\.release\/source-tree\.txt/);
  const promotion=step("Promote successful PR artifact only when merge source tree is identical");
  assert.match(promotion,/if: github.event_name == 'push' && github.ref == 'refs\/heads\/main'/);
  assert.match(promotion,/commits\/\$GITHUB_SHA\/pulls/);
  assert.match(promotion,/\.merge_commit_sha == \$sha/);
  assert.match(promotion,/\.head\.repo\.full_name == \$repo/);
  assert.match(promotion,/\.conclusion == "success"/);
  assert.match(promotion,/gh run download "\$source_run"/);
  assert.match(promotion,/cat \.release\/source-tree\.txt/);
  assert.match(promotion,/test "\$\(cat \.release\/source-tree\.txt\)" = "\$main_tree"/);
  assert.match(promotion,/sha256sum --check --strict/);
  assert.match(promotion,/cmp rust-party\/build\/index_bg\.wasm/);
  assert.match(promotion,/printf '%s\\n' "\$GITHUB_SHA" > \.release\/source-sha\.txt/);
  assert.match(step("Preserve exact compiled bundle for the protected deployment"),
    /if: github.event_name == 'push' && github.ref == 'refs\/heads\/main'/);
  assert.match(workflow,/gh run download "\$GITHUB_RUN_ID"/);
  assert.match(workflow,/sha256sum --check --strict \.release\/compiled-sha256\.txt/);
  assert.match(workflow,/node apps\/cloudflare-crm\/e2e\/live\.mjs/);
  assert.match(workflow,/node apps\/cloudflare-crm\/scripts\/deploy\.mjs verify-data/);
  assert.match(workflow,/node apps\/cloudflare-crm\/scripts\/deploy\.mjs rollback/);
});

test("protected release tolerates eventually consistent Cloudflare deployment readback",()=>{
  const deploy=read("scripts/deploy.mjs");
  assert.match(deploy,/async function waitForDeployment\(/);
  assert.match(deploy,/const newVersion=await waitForDeployment\(null,sha,"deploy"\)/);
  assert.match(deploy,/await waitForDeployment\(p\.previous_version,p\.previous_sha,"rollback"\)/);
  assert.doesNotMatch(deploy,/if\(await version\(\)===p\.previous_version\)/,
    "stale deployment readback must never skip rollback");
  assert.match(deploy,/CRM deployment readback mismatch stage=/);
});
test("live doctor read-only and never invents CI acceptance",()=>{
  const rust=read("rust-party/src/lib.rs");
  assert.match(rust,/async fn native_doctor\(/);
  assert.match(rust,/tenant_reads/);
  assert.match(rust,/COUNT\(request_id\)/);
  assert.match(rust,/EXTERNAL_PROTECTED_CI_CHECK_REQUIRED/);
  assert.match(rust,/EXTERNAL_CLOUDFLARE_DEPLOYMENTS_CHECK_REQUIRED/);
});

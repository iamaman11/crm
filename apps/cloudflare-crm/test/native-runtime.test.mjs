// Native-only developer contract. No duplicate JS or pilot Rust business code.
import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync,existsSync} from "node:fs";
import {fileURLToPath} from "node:url";
import {resolve,dirname} from "node:path";
import {modules,implementationOwners,ownerFiles} from "../shared/catalog.mjs";
import {uiInventory,requiredUiActions} from "../e2e/ui-inventory.mjs";
import {amountToMinorUnits,amountFromMinorUnits,formatDealMoney,
  percentToBasisPoints,percentFromBasisPoints} from "../web/src/deal-presentation.mjs";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const read=p=>readFileSync(resolve(root,p),"utf8");
const config=JSON.parse(read("wrangler.jsonc"));
test("one published native Worker with one D1 and no competing backend",()=>{
  assert.equal(config.name,"crm-dev");
  assert.equal(config.main,"./rust-party/build/index.js");
  assert.deepEqual(config.assets.run_worker_first,
    ["/api/*","/crm.sales.v1.DealService/*"]);
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
  assert.match(rust,/mod connect_sales;/);
  assert.ok(existsSync(resolve(root,"rust-party/src/connect_sales.rs")));
  assert.match(read("rust-party/src/connect_sales.rs"),/impl sales::DealService for SalesService/);
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
test("Identity Resolution comparison uses canonical Rust pair and no invented matcher",()=>{
  const app=read("web/src/main.jsx");
  const ui=read("web/src/IdentityResolution.jsx");
  const owner=read("rust-party/src/identity_resolution.rs");
  const route=read("rust-party/src/lib.rs");
  assert.ok(app.includes("<IdentityResolution parties={items.parties}"));
  assert.ok(!ui.includes("setItems("));
  assert.ok(ui.includes('load("identity-resolution/compare/"'));
  assert.ok(owner.includes("CanonicalPartyPair::try_new"));
  assert.ok(owner.includes("parties::stored"));
  assert.ok(owner.includes('"candidate_cases": "NOT_CONNECTED"'));
  assert.ok(!owner.includes("MatchEvidenceSnapshot::try_new"));
  assert.ok(route.includes("identity_resolution::route"));
  assert.ok(ui.includes('load("identity-resolution/candidates")'));
  assert.ok(owner.includes("discover_shared_recorded_emails"));
  assert.ok(owner.includes("contact_points::decode"));
  assert.ok(owner.includes("point.is_valid_at(now)"));
  assert.ok(owner.includes("LIMIT 301"));
  assert.ok(!owner.includes("MatchEvidenceSnapshot::try_new"));
  assert.equal(ownerFiles["crm-identity-resolution"].api,
    "rust-party/src/identity_resolution.rs");
  assert.equal(ownerFiles["crm-identity-resolution"].browser,"e2e/live.mjs");
});
test("data quality uses original Rust name rules, one Party list and no shadow score",()=>{
  const app=read("web/src/main.jsx");
  const ui=read("web/src/DataQuality.jsx");
  const adapter=read("rust-party/src/data_quality.rs");
  const domain=read("../../modules/crm-data-quality/src/domain.rs");
  const lib=read("rust-party/src/lib.rs");
  assert.ok(app.includes("<DataQuality parties={items.parties}"));
  assert.ok(ui.includes('load("data-quality/parties/"'));
  assert.ok(!ui.includes("setItems("));
  assert.ok(adapter.includes("builtin_party_name_review_rules_v1"));
  assert.ok(adapter.includes("parties::stored"));
  assert.ok(adapter.includes('"completeness_score": "NOT_COMPUTED"'));
  assert.ok(domain.includes("PartyRuleSetVersion::publish"));
  assert.ok(lib.includes("data_quality::route"));
  assert.equal(ownerFiles["crm-data-quality"].api,"rust-party/src/data_quality.rs");
  assert.equal(ownerFiles["crm-data-quality"].browser,"e2e/live.mjs");
});
test("customer-data-operations bounded CSV export uses original domain and one D1",()=>{
  const ui=read("web/src/DataExport.jsx");
  const app=read("web/src/main.jsx");
  const source=read("rust-party/src/customer_data_operations.rs");
  const runtime=read("rust-party/src/lib.rs");
  assert.ok(ui.includes('run("preview")'));
  assert.ok(ui.includes('run("download")'));
  assert.ok(app.includes("<DataExport load={api} writable={writable}"));
  assert.ok(source.includes("PartyExportSpecification::try_new"));
  assert.ok(source.includes("canonical_party_export_csv_record"));
  assert.ok(source.includes("party_export_sha256"));
  assert.ok(source.includes("role != Some(\"admin\")"));
  assert.ok(source.includes("LIMIT 101"));
  assert.ok(source.includes("EXPORT_SCOPE_LIMIT_REACHED"));
  assert.ok(runtime.includes("customer_data_operations::route"));
  assert.ok(!source.includes("INSERT INTO"));
  assert.ok(source.includes("ImportRow::create"));
  assert.ok(source.includes("row.derived_target_party_id"));
  assert.ok(source.includes("row.target_idempotency_key"));
  assert.ok(source.includes("parse_import_source"));
  assert.ok(source.includes("parties::import_batch"));
  assert.ok(read("rust-party/src/parties.rs").includes("db.batch(statements)"));
  assert.ok(ui.includes('sendImport("preview")'));
  assert.ok(ui.includes('sendImport("apply")'));
  assert.equal(ownerFiles["crm-customer-data-operations"].api,
    "rust-party/src/customer_data_operations.rs");
});
test("privacy request intake uses only original PrivacyCase and one D1",()=>{
  const ui=read("web/src/PrivacyCases.jsx");
  const api=read("rust-party/src/customer_privacy.rs");
  const runtime=read("rust-party/src/lib.rs");
  const migration=read("migrations/0011_original_customer_privacy_intake.sql");
  assert.ok(ui.includes("customer-privacy/cases"));
  assert.ok(ui.includes("Отправить заявку"));
  assert.ok(ui.includes("Отменить заявку"));
  assert.ok(api.includes("PrivacyCase::new"));
  assert.ok(api.includes("case.submit"));
  assert.ok(api.includes("case.cancel"));
  assert.ok(api.includes("encode_privacy_case_state"));
  assert.ok(api.includes("decode_privacy_case_state"));
  assert.ok(runtime.includes("customer_privacy::route"));
  assert.ok(migration.includes("crm_privacy_case_receipts"));
  assert.ok(migration.includes("crm_audit"));
  assert.equal(ownerFiles["crm-customer-privacy"].api,
    "rust-party/src/customer_privacy.rs");
  assert.equal(ownerFiles["crm-customer-privacy"].browser,"e2e/live.mjs");
  assert.ok(!api.includes("verify_subject("));
  assert.ok(!api.includes("erase("));
});
test("Party relationships UI uses original Rust owner with one App list",()=>{
  const app=read("web/src/main.jsx"),ui=read("web/src/Relationships.jsx");
  const owner=read("rust-party/src/party_relationships.rs");
  assert.match(app,/api\("party-relationships"\)/);
  assert.match(app,/relationships:relationships\.items/);
  assert.match(app,/<Relationships items=\{items\.relationships\}/);
  assert.match(ui,/await onSaved\(\);\s*setForm\(null\);\s*setNotice/);
  assert.doesNotMatch(ui,/useEffect|setItems\(|load\("party-relationships"\)/);
  assert.match(ui,/valid_from_unix_nanos/);
  assert.match(ui,/expected_version/);
  assert.match(ui,/idempotency-key/);
  assert.match(owner,/PartyRelationship::create/);
  assert.match(owner,/relation\.apply_update/);
  assert.match(owner,/PARTY_RELATIONSHIPS_PARTY_NOT_FOUND/);
  assert.match(owner,/crm_party_relationship_records/);
  assert.ok(existsSync(resolve(root,"web/src/Relationships.jsx")));
  assert.equal(ownerFiles["original-relationships"].ui,"web/src/Relationships.jsx");
  assert.equal(ownerFiles["original-relationships"].browser,"e2e/live.mjs");
});
test("original Consent UI has a single App list, deny/evaluate only and no forged grant",()=>{
  const app=read("web/src/main.jsx"),ui=read("web/src/Consents.jsx");
  const owner=read("rust-party/src/consents.rs");
  assert.ok(app.includes('api("consents")'));
  assert.ok(app.includes("consents:consents.items"));
  assert.ok(app.includes('<Consents items={items.consents}'));
  assert.ok(ui.includes('load("consents/evaluate"'));
  assert.ok(ui.includes('load("consents/suppress"'));
  assert.ok(ui.includes('await onSaved();'));
  assert.ok(ui.includes('idempotency-key'));
  assert.ok(ui.includes('subject_consent_evidence'));
  assert.ok(ui.includes('Подтвердить запрет'));
  assert.ok(!ui.includes('load("consents/grant"'));
  assert.ok(!ui.includes('setItems('));
  assert.ok(owner.includes('ConsentEffect::Deny'));
  assert.ok(owner.includes('evaluate_communication_authorization'));
  assert.ok(owner.includes('CONSENTS_CONTACT_POINT_PARTY_MISMATCH'));
  assert.ok(owner.includes('grant_ingress'));
  assert.equal(ownerFiles["original-consents"].ui,"web/src/Consents.jsx");
  assert.equal(ownerFiles["original-consents"].browser,"e2e/live.mjs");
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
test("permanent dev branch builds once, publishes the same tested artifact",()=>{
  const w=read("../../.github/workflows/cloudflare-crm-spike.yml");
  const deploy=read("scripts/deploy.mjs");
  assert.match(w,/push:\n    branches:\n      - dev\n/);
  assert.match(w,/pull_request:\n    branches:\n      - dev\n      - main\n/);
  assert.doesNotMatch(w,/push:\n    branches:\n      - main\n/);
  assert.match(w,/if: github.event_name == 'push' && github.ref == 'refs\/heads\/dev'/);
  assert.match(w,/environment: crm-dev/);
  assert.doesNotMatch(w,/Promote successful PR artifact|crm-native-pr-bundle|source-tree\.txt|promotion\.json/);
  assert.equal((w.match(/worker-build --release --features native\n/g)||[]).length,1);
  for(const name of [
    "Test original Rust Party through workerd and isolated native D1",
    "Test native WebCrypto JWT and all React CRM screens under workerd",
    "Restore compiled production Worker after JWT test",
    "Seal exactly tested development bundle",
    "Preserve exact compiled bundle for the protected deployment",
    "Browser E2E on published CRM",
    "Verify native Rust Party persisted data and D1 audit",
    "Restore previous Worker if release acceptance failed"
  ])assert.ok(w.includes(name),"missing gate: "+name);
  assert.match(w,/gh run download "\$GITHUB_RUN_ID"/);
  assert.match(w,/sha256sum --check --strict \.release\/compiled-sha256\.txt/);
  assert.match(w,/node apps\/cloudflare-crm\/e2e\/live\.mjs/);
  assert.match(w,/node apps\/cloudflare-crm\/scripts\/deploy\.mjs verify-data/);
  assert.match(w,/node apps\/cloudflare-crm\/scripts\/deploy\.mjs rollback/);
  assert.match(deploy,/process\.env\.GITHUB_REF!=="refs\/heads\/dev"/);
  assert.doesNotMatch(deploy,/process\.env\.GITHUB_REF!=="refs\/heads\/main"/);
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

test("Privacy intake and readback failures cannot be mistaken for failed writes",()=>{
  const privacy=read("web/src/PrivacyCases.jsx");
  const react=read("web/src/main.jsx");
  const native=read("rust-party/test/native.mjs");
  const doctor=read("scripts/doctor.mjs");
  assert.match(react,/async function refreshAfterSave\(\)/);
  assert.match(react,/setNotice\(saved\.replayed/);
  assert.match(react,/await refreshAfterSave\(\)/);
  assert.doesNotMatch(react,/onSaved=\{reload\}/);
  assert.match(privacy,/async function refreshAfterMutation\(/);
  assert.match(privacy,/Заявка сохранена, но список не обновился/);
  assert.match(native,/Committed Account update \+ failed readback/);
  assert.match(doctor,/!sourceFor\(module\)\.api\?"NOT_CONNECTED"/);
  assert.doesNotMatch(doctor,/module==="customer-privacy"\?/);
  assert.doesNotMatch(read("AGENTS.md"),/Five still-unconnected crates|Five still-unconnected|five still-unconnected/i);
});

test("public edge readback remains strict, bounded and diagnoses stale routing",()=>{
  const deploy=read("scripts/deploy.mjs");
  assert.match(deploy,/deadline=start\+120000/);
  assert.match(deploy,/last\.source===expected/);
  assert.match(deploy,/cache_status:r\.headers\.get\("cf-cache-status"\)/);
  assert.match(deploy,/header_sha:r\.headers\.get\("x-crm-source-sha"\)/);
  assert.match(deploy,/Public Worker SHA readback mismatch stage=/);
  assert.match(deploy,/node apps\/cloudflare-crm\/scripts\/deploy\.mjs rollback|async function rollback\(/);
});

test("original Customer Enrichment immutable definitions use one native Rust owner and D1",()=>{
  const app=read("web/src/main.jsx"),ui=read("web/src/Enrichment.jsx");
  const adapter=read("rust-party/src/customer_enrichment.rs");
  const core=read("../../modules/crm-customer-enrichment/src/definitions.rs");
  const codec=read("../../modules/crm-customer-enrichment/src/definition_persistence.rs");
  const migration=read("migrations/0012_original_customer_enrichment_definitions.sql");
  const runtime=read("rust-party/src/lib.rs");
  const e2e=read("e2e/live.mjs"),local=read("rust-party/test/native.mjs");
  assert.ok(app.includes("<Enrichment load={api}"));
  assert.ok(ui.includes('load("customer-enrichment/profiles")'));
  assert.ok(ui.includes('load("customer-enrichment/mappings")'));
  assert.ok(ui.includes('Сохранить профиль'));
  assert.ok(ui.includes('Сохранить сопоставление'));
  assert.ok(adapter.includes("ProviderProfileVersion::publish"));
  assert.ok(adapter.includes("MappingVersion::publish"));
  assert.ok(adapter.includes("encode_provider_profile_version_state"));
  assert.ok(adapter.includes("decode_provider_profile_version_state"));
  assert.ok(adapter.includes("decode_mapping_version_state"));
  assert.ok(adapter.includes("profile.version_id().clone()"));
  assert.ok(core.includes("pub fn is_effective_at("));
  assert.ok(codec.includes("validate_expected_id("));
  assert.ok(runtime.includes("customer_enrichment::route("));
  assert.match(runtime,/const EXPECTED: \[&str; 23\]/);
  assert.ok(runtime.includes('provider_dispatch":"NOT_CONNECTED"'));
  assert.ok(migration.includes("crm_enrichment_profile_audit"));
  assert.ok(migration.includes("crm_enrichment_mapping_audit"));
  assert.ok(migration.includes("customer_enrichment.profile_publish"));
  assert.ok(migration.includes("customer_enrichment.mapping_publish"));
  assert.ok(local.includes("crm_enrichment_receipts"));
  assert.ok(e2e.includes('cover("customer-enrichment.profile_publish")'));
  assert.ok(e2e.includes('cover("customer-enrichment.mapping_publish")'));
  assert.equal(ownerFiles["crm-customer-enrichment"].api,"rust-party/src/customer_enrichment.rs");
  assert.equal(ownerFiles["crm-customer-enrichment"].browser,"e2e/live.mjs");
  assert.equal(config.d1_databases.length,1);
  assert.ok(!adapter.includes("fetch("));
  assert.ok(!adapter.includes("parties::update"));
});

test("new customer user journey composes original Party and Account without a second backend",()=>{
  const app=read("web/src/main.jsx");
  const parties=read("web/src/Parties.jsx");
  const browser=read("e2e/live.mjs");
  assert.match(app,/function createCustomerWithNewParty\(\)/);
  assert.match(app,/function createAccountForParty\(party,created=false\)/);
  assert.match(app,/relatedParty\?\.id/);
  assert.match(app,/onCreateAccount=\{createAccountForParty\}/);
  assert.match(parties,/createForCustomer\?newForm\(\):null/);
  assert.match(parties,/onCreateAccount\(result\.item,true\)/);
  assert.match(parties,/onCancelCustomer\(\)/);
  assert.match(browser,/accounts\.create\.new-party-flow/);
  assert.match(browser,/accounts\.create\.new-party-linked/);
  assert.match(browser,/directCustomer\.parties\[0\]\.id/);
  assert.doesNotMatch(parties,/\/api\/accounts|UPDATE crm_account|INSERT INTO crm_account/);
});

test("Deal UI converts major amounts to exact Rust minor units without floating point",()=>{
  assert.equal(amountToMinorUnits("127,50"),12750);
  assert.equal(amountToMinorUnits("127.50"),12750);
  assert.equal(amountToMinorUnits("0"),0);
  assert.equal(amountToMinorUnits("0,05"),5);
  assert.equal(amountToMinorUnits("90071992547409.91"),Number.MAX_SAFE_INTEGER);
  for(const bad of ["","12.345","-1.00","1,2,3","1.2e3","01.10","90071992547409.92"]){
    assert.throws(()=>amountToMinorUnits(bad),"must reject "+bad);
  }
  assert.equal(amountFromMinorUnits("12750"),"127.50");
  assert.equal(amountFromMinorUnits("0"),"0.00");
  assert.equal(amountFromMinorUnits("-5"),"-0.05");
  assert.equal(formatDealMoney({minor_units:"12750",currency:"USD"}),"127,50 USD");
  assert.equal(formatDealMoney({minor_units:"123456789",currency:"PLN"}),"1\u00a0234\u00a0567,89 PLN");
  assert.equal(formatDealMoney(null),"—");
});
test("Deal UI shows percent, sends exact basis points and rejects invalid values",()=>{
  assert.equal(percentToBasisPoints("37,5"),3750);
  assert.equal(percentToBasisPoints("12.34"),1234);
  assert.equal(percentToBasisPoints("100"),10000);
  assert.equal(percentToBasisPoints("0.01"),1);
  assert.equal(percentFromBasisPoints(2500),"25");
  assert.equal(percentFromBasisPoints(1234),"12.34");
  assert.equal(percentFromBasisPoints(1),"0.01");
  for(const bad of ["","100,01","-1","12,345","101","1e2","25%"]){
    assert.throws(()=>percentToBasisPoints(bad),"must reject "+bad);
  }
  const app=read("web/src/main.jsx"),browser=read("e2e/live.mjs");
  assert.match(app,/amountToMinorUnits\(form\.amount_major\)/);
  assert.match(app,/probability_bps:percentToBasisPoints\(form\.probability_percent\)/);
  assert.match(app,/formatDealMoney\(row\.amount\)/);
  assert.match(app,/Вероятность, %/);
  assert.match(browser,/sales-deals\.invalid-amount/);
  assert.match(browser,/sales-deals\.invalid-probability/);
  assert.match(browser,/sales-deals\.edit-exact-conversion/);
});

test("local and live Chromium both exercise readable Deal amount and percent",()=>{
  const native=read("rust-party/test/native.mjs");
  const live=read("e2e/live.mjs");
  assert.doesNotMatch(native,/Сумма \(минимальные единицы\)/);
  assert.doesNotMatch(live,/Сумма \(минимальные единицы\)/);
  assert.match(native,/getByLabel\("Вероятность, %"\)/);
  assert.match(native,/probability_bps,6250/);
  assert.match(live,/sales-deals\.invalid-probability/);
});

test("every CRM JSX button family has an explicit reviewed live-browser proof",()=>{
  const live=read("e2e/live.mjs");
  const files=uiInventory.map(screen=>screen.file);
  assert.equal(uiInventory.length,10);
  assert.equal(new Set(files).size,10,"duplicate inventory component");
  assert.equal(uiInventory.reduce((n,screen)=>n+screen.buttons,0),60);
  assert.equal(requiredUiActions.length,93);
  assert.equal(new Set(requiredUiActions).size,requiredUiActions.length,
    "ambiguous coverage action identifiers");
  for(const screen of uiInventory){
    const source=read("web/src/"+screen.file);
    for(const [tag,expected] of [
      ["button",screen.buttons],["form",screen.forms],
      ["input",screen.inputs],["select",screen.selects]
    ]){
      const observed=(source.match(new RegExp("<"+tag+"\\b","g"))||[]).length;
      assert.equal(observed,expected,
        "UI changed without corresponding inventory review: "+screen.file+" "+tag);
    }
    assert.ok(screen.actions.length>0,"screen has no tracked actions: "+screen.file);
    for(const action of screen.actions){
      assert.ok(source.includes(action.source),
        "UI action is absent: "+screen.file+" "+action.id+" "+action.source);
      assert.ok(live.includes('"'+action.id+'"'),
        "real protected Chromium is missing action: "+action.id);
    }
  }
  assert.match(live,/requiredUiActions\.filter\(x=>!uiCoverage\.has\(x\)\)/);
  assert.match(live,/uiInventory\.reduce\(\(count,x\)=>count\+x\.buttons,0\)/);
  const card=read("web/src/Customer360.jsx");
  assert.ok(card.includes('["party_relationships","Связи","relationships"'));
  assert.ok(card.includes('["contacts","deals"].includes(route)'));
  assert.doesNotMatch(card,/\["party_relationships","Связи","parties"/);
  assert.ok(live.includes('"customer360.navigate.relationships"'));
});

test("Cloudflare CRM CI triggers on every dev change and both PR bases",()=>{
  const workflow=read("../../.github/workflows/cloudflare-crm-spike.yml");
  const triggers=workflow.split("jobs:")[0];
  assert.match(triggers,/push:\s*\n\s*branches:\s*\n\s*- dev/);
  assert.match(triggers,/pull_request:\s*\n\s*branches:\s*\n\s*- dev\s*\n\s*- main/);
  assert.doesNotMatch(triggers,/^\s+paths:/m,
    "path filters would silently skip Protobuf or shared Rust owner changes");
});

// Fast local contract verification and feature navigation. Zero network/database mutations.
import { readFileSync, existsSync, readdirSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { modules, readModels, nativeModels, backendModels, errors, implementationOwners, ownerFiles } from "../shared/catalog.mjs";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const started = performance.now();
const read = name => readFileSync(resolve(root, name), "utf8");
const rootOwner = ownerFiles["pilot-core"];
const sourceFor = feature => ({
 registry:"shared/catalog.mjs", ...(ownerFiles[implementationOwners[feature]] || {}),
});
const sourceExists = path => typeof path === "string" &&
  existsSync(resolve(root, path.split(":")[0]));
const requested = process.argv.slice(2).join(" ").trim().toLowerCase();
const alias = { client:"customers", customer:"customers", клиент:"customers",
  contact:"contacts", контакт:"contacts", deal:"deals", сделк:"deals",
  task:"tasks", задач:"tasks" };
const features = { ...modules, ...readModels, ...nativeModels, ...backendModels };
const moduleKey = Object.keys(features).find(key =>
  key === requested || key.includes(requested) && requested.length >= 2 ||
  features[key].title.toLowerCase().includes(requested) && requested.length >= 2) ||
  Object.entries(alias).find(([key]) => requested.startsWith(key))?.[1];
if(requested) {
  const code = Object.keys(errors).find(key => key.toLowerCase() === requested || key.toLowerCase().includes(requested));
  if(!moduleKey && !code) {
    console.error(JSON.stringify({ event:"crm.locate", result:"UNKNOWN", query:requested,
      available:Object.keys(features), errors:Object.keys(errors) }));
    process.exitCode=2;
  } else {
    console.log(JSON.stringify({ event:"crm.locate", result:"FOUND", query:requested,
      module:moduleKey||null, error:code||null, fields:moduleKey?(features[moduleKey].fields||[]).map(([key])=>key):undefined,
      owner:moduleKey ? implementationOwners[moduleKey] : "pilot-core",
      ui:moduleKey && backendModels[moduleKey] ? "NOT_CONNECTED" : undefined,
      integration:moduleKey && backendModels[moduleKey] ? "API_D1_SOURCE_PRESENT_RUNTIME_PROOF_REQUIRED" : undefined,
      files:moduleKey ? sourceFor(moduleKey) : { registry:"shared/catalog.mjs", ...rootOwner },
      duration_ms:+(performance.now()-started).toFixed(2) }));
  }
} else {
  // All migrations are read-only inputs; new ordered migrations need no doctor changes.
  const migration = readdirSync(resolve(root,"migrations"))
    .filter(name => /^\d+.*\.sql$/.test(name)).sort()
    .map(name => read("migrations/"+name)).join("\n");
  const ui=read(rootOwner.ui), api=read(rootOwner.api), test=read(rootOwner.tests);
  const checks=[];
  for (const [name, model] of Object.entries(modules)) {
    const block = migration.match(new RegExp("CREATE TABLE IF NOT EXISTS "+model.table+" \\(([\\s\\S]*?)\\);"));
    const missing = model.fields.map(([key]) => key).filter(key =>
      !(block && new RegExp("(?:^|[,\\n])\\s*"+key+"\\s", "m").test(block[1])) &&
      !new RegExp("ALTER\\s+TABLE\\s+"+model.table+"\\s+ADD\\s+(?:COLUMN\\s+)?"+key+"\\s","i").test(migration)
    );
    checks.push({ module:name, schema:block && missing.length===0?"PASS":"FAIL", missing });
  }
  checks.push({check:"shared-ui-catalog",result:ui.includes('shared/catalog.mjs')?"PASS":"FAIL"});
  // Rust is the only runtime API implementation. Compare exact pilot columns
  // with React's field registry; an old JS file must not satisfy this gate.
  const nativeFieldsMatch=Object.entries(modules).every(([name,model])=>{
    const pattern=new RegExp('"'+name+'"\\s*=> Some\\(Model \\{ table: "'+model.table+
      '", fields: &\\[([^\\]]+)\\]');
    const match=api.match(pattern);
    if(!match)return false;
    let fields;
    try{fields=JSON.parse('['+match[1]+']')}catch{return false}
    return JSON.stringify(fields)===JSON.stringify(model.fields.map(([key])=>key));
  });
  checks.push({check:"native-rust-api-catalog",result:nativeFieldsMatch?"PASS":"FAIL"});
  checks.push({check:"test-present",result:test.includes('Native Rust-only customers/contacts/deals/tasks/Customer360')?"PASS":"FAIL"});
  checks.push({check:"browser-present",result:existsSync(resolve(root,rootOwner.browser))?"PASS":"FAIL"});
  // A new feature cannot silently inherit the old pilot's shared files.
  const keys=Object.keys(features);
  const ownershipCorrect=keys.length===Object.keys(implementationOwners).length &&
    keys.every(name => {
      const owner=implementationOwners[name], files=ownerFiles[owner];
      if (!files || (owner==="pilot-core" &&
          !["customers","contacts","deals","tasks"].includes(name))) return false;
      // A backend-only owner must not pretend to have a React screen or browser proof.
      if (backendModels[name]) {
        return backendModels[name].ui==="NOT_CONNECTED" &&
          files.ui===null && files.browser===null &&
          ["api","validation","codec","migration","tests"].every(part=>sourceExists(files[part]));
      }
      return ["ui","api","validation","migration","tests","browser"].every(part =>
        sourceExists(files[part]));
    });
  checks.push({check:"explicit-feature-ownership",result:ownershipCorrect?"PASS":"FAIL"});
  // This proves source wiring, not a live D1 operation or user-visible UI.
  const originalAccountSource=sourceFor("accounts");
  const originalAccountConnected=backendModels.accounts.ui==="NOT_CONNECTED" &&
    migration.includes("CREATE TABLE IF NOT EXISTS crm_account_records") &&
    read(originalAccountSource.api).includes("Account::create") &&
    read(originalAccountSource.api).includes("decode_account_state") &&
    read(originalAccountSource.tests).includes("crm.native-original-account-d1");
  checks.push({check:"original-account-backend-source",result:originalAccountConnected?"PASS":"FAIL",
    ui:"NOT_CONNECTED",runtime_proof:"EXTERNAL_CI_REQUIRED"});
  const originalContactSource=sourceFor("contact-points");
  const originalContactConnected=backendModels["contact-points"].ui==="NOT_CONNECTED" &&
    backendModels["contact-points"].verification==="TRUSTED_EVIDENCE_NOT_CONNECTED" &&
    migration.includes("CREATE TABLE IF NOT EXISTS crm_contact_point_records") &&
    read(originalContactSource.api).includes("ContactPoint::create") &&
    read(originalContactSource.api).includes("decode_contact_point_state") &&
    read(originalContactSource.api).includes("client-submitted arbitrary evidence is NOT proof") &&
    read(originalContactSource.tests).includes("crm.native-original-contact-point-d1");
  checks.push({check:"original-contact-point-backend-source",
    result:originalContactConnected?"PASS":"FAIL",ui:"NOT_CONNECTED",
    verification:"TRUSTED_EVIDENCE_NOT_CONNECTED",consent:"NOT_CONNECTED",
    runtime_proof:"EXTERNAL_CI_REQUIRED"});
  const relSource=sourceFor("party-relationships");
  const relReady=backendModels["party-relationships"].ui==="NOT_CONNECTED" &&
    migration.includes("CREATE TABLE IF NOT EXISTS crm_party_relationship_records") &&
    read(relSource.api).includes("PartyRelationship::create") &&
    read(relSource.api).includes("decode_party_relationship_state") &&
    read(relSource.tests).includes("crm.native-original-party-relationships-d1");
  checks.push({check:"original-party-relationships-backend-source",
    result:relReady?"PASS":"FAIL",ui:"NOT_CONNECTED",
    runtime_proof:"EXTERNAL_CI_REQUIRED"});
  const taskSource=sourceFor("activity-tasks");
  const tasksReady=backendModels["activity-tasks"].ui==="NOT_CONNECTED" &&
    migration.includes("CREATE TABLE IF NOT EXISTS crm_activity_task_records") &&
    read(taskSource.api).includes("Task::create") &&
    read(taskSource.api).includes("task.complete(CompleteTask") &&
    read(taskSource.api).includes("decode_task_state") &&
    read(taskSource.tests).includes("crm.native-original-activities-d1");
  checks.push({check:"original-activities-backend-source",
    result:tasksReady?"PASS":"FAIL",ui:"NOT_CONNECTED",
    runtime_proof:"EXTERNAL_CI_REQUIRED"});
  const salesSource=sourceFor("sales-deals");
  const salesReady=backendModels["sales-deals"].ui==="NOT_CONNECTED" &&
    backendModels["sales-deals"].primary_contact==="NOT_CONNECTED" &&
    migration.includes("CREATE TABLE IF NOT EXISTS crm_sales_deal_records") &&
    read(salesSource.api).includes("Deal::create") &&
    read(salesSource.api).includes("deal.advance_stage(AdvanceDealStage") &&
    read(salesSource.api).includes("decode_deal_state") &&
    read(salesSource.tests).includes("crm.native-original-sales-d1");
  checks.push({check:"original-sales-backend-source",
    result:salesReady?"PASS":"FAIL",ui:"NOT_CONNECTED",
    primary_contact:"NOT_CONNECTED",runtime_proof:"EXTERNAL_CI_REQUIRED"});
  const consentSource=sourceFor("consents");
  const consentReady=backendModels.consents.ui==="NOT_CONNECTED" &&
    backendModels.consents.grant_evidence==="NOT_CONNECTED" &&
    migration.includes("CREATE TABLE IF NOT EXISTS crm_consent_records") &&
    read(consentSource.api).includes("ConsentAuthorization::create") &&
    read(consentSource.api).includes("evaluate_communication_authorization") &&
    read(consentSource.api).includes("ConsentEffect::Deny") &&
    read(consentSource.api).includes('"/api/consents/suppress"') &&
    read(consentSource.tests).includes("crm.native-original-consents-d1");
  checks.push({check:"original-consents-backend-source",
    result:consentReady?"PASS":"FAIL",ui:"NOT_CONNECTED",
    grant_evidence:"NOT_CONNECTED",operator_suppressions:"ENABLED",
    runtime_proof:"EXTERNAL_CI_REQUIRED"});
  const validCatalog=Object.keys(modules).every(name => {
    const module=modules[name], fieldKeys=module.fields.map(([key])=>key);
    return /^[a-z][a-z0-9_]*$/.test(module.table) && fieldKeys.length>0 &&
      fieldKeys.length===new Set(fieldKeys).size &&
      fieldKeys.every(key=>/^[a-z][a-z0-9_]*$/.test(key));
  });
  checks.push({check:"valid-feature-contracts",result:validCatalog?"PASS":"FAIL"});
  const nativeModelsValid=Object.entries(nativeModels).every(([name,model])=>
    model.kind==="owner"&&model.route===name&&
    implementationOwners[name]==="rust-party"&&
    ownerFiles["rust-party"].validation==="rust-party/src/lib.rs");
  checks.push({check:"native-rust-party-owner",result:nativeModelsValid?"PASS":"FAIL"});
  const readModelsValid=Object.entries(readModels).every(([name,feature])=>
    feature.kind==="read" && feature.route===name && !modules[name] &&
    implementationOwners[name]!== "pilot-core");
  checks.push({check:"read-model-no-table",result:readModelsValid?"PASS":"FAIL"});
  const guide=read("AGENTS.md");
  const guideOk=["iamaman11/crm","implementationOwners","ownerFiles",
    "GitHub Actions","npm run doctor","npm run locate"].every(t=>guide.includes(t));
  checks.push({check:"durable-developer-contract",result:guideOk?"PASS":"FAIL"});
  const ok=checks.every(c=>(c.schema||c.result)==="PASS");
  console.log(JSON.stringify({event:"crm.doctor",result:ok?"PASS":"FAIL",
    checks,modules:Object.keys(features),error_codes:Object.keys(errors),
    duration_ms:+(performance.now()-started).toFixed(2)}));
  if(!ok)process.exitCode=1;
}

// Fast local contract verification and feature navigation. Zero network/database mutations.
import { readFileSync, existsSync, readdirSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { modules, errors, implementationOwners, ownerFiles } from "../shared/catalog.mjs";
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
const moduleKey = Object.keys(modules).find(key =>
  key === requested || key.includes(requested) && requested.length >= 2 ||
  modules[key].title.toLowerCase().includes(requested) && requested.length >= 2) ||
  Object.entries(alias).find(([key]) => requested.startsWith(key))?.[1];
if(requested) {
  const code = Object.keys(errors).find(key => key.toLowerCase() === requested || key.toLowerCase().includes(requested));
  if(!moduleKey && !code) {
    console.error(JSON.stringify({ event:"crm.locate", result:"UNKNOWN", query:requested,
      available:Object.keys(modules), errors:Object.keys(errors) }));
    process.exitCode=2;
  } else {
    console.log(JSON.stringify({ event:"crm.locate", result:"FOUND", query:requested,
      module:moduleKey||null, error:code||null, fields:moduleKey?modules[moduleKey].fields.map(([key])=>key):undefined,
      owner:moduleKey ? implementationOwners[moduleKey] : "pilot-core",
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
  checks.push({check:"shared-api-catalog",result:api.includes('shared/catalog.mjs')?"PASS":"FAIL"});
  checks.push({check:"test-present",result:test.includes('read-only diagnostics verify schema')?"PASS":"FAIL"});
  checks.push({check:"browser-present",result:existsSync(resolve(root,rootOwner.browser))?"PASS":"FAIL"});
  // A new feature cannot silently inherit the old pilot's shared files.
  const keys=Object.keys(modules);
  const ownershipCorrect=keys.length===Object.keys(implementationOwners).length &&
    keys.every(name => {
      const owner=implementationOwners[name], files=ownerFiles[owner];
      return !!files && (owner!=="pilot-core" ||
        ["customers","contacts","deals","tasks"].includes(name)) &&
        ["ui","api","validation","migration","tests","browser"].every(part =>
          sourceExists(files[part]));
    });
  checks.push({check:"explicit-feature-ownership",result:ownershipCorrect?"PASS":"FAIL"});
  const validCatalog=keys.every(name => {
    const module=modules[name], fieldKeys=module.fields.map(([key])=>key);
    return /^[a-z][a-z0-9_]*$/.test(module.table) && fieldKeys.length>0 &&
      fieldKeys.length===new Set(fieldKeys).size &&
      fieldKeys.every(key=>/^[a-z][a-z0-9_]*$/.test(key));
  });
  checks.push({check:"valid-feature-contracts",result:validCatalog?"PASS":"FAIL"});
  const guide=read("AGENTS.md");
  const guideOk=["iamaman11/crm","implementationOwners","ownerFiles",
    "GitHub Actions","npm run doctor","npm run locate"].every(t=>guide.includes(t));
  checks.push({check:"durable-developer-contract",result:guideOk?"PASS":"FAIL"});
  const ok=checks.every(c=>(c.schema||c.result)==="PASS");
  console.log(JSON.stringify({event:"crm.doctor",result:ok?"PASS":"FAIL",
    checks,modules:Object.keys(modules),error_codes:Object.keys(errors),
    duration_ms:+(performance.now()-started).toFixed(2)}));
  if(!ok)process.exitCode=1;
}

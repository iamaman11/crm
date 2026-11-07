// Fast local contract verification and feature navigation. Zero network/database mutations.
import { readFileSync, existsSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { modules, errors } from "../shared/catalog.mjs";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const started = performance.now();
const read = name => readFileSync(resolve(root, name), "utf8");
const owner = {
 registry: "shared/catalog.mjs",
 ui: "web/src/main.jsx",
 api: "worker/index.mjs",
 validation: "worker/index.mjs:parseRecord",
 migration: "migrations/0001_initial.sql",
 tests: "test/worker.test.mjs",
 browser: "e2e/live.mjs",
};
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
      files:owner, duration_ms:+(performance.now()-started).toFixed(2) }));
  }
} else {
  const migration = read(owner.migration);
  const ui=read(owner.ui), api=read(owner.api), test=read(owner.tests);
  const checks=[];
  for (const [name, model] of Object.entries(modules)) {
    const block = migration.match(new RegExp("CREATE TABLE IF NOT EXISTS "+model.table+" \\(([\\s\\S]*?)\\);"));
    const missing = model.fields.map(([key])=>key).filter(key=> !block ||
      !new RegExp("(?:^|[,\\n])\\s*"+key+"\\s", "m").test(block[1]));
    checks.push({ module:name, schema:block && missing.length===0?"PASS":"FAIL", missing });
  }
  checks.push({check:"shared-ui-catalog",result:ui.includes('shared/catalog.mjs')?"PASS":"FAIL"});
  checks.push({check:"shared-api-catalog",result:api.includes('shared/catalog.mjs')?"PASS":"FAIL"});
  checks.push({check:"test-present",result:test.includes('read-only diagnostics verify schema')?"PASS":"FAIL"});
  checks.push({check:"browser-present",result:existsSync(resolve(root,owner.browser))?"PASS":"FAIL"});
  const ok=checks.every(c=>(c.schema||c.result)==="PASS");
  console.log(JSON.stringify({event:"crm.doctor",result:ok?"PASS":"FAIL",
    checks,modules:Object.keys(modules),error_codes:Object.keys(errors),
    duration_ms:+(performance.now()-started).toFixed(2)}));
  if(!ok)process.exitCode=1;
}

// Fast read-only source locator and contract checks. No Cloudflare, D1,
// automatic migration, or fake claim of a completed live browser acceptance.
import {readFileSync,existsSync,readdirSync} from "node:fs";
import {performance} from "node:perf_hooks";
import {fileURLToPath} from "node:url";
import {dirname,resolve} from "node:path";
import {modules,nativeModels,readModels,backendModels,
  errors,implementationOwners,ownerFiles} from "../shared/catalog.mjs";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const started=performance.now();
const read=path=>readFileSync(resolve(root,path),"utf8");
const exists=path=>typeof path==="string" && existsSync(resolve(root,path));
const features={...modules,...nativeModels,...readModels,...backendModels};
const sourceFor=key=>({
  registry:"shared/catalog.mjs",
  ...(ownerFiles[implementationOwners[key]]||{})
});
const aliases={client:"accounts",customer:"accounts",клиент:"accounts",
  contact:"contacts",контакт:"contacts",
  deal:"deals",сделк:"deals",
  task:"tasks",задач:"tasks"};
const requested=process.argv.slice(2).join(" ").trim().toLocaleLowerCase("ru");
if(requested){
  const module=Object.keys(features).find(key=>key===requested ||
    key.includes(requested)&&requested.length>=2 ||
    features[key].title.toLocaleLowerCase("ru").includes(requested)&&requested.length>=2) ||
    Object.entries(aliases).find(([alias])=>requested.startsWith(alias))?.[1];
  const code=Object.keys(errors).find(key=>key.toLowerCase()===requested ||
    key.toLowerCase().includes(requested));
  if(!module&&!code){
    console.error(JSON.stringify({event:"crm.locate",result:"UNKNOWN",
      query:requested,available:Object.keys(features),errors:Object.keys(errors)}));
    process.exitCode=2;
  }else{
    console.log(JSON.stringify({event:"crm.locate",result:"FOUND",query:requested,
      module:module||null,error:code||null,
      owner:module?implementationOwners[module]:null,
      runtime:module&&["accounts","contacts","deals","tasks","parties","party-relationships","consents"].includes(module)?
        "ORIGINAL_NATIVE_RUST":["identity-resolution","data-quality"].includes(module)?"READ_ONLY_ORIGINAL_RUST":"NOT_CONNECTED",
      files:module?sourceFor(module):{registry:"shared/catalog.mjs"},
      duration_ms:+(performance.now()-started).toFixed(2)}));
  }
}else{
  const rust=read("rust-party/src/lib.rs"),ui=read("web/src/main.jsx"),
    card=read("web/src/Customer360.jsx");
  const migrations=readdirSync(resolve(root,"migrations"))
    .filter(name=>/^\d+.*\.sql$/.test(name)).sort()
    .map(name=>read("migrations/"+name)).join("\n");
  const checks=[];
  const check=(name,ok,extra={})=>checks.push({check:name,result:ok?"PASS":"FAIL",...extra});
  check("one-native-worker",rust.includes('access::actor(&req, &env)') &&
    rust.includes('account_overview::overview') &&
    !rust.includes("pilot::route") && !rust.includes("mod pilot;") &&
    JSON.parse(read("wrangler.jsonc")).main==="./rust-party/build/index.js");
  const owners=Object.keys(features);
  check("original-source-ownership",owners.length===Object.keys(implementationOwners).length &&
    owners.every(key=>{
      const files=sourceFor(key),backend=backendModels[key];
      if(!files.validation||!exists(files.validation))return false;
      if(backend&&backend.ui==="NOT_CONNECTED")return files.ui===null;
      if(key==="sales-activities-link")return files.ui===null;
      return ["ui","api","tests"].every(field=>exists(files[field]));
    }));
  const expected=[
    ["accounts","/api/accounts","crm_account_records",
      "rust-party/src/accounts.rs","Account::create"],
    ["contacts","/api/contact-points","crm_contact_point_records",
      "rust-party/src/contact_points.rs","ContactPoint::create"],
    ["deals","/api/sales-deals","crm_sales_deal_records",
      "rust-party/src/sales_deals.rs","Deal::create"],
    ["tasks","/api/activity-tasks","crm_activity_task_records",
      "rust-party/src/activity_tasks.rs","Task::create"]
  ];
  for(const [name,route,table,source,symbol] of expected){
    check("original-"+name+"-ui-api-owner",
      ui.includes('api("'+route.slice(5)+'"') ||
      (ui.includes(route.slice(5))&&ui.includes("const sections")),
      {ui:"REACT_CONNECTED_NOT_LIVE",live:"NOT_TESTED"});
    const sourceReady=rust.includes(route)&&read(source).includes(symbol)&&
      migrations.includes("CREATE TABLE IF NOT EXISTS "+table);
    check("original-"+name+"-native-d1-source",sourceReady,
      {runtime:"EXTERNAL_WORKERD_OR_CLOUDFLARE_REQUIRED"});
  }
  check("original-account-overview-ui",
    card.includes('"accounts/"+accountId+"/overview"') &&
    rust.includes("account_overview::overview") &&
    read("rust-party/src/account_overview.rs").includes("crm_sales_deal_records"));
  check("no-pilot-api-or-frontend",
    !exists("rust-party/src/pilot.rs") &&
    !ui.includes('/api/customers') && !ui.includes('api("customers"') &&
    !card.includes('load("customer360/'));
  check("native-test-real-source",
    read("rust-party/test/native.mjs").includes("crm.native-original-sales-update-d1") &&
    read("rust-party/test/native.mjs").includes("React Account create/update"));
  check("durable-developer-contract",["iamaman11/crm","npm run locate",
    "GitHub Actions"].every(x=>read("AGENTS.md").includes(x)));
  const ok=checks.every(x=>x.result==="PASS");
  console.log(JSON.stringify({event:"crm.doctor",result:ok?"PASS":"FAIL",
    checks,modules:Object.keys(features),error_codes:Object.keys(errors),
    duration_ms:+(performance.now()-started).toFixed(2)}));
  if(!ok)process.exitCode=1;
}

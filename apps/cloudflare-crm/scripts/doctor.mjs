// Fast read-only source locator and contract checks. No Cloudflare, D1,
// automatic migration, or fake claim of a completed live browser acceptance.
import {performance} from "node:perf_hooks";
import {fileURLToPath} from "node:url";
import {dirname,resolve} from "node:path";
import {modules,nativeModels,readModels,backendModels,
  errors,implementationOwners,ownerFiles} from "../shared/catalog.mjs";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const started=performance.now();
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
      // Runtime is derived from the single owner locator, not a second hardcoded
      // status list. "Connected" describes source wiring, never live acceptance.
      runtime:module? !sourceFor(module).api?"NOT_CONNECTED":
        readModels[module]?"NATIVE_READ_COMPOSITION":
        backendModels[module]?.kind==="internal"?"INTERNAL_NATIVE_RUST":
        "ORIGINAL_NATIVE_RUST":null,
      files:module?sourceFor(module):{registry:"shared/catalog.mjs"},
      duration_ms:+(performance.now()-started).toFixed(2)}));
  }
}else{
  console.log("Source navigation only. Usage: pnpm run locate <module-or-error>");
  console.log("Architecture acceptance is performed by the compiler and runtime checks in GitHub Actions.");
}

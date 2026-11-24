// One typed, complete, tenant-bound catalog read shared by Sales and Customer360.
// The original Rust Sales owner supplies every stage name (including archived
// stages). Customer360 never writes a second stage or pipeline projection.
import type {SalesPipeline} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
import {currencyCatalog} from "./money";
import type {CurrencyCatalog} from "./money";

export async function allPipelines(signal:AbortSignal):Promise<{
  pipelines:SalesPipeline[];currencies:CurrencyCatalog
}> {
  const all:SalesPipeline[]=[];
  let currencies:CurrencyCatalog|undefined;
  let cursor="";
  const seen=new Set<string>();
  do {
    const page=await api.deals.listPipelines({
      page:{pageSize:100,pageToken:cursor},includeArchived:true
    },{signal});
    if(!cursor)currencies=currencyCatalog(page.currencies);
    all.push(...page.pipelines);
    cursor=page.pageInfo?.nextPageToken??"";
    if(cursor&&seen.has(cursor))throw Error("Повторный курсор воронок");
    if(cursor)seen.add(cursor);
  } while(cursor);
  if(!currencies)throw Error("Справочник валют не получен");
  return {pipelines:all,currencies};
}

// Immutable Deal.stageId stays authoritative; a renamed/archived stage is
// displayed by its CURRENT owner catalog. Unknown IDs are never invented.
export function stageName(pipeline:SalesPipeline|undefined,stageId:string):string {
  return pipeline?.stages.find(stage=>stage.stageId===stageId)?.name??stageId;
}

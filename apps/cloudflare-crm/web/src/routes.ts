import type {AccountStatus} from "@ultimate-crm/client/cloudflare";

export const sectionLabels={
  parties:"Участники",
  contacts:"Контакты",
  relationships:"Связи",
  sales:"Сделки",
  tasks:"Задачи",
  consents:"Коммуникации",
  review:"Качество и совпадения",
  privacy:"Запросы о данных",
  enrichment:"Обогащение",
  data:"Импорт и экспорт",
} as const;
export type Section=keyof typeof sectionLabels;
export type AccountFilterStatus = AccountStatus.ACTIVE | AccountStatus.INACTIVE;
export type AccountFilters = {search:string;status:AccountFilterStatus|null;cursor:string};
export type Route =
  | {kind:"insights";start:string;end:string;pipelineId:string;includeArchived:boolean;compare:boolean}
  | {kind:"workday";day:string}
  | ({kind:"accounts"} & AccountFilters)
  | {kind:"account";accountId:string}
  | {kind:"account-create";partyId:string}
  | {kind:"deal";dealId:string}
  | {kind:"search";query:string}
  | {kind:"section";screen:Section;partyId:string;accountId:string;taskId?:string;dealId?:string;contactPointId?:string;relationshipId?:string;search?:string;overdue?:boolean;viewId?:string;pipelineId?:string}
  | {kind:"not-found";path:string};

function isSection(path:string):path is Section {return Object.prototype.hasOwnProperty.call(sectionLabels,path);}

/** The URL/HTML boundary must accept only known generated AccountStatus members. */
export function accountFilterStatus(raw:string|null,allowed:readonly AccountFilterStatus[]):AccountFilterStatus|null {
  if(raw===null||raw==="")return null;
  const status=allowed.find(value=>String(value)===raw);
  if(status===undefined)throw Error("Недопустимый статус клиента в маршруте");
  return status;
}
export function parseRoute(hash:string,allowedStatuses:readonly AccountFilterStatus[]):Route {
  const source=hash.replace(/^#/,"")||"accounts";
  const question=source.indexOf("?");
  const path=question<0?source:source.slice(0,question);
  const params=new URLSearchParams(question<0?"":source.slice(question+1));
  if(path==="workday"){
    const dates=params.getAll("date");
    const day=dates[0]??"";
    let invalid=false;
    params.forEach((_,key)=>{if(key!=="date")invalid=true;});
    if(dates.length>1||invalid)return {kind:"not-found",path:source};
    if(day){
      if(!/^\d{4}-\d{2}-\d{2}$/.test(day))return {kind:"not-found",path:source};
      const [year,month,date]=day.split("-").map(Number);
      if(year===undefined||month===undefined||date===undefined||
        year<2000||year>2100)return {kind:"not-found",path:source};
      const valid=new Date(Date.UTC(year,month-1,date));
      if(valid.getUTCFullYear()!==year||valid.getUTCMonth()!==month-1||
        valid.getUTCDate()!==date)return {kind:"not-found",path:source};
    }
    return {kind:"workday",day};
  }
  if(path==="insights"){
    const allowed=new Set(["start","end","pipelineId","includeArchived","compare"]);
    let invalid=false;
    params.forEach((_,key)=>{if(!allowed.has(key)||params.getAll(key).length!==1)invalid=true;});
    const start=params.get("start")??"",end=params.get("end")??"";
    const pipelineId=params.get("pipelineId")??"";
    const include=params.get("includeArchived")??"";
    const compare=params.get("compare")??"";
    const validDate=(day:string)=>{
      if(!/^\d{4}-\d{2}-\d{2}$/.test(day))return false;
      const [y,m,d]=day.split("-").map(Number);
      if(y===undefined||m===undefined||d===undefined||y<2000||y>2100)return false;
      const date=new Date(Date.UTC(y,m-1,d));
      return date.getUTCFullYear()===y&&date.getUTCMonth()===m-1&&date.getUTCDate()===d;
    };
    if(invalid||(!!start!==!!end)||(start&&(!validDate(start)||!validDate(end)||start>end))||
       (pipelineId&&(pipelineId.length>128||pipelineId.includes("/")||
         pipelineId.includes("\\")||pipelineId.includes("%")||
         pipelineId==="."||pipelineId===".."))||
       (include!==""&&include!=="1")||(compare!==""&&compare!=="1")||
       (compare==="1"&&!start))return {kind:"not-found",path:source};
    return {kind:"insights",start,end,pipelineId,includeArchived:include==="1",compare:compare==="1"};
  }
  if(path==="search"){
    const queries=params.getAll("query");
    const query=queries[0]??"";
    let unknownParameter=false;
    params.forEach((_,key)=>{if(key!=="query")unknownParameter=true;});
    if(queries.length>1||unknownParameter||[...query].length>200)
      return {kind:"not-found",path:source};
    return {kind:"search",query};
  }
  if(path==="accounts"){
    try {
      const status=accountFilterStatus(params.get("status"),allowedStatuses);
      return {kind:"accounts",search:params.get("search")??"",status,cursor:params.get("cursor")??""};
    }catch{return {kind:"not-found",path:source};}
  }
  if(path==="accounts/new"){
    const ids=params.getAll("partyId");
    const partyId=ids[0];
    if(ids.length===1&&partyId&&partyId.length<=128&&!partyId.includes("/")&&
       !partyId.includes("\\")&&partyId!=="."&&partyId!==".."&&!partyId.includes("%"))
      return {kind:"account-create",partyId};
    return {kind:"not-found",path:source};
  }
  if(path.startsWith("accounts/")&&question<0){
    try {
      const id=decodeURIComponent(path.slice("accounts/".length));
      if(id&&!id.includes("/")&&!id.includes("\\")&&id!=="."&&id!=="..")return {kind:"account",accountId:id};
    }catch {/* Invalid percent-encoding must never crash React. */}
  }
  if(path.startsWith("sales/")&&question<0){
    try {
      const id=decodeURIComponent(path.slice("sales/".length));
      if(id&&id.length<=128&&!id.includes("/")&&!id.includes("\\")&&id!=="."&&id!=="..")
        return {kind:"deal",dealId:id};
    }catch {/* Malformed URLs do not reach API. */}
  }
  if(isSection(path)){
    const ids=path==="tasks"?params.getAll("taskId"):[];
    const deals=path==="tasks"?params.getAll("dealId"):[];
    const searches=path==="sales"||path==="tasks"?params.getAll("search"):[];
    const overdueValues=path==="tasks"?params.getAll("overdue"):[];
    const viewIds=path==="sales"?params.getAll("viewId"):[];
    const pipelineIds=path==="sales"?params.getAll("pipelineId"):[];
    const contacts=path==="contacts"?params.getAll("contactPointId"):[];
    const relations=path==="relationships"?params.getAll("relationshipId"):[];
    const taskId=ids[0]??"",dealId=deals[0]??"";
    const contactPointId=contacts[0]??"",relationshipId=relations[0]??"";
    const partyIds=params.getAll("partyId");
    const partyId=partyIds[0]??"";
    const focusedPartyRecord=contacts.length>0||relations.length>0;
    if(params.getAll("accountId").length>1||partyIds.length>1||ids.length>1||deals.length>1||searches.length>1||
       overdueValues.length>1||overdueValues.some(value=>value!=="1")||
       (taskId&&overdueValues.length>0)||viewIds.length>1||
       pipelineIds.length>1||(viewIds.length>0&&pipelineIds.length>0)||
       pipelineIds.some(id=>!id)||
       contacts.length>1||relations.length>1||
       (focusedPartyRecord&&(partyIds.length!==1||!partyId||!contactPointId&&!relationshipId))||
       searches.some(search=>[...search].length>200)||(taskId&&dealId)||
       [taskId,dealId,contactPointId,relationshipId,...viewIds,...pipelineIds,...(focusedPartyRecord?[partyId]:[])].some(id=>
         id.length>128||id.includes("/")||id.includes("\\")||id==="."||id===".."||id.includes("%")))
      return {kind:"not-found",path:source};
    return {kind:"section",screen:path,partyId,
      accountId:params.get("accountId")??"",...(taskId?{taskId}:{}),...(dealId?{dealId}:{}),
      ...(contactPointId?{contactPointId}:{}),...(relationshipId?{relationshipId}:{}),
      ...(searches[0]?{search:searches[0]}:{}),
      ...(overdueValues.length?{overdue:true}:{}),
      ...(viewIds[0]?{viewId:viewIds[0]}:{}),
      ...(pipelineIds[0]?{pipelineId:pipelineIds[0]}:{})};
  }
  return {kind:"not-found",path:source};
}
export function intelligenceHref(opts:{start?:string;end?:string;pipelineId?:string;
  includeArchived?:boolean;compare?:boolean}={}):string {
  const p=new URLSearchParams();
  if(opts.start)p.set("start",opts.start);
  if(opts.end)p.set("end",opts.end);
  if(opts.pipelineId)p.set("pipelineId",opts.pipelineId);
  if(opts.includeArchived)p.set("includeArchived","1");
  if(opts.compare&&opts.start&&opts.end)p.set("compare","1");
  return "#insights"+(p.size?"?"+p.toString():"");
}
export function workdayHref(day=""):string {
  return "#workday"+(day?"?"+new URLSearchParams({date:day}).toString():"");
}
export function accountsHref(filters:AccountFilters={search:"",status:null,cursor:""}):string {
  const params=new URLSearchParams();
  if(filters.search)params.set("search",filters.search);
  if(filters.status!==null)params.set("status",String(filters.status));
  if(filters.cursor)params.set("cursor",filters.cursor);
  const query=params.toString();
  return "#accounts"+(query?"?"+query:"");
}
export function accountHref(accountId:string):string{return "#accounts/"+encodeURIComponent(accountId);}
export function accountCreateHref(partyId:string):string {
  return "#accounts/new?"+new URLSearchParams({partyId}).toString();
}
export function partyHref(partyId:string):string {
  return "#parties?"+new URLSearchParams({partyId}).toString();
}
/** Recheck Data Quality for this exact Party, including after a full reload. */
export function reviewHref(partyId:string):string {
  return "#review?"+new URLSearchParams({partyId}).toString();
}
export function scopeHref(screen:"contacts"|"relationships"|"sales"|"tasks",id:string):string {
  const parameter=screen==="sales"||screen==="tasks"?"accountId":"partyId";
  return "#"+screen+"?"+new URLSearchParams({[parameter]:id}).toString();
}

/** One canonical deep-link per record, scoped to the original Party. */
export function contactHref(contactPointId:string,partyId:string):string {
  return "#contacts?"+new URLSearchParams({partyId,contactPointId}).toString();
}
export function relationshipHref(relationshipId:string,partyId:string):string {
  return "#relationships?"+new URLSearchParams({partyId,relationshipId}).toString();
}

export function taskHref(taskId:string,accountId:string):string {
  return "#tasks?"+new URLSearchParams({accountId,taskId}).toString();
}

/** One URL-owned Task lookup, preserving an Account or concrete Deal scope. */
export function taskSearchHref(accountId:string,search:string,dealId="",overdue=false):string {
  const params=new URLSearchParams();
  if(accountId)params.set("accountId",accountId);
  if(dealId)params.set("dealId",dealId);
  if(search.trim())params.set("search",search.trim());
  if(overdue)params.set("overdue","1");
  const query=params.toString();
  return "#tasks"+(query?"?"+query:"");
}

export function dealTasksHref(dealId:string,accountId:string):string {
  return "#tasks?"+new URLSearchParams({accountId,dealId}).toString();
}

export function salesSearchHref(initialAccountId:string,search:string,viewId="",pipelineId=""):string {
  const params=new URLSearchParams();
  if(initialAccountId)params.set("accountId",initialAccountId);
  if(search)params.set("search",search);
  if(viewId)params.set("viewId",viewId);
  if(pipelineId&&!viewId)params.set("pipelineId",pipelineId);
  const query=params.toString();
  return "#sales"+(query?"?"+query:"");
}

/** Reuse the one typed Sales route for saved-view selection and Deal search. */
export function salesViewHref(initialAccountId:string,viewId:string,search=""):string {
  return salesSearchHref(initialAccountId,search,viewId);
}

/** A manually selected Kanban uses the same URL owner, never storage. */
export function salesPipelineHref(initialAccountId:string,pipelineId:string,search=""):string {
  return salesSearchHref(initialAccountId,search,"",pipelineId);
}

export function dealHref(dealId:string):string {
  return "#sales/"+encodeURIComponent(dealId);
}

/** The one URL-owned query survives Back and reload; no browser-side result cache. */
export function searchHref(query:string):string {
  const term=query.trim();
  return "#search"+(term?"?"+new URLSearchParams({query:term}).toString():"");
}

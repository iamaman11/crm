import type {AccountStatus} from "@ultimate-crm/client/cloudflare";

export type Section = "parties"|"contacts"|"relationships"|"sales"|"tasks"|"consents"|"review"|"privacy"|"enrichment"|"data";
export type AccountFilterStatus = AccountStatus.ACTIVE | AccountStatus.INACTIVE;
export type AccountFilters = {search:string;status:AccountFilterStatus|null;cursor:string};
export type Route =
  | ({kind:"accounts"} & AccountFilters)
  | {kind:"account";accountId:string}
  | {kind:"account-create";partyId:string}
  | {kind:"section";screen:Section;partyId:string;accountId:string}
  | {kind:"not-found";path:string};

const sections:readonly Section[]=["parties","contacts","relationships","sales","tasks","consents","review","privacy","enrichment","data"];
function isSection(path:string):path is Section {return sections.some(screen=>screen===path);}

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
  if(isSection(path))return {kind:"section",screen:path,partyId:params.get("partyId")??"",accountId:params.get("accountId")??""};
  return {kind:"not-found",path:source};
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
export function scopeHref(screen:"contacts"|"relationships"|"sales"|"tasks",id:string):string {
  const parameter=screen==="sales"||screen==="tasks"?"accountId":"partyId";
  return "#"+screen+"?"+new URLSearchParams({[parameter]:id}).toString();
}

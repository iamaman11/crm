export type Section = "parties"|"contacts"|"relationships"|"sales"|"tasks"|"consents"|"review"|"privacy"|"enrichment"|"data";
export type Route =
  | {kind:"accounts";search:string;status:string;cursor:string}
  | {kind:"account";accountId:string}
  | {kind:"section";screen:Section;partyId:string;accountId:string}
  | {kind:"not-found";path:string};

const sections:readonly Section[]=["parties","contacts","relationships","sales","tasks","consents","review","privacy","enrichment","data"];
function isSection(path:string):path is Section {return sections.some(screen=>screen===path);}
export function parseRoute(hash:string):Route {
  const source=hash.replace(/^#/,"")||"accounts";
  const question=source.indexOf("?");
  const path=question<0?source:source.slice(0,question);
  const params=new URLSearchParams(question<0?"":source.slice(question+1));
  if(path==="accounts")return {kind:"accounts",search:params.get("search")??"",status:params.get("status")??"",cursor:params.get("cursor")??""};
  if(path.startsWith("accounts/")&&question<0){
    try {
      const id=decodeURIComponent(path.slice("accounts/".length));
      if(id&&!id.includes("/")&&!id.includes("\\")&&id!=="."&&id!=="..")return {kind:"account",accountId:id};
    }catch {/* Invalid percent-encoding must never crash React. */}
  }
  if(isSection(path))return {kind:"section",screen:path,partyId:params.get("partyId")??"",accountId:params.get("accountId")??""};
  return {kind:"not-found",path:source};
}
export function accountsHref(filters:{search:string;status:string;cursor:string}={search:"",status:"",cursor:""}):string {
  const params=new URLSearchParams();
  if(filters.search)params.set("search",filters.search);
  if(filters.status)params.set("status",filters.status);
  if(filters.cursor)params.set("cursor",filters.cursor);
  const query=params.toString();
  return "#accounts"+(query?"?"+query:"");
}
export function accountHref(accountId:string):string{return "#accounts/"+encodeURIComponent(accountId);}
export function scopeHref(screen:"contacts"|"relationships"|"sales"|"tasks",id:string):string {
  const parameter=screen==="sales"||screen==="tasks"?"accountId":"partyId";
  return "#"+screen+"?"+new URLSearchParams({[parameter]:id}).toString();
}

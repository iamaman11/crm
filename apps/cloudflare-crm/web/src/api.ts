import { createCloudflareCrmClient } from "@ultimate-crm/client/cloudflare";
export const api=createCloudflareCrmClient(window.location.origin);
export const newId=()=>crypto.randomUUID();
export function field(data:FormData,name:string):string {
  const value=data.get(name);
  if(typeof value!=="string")throw Error("Поле не заполнено: "+name);
  return value;
}
export function timestamp(value:string) {
  if(!value)return undefined;
  const ms=Date.parse(value);
  if(!Number.isFinite(ms))throw Error("Некорректная дата");
  return {seconds:BigInt(Math.floor(ms/1000)),nanos:(ms%1000)*1000000};
}
export function dateText(value:{seconds:bigint;nanos:number}|undefined):string {
  if(!value)return "—";
  return new Date(Number(value.seconds)*1000+Math.floor(value.nanos/1000000)).toLocaleString("ru");
}

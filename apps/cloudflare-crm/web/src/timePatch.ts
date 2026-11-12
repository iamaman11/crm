import {field,timestamp} from "./api";
export function timePatch(data:FormData,name:string) {
  const mode=field(data,name+"Mode");
  if(mode==="keep")return undefined;
  if(mode==="clear")return {operation:{case:"clear" as const,value:true}};
  const value=timestamp(field(data,name));
  if(!value)throw Error("Укажите дату");
  return {operation:{case:"set" as const,value}};
}

import type {Member} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";

// One complete, tenant-authorized staff catalog for administration and Task
// assignment pickers. The browser cannot invent someone else's membership.
export async function allMembers(signal:AbortSignal):Promise<Member[]> {
  const all:Member[]=[];
  let next="";
  const seen=new Set<string>();
  do {
    const page=await api.members.listMembers({
      includeInactive:true,page:{pageSize:100,pageToken:next}
    },{signal});
    all.push(...page.members);
    next=page.pageInfo?.nextPageToken??"";
    if(next&&seen.has(next))throw Error("Повторный курсор сотрудников");
    if(next)seen.add(next);
  }while(next);
  return all;
}
export function humanMember(identity:string):boolean {
  return identity.includes("@")&&!identity.startsWith("service:");
}

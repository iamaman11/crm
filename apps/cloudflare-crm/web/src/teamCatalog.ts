import type {Team} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
export async function allTeams(signal:AbortSignal):Promise<Team[]> {
  const result:Team[]=[];
  const seen=new Set<string>();
  let next="";
  do {
    const page=await api.members.listTeams({includeInactive:true,
      page:{pageSize:100,pageToken:next}},{signal});
    result.push(...page.teams);
    next=page.pageInfo?.nextPageToken??"";
    if(next&&seen.has(next))throw Error("Повторный курсор команд");
    if(next)seen.add(next);
  }while(next);
  return result;
}

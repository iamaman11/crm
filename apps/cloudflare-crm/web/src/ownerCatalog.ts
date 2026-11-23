import type {ActorOrTeamOwner} from "@ultimate-crm/client/cloudflare";
import {allMembers,humanMember} from "./memberCatalog";
import {allTeams} from "./teamCatalog";

export async function allOwners(signal:AbortSignal) {
  const [members,teams]=await Promise.all([allMembers(signal),allTeams(signal)]);
  return {
    choices:[
      ...members.map(m=>({value:"actor:"+m.email,label:m.email+(m.active?"":" (неактивен)"),
        eligible:m.active&&humanMember(m.email)})),
      ...teams.map(t=>({value:"team:"+t.teamId,label:"Команда: "+t.name+(t.active?"":" (архив)"),
        eligible:t.active&&t.memberEmails.some(email=>members.some(m=>m.email===email&&m.active&&humanMember(m.email)))}))
    ]
  };
}
export function ownerValue(owner:ActorOrTeamOwner|undefined):string {
  if(owner?.owner.case==="assigneeIdentity")return "actor:"+owner.owner.value;
  if(owner?.owner.case==="teamId")return "team:"+owner.owner.value;
  return "";
}
export function ownerWire(value:string) {
  if(value.startsWith("actor:"))return {owner:{case:"assigneeIdentity" as const,value:value.slice(6)}};
  if(value.startsWith("team:"))return {owner:{case:"teamId" as const,value:value.slice(5)}};
  return undefined;
}
export function ownerLabel(owner:ActorOrTeamOwner|undefined,choices:ReadonlyArray<{value:string;label:string}>):string {
  const key=ownerValue(owner);
  return choices.find(c=>c.value===key)?.label??(key.startsWith("team:")?"Команда: "+key.slice(5):key.startsWith("actor:")?key.slice(6):"Неизвестен");
}

// The form's empty choice means Keep/default signed creator. Both Deal and
// Task clients send the same generated owner oneof, not independent scalars.
export function taskOwnerFields(value:string) {
  const owner=ownerWire(value.startsWith("team:")?value:value?"actor:"+value:"");
  return owner?{owner}:{};
}

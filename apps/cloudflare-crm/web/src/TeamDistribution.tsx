import {useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import type {TeamDistributionOptions,TeamDistributionScope} from "@ultimate-crm/client/cloudflare";

// One UI projection of the Access owner's generated options for both owners.
export function TeamDistribution({options,pending,onChanged,onAssign}:{options:TeamDistributionOptions;pending:boolean;onChanged:()=>void;onAssign:(scope:TeamDistributionScope,email:string)=>void}){
  const [email,setEmail]=useState("");
  if(!options.scope||!options.eligibleMemberEmails.length)return null;
  const scope=options.scope;
  return <form aria-label="Распределение из очереди команды" onSubmit={event=>{
    event.preventDefault();if(email)onAssign(scope,email);
  }}>
    <SelectField label="Назначить сотруднику команды" value={email} onChange={event=>{setEmail(event.target.value);onChanged();}} required>
      <option value="">Выберите сотрудника</option>
      {options.eligibleMemberEmails.map(member=><option key={member} value={member}>{member}</option>)}
    </SelectField>
    <Button type="submit" disabled={pending||!email}>Распределить из очереди</Button>
  </form>;
}

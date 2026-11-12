import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import {api} from "./api";
import {useQuery,QueryNotice} from "./hooks";

export function PartyPicker({name="partyId",label="Участник"}:{name?:string;label?:string}) {
  const [cursor,setCursor]=useState("");
  const [value,setValue]=useState("");
  const query=useQuery(useCallback(()=>api.partyRecords.listParties({page:{pageSize:100,pageToken:cursor}}),[cursor]));
  return <div><SelectField label={label} name={name} value={value} required onChange={e=>setValue(e.target.value)}>
    <option value="">Выберите участника</option>
    {query.data?.parties.map(p=><option value={p.partyId} key={p.partyId}>{p.displayName} · {p.partyId}</option>)}
  </SelectField><QueryNotice query={query}/>
    {query.data?.pageInfo?.nextPageToken&&<Button variant="secondary" onClick={()=>{setValue("");setCursor(query.data?.pageInfo?.nextPageToken??"");}}>Другие участники</Button>}
    {cursor&&<Button variant="secondary" onClick={()=>{setValue("");setCursor("");}}>В начало списка</Button>}
  </div>;
}
export function AccountPicker() {
  const [cursor,setCursor]=useState("");
  const [value,setValue]=useState("");
  const query=useQuery(useCallback(()=>api.accounts.listAccounts({page:{pageSize:100,pageToken:cursor}}),[cursor]));
  return <div><SelectField label="Клиент" name="accountId" value={value} required onChange={e=>setValue(e.target.value)}>
    <option value="">Выберите клиента</option>
    {query.data?.accounts.map(a=><option value={a.accountId} key={a.accountId}>{a.name} · {a.accountId}</option>)}
  </SelectField><QueryNotice query={query}/>
    {query.data?.pageInfo?.nextPageToken&&<Button variant="secondary" onClick={()=>{setValue("");setCursor(query.data?.pageInfo?.nextPageToken??"");}}>Другие клиенты</Button>}
    {cursor&&<Button variant="secondary" onClick={()=>{setValue("");setCursor("");}}>В начало списка</Button>}
  </div>;
}

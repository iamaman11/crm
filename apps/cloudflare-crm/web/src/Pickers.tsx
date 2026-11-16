import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import {api} from "./api";
import type {Party} from "@ultimate-crm/client/cloudflare";
import {useQuery,QueryNotice} from "./hooks";
import {SearchField} from "./SearchField";

export function PartyPicker({name="partyId",label="Участник",initialParty}:{
  name?:string;label?:string;initialParty?:Pick<Party,"partyId"|"displayName">;
}) {
  const [cursor,setCursor]=useState("");
  const [value,setValue]=useState(initialParty?.partyId??"");
  const [search,setSearch]=useState("");
  const query=useQuery(useCallback(()=>api.partyRecords.listParties({page:{pageSize:100,pageToken:cursor},search}),[cursor,search]));
  return <div><SearchField label={"Поиск: "+label} onSearch={text=>{setSearch(text);setCursor("");setValue("");}}/>
    <SelectField label={label} name={name} value={value} required disabled={!initialParty&&(query.busy||!!query.error)} onChange={e=>setValue(e.target.value)}>
    <option value="">Выберите участника</option>
    {initialParty&&!query.data?.parties.some(p=>p.partyId===initialParty.partyId)&&
      <option value={initialParty.partyId}>{initialParty.displayName} · {initialParty.partyId}</option>}
    {!query.busy&&!query.error&&query.data?.parties.map(p=><option value={p.partyId} key={p.partyId}>{p.displayName} · {p.partyId}</option>)}
  </SelectField><QueryNotice query={query}/>
    {!query.busy&&!query.error&&query.data?.parties.length===0&&<p>Участники не найдены.</p>}
    {query.data?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={query.busy||!!query.error} onClick={()=>{setValue("");setCursor(query.data?.pageInfo?.nextPageToken??"");}}>Другие участники</Button>}
    {cursor&&<Button variant="secondary" onClick={()=>{setValue("");setCursor("");}}>В начало списка</Button>}
  </div>;
}
export function AccountPicker({onSelect}:{onSelect?:(id:string)=>void}={}) {
  const [cursor,setCursor]=useState("");
  const [value,setValue]=useState("");
  const [search,setSearch]=useState("");
  const query=useQuery(useCallback(()=>api.accounts.listAccounts({page:{pageSize:100,pageToken:cursor},search}),[cursor,search]));
  return <div><SearchField label="Поиск: Клиент" onSearch={text=>{setSearch(text);setCursor("");setValue("");onSelect?.("");}}/>
    <SelectField label="Клиент" name="accountId" value={value} required disabled={query.busy||!!query.error} onChange={e=>{setValue(e.target.value);onSelect?.(e.target.value);}}>
    <option value="">Выберите клиента</option>
    {!query.busy&&!query.error&&query.data?.accounts.map(a=><option value={a.accountId} key={a.accountId}>{a.name} · {a.accountId}</option>)}
  </SelectField><QueryNotice query={query}/>
    {!query.busy&&!query.error&&query.data?.accounts.length===0&&<p>Клиенты не найдены.</p>}
    {query.data?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={query.busy||!!query.error} onClick={()=>{setValue("");onSelect?.("");setCursor(query.data?.pageInfo?.nextPageToken??"");}}>Другие клиенты</Button>}
    {cursor&&<Button variant="secondary" onClick={()=>{setValue("");onSelect?.("");setCursor("");}}>В начало списка</Button>}
  </div>;
}

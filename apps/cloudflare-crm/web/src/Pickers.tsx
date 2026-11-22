import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import {api} from "./api";
import type {Party} from "@ultimate-crm/client/cloudflare";
import {useQuery,QueryNotice} from "./hooks";
import {SearchField} from "./SearchField";
import {accountHref,partyHref} from "./routes";

/** A picker retains opaque typed IDs for the RPC, but presents only authorized names. */
export function PartyPicker({name="partyId",label="Участник",initialParty,initialPartyId="",value:controlled,onSelect,required=true}:{
  name?:string;label?:string;initialParty?:Pick<Party,"partyId"|"displayName">;
  initialPartyId?:string;value?:string;onSelect?:(id:string)=>void;required?:boolean;
}) {
  const [cursor,setCursor]=useState("");
  const [chosen,setChosen]=useState(initialParty?.partyId??initialPartyId);
  const [search,setSearch]=useState("");
  const value=controlled??chosen;
  const query=useQuery(useCallback((signal:AbortSignal)=>api.partyRecords.listParties({
    page:{pageSize:100,pageToken:cursor},search
  },{signal}),[cursor,search]));
  const selected=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!value||initialParty?.partyId===value)return undefined;
    return (await api.partyRecords.getParty({partyId:value},{signal})).party;
  },[value,initialParty?.partyId]));
  const select=(id:string)=>{setChosen(id);onSelect?.(id);};
  const reset=()=>{setCursor("");select("");};
  const displayed=query.data?.parties.some(p=>p.partyId===value)??false;
  return <div>
    <SearchField label={"Поиск: "+label} onSearch={text=>{setSearch(text);reset();}}/>
    <SelectField label={label} name={name} value={value} required={required}
      disabled={!initialParty&&!value&&(query.busy||!!query.error)} onChange={e=>select(e.target.value)}>
      <option value="">{required?"Выберите участника":"Все участники"}</option>
      {value&&!displayed&&<option value={value}>
        {initialParty?.partyId===value?initialParty.displayName:selected.data?.displayName??"Выбранный участник"}
      </option>}
      {!query.busy&&!query.error&&query.data?.parties.map(p=>
        <option value={p.partyId} key={p.partyId}>{p.displayName}</option>)}
    </SelectField>
    <QueryNotice query={query}/>{value&&selected.error&&<QueryNotice query={selected}/>}
    {!query.busy&&!query.error&&query.data?.parties.length===0&&<p>Участники не найдены.</p>}
    {query.data?.pageInfo?.nextPageToken&&<Button variant="secondary"
      disabled={query.busy||!!query.error} onClick={()=>{setCursor(query.data?.pageInfo?.nextPageToken??"");select("");}}>
      Другие участники
    </Button>}
    {cursor&&<Button variant="secondary" onClick={reset}>В начало списка</Button>}
  </div>;
}

/** The same Account owner list is used for forms and named filters. */
export function AccountPicker({onSelect,initialAccountId="",value:controlled,required=true,label="Клиент"}:{
  onSelect?:(id:string)=>void;initialAccountId?:string;value?:string;required?:boolean;label?:string;
}={}) {
  const [cursor,setCursor]=useState("");
  const [chosen,setChosen]=useState(initialAccountId);
  const [search,setSearch]=useState("");
  const value=controlled??chosen;
  const query=useQuery(useCallback((signal:AbortSignal)=>api.accounts.listAccounts({
    page:{pageSize:100,pageToken:cursor},search
  },{signal}),[cursor,search]));
  const selected=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!value)return undefined;
    return (await api.accounts.getAccount({accountId:value},{signal})).account;
  },[value]));
  const select=(id:string)=>{setChosen(id);onSelect?.(id);};
  const reset=()=>{setCursor("");select("");};
  const displayed=query.data?.accounts.some(a=>a.accountId===value)??false;
  return <div>
    <SearchField label={"Поиск: "+label} onSearch={text=>{setSearch(text);reset();}}/>
    <SelectField label={label} name="accountId" value={value} required={required}
      disabled={query.busy||!!query.error} onChange={e=>select(e.target.value)}>
      <option value="">{required?"Выберите клиента":"Все клиенты"}</option>
      {value&&!displayed&&<option value={value}>{selected.data?.name??"Выбранный клиент"}</option>}
      {!query.busy&&!query.error&&query.data?.accounts.map(a=>
        <option value={a.accountId} key={a.accountId}>{a.name}</option>)}
    </SelectField>
    <QueryNotice query={query}/>{value&&selected.error&&<QueryNotice query={selected}/>}
    {!query.busy&&!query.error&&query.data?.accounts.length===0&&<p>Клиенты не найдены.</p>}
    {query.data?.pageInfo?.nextPageToken&&<Button variant="secondary"
      disabled={query.busy||!!query.error} onClick={()=>{setCursor(query.data?.pageInfo?.nextPageToken??"");select("");}}>
      Другие клиенты
    </Button>}
    {cursor&&<Button variant="secondary" onClick={reset}>В начало списка</Button>}
  </div>;
}

/** Names are read through authorized owner Get RPCs, never inferred from an ID. */
export function PartyName({partyId}:{partyId:string}) {
  const query=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.partyRecords.getParty({partyId},{signal})).party,[partyId]));
  if(query.error)return <span>Участник недоступен</span>;
  return query.data
    ?<a href={partyHref(partyId)}>{query.data.displayName}</a>
    :<span>Участник загружается</span>;
}

export function AccountName({accountId}:{accountId:string}) {
  const query=useQuery(useCallback(async(signal:AbortSignal)=>
    (await api.accounts.getAccount({accountId},{signal})).account,[accountId]));
  if(query.error)return <span>Клиент недоступен</span>;
  return query.data
    ?<a href={accountHref(accountId)}>{query.data.name}</a>
    :<span>Клиент загружается</span>;
}

import {useCallback,useEffect,useState} from "react";
import type {MouseEvent} from "react";
import {AccountStatus} from "@ultimate-crm/client/cloudflare";
import {Button,FeedbackPanel,RecordCard,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,newId} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {SearchField} from "./SearchField";
import {AccountDetails,AccountEditor,accountStatusLabel} from "./AccountRecord";
import {accountsHref,accountHref} from "./routes";
import type {RecordMode} from "@ultimate-crm/ui";

type AccountFilters={search:string;status:string;cursor:string};
const allowed=[AccountStatus.ACCOUNT_STATUS_ACTIVE,AccountStatus.ACCOUNT_STATUS_INACTIVE] as const;
function rememberAndNavigate(event:MouseEvent<HTMLAnchorElement>,accountId:string) {
  if(event.button!==0||event.ctrlKey||event.metaKey||event.shiftKey||event.altKey)return;
  event.preventDefault();
  const from=window.location.hash||"#accounts";
  window.history.replaceState({crmAccountScroll:window.scrollY},"",window.location.href);
  window.history.pushState({crmReturnTo:from},"",accountHref(accountId));
  window.dispatchEvent(new PopStateEvent("popstate"));
}
function canDiscard(dirty:boolean) {
  return !dirty||window.confirm("Есть несохранённые изменения. Закрыть редактирование без сохранения?");
}
export function AccountScreen({filters}:{filters:AccountFilters}) {
  const {search,status,cursor}=filters;
  const [expanded,setExpanded]=useState("");
  const [mode,setMode]=useState<"view"|"edit">("view");
  const [dirty,setDirty]=useState(false);
  const [savedNotice,setSavedNotice]=useState("");
  const [createOpen,setCreateOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.accounts.listAccounts({
    page:{pageSize:25,pageToken:cursor},search,
    ...(status?{status:enumValue(status,allowed,"Фильтр статуса")}:{})
  }),[cursor,status,search]));
  const navigate=(next:Partial<AccountFilters>)=>{
    if(!canDiscard(dirty))return;
    setDirty(false);setExpanded("");setSavedNotice("");
    window.location.hash=accountsHref({...filters,...next});
  };
  const expand=(id:string,nextMode:"view"|"edit")=>{
    if((expanded!==id||mode!==nextMode)&&!canDiscard(dirty))return;
    setDirty(false);setSavedNotice("");
    if(expanded===id&&mode==="view"&&nextMode==="view")setExpanded("");
    else {setExpanded(id);setMode(nextMode);}
  };
  useEffect(()=>{
    if(query.busy||!query.data)return;
    const state:unknown=window.history.state;
    if(state&&typeof state==="object"&&"crmAccountScroll" in state&&typeof state.crmAccountScroll==="number") {
      const top=state.crmAccountScroll;
      requestAnimationFrame(()=>window.scrollTo(0,top));
    }
  },[query.busy,query.data]);
  return <>
    <PageHeader title="Клиенты" description="Клиент связывается с человеком или организацией из раздела участников."
      actions={<Button onClick={()=>{
        if(!canDiscard(dirty))return;
        setExpanded("");setDirty(false);setCreateOpen(true);mutation.changed();
      }}>Создать клиента</Button>}/>
    <SearchField label="Поиск клиентов" submitted={search} onSearch={value=>navigate({search:value,cursor:""})}/>
    <SelectField label="Статус клиента" value={status} onChange={event=>navigate({status:event.target.value,cursor:""})}>
      <option value="">Все</option>
      <option value={AccountStatus.ACCOUNT_STATUS_ACTIVE}>Активные</option>
      <option value={AccountStatus.ACCOUNT_STATUS_INACTIVE}>Неактивные</option>
    </SelectField>
    <QueryNotice query={query}/>
    {savedNotice&&<FeedbackPanel tone="success" title={savedNotice}/>}
    {!query.busy&&!query.error&&query.data?.accounts.map(a=>{
      const active=expanded===a.accountId;
      const current:RecordMode=active?mode:"collapsed";
      return <RecordCard key={a.accountId} title={a.name} mode={current}
        summary={<>{accountStatusLabel(a.status)}</>}
        onToggle={()=>expand(a.accountId,"view")}
        actions={<>
          <Button variant="secondary" onClick={()=>expand(a.accountId,"edit")}>Редактировать</Button>
          <a href={accountHref(a.accountId)} onClick={event=>{
            if(!canDiscard(dirty)){event.preventDefault();return;}
            rememberAndNavigate(event,a.accountId);
          }}>Полная страница</a>
        </>}>
        {active&&(mode==="edit"?<AccountEditor key={a.accountId} accountId={a.accountId}
          onDirtyChange={setDirty} onCancel={()=>expand(a.accountId,"view")}
          onSaved={async()=>{await query.reload();setSavedNotice("Сохранено.");setMode("view");}}/>:
          <AccountDetails key={a.accountId} accountId={a.accountId}/>)}
      </RecordCard>;
    })}
    {!query.busy&&!query.error&&query.data?.accounts.length===0&&<p>{search?"Клиенты не найдены.":"Клиентов пока нет."}</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
      onFirst={()=>navigate({cursor:""})} onNext={()=>navigate({cursor:query.data?.pageInfo?.nextPageToken??""})}/>
    <RecordPanel open={createOpen} title="Новый клиент" onClose={()=>setCreateOpen(false)}>
      <form key={draftId} onChange={mutation.changed} onSubmit={event=>{
        event.preventDefault();
        const data=new FormData(event.currentTarget);
        void mutation.run(()=>api.accounts.createAccount({
          accountId:draftId,name:field(data,"name"),primaryPartyId:field(data,"partyId"),
          precondition:{idempotencyKey:mutation.key}
        }),async()=>{setCreateOpen(false);setDraftId(newId());await query.reload();});
      }}>
        <FormGrid><TextField label="Название клиента" name="name" required/><PartyPicker label="Основной участник"/></FormGrid>
        <Button type="submit" disabled={mutation.busy}>Сохранить клиента</Button>
      </form>
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}
export function AccountPage({accountId}:{accountId:string}) {
  const [mode,setMode]=useState<"view"|"edit">("view");
  const [dirty,setDirty]=useState(false);
  const [revision,setRevision]=useState(0);
  const [savedNotice,setSavedNotice]=useState("");
  useEffect(()=>{
    if(!dirty)return;
    const unload=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};
    const hashChanged=(event:HashChangeEvent)=>{
      if(!window.confirm("Есть несохранённые изменения. Покинуть страницу?")){
        window.history.replaceState(window.history.state,"",event.oldURL);
        window.dispatchEvent(new PopStateEvent("popstate"));
      }
    };
    window.addEventListener("beforeunload",unload);
    window.addEventListener("hashchange",hashChanged);
    return ()=>{window.removeEventListener("beforeunload",unload);window.removeEventListener("hashchange",hashChanged);};
  },[dirty]);
  const goBack=()=>{
    if(!canDiscard(dirty))return;
    const state:unknown=window.history.state;
    if(state&&typeof state==="object"&&"crmReturnTo" in state&&typeof state.crmReturnTo==="string")window.history.back();
    else window.location.hash=accountsHref();
  };
  return <>
    <PageHeader title="Карточка клиента" description="Полная страница того же клиента."
      actions={<Button variant="secondary" onClick={goBack}>К списку клиентов</Button>}/>
    {savedNotice&&<FeedbackPanel tone="success" title={savedNotice}/>}
    <div><Button variant="secondary" onClick={()=>{
      if(mode==="edit"&&!canDiscard(dirty))return;
      setDirty(false);setMode(mode==="edit"?"view":"edit");
    }}>{mode==="edit"?"Просмотр":"Редактировать"}</Button></div>
    {mode==="edit"?<AccountEditor key={accountId} accountId={accountId}
      onDirtyChange={setDirty} onCancel={()=>{if(canDiscard(dirty)){setDirty(false);setMode("view");}}}
      onSaved={async()=>{setSavedNotice("Сохранено.");setRevision(value=>value+1);setMode("view");}}/>:
      <AccountDetails key={accountId+":"+revision} accountId={accountId}/>}
  </>;
}

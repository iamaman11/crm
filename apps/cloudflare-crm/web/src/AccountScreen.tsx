import {useCallback,useEffect,useLayoutEffect,useState} from "react";
import {AccountStatus} from "@ultimate-crm/client/cloudflare";
import {Button,FeedbackPanel,RecordCard,RecordPanel,PageHeader,SelectField} from "@ultimate-crm/ui";
import {api} from "./api";
import {useQuery,QueryNotice,Paging,hasPendingRecovery,pendingRecoveryResourceId} from "./hooks";
import {SearchField} from "./SearchField";
import {AccountDetails,AccountEditor,AccountCreator,accountStatusLabel} from "./AccountRecord";
import {accountsHref,accountHref,accountFilterStatus} from "./routes";
import type {AccountFilters} from "./routes";
import {navigateTo,openAccountFromList,returnToList,savedAccountListScroll,returnToParty,replaceCommittedAccount} from "./navigation";
import type {RecordMode} from "@ultimate-crm/ui";

type RegisterBlocker=(blocker:(()=>boolean)|null)=>void;
const dirtyPrompt="Есть несохранённые изменения. Покинуть страницу без сохранения?";
export const accountFilterStatuses=[AccountStatus.ACTIVE,AccountStatus.INACTIVE] as const;
function canDiscard(dirty:boolean) {
  return !dirty||window.confirm("Есть несохранённые изменения. Закрыть редактирование без сохранения?");
}
export function AccountScreen({filters,registerBlocker}:{filters:AccountFilters;registerBlocker:RegisterBlocker}) {
  const {search,status,cursor}=filters;
  const [expanded,setExpanded]=useState(()=>pendingRecoveryResourceId("crm.account.update.pending.v1")??"");
  const [mode,setMode]=useState<"view"|"edit">(()=>hasPendingRecovery("crm.account.update.pending.v1")?"edit":"view");
  const [dirty,setDirty]=useState(false);
  const [savedNotice,setSavedNotice]=useState("");
  const [createOpen,setCreateOpen]=useState(()=>hasPendingRecovery("crm.account.create.pending.v1"));
  const [createDirty,setCreateDirty]=useState(false);
  const [createWarning,setCreateWarning]=useState("");
  useLayoutEffect(()=>{
    registerBlocker(dirty||createOpen&&createDirty?()=>window.confirm(dirtyPrompt):null);
    return ()=>registerBlocker(null);
  },[dirty,createOpen,createDirty,registerBlocker]);
  const query=useQuery(useCallback((signal:AbortSignal)=>api.accounts.listAccounts({
    page:{pageSize:25,pageToken:cursor},search,
    ...(status!==null?{status}:{})
  },{signal}),[cursor,status,search]));
  const navigate=(next:Partial<AccountFilters>)=>{
    if(!navigateTo(accountsHref({...filters,...next})))return;
    setDirty(false);setExpanded("");setSavedNotice("");
  };
  const expand=(id:string,nextMode:"view"|"edit")=>{
    if(hasPendingRecovery("crm.account.update.pending.v1")&&(expanded!==id||nextMode!=="edit"))return;
    if((expanded!==id||mode!==nextMode)&&!canDiscard(dirty))return;
    setDirty(false);setSavedNotice("");
    if(expanded===id&&mode==="view"&&nextMode==="view")setExpanded("");
    else {setExpanded(id);setMode(nextMode);}
  };
  const collapse=()=>{
    if(hasPendingRecovery("crm.account.update.pending.v1")||!canDiscard(dirty))return;
    setDirty(false);setSavedNotice("");setExpanded("");
  };
  useEffect(()=>{
    if(query.busy||!query.data)return;
    const top=savedAccountListScroll();
    if(top!==null)requestAnimationFrame(()=>window.scrollTo(0,top));
  },[query.busy,query.data]);
  return <>
    <PageHeader title="Клиенты" description="Клиент связывается с человеком или организацией из раздела участников."
      actions={<Button onClick={()=>{
        if(!canDiscard(dirty))return;
        setExpanded("");setDirty(false);setCreateDirty(false);setCreateWarning("");setCreateOpen(true);
      }}>Создать клиента</Button>}/>
    <SearchField label="Поиск клиентов" submitted={search} onSearch={value=>navigate({search:value,cursor:""})}/>
    <SelectField label="Статус клиента" value={status??""} onChange={event=>navigate({status:accountFilterStatus(event.target.value,accountFilterStatuses),cursor:""})}>
      <option value="">Все</option>
      <option value={AccountStatus.ACTIVE}>Активные</option>
      <option value={AccountStatus.INACTIVE}>Неактивные</option>
    </SelectField>
    <QueryNotice query={query}/>
    {savedNotice&&<FeedbackPanel tone="success" title={savedNotice}/>}
    {createWarning&&<FeedbackPanel tone="warning" title={createWarning}/>}
    {!query.busy&&!query.error&&query.data?.accounts.map(a=>{
      const active=expanded===a.accountId;
      const current:RecordMode=active?mode:"collapsed";
      return <RecordCard key={a.accountId} title={a.name} mode={current}
        summary={<>{accountStatusLabel(a.status)}</>}
        onToggle={()=>active?collapse():expand(a.accountId,"view")}
        actions={<>
          <Button variant="secondary" onClick={()=>expand(a.accountId,"edit")}>Редактировать</Button>
          <a href={accountHref(a.accountId)} onClick={event=>{
            if(event.button!==0||event.ctrlKey||event.metaKey||event.shiftKey||event.altKey)return;
            event.preventDefault();
            if(openAccountFromList(accountHref(a.accountId)))setDirty(false);
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
    {/* A restored in-flight account may be outside the active filter/page.
        Its original GetAccount receipt must still be reachable without a new write. */}
    {mode==="edit"&&expanded&&hasPendingRecovery("crm.account.update.pending.v1")&&
      !query.data?.accounts.some(account=>account.accountId===expanded)&&
      <section aria-label="Восстановление изменения клиента">
        <AccountEditor key={expanded} accountId={expanded} onDirtyChange={setDirty}
          onCancel={()=>{}} onSaved={async()=>{await query.reload();setMode("view");setExpanded("");}}/>
      </section>}
    <RecordPanel open={createOpen} dirty={createDirty} title="Новый клиент" onClose={()=>{
      if(hasPendingRecovery("crm.account.create.pending.v1")||!canDiscard(createDirty))return;
      setCreateDirty(false);setCreateOpen(false);
    }}>
      <AccountCreator onDirtyChange={setCreateDirty} onCancel={()=>{
        if(hasPendingRecovery("crm.account.create.pending.v1")||!canDiscard(createDirty))return;
        setCreateDirty(false);setCreateOpen(false);
      }} onSaved={async()=>{
        setCreateDirty(false);setCreateOpen(false);setSavedNotice("Сохранено. Клиент создан.");
        try {await query.reload();}
        catch(error){setCreateWarning("Клиент создан, но список устарел: "+api.describeError(error));}
      }}/>
    </RecordPanel>
  </>;
}
export function AccountPage({accountId,registerBlocker}:{accountId:string;registerBlocker:RegisterBlocker}) {
  const [mode,setMode]=useState<"view"|"edit">(()=>pendingRecoveryResourceId("crm.account.update.pending.v1")===accountId?"edit":"view");
  const [dirty,setDirty]=useState(false);
  const [revision,setRevision]=useState(0);
  const [savedNotice,setSavedNotice]=useState("");
  useLayoutEffect(()=>{
    registerBlocker(dirty?()=>window.confirm(dirtyPrompt):null);
    if(!dirty)return ()=>registerBlocker(null);
    const unload=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};
    window.addEventListener("beforeunload",unload);
    return ()=>{registerBlocker(null);window.removeEventListener("beforeunload",unload);};
  },[dirty,registerBlocker]);
  const goBack=()=>{
    if(returnToList(accountsHref()))setDirty(false);
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

/** Party-origin creation uses the same AccountCreator as the Account list. */
export function AccountCreatePage({partyId,registerBlocker}:{
  partyId:string;registerBlocker:RegisterBlocker;
}) {
  const [dirty,setDirty]=useState(false);
  const query=useQuery(useCallback((signal:AbortSignal)=>api.partyRecords.getParty({partyId},{signal}),[partyId]));
  useLayoutEffect(()=>{
    registerBlocker(dirty?()=>window.confirm(dirtyPrompt):null);
    if(!dirty)return ()=>registerBlocker(null);
    const unload=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};
    window.addEventListener("beforeunload",unload);
    return ()=>{registerBlocker(null);window.removeEventListener("beforeunload",unload);};
  },[dirty,registerBlocker]);
  const cancel=()=>{if(returnToParty(partyId))setDirty(false);};
  return <>
    <PageHeader title="Новый клиент" description="Создание клиента на основе выбранного человека или организации."
      actions={<Button variant="secondary" onClick={cancel}>К участнику</Button>}/>
    <QueryNotice query={query}/>
    {!query.busy&&!query.error&&!query.data?.party&&<p>Участник не найден.</p>}
    {query.data?.party&&<AccountCreator key={partyId} initialParty={query.data.party}
      onDirtyChange={setDirty} onCancel={cancel} onSaved={accountId=>{
        setDirty(false);
        registerBlocker(null);
        replaceCommittedAccount(accountHref(accountId));
      }}/>}
  </>;
}

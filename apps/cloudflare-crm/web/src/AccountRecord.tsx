import {useCallback,useState} from "react";
import {Button,FormGrid,SelectField,TextField} from "@ultimate-crm/ui";
import {AccountStatus,TaskStatus} from "@ultimate-crm/client/cloudflare";
import type {Account,AccountOverview,Deal,Party} from "@ultimate-crm/client/cloudflare";
import {api,field,dateText,newId} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {scopeHref,partyHref,dealHref,taskHref,contactHref,relationshipHref} from "./routes";
import {PartyPicker,PartyName} from "./Pickers";
import {allPipelines,stageName} from "./salesPipelineCatalog";
import {InteractionTimeline} from "./InteractionTimeline";
import {allOwners,ownerLabel} from "./ownerCatalog";

export function accountStatusLabel(status:AccountStatus):string {
  switch(status) {
    case AccountStatus.ACTIVE:return "Активен";
    case AccountStatus.INACTIVE:return "Неактивен";
    default:return "Неизвестный статус";
  }
}
// One owner-authorized stage catalog read per rendered Account Deal list, not
// a request per Deal and not another Client360 projection. Original typed Deal
// refs and Account ID remain unchanged; if the catalog is missing we preserve
// the stored immutable stage ID rather than claiming a fabricated stage name.
function AccountDeals({deals,choices}:{deals:readonly Deal[];choices:ReadonlyArray<{value:string;label:string}>}) {
  const catalog=useQuery(useCallback(allPipelines,[]));
  const pipelines=catalog.data?.pipelines??[];
  return <>
    <QueryNotice query={catalog}/>
    <ul>{deals.map(deal=>{
      const pipeline=pipelines.find(item=>item.pipelineId===deal.stage?.pipelineId);
      const stage=deal.stage?.stageId?stageName(pipeline,deal.stage.stageId):"неизвестен";
      return <li key={deal.dealId}>
        <a href={dealHref(deal.dealId)}>{deal.name}</a>
        {" · "}{catalog.busy&&!catalog.data?"Этапы загружаются…":
          "Воронка: "+(pipeline?.name??deal.stage?.pipelineId??"неизвестна")+
          " · этап: "+stage}
        <span>{" · Ответственный: "+ownerLabel(deal.owner,choices)}</span>
      </li>;
    })}</ul>
  </>;
}
/** Both inline and full-page views use precisely the same read-only Customer360 projection. */
export function AccountDetails({accountId}:{accountId:string}) {
  const query=useQuery(useCallback((signal:AbortSignal)=>api.overview.getAccountOverview({accountId},{signal}),[accountId]));
  const card:AccountOverview|undefined=query.error?undefined:query.data?.overview;
  const owners=useQuery(useCallback(allOwners,[]));
  const choices=owners.data?.choices??[];
  return <div><QueryNotice query={query}/>{!query.error&&card&&<>
    <h3>{card.account?.name}</h3>
    <p>Статус: {card.account?accountStatusLabel(card.account.status):"Неизвестно"}</p>
    <p>ID: {accountId} · версия {card.account?.version.toString()??"—"}</p>
    <p>Основной участник: {card.primaryParty?<a href={partyHref(card.primaryParty.partyId)}>{card.primaryParty.displayName}</a>:"Не указан"}</p>
    <h3>Контакты</h3><ul>{card.contactPoints.map(p=><li key={p.contactPointId}>
      <a href={contactHref(p.contactPointId,p.partyId)}>{p.displayValue}</a>{p.verified?" · подтверждён":""}</li>)}</ul>
    {card.truncated?.contactPoints&&<p>Показаны первые {card.perCollectionLimit} контактов.</p>}
    {card.primaryParty&&<a href={scopeHref("contacts",card.primaryParty.partyId)}>Все контакты участника</a>}
    <h3>Связи</h3><ul>{card.relationships.map(r=><li key={r.relationshipId}>
      <a href={relationshipHref(r.relationshipId,card.primaryParty?.partyId??r.fromPartyId)}>
        {r.type?.code??"Связь"}</a>: <PartyName partyId={r.fromPartyId}/> → <PartyName partyId={r.toPartyId}/></li>)}</ul>
    {card.truncated?.relationships&&<p>Показаны первые {card.perCollectionLimit} связей.</p>}
    {card.primaryParty&&<a href={scopeHref("relationships",card.primaryParty.partyId)}>Все связи участника</a>}
    <QueryNotice query={owners}/>
    <h3>Сделки</h3>{card.deals.length>0&&<AccountDeals deals={card.deals} choices={choices}/>} 
    {card.truncated?.deals&&<p>Показаны первые {card.perCollectionLimit} сделок.</p>}
    <a href={scopeHref("sales",accountId)}>Все сделки клиента</a>
    <h3>Задачи</h3><ul>{card.tasks.map(t=><li key={t.taskId}><a href={taskHref(t.taskId,accountId)}>{t.subject}</a> · {t.status===TaskStatus.COMPLETED?"Выполнена":t.status===TaskStatus.OPEN?"Открыта"+(t.dueAt?" · срок: "+dateText(t.dueAt):""):"Неизвестный статус"}
      <span>{" · Ответственный: "+ownerLabel(t.owner,choices)}</span></li>)}</ul>
    {card.truncated?.tasks&&<p>Показаны первые {card.perCollectionLimit} задач.</p>}
    <a href={scopeHref("tasks",accountId)}>Все задачи клиента</a>
    <InteractionTimeline accountId={accountId}/>
  </>}</div>;
}
/** A single typed editor, GetAccount and UpdateAccount implementation for every Account projection. */
export function AccountEditor({accountId,onSaved,onCancel,onDirtyChange}:{
  accountId:string;onSaved:()=>Promise<unknown>;onCancel:()=>void;onDirtyChange:(dirty:boolean)=>void;
}) {
  const query=useQuery(useCallback((signal:AbortSignal)=>api.accounts.getAccount({accountId},{signal}),[accountId]));
  const mutation=useMutation({
    storageKey:"crm.account.update.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.accounts.getAccount({
      accountId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async()=>{onDirtyChange(false);await query.reload();await onSaved();}
  });
  const account:Account|undefined=query.data?.account;
  return <div><QueryNotice query={query}/>
    {account&&<form key={account.accountId+":"+account.version.toString()}
      onChange={()=>{mutation.changed();onDirtyChange(true);}}
      onSubmit={event=>{
        event.preventDefault();
        const data=new FormData(event.currentTarget);
        void mutation.runIntent(()=>prepareMutation(api.accounts.updateAccount,{
          accountId:account.accountId,name:field(data,"name"),
          status:enumValue(field(data,"status"),[
            AccountStatus.ACTIVE,AccountStatus.INACTIVE
          ],"Статус клиента"),
          precondition:{idempotencyKey:mutation.key,expectedVersion:account.version}
        },account.accountId),async()=>{
          // A committed RPC is no longer a dirty draft, even if the readback is slow.
          // Clear the navigation guard before the first awaited refresh.
          onDirtyChange(false);
          await query.reload();
          await onSaved();
        });
      }}>
      <FormGrid>
        <TextField label="Название клиента" name="name" defaultValue={account.name} required/>
        <SelectField label="Статус" name="status" defaultValue={account.status}>
          <option value={AccountStatus.ACTIVE}>Активен</option>
          <option value={AccountStatus.INACTIVE}>Неактивен</option>
        </SelectField>
      </FormGrid>
      <Button type="submit" disabled={mutation.busy||mutation.pending}>Сохранить клиента</Button>{" "}
      <Button type="button" variant="secondary" disabled={mutation.busy||mutation.pending} onClick={onCancel}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
  </div>;
}

/** The only CreateAccount form and command, shared by list and Party-origin creation. */
export function AccountCreator({initialParty,onSaved,onCancel,onDirtyChange}:{
  initialParty?:Pick<Party,"partyId"|"displayName">;
  onSaved:(accountId:string)=>Promise<unknown>|unknown;
  onCancel:()=>void;
  onDirtyChange?:(dirty:boolean)=>void;
}) {
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation({
    storageKey:"crm.account.create.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.accounts.getAccount({
      accountId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async(marker)=>{onDirtyChange?.(false);await onSaved(marker.resourceId);}
  });
  return <>
    <form key={draftId} onChange={()=>{
      mutation.changed();
      onDirtyChange?.(true);
    }} onSubmit={event=>{
      event.preventDefault();
      const data=new FormData(event.currentTarget);
      void mutation.runIntent(()=>prepareMutation(api.accounts.createAccount,{
        accountId:draftId,
        name:field(data,"name"),
        primaryPartyId:field(data,"partyId"),
        precondition:{idempotencyKey:mutation.key}
      },draftId),async()=>{
        // The write is committed: navigation must not treat a successful save as a dirty draft.
        onDirtyChange?.(false);
        await onSaved(draftId);
        setDraftId(newId());
      });
    }}>
      <FormGrid>
        <TextField label="Название клиента" name="name" defaultValue={initialParty?.displayName??""} required/>
        <PartyPicker label="Основной участник" {...(initialParty?{initialParty}:{})}/>
      </FormGrid>
      <Button type="submit" disabled={mutation.busy||mutation.pending}>Сохранить клиента</Button>{" "}
      <Button type="button" variant="secondary" disabled={mutation.busy||mutation.pending} onClick={onCancel}>Отмена</Button>
    </form>
    <MutationNotice state={mutation}/>
  </>;
}

import {useCallback,useState} from "react";
import {Button,FormGrid,SelectField,TextField} from "@ultimate-crm/ui";
import {AccountStatus,TaskStatus} from "@ultimate-crm/client/cloudflare";
import type {Account,AccountOverview,Party} from "@ultimate-crm/client/cloudflare";
import {api,field,dateText,newId} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice} from "./hooks";
import {scopeHref} from "./routes";
import {PartyPicker} from "./Pickers";

export function accountStatusLabel(status:AccountStatus):string {
  switch(status) {
    case AccountStatus.ACTIVE:return "Активен";
    case AccountStatus.INACTIVE:return "Неактивен";
    default:return "Неизвестный статус";
  }
}
/** Both inline and full-page views use precisely the same read-only Customer360 projection. */
export function AccountDetails({accountId}:{accountId:string}) {
  const query=useQuery(useCallback((signal:AbortSignal)=>api.overview.getAccountOverview({accountId},{signal}),[accountId]));
  const card:AccountOverview|undefined=query.error?undefined:query.data?.overview;
  return <div><QueryNotice query={query}/>{!query.error&&card&&<>
    <h3>{card.account?.name}</h3>
    <p>Статус: {card.account?accountStatusLabel(card.account.status):"Неизвестно"}</p>
    <p>ID: {accountId} · версия {card.account?.version.toString()??"—"}</p>
    <p>Основной участник: {card.primaryParty?.displayName}</p>
    <h3>Контакты</h3><ul>{card.contactPoints.map(p=><li key={p.contactPointId}>{p.displayValue}{p.verified?" · подтверждён":""}</li>)}</ul>
    {card.truncated?.contactPoints&&<p>Показаны первые {card.perCollectionLimit} контактов.</p>}
    {card.primaryParty&&<a href={scopeHref("contacts",card.primaryParty.partyId)}>Все контакты участника</a>}
    <h3>Связи</h3><ul>{card.relationships.map(r=><li key={r.relationshipId}>{r.type?.code}: {r.fromPartyId} → {r.toPartyId}</li>)}</ul>
    {card.truncated?.relationships&&<p>Показаны первые {card.perCollectionLimit} связей.</p>}
    {card.primaryParty&&<a href={scopeHref("relationships",card.primaryParty.partyId)}>Все связи участника</a>}
    <h3>Сделки</h3><ul>{card.deals.map(d=><li key={d.dealId}>{d.name} · {d.stage?.stageId}</li>)}</ul>
    {card.truncated?.deals&&<p>Показаны первые {card.perCollectionLimit} сделок.</p>}
    <a href={scopeHref("sales",accountId)}>Все сделки клиента</a>
    <h3>Задачи</h3><ul>{card.tasks.map(t=><li key={t.taskId}>{t.subject} · {t.status===TaskStatus.COMPLETED?"Выполнена":t.status===TaskStatus.OPEN?dateText(t.dueAt):"Неизвестный статус"}</li>)}</ul>
    {card.truncated?.tasks&&<p>Показаны первые {card.perCollectionLimit} задач.</p>}
    <a href={scopeHref("tasks",accountId)}>Все задачи клиента</a>
  </>}</div>;
}
/** A single typed editor, GetAccount and UpdateAccount implementation for every Account projection. */
export function AccountEditor({accountId,onSaved,onCancel,onDirtyChange}:{
  accountId:string;onSaved:()=>Promise<unknown>;onCancel:()=>void;onDirtyChange:(dirty:boolean)=>void;
}) {
  const query=useQuery(useCallback((signal:AbortSignal)=>api.accounts.getAccount({accountId},{signal}),[accountId]));
  const mutation=useMutation();
  const account:Account|undefined=query.data?.account;
  return <div><QueryNotice query={query}/>
    {account&&<form key={account.accountId+":"+account.version.toString()}
      onChange={()=>{mutation.changed();onDirtyChange(true);}}
      onSubmit={event=>{
        event.preventDefault();
        const data=new FormData(event.currentTarget);
        void mutation.run(()=>api.accounts.updateAccount({
          accountId:account.accountId,name:field(data,"name"),
          status:enumValue(field(data,"status"),[
            AccountStatus.ACTIVE,AccountStatus.INACTIVE
          ],"Статус клиента"),
          precondition:{idempotencyKey:mutation.key,expectedVersion:account.version}
        }),async()=>{
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
      <Button type="submit" disabled={mutation.busy||mutation.notice?.tone==="warning"}>Сохранить клиента</Button>{" "}
      <Button type="button" variant="secondary" disabled={mutation.busy} onClick={onCancel}>Отмена</Button>
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
  const mutation=useMutation();
  return <>
    <form key={draftId} onChange={()=>{
      mutation.changed();
      onDirtyChange?.(true);
    }} onSubmit={event=>{
      event.preventDefault();
      const data=new FormData(event.currentTarget);
      void mutation.run(()=>api.accounts.createAccount({
        accountId:draftId,
        name:field(data,"name"),
        primaryPartyId:field(data,"partyId"),
        precondition:{idempotencyKey:mutation.key}
      }),async()=>{
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
      <Button type="submit" disabled={mutation.busy}>Сохранить клиента</Button>{" "}
      <Button type="button" variant="secondary" disabled={mutation.busy} onClick={onCancel}>Отмена</Button>
    </form>
    <MutationNotice state={mutation}/>
  </>;
}

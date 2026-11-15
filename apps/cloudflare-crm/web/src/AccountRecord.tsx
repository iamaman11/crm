import {useCallback} from "react";
import {Button,FormGrid,SelectField,TextField} from "@ultimate-crm/ui";
import {AccountStatus,TaskStatus} from "@ultimate-crm/client/cloudflare";
import type {Account,AccountOverview} from "@ultimate-crm/client/cloudflare";
import {api,field,dateText} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice} from "./hooks";
import {scopeHref} from "./routes";

export function accountStatusLabel(status:AccountStatus):string {
  switch(status) {
    case AccountStatus.ACCOUNT_STATUS_ACTIVE:return "Активен";
    case AccountStatus.ACCOUNT_STATUS_INACTIVE:return "Неактивен";
    default:return "Неизвестный статус";
  }
}
/** Both inline and full-page views use precisely the same read-only Customer360 projection. */
export function AccountDetails({accountId}:{accountId:string}) {
  const query=useQuery(useCallback(()=>api.overview.getAccountOverview({accountId}),[accountId]));
  const card:AccountOverview|undefined=query.data?.overview;
  return <div><QueryNotice query={query}/>{card&&<>
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
    <h3>Задачи</h3><ul>{card.tasks.map(t=><li key={t.taskId}>{t.subject} · {t.status===TaskStatus.TASK_STATUS_COMPLETED?"Выполнена":t.status===TaskStatus.TASK_STATUS_OPEN?dateText(t.dueAt):"Неизвестный статус"}</li>)}</ul>
    {card.truncated?.tasks&&<p>Показаны первые {card.perCollectionLimit} задач.</p>}
    <a href={scopeHref("tasks",accountId)}>Все задачи клиента</a>
  </>}</div>;
}
/** A single typed editor, GetAccount and UpdateAccount implementation for every Account projection. */
export function AccountEditor({accountId,onSaved,onCancel,onDirtyChange}:{
  accountId:string;onSaved:()=>Promise<unknown>;onCancel:()=>void;onDirtyChange:(dirty:boolean)=>void;
}) {
  const query=useQuery(useCallback(()=>api.accounts.getAccount({accountId}),[accountId]));
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
            AccountStatus.ACCOUNT_STATUS_ACTIVE,AccountStatus.ACCOUNT_STATUS_INACTIVE
          ],"Статус клиента"),
          precondition:{idempotencyKey:mutation.key,expectedVersion:account.version}
        }),async()=>{
          await Promise.all([query.reload(),onSaved()]);
          onDirtyChange(false);
        });
      }}>
      <FormGrid>
        <TextField label="Название клиента" name="name" defaultValue={account.name} required/>
        <SelectField label="Статус" name="status" defaultValue={account.status}>
          <option value={AccountStatus.ACCOUNT_STATUS_ACTIVE}>Активен</option>
          <option value={AccountStatus.ACCOUNT_STATUS_INACTIVE}>Неактивен</option>
        </SelectField>
      </FormGrid>
      <Button type="submit" disabled={mutation.busy}>Сохранить клиента</Button>{" "}
      <Button type="button" variant="secondary" disabled={mutation.busy} onClick={onCancel}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
  </div>;
}

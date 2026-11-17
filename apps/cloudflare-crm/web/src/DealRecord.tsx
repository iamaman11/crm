import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import type {ContactPoint,Deal} from "@ultimate-crm/client/cloudflare";
import {DealEventKind} from "@ultimate-crm/client/cloudflare";
import {ContactPointStatus,DealStatus} from "@ultimate-crm/client/cloudflare";
import {api,dateText} from "./api";
import {useQuery,QueryNotice} from "./hooks";
import {moneyText,percent} from "./money";
import {accountHref,scopeHref,taskHref} from "./routes";

export type Calendar={year:number;month:number;day:number};
export function inputDate(date:Calendar|undefined):string {
  return date?String(date.year).padStart(4,"0")+"-"+String(date.month).padStart(2,"0")+
    "-"+String(date.day).padStart(2,"0"):"";
}
export function parseDate(value:string):Calendar|undefined {
  if(!value)return undefined;
  if(!/^\d{4}-\d{2}-\d{2}$/.test(value))throw Error("Неверная дата закрытия");
  const [year,month,day]=value.split("-").map(Number);
  if(year===undefined||month===undefined||day===undefined||year<1||year>9999)
    throw Error("Неверная дата закрытия");
  const leap=year%4===0&&(year%100!==0||year%400===0);
  const maximum=[31,leap?29:28,31,30,31,30,31,31,30,31,30,31][month-1];
  if(maximum===undefined||day<1||day>maximum)throw Error("Неверная дата закрытия");
  return {year,month,day};
}

// Presentation-only picker. Original Account/ContactPoint owners provide
// choices; original crm-sales Rust validates tenant, Party, Account and status.
export function DealContactPicker({accountId,initialContactId=""}:{
  accountId:string;initialContactId?:string;
}) {
  const [value,setValue]=useState(initialContactId);
  const contacts=useQuery(useCallback(async()=>{
    if(!accountId)return [] as ContactPoint[];
    const account=(await api.accounts.getAccount({accountId})).account;
    if(!account)throw Error("Клиент не найден");
    const result:ContactPoint[]=[];
    for(const party of account.parties) {
      let token="";
      const seen=new Set<string>();
      do {
        const page=await api.contactPoints.listContactPoints({
          partyId:party.partyId,page:{pageSize:100,pageToken:token}
        });
        result.push(...page.contactPoints.filter(point=>point.status===ContactPointStatus.ACTIVE));
        token=page.pageInfo?.nextPageToken??"";
        if(token&&seen.has(token))throw Error("Повторный курсор контактов");
        if(token)seen.add(token);
      }while(token);
    }
    return result;
  },[accountId]));
  return <div>
    <SelectField label="Основной контакт сделки" name="primaryContactId"
      value={value} disabled={!accountId||contacts.busy||!!contacts.error}
      onChange={event=>setValue(event.target.value)}>
      <option value="">Без основного контакта</option>
      {contacts.data?.map(p=><option key={p.contactPointId} value={p.contactPointId}>
        {p.displayValue} · {p.contactPointId}
      </option>)}
      {value&&!contacts.busy&&!contacts.data?.some(p=>p.contactPointId===value)&&
        <option value={value}>Ранее выбранный контакт (недоступен) · {value}</option>}
    </SelectField>
    <QueryNotice query={contacts}/>
    {!accountId&&<p>Сначала выберите клиента.</p>}
  </div>;
}

function eventLabel(kind:DealEventKind):string {
  switch(kind){
    case DealEventKind.CREATED:return "Сделка создана";
    case DealEventKind.UPDATED:return "Поля обновлены";
    case DealEventKind.MOVED:return "Изменён этап";
    case DealEventKind.CLOSED:return "Сделка завершена";
    case DealEventKind.REOPENED:return "Сделка повторно открыта";
    default:return "Неизвестное событие";
  }
}
// One original, owner-authorized event projection for inline/full Deal pages.
// No reconstruction of history from mutable client-side snapshots.
export function DealHistory({dealId,accountId}:{dealId:string;accountId:string}){
  const [cursor,setCursor]=useState("");
  const history=useQuery(useCallback(
    ()=>api.deals.listDealEvents({dealId,page:{pageSize:25,pageToken:cursor}}),
    [dealId,cursor]
  ));
  const next=history.data?.pageInfo?.nextPageToken??"";
  return <section aria-label={"История сделки "+dealId}>
    <h4>История сделки</h4>
    <QueryNotice query={history}/>
    <ol>
      {history.data?.events.map(event=><li key={event.dealVersion.toString()}>
        <strong>{eventLabel(event.kind)}</strong>
        {" · версия "}{event.dealVersion.toString()}
        {" · "}{dateText(event.occurredAt)}
        <p>Этап: {event.previousStageId?event.previousStageId+" → ":""}{event.stageId}</p>
        {event.reasonCode&&<p>Причина: {event.reasonCode}</p>}
        <p>Исполнитель: {event.actor} · запрос: {event.requestId}</p>
        {event.linkedTaskId&&<p>Связанная задача: <a href={taskHref(event.linkedTaskId,accountId)}>
          {event.linkedTaskId}</a></p>}
      </li>)}
    </ol>
    {!history.busy&&history.data?.events.length===0&&<p>Событий на этой странице нет.</p>}
    {next&&<Button variant="secondary" onClick={()=>setCursor(next)}>Ранее</Button>}
    {cursor&&<Button variant="secondary" onClick={()=>setCursor("")}>К последним</Button>}
  </section>;
}

// One canonical GetDeal/ContactPoint read projection for inline/full page.
export function DealDetails({dealId,onEdit,onClose,onReopen}:{dealId:string;
 onEdit?:()=>void;onClose?:()=>void;onReopen?:()=>void}) {
  const details=useQuery(useCallback(async()=>{
    const deal=(await api.deals.getDeal({dealId})).deal;
    if(!deal)throw Error("Сделка не найдена");
    const contact=deal.primaryContactId?
      (await api.contactPoints.getContactPoint({contactPointId:deal.primaryContactId})).contactPoint:
      undefined;
    return {deal,contact};
  },[dealId]));
  const deal:Deal|undefined=details.data?.deal;
  return <section aria-label={"Подробности сделки "+dealId}>
    <QueryNotice query={details}/>
    {deal&&<>
      <h3>{deal.name}</h3>
      <p>ID: {deal.dealId} · версия {deal.version.toString()}</p>
      <p>Клиент: <a href={accountHref(deal.accountId)}>{deal.accountId}</a></p>
      <p>Основной контакт: {details.data?.contact?
        <a href={scopeHref("contacts",details.data.contact.partyId)}>
          {details.data.contact.displayValue}</a>:"Не выбран"}</p>
      <p>Воронка: {deal.stage?.pipelineId??"—"} · этап: {deal.stage?.stageId??"—"}</p>
      <p>Сумма: {moneyText(deal.amount)} · вероятность: {percent(deal.probabilityBasisPoints)}%</p>
      <p>Ожидаемое закрытие: {inputDate(deal.expectedCloseDate) || "Не назначено"}</p>
      <p>Статус: {deal.status===DealStatus.OPEN?"Открыта":
        deal.status===DealStatus.WON?"Выиграна":"Проиграна"}</p>
      {deal.closeReasonCode&&<p>Причина закрытия: {deal.closeReasonCode}</p>}
      <p>Создана: {dateText(deal.createdAt)} · обновлена: {dateText(deal.updatedAt)}</p>
      {deal.closedAt&&<p>Закрыта: {dateText(deal.closedAt)}</p>}
      <p><a href={scopeHref("tasks",deal.accountId)}>Задачи клиента</a></p>
      {deal.status===DealStatus.OPEN?<div>
        {onEdit&&<Button variant="secondary" onClick={onEdit}>Редактировать сделку</Button>}
        {" "}{onClose&&<Button variant="secondary" onClick={onClose}>Завершить сделку</Button>}
      </div>:onReopen?<Button variant="secondary" onClick={onReopen}>
        Открыть повторно
      </Button>:null}
      <DealHistory key={deal.dealId} dealId={deal.dealId} accountId={deal.accountId}/>
    </>}
  </section>;
}

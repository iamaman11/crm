import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import type {ContactPoint,Deal,SalesPipeline} from "@ultimate-crm/client/cloudflare";
import {DealEventKind} from "@ultimate-crm/client/cloudflare";
import {ContactPointStatus,DealStatus,ForecastCategory,ProbabilitySource} from "@ultimate-crm/client/cloudflare";
import {api,dateText} from "./api";
import {contactKindLabel} from "./contactLabels";
import {AccountName} from "./Pickers";
import {useQuery,QueryNotice} from "./hooks";
import {moneyText,percent} from "./money";
import type {CurrencyCatalog} from "./money";
import {inputDate} from "./calendarInput";
import {scopeHref,contactHref,taskHref,dealTasksHref} from "./routes";
import {stageName as stageLabel} from "./salesPipelineCatalog";

export function dealStatusText(status:DealStatus):string {
  switch(status){
    case DealStatus.OPEN:return "Открыта";
    case DealStatus.WON:return "Выиграна";
    case DealStatus.LOST:return "Проиграна";
    default:return "Неизвестный статус";
  }
}
export function forecastCategoryText(category:ForecastCategory):string {
  switch(category){
    case ForecastCategory.PIPELINE:return "Воронка";
    case ForecastCategory.BEST_CASE:return "Оптимистичный";
    case ForecastCategory.COMMIT:return "Обязательство";
    case ForecastCategory.OMITTED:return "Исключено из прогноза";
    default:return "Неизвестная категория";
  }
}
export function probabilitySourceText(source:ProbabilitySource):string {
  switch(source){
    case ProbabilitySource.STAGE_DEFAULT:return "Вероятность этапа";
    case ProbabilitySource.MANUAL:return "Индивидуальная вероятность";
    default:return "Неизвестный источник";
  }
}
// Presentation-only picker. Original Account/ContactPoint owners provide
// choices; original crm-sales Rust validates tenant, Party, Account and status.
export function DealContactPicker({accountId,initialContactId=""}:{
  accountId:string;initialContactId?:string;
}) {
  const [value,setValue]=useState(initialContactId);
  const contacts=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!accountId)return [] as ContactPoint[];
    const account=(await api.accounts.getAccount({accountId},{signal})).account;
    if(!account)throw Error("Клиент не найден");
    const result:ContactPoint[]=[];
    for(const party of account.parties) {
      let token="";
      const seen=new Set<string>();
      do {
        const page=await api.contactPoints.listContactPoints({
          partyId:party.partyId,page:{pageSize:100,pageToken:token}
        },{signal});
        result.push(...page.contactPoints.filter(point=>point.status===ContactPointStatus.ACTIVE));
        token=page.pageInfo?.nextPageToken??"";
        if(token&&seen.has(token))throw Error("Повторный курсор контактов");
        if(token)seen.add(token);
      }while(token);
    }
    // Presentation ordering, never a substitute for original Sales owner validation.
    // Prefer the customer's selected channel, then a verified value.
    return result.sort((a,b)=>Number(b.preferred)-Number(a.preferred)||
      Number(b.verified)-Number(a.verified)||a.displayValue.localeCompare(b.displayValue));
  },[accountId]));
  return <div>
    <SelectField label="Основной контакт сделки" name="primaryContactId"
      value={value} disabled={!accountId||contacts.busy||!!contacts.error}
      onChange={event=>setValue(event.target.value)}>
      <option value="">Без основного контакта</option>
      {contacts.data?.map(p=><option key={p.contactPointId} value={p.contactPointId}>
        {p.displayValue} · {contactKindLabel(p.kind)}{p.preferred?" · предпочтительный":""}{p.verified?" · подтверждён":""}
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
// Only a user-facing label. Immutable events still come from the original
// Sales owner with stable IDs; an archived/removed catalog stage falls back
// to that exact stored ID, never an invented historical name.
// One original, owner-authorized event projection for inline/full Deal pages.
// No reconstruction of history from mutable client-side snapshots.
export function DealHistory({dealId,accountId,pipeline}:{
  dealId:string;accountId:string;pipeline:SalesPipeline|undefined;
}){
  const [cursor,setCursor]=useState("");
  const history=useQuery(useCallback(
    (signal:AbortSignal)=>api.deals.listDealEvents({dealId,page:{pageSize:25,pageToken:cursor}},{signal}),
    [dealId,cursor]
  ));
  const next=history.data?.pageInfo?.nextPageToken??"";
  return <section aria-label={"История сделки "+dealId}>
    <h4>История сделки</h4>
    {pipeline&&<p>Названия этапов показаны по текущему справочнику воронки; история хранит исходные коды.</p>}
    <QueryNotice query={history}/>
    <ol>
      {history.data?.events.map(event=><li key={event.dealVersion.toString()}>
        <strong>{eventLabel(event.kind)}</strong>
        {" · версия "}{event.dealVersion.toString()}
        {" · "}{dateText(event.occurredAt)}
        <p>Этап: {event.previousStageId?stageLabel(pipeline,event.previousStageId)+" → ":""}{stageLabel(pipeline,event.stageId)}</p>
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
export function DealDetails({dealId,currencies,pipelines,onEdit,onClose,onReopen}:{
 dealId:string;currencies:CurrencyCatalog;pipelines:readonly SalesPipeline[];
 onEdit?:()=>void;onClose?:()=>void;onReopen?:()=>void;
}) {
  const details=useQuery(useCallback(async(signal:AbortSignal)=>{
    const deal=(await api.deals.getDeal({dealId},{signal})).deal;
    if(!deal)throw Error("Сделка не найдена");
    const contact=deal.primaryContactId?
      (await api.contactPoints.getContactPoint({contactPointId:deal.primaryContactId},{signal})).contactPoint:
      undefined;
    return {deal,contact};
  },[dealId]));
  const deal:Deal|undefined=details.data?.deal;
  // The parent SalesScreen already owns the paginated ListPipelines read.
  // Use that catalog for all three Deal projections, no extra RPC.
  const pipeline=pipelines.find(p=>p.pipelineId===deal?.stage?.pipelineId);
  return <section aria-label={"Подробности сделки "+dealId}>
    <QueryNotice query={details}/>
    {deal&&<>
      <h3>{deal.name}</h3>
      <p>ID: {deal.dealId} · версия {deal.version.toString()}</p>
      <p>Клиент: <AccountName accountId={deal.accountId}/></p>
      <p>Основной контакт: {details.data?.contact?
        <a href={contactHref(details.data.contact.contactPointId,details.data.contact.partyId)}>
          {details.data.contact.displayValue}</a>:"Не выбран"}</p>
      <p>Воронка: {pipeline?.name??deal.stage?.pipelineId??"—"} · этап: {deal.stage?.stageId?stageLabel(pipeline,deal.stage.stageId):"—"}</p>
      <p>Сумма: {moneyText(deal.amount,currencies)} · вероятность: {percent(deal.probabilityBasisPoints)}%</p>
      {deal.status===DealStatus.OPEN&&<p>{probabilitySourceText(deal.probabilitySource)} · {forecastCategoryText(deal.forecastCategory)}</p>}
      <p>Ожидаемое закрытие: {inputDate(deal.expectedCloseDate) || "Не назначено"}</p>
      <p>Статус: {dealStatusText(deal.status)}</p>
      {deal.closeReasonCode&&<p>Причина закрытия: {deal.closeReasonCode}</p>}
      <p>Создана: {dateText(deal.createdAt)} · обновлена: {dateText(deal.updatedAt)}</p>
      {deal.closedAt&&<p>Закрыта: {dateText(deal.closedAt)}</p>}
      <p><a href={dealTasksHref(deal.dealId,deal.accountId)}>Задачи этой сделки</a>
        {" · "}<a href={scopeHref("tasks",deal.accountId)}>Все задачи клиента</a></p>
      {deal.status===DealStatus.OPEN?<div>
        {onEdit&&<Button variant="secondary" onClick={onEdit}>Редактировать сделку</Button>}
        {" "}{onClose&&<Button variant="secondary" onClick={onClose}>Завершить сделку</Button>}
      </div>:onReopen?<Button variant="secondary" onClick={onReopen}>
        Открыть повторно
      </Button>:null}
      <DealHistory key={deal.dealId} dealId={deal.dealId} accountId={deal.accountId} pipeline={pipeline}/>
    </>}
  </section>;
}

import {useCallback,useState} from "react";
import {Button,SelectField} from "@ultimate-crm/ui";
import type {ContactPoint,Deal,DealCapabilities,SalesPipeline,CommercialCloseFact} from "@ultimate-crm/client/cloudflare";
import {DealEventKind,OpportunityTeamRole} from "@ultimate-crm/client/cloudflare";
import {ContactPointStatus,DealStatus,ForecastCategory,ProbabilitySource} from "@ultimate-crm/client/cloudflare";
import {api,dateText} from "./api";
import {contactKindLabel} from "./contactLabels";
import {AccountName} from "./Pickers";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {moneyText,percent} from "./money";
import type {CurrencyCatalog} from "./money";
import {inputDate} from "./calendarInput";
import {scopeHref,contactHref,taskHref,dealTasksHref} from "./routes";
import {stageName as stageLabel} from "./salesPipelineCatalog";
import {InteractionTimeline} from "./InteractionTimeline";
import {allOwners,ownerLabel} from "./ownerCatalog";
import {RoutingExplanation} from "./SalesRouting";
import {enumValue} from "./enumFields";
import {TeamDistribution} from "./TeamDistribution";

const opportunityRoles=[OpportunityTeamRole.OBSERVER,OpportunityTeamRole.CONTRIBUTOR,OpportunityTeamRole.LEAD] as const;
function opportunityRoleText(role:OpportunityTeamRole):string {
  switch(role){
    case OpportunityTeamRole.OBSERVER:return "Наблюдатель";
    case OpportunityTeamRole.CONTRIBUTOR:return "Участник";
    case OpportunityTeamRole.LEAD:return "Руководитель сделки";
    default:return "Неизвестная роль";
  }
}

function OpportunityTeam({deal,canManage,choices,refresh}:{deal:Deal;canManage:boolean;choices:Awaited<ReturnType<typeof allOwners>>["choices"];refresh:()=>Promise<unknown>}){
  const [email,setEmail]=useState("");
  const [role,setRole]=useState<OpportunityTeamRole>(OpportunityTeamRole.CONTRIBUTOR);
  const mutation=useMutation({
    storageKey:"crm.deal.opportunity.pending.v1:"+deal.dealId,
    lookup:async({resourceId,key},signal)=>(await api.deals.getDeal({dealId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{await refresh();}
  });
  const change=(member:string,next?:OpportunityTeamRole)=>void mutation.runIntent(()=>prepareMutation(
    api.deals.setOpportunityParticipant,{
      dealId:deal.dealId,email:member,...(next===undefined?{}:{role:next}),
      precondition:{idempotencyKey:mutation.key,expectedVersion:deal.version}
    },deal.dealId
  ),async()=>{await refresh();setEmail("");});
  return <section aria-label="Команда сделки">
    <h4>Команда сделки</h4>
    <p>Отдельно от единственного ответственного. Наблюдатель читает; участник меняет название, контакт и дату; руководитель также меняет коммерческие поля и этап и может закрыть сделку, на которую получил полномочие. Повторное открытие, ручная коммерческая атрибуция, смена ответственного и управление доступом — только администратор. Изменение версии сотрудника прекращает прежнее делегирование.</p>
    {deal.opportunityTeam.length===0?<p>Участники не назначены.</p>:<ul>
      {deal.opportunityTeam.map(member=><li key={member.email}>{member.email} — {opportunityRoleText(member.role)} · версия сотрудника {member.memberVersion.toString()}
        {canManage&&<Button variant="secondary" disabled={mutation.busy||mutation.pending} onClick={()=>change(member.email)}>Отозвать доступ {member.email}</Button>}
      </li>)}
    </ul>}
    {canManage&&<form aria-label="Назначить участника сделки" onSubmit={event=>{event.preventDefault();change(email,role);}}>
      <SelectField label="Участник сделки" value={email} onChange={event=>{setEmail(event.target.value);mutation.changed();}} required>
        <option value="">Выберите сотрудника</option>
        {choices.filter(choice=>choice.eligible&&choice.value.startsWith("actor:")).map(choice=>
          <option key={choice.value} value={choice.value.slice(6)}>{choice.label}</option>)}
      </SelectField>
      <SelectField label="Коммерческая роль" value={role} onChange={event=>{setRole(enumValue(event.target.value,opportunityRoles,"Коммерческая роль"));mutation.changed();}}>
        {opportunityRoles.map(value=><option key={value} value={value}>{opportunityRoleText(value)}</option>)}
      </SelectField>
      <Button type="submit" disabled={!email||mutation.busy||mutation.pending}>Назначить роль сделки</Button>
    </form>}
    <MutationNotice state={mutation}/>
  </section>;
}

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
function CommercialAttribution({fact,currencies,reversal=false}:{fact:CommercialCloseFact;currencies:CurrencyCatalog;reversal?:boolean}) {
  const booked=fact.outcome===DealStatus.WON;
  return <section aria-label="Историческая коммерческая атрибуция">
    <p>{reversal?"Обратная корректировка":"Факт закрытия"} · версия {fact.closeVersion.toString()} · {dateText(fact.closedAt)}</p>
    <p>Сумма на момент закрытия: {moneyText(fact.amount,currencies)} · {booked?(reversal?"Сторно выигранной сделки":"Выигранная сделка"):"Без коммерческого начисления"}</p>
    <p>Персональная атрибуция: {fact.creditedActor??"Не распределена"}</p>
    <p>Команда: {fact.creditedTeam?.name??"Не распределена"} · территория: {fact.creditedTerritory?.name??"Не распределена"}</p>
    <p>Воронка: {fact.pipelineId} · этап: {fact.stageId}</p>
  </section>;
}

export function DealHistory({dealId,accountId,pipeline,currencies,choices=[]}:{
  dealId:string;accountId:string;pipeline:SalesPipeline|undefined;currencies:CurrencyCatalog;choices?:Awaited<ReturnType<typeof allOwners>>["choices"];
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
        {event.initialRouting&&<RoutingExplanation decision={event.initialRouting} choices={choices}/>}
        {event.commercialCloseFact?<CommercialAttribution fact={event.commercialCloseFact} currencies={currencies} reversal={event.kind===DealEventKind.REOPENED}/>:((event.kind===DealEventKind.CLOSED||event.kind===DealEventKind.REOPENED)&&<p>Историческая коммерческая атрибуция неизвестна.</p>)}
        {event.opportunityTeamAfter&&<><p>Изменение команды сделки:</p><ul>{event.opportunityTeamAfter.participants.map(member=><li key={member.email}>{member.email} — {opportunityRoleText(member.role)} · версия {member.memberVersion.toString()}</li>)}</ul>{event.opportunityTeamAfter.participants.length===0&&<p>Все делегированные права отозваны.</p>}</>}
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

// Shared action projection: only generated server capabilities grant actions.
export function DealActions({capabilities,variant="record",onEdit,onAdvance,onClose,onReopen}:{
 capabilities?:DealCapabilities;variant?:"record"|"list"|"board";
 onEdit?:()=>void;onAdvance?:()=>void;onClose?:()=>void;onReopen?:()=>void;
}){
  return <div>
    {onEdit&&capabilities?.edit&&<Button variant="secondary" onClick={onEdit}>{variant==="list"?"Редактировать":variant==="board"?"Изменить сделку":"Редактировать сделку"}</Button>}
    {" "}{onAdvance&&capabilities?.advance&&<Button variant="secondary" onClick={onAdvance}>{variant==="record"?"Изменить этап сделки":"Изменить этап"}</Button>}
    {" "}{onClose&&capabilities?.close&&<Button variant="secondary" onClick={onClose}>{variant==="list"?"Завершить":"Завершить сделку"}</Button>}
    {" "}{onReopen&&capabilities?.reopen&&<Button variant="secondary" onClick={onReopen}>Открыть повторно</Button>}
  </div>;
}

// One canonical GetDeal/ContactPoint read projection for inline/full page.
export function DealDetails({dealId,currencies,pipelines,onEdit,onAdvance,onClose,onReopen}:{
 dealId:string;currencies:CurrencyCatalog;pipelines:readonly SalesPipeline[];
 onEdit?:()=>void;onAdvance?:()=>void;onClose?:()=>void;onReopen?:()=>void;
}) {
  const details=useQuery(useCallback(async(signal:AbortSignal)=>{
    const response=await api.deals.getDeal({dealId},{signal});
    const deal=response.deal;
    if(!deal||!response.capabilities)throw Error("Сделка или полномочия недоступны");
    const contact=deal.primaryContactId?
      (await api.contactPoints.getContactPoint({contactPointId:deal.primaryContactId},{signal})).contactPoint:
      undefined;
    return {deal,contact,capabilities:response.capabilities,teamDistribution:response.teamDistribution};
  },[dealId]));
  const deal:Deal|undefined=details.data?.deal;
  const capabilities=details.data?.capabilities;
  const distribution=useMutation({
    storageKey:"crm.deal.distribution.pending.v1:"+dealId,
    lookup:async({resourceId,key},signal)=>(await api.deals.getDeal({dealId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{await details.reload();}
  });
  const owners=useQuery(useCallback(allOwners,[]));
  const choices=owners.data?.choices??[];
  // The parent SalesScreen already owns the paginated ListPipelines read.
  // Use that catalog for all three Deal projections, no extra RPC.
  const pipeline=pipelines.find(p=>p.pipelineId===deal?.stage?.pipelineId);
  return <section aria-label={"Подробности сделки "+dealId}>
    <QueryNotice query={details}/>
    {deal&&<>
      <h3>{deal.name}</h3>
      <p>Ответственный: {ownerLabel(deal.owner,choices)}</p>
      {deal.initialRouting&&<RoutingExplanation decision={deal.initialRouting} choices={choices}/>}
      {deal.commercialCloseFact?<CommercialAttribution fact={deal.commercialCloseFact} currencies={currencies}/>:deal.status!==DealStatus.OPEN&&<p>Историческая коммерческая атрибуция неизвестна.</p>}
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
      <DealActions capabilities={capabilities} onEdit={onEdit} onAdvance={onAdvance} onClose={onClose} onReopen={onReopen}/>
      {details.data?.teamDistribution&&<TeamDistribution key={"distribution:"+deal.dealId+":"+deal.version} options={details.data.teamDistribution}
        pending={distribution.busy||distribution.pending} onChanged={distribution.changed} onAssign={(scope,email)=>void distribution.runIntent(()=>prepareMutation(api.deals.updateDeal,{
          dealId:deal.dealId,owner:{owner:{case:"assigneeIdentity",value:email}},teamDistribution:scope,
          precondition:{expectedVersion:deal.version,idempotencyKey:distribution.key}
        },deal.dealId),async()=>{await details.reload();})}/>}
      <MutationNotice state={distribution}/>
      <OpportunityTeam deal={deal} canManage={capabilities?.manageOpportunityTeam??false} choices={choices} refresh={details.reload}/>
      <DealHistory key={"history:"+deal.dealId+":"+deal.version} dealId={deal.dealId} accountId={deal.accountId} pipeline={pipeline} currencies={currencies} choices={choices}/>
      <InteractionTimeline accountId={deal.accountId} dealId={deal.dealId}/>
    </>}
  </section>;
}


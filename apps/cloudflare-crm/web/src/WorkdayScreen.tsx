import {useCallback,useMemo,useState} from "react";
import {Button,PageHeader,SelectField} from "@ultimate-crm/ui";
import {InteractionKind,TaskStatus} from "@ultimate-crm/client/cloudflare";
import {api,dateText,timestamp} from "./api";
import {useQuery,QueryNotice} from "./hooks";
import {accountHref,dealHref,taskHref,workdayHref} from "./routes";
import {navigateTo} from "./navigation";
import {allTeams} from "./teamCatalog";

function localToday():string {
  const now=new Date();
  return [String(now.getFullYear()).padStart(4,"0"),
    String(now.getMonth()+1).padStart(2,"0"),
    String(now.getDate()).padStart(2,"0")].join("-");
}

type BucketKind="before"|"today"|"after"|"undated";
function TaskBucket({title,kind,start,before,mine,teamId}:{title:string;kind:BucketKind;
  start:string;before:string;mine:boolean;teamId:string}) {
  const [cursor,setCursor]=useState("");
  const query=useQuery(useCallback((signal:AbortSignal)=>api.tasks.listTasks({
    page:{pageSize:12,pageToken:cursor},
    status:TaskStatus.OPEN,
    assignedToMe:mine,
    ...(teamId?{ownerTeamId:teamId}:{}),
    sortByDue:kind!=="undated",
    withoutDueOnly:kind==="undated",
    ...(kind==="today"||kind==="after"?{dueFrom:timestamp(kind==="today"?start:before)}:{}),
    ...(kind==="before"||kind==="today"?{dueBefore:timestamp(kind==="before"?start:before)}:{}),
  },{signal}),[kind,start,before,cursor,mine,teamId]));
  const tasks=query.data?.tasks??[];
  const next=query.data?.pageInfo?.nextPageToken??"";
  return <section aria-label={title}>
    <h2>{title}</h2>
    <QueryNotice query={query}/>
    {!query.busy&&!query.error&&tasks.length===0&&<p>Нет открытых задач в этом разделе.</p>}
    <ul>{tasks.map(task=><li key={task.taskId}>
      <a href={taskHref(task.taskId,task.accountId)}><strong>{task.subject}</strong></a>
      {" · "}{dateText(task.dueAt)}
      <p>Клиент: <a href={accountHref(task.accountId)}>
        {query.data?.accountNames[task.accountId]||"Открыть клиента"}
      </a>{task.relatedDealId?<> · <a href={dealHref(task.relatedDealId)}>Связанная сделка</a></>:null}</p>
    </li>)}</ul>
    {next&&<Button variant="secondary" type="button" disabled={query.busy}
      onClick={()=>setCursor(next)}>Следующие задачи: {title}</Button>}
    {cursor&&<Button variant="secondary" type="button" disabled={query.busy}
      onClick={()=>setCursor("")}>К первой странице: {title}</Button>}
  </section>;
}

function interactionKind(kind:InteractionKind):string {
  if(kind===InteractionKind.CALL)return "Звонок";
  if(kind===InteractionKind.MEETING)return "Встреча";
  if(kind===InteractionKind.NOTE)return "Заметка";
  return "Событие";
}

function RecentInteractions() {
  const [cursor,setCursor]=useState("");
  const query=useQuery(useCallback((signal:AbortSignal)=>api.interactions.listInteractions({
    recentAcrossAccounts:true,page:{pageSize:12,pageToken:cursor}
  },{signal}),[cursor]));
  const items=query.data?.interactions??[];
  const next=query.data?.pageInfo?.nextPageToken??"";
  return <section aria-label="Последние взаимодействия">
    <h2>Последние взаимодействия</h2>
    <p>Реальные звонки, встречи и заметки из доступного tenant, по времени события.</p>
    <QueryNotice query={query}/>
    {!query.busy&&!query.error&&items.length===0&&<p>Взаимодействий ещё нет.</p>}
    <ul>{items.map(item=><li key={item.interactionId}>
      <strong>{interactionKind(item.kind)}</strong>{" · "}{dateText(item.occurredAt)}
      <p>{item.text}</p>
      <p>Клиент: <a href={accountHref(item.accountId)}>
        {query.data?.accountNames[item.accountId]||"Открыть клиента"}
      </a>{item.dealId?<> · <a href={dealHref(item.dealId)}>Связанная сделка</a></>:null}
      {" · "}Автор: {item.author}</p>
    </li>)}</ul>
    {next&&<Button type="button" variant="secondary" disabled={query.busy}
      onClick={()=>setCursor(next)}>Более ранние взаимодействия</Button>}
    {cursor&&<Button type="button" variant="secondary" disabled={query.busy}
      onClick={()=>setCursor("")}>К последним взаимодействиям</Button>}
  </section>;
}

/** SK-06 uses the same generated Task and Interaction clients and original owners. */
export function WorkdayScreen({day,mine=false,teamId=""}:{day:string;mine?:boolean;teamId?:string}) {
  const teams=useQuery(useCallback(allTeams,[]));
  const chosen=day||localToday();
  // Browser-local calendar bounds; explicit local date, not a 24h UTC guess.
  // Adjacent local midnights also cover daylight-saving transitions correctly.
  const bounds=useMemo(()=>{
    const [year,month,date]=chosen.split("-").map(Number);
    if(year===undefined||month===undefined||date===undefined)
      throw Error("Некорректная дата рабочего дня");
    const start=new Date(year,month-1,date);
    const before=new Date(year,month-1,date+1);
    return {start:start.toISOString(),before:before.toISOString()};
  },[chosen]);
  return <>
    <PageHeader title="Рабочий день"
      description="Действующие задачи по сроку, связанные клиенты и сделки, последние взаимодействия."/>
    <p><a href={workdayHref(day,false)}>Общая очередь</a>{" · "}
      <a href={workdayHref(day,true)}>Моя очередь по подписанной сессии</a>{" · "}
      <a href="#team">Команда и нагрузка</a></p>
    <QueryNotice query={teams}/>
    <SelectField label="Командная очередь" value={teamId} onChange={e=>navigateTo(workdayHref(day,false,e.target.value))}>
      <option value="">Все команды</option>{teams.data?.map(t=><option key={t.teamId} value={t.teamId}>{t.name}{t.active?"":" (архив)"}</option>)}
    </SelectField>
    <p><label>Дата рабочего дня{" "}
      <input type="date" value={chosen} min="2000-01-01" max="2100-12-31"
        onChange={event=>{if(event.target.value)navigateTo(workdayHref(event.target.value,mine,teamId));}}/>
    </label>{" · "}<a href={workdayHref("",mine,teamId)}>Сегодня</a></p>
    <p>Время считается по часовому поясу браузера: {Intl.DateTimeFormat().resolvedOptions().timeZone}.
      Показаны {mine?"только задачи подписанного актора":teamId?"задачи выбранной команды":"все доступные открытые задачи"}
      текущего tenant. Список сортируется и ограничивается исходным владельцем до выдачи,
      а не по неполной странице в браузере. Режим «Моя очередь» сверяет настоящую
      идентичность Cloudflare Access, не строку email из браузера.</p>
    <TaskBucket key={chosen+"/before"} title="До выбранной даты" kind="before"
      start={bounds.start} before={bounds.before} mine={mine} teamId={teamId}/>
    <TaskBucket key={chosen+"/today"} title="На выбранную дату" kind="today"
      start={bounds.start} before={bounds.before} mine={mine} teamId={teamId}/>
    <TaskBucket key={chosen+"/after"} title="Ближайшие после даты" kind="after"
      start={bounds.start} before={bounds.before} mine={mine} teamId={teamId}/>
    <TaskBucket key={chosen+"/undated"} title="Без срока" kind="undated"
      start={bounds.start} before={bounds.before} mine={mine} teamId={teamId}/>
    <RecentInteractions/>
    <p><a href="#tasks">Все задачи</a>{" · "}<a href="#sales">Все сделки</a>
      {" · "}<a href="#insights">Аналитика бизнеса и риски</a></p>
  </>;
}

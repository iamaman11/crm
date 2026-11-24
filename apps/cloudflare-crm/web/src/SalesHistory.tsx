import {useCallback,useState} from "react";
import {Button,FilterForm,MetricGrid,ScrollTable,TextField} from "@ultimate-crm/ui";
import {DealStatus} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
import {parseDate} from "./calendarInput";
import {QueryNotice,useQuery} from "./hooks";
import {dealHref,intelligenceHref} from "./routes";
import {navigateTo,currentRouteHash} from "./navigation";
import {percent} from "./money";

type Response=Awaited<ReturnType<typeof api.deals.getSalesHistory>>;
type Window=NonNullable<Response["current"]>;
type Props={start:string;end:string;pipelineId:string;compare:boolean;historyZone:string;includeArchived:boolean;owner:string};
const count=(value:bigint|undefined)=>value?.toString()??"—";
const duration=(value:bigint|undefined)=>value===undefined?"—":value.toString()+" с";
const rate=(value:number|undefined)=>value===undefined?"—":percent(value)+"%";
const instant=(value:bigint|undefined)=>value===undefined?"—":new Date(Number(value/1000000n)).toISOString();
function status(value:DealStatus|undefined):string {
  if(value===DealStatus.OPEN)return "Открыта";
  if(value===DealStatus.WON)return "Выиграна";
  if(value===DealStatus.LOST)return "Проиграна";
  return "История неизвестна";
}

function HistoryWindow({window,title}:{window:Window;title:string}) {
  const state=window.stateAtCutoff,closed=window.finalCloses,cohort=window.creationCohort;
  return <section aria-label={title}>
    <h3>{title}</h3>
    <p>{instant(window.startUnixNanos)} — {instant(window.endExclusiveUnixNanos)} (исключительно).
      {window.futurePeriod?" Будущий период.":window.completePeriod?" Завершённый период.":" Неполный период."}
      {" "}Наблюдение до {instant(window.observedUntilExclusiveUnixNanos)}.</p>
    {!window.futurePeriod&&<>
      <MetricGrid>
        <div><dt>Состояние на дату: открыто / выиграно / проиграно</dt><dd>{count(state?.openCount)} / {count(state?.wonCount)} / {count(state?.lostCount)}</dd></div>
        <div><dt>Уникальные финальные закрытия: выиграно / проиграно</dt><dd>{count(closed?.wonCount)} / {count(closed?.lostCount)}</dd></div>
        <div><dt>Исторический Win rate</dt><dd>{rate(closed?.winRateBasisPoints)} · {count(closed?.denominator)} в знаменателе</dd></div>
        <div><dt>События закрытия / переоткрытия</dt><dd>{count(window.closeEventCount)} / {count(window.reopenEventCount)}</dd></div>
        <div><dt>Когорта созданных сделок</dt><dd>{count(cohort?.totalCount)} · открыто {count(cohort?.openCount)} / выиграно {count(cohort?.wonCount)} / проиграно {count(cohort?.lostCount)}</dd></div>
        <div><dt>Доля выигранных от всей когорты</dt><dd>{rate(cohort?.cohortConversionBasisPoints)}</dd></div>
        <div><dt>Цикл от создания до финального закрытия (среднее / P50 / P90)</dt><dd>{duration(window.cycle?.meanSeconds)} / {duration(window.cycle?.medianSeconds)} / {duration(window.cycle?.p90Seconds)}</dd></div>
        <div><dt>Открытый эпизод до закрытия (среднее / P50 / P90)</dt><dd>{duration(window.closeEpisode?.meanSeconds)} / {duration(window.closeEpisode?.medianSeconds)} / {duration(window.closeEpisode?.p90Seconds)}</dd></div>
        <div><dt>Сделки с неполной историей (исключены)</dt><dd>{count(window.unknownHistoryCount)}</dd></div>
      </MetricGrid>
      <h4>Причины выигрышей и потерь</h4>
      <ScrollTable label={title+" — причины"}><table><thead><tr><th scope="col">Исход</th><th scope="col">Причина</th><th scope="col">Сделок</th></tr></thead><tbody>
        {window.reasons.map(row=><tr key={row.outcome+":"+row.reasonCode}><th scope="row">{status(row.outcome)}</th><td>{row.reasonCode??"Причина неизвестна"}</td><td>{count(row.dealCount)}</td></tr>)}
      </tbody></table></ScrollTable>
      {window.reasons.length===0&&<p>Нет известных финальных закрытий в периоде.</p>}
      <h4>Продолжительность завершённых посещений этапов</h4>
      <ScrollTable label={title+" — этапы"}><table><thead><tr><th scope="col">Воронка / этап</th><th scope="col">Версия каталога при входе</th><th scope="col">Посещений</th><th scope="col">Среднее</th><th scope="col">P50</th><th scope="col">P90</th></tr></thead><tbody>
        {window.stages.map(row=><tr key={row.pipelineId+":"+row.stageId+":"+row.catalogVersion}><th scope="row">{row.pipelineId} / {row.stageId}</th><td>{count(row.catalogVersion)}</td><td>{count(row.duration?.count)}</td><td>{duration(row.duration?.meanSeconds)}</td><td>{duration(row.duration?.medianSeconds)}</td><td>{duration(row.duration?.p90Seconds)}</td></tr>)}
      </tbody></table></ScrollTable>
      {window.stages.length===0&&<p>Нет известных завершённых посещений.</p>}
      <details><summary>Исходные сделки и события ({window.deals.length})</summary>
        <ScrollTable label={title+" — сделки"}><table><thead><tr><th scope="col">Сделка</th><th scope="col">Состояние на дату</th><th scope="col">Создание / когорта</th><th scope="col">Финальное закрытие</th><th scope="col">Цикл / эпизод</th><th scope="col">Версии событий</th></tr></thead><tbody>
          {window.deals.map(deal=><tr key={deal.dealId}><th scope="row"><a href={dealHref(deal.dealId)}>{deal.dealId}</a></th><td>{status(deal.status)} · v{count(deal.lastEventVersion)}</td><td>{instant(deal.createdAtUnixNanos)}{deal.creationCohort?" · когорта":""}</td><td>{instant(deal.finalCloseAtUnixNanos)} · v{count(deal.finalCloseVersion)} · {deal.reasonCode??"—"}{deal.finalCloseInPeriod?" · в периоде":""}</td><td>{duration(deal.cycleSeconds)} / {duration(deal.episodeSeconds)}</td><td>Закрытия: {deal.closeVersions.map(String).join(", ")||"—"}; переоткрытия: {deal.reopenVersions.map(String).join(", ")||"—"}
            {deal.visits.length>0&&<details><summary>Посещения этапов</summary>{deal.visits.map(visit=><p key={visit.exitVersion.toString()}>{visit.stageId} · v{count(visit.entryVersion)} → v{count(visit.exitVersion)} · {duration(visit.durationSeconds)} · {instant(visit.entryAtUnixNanos)} → {instant(visit.exitAtUnixNanos)}</p>)}</details>}</td></tr>)}
        </tbody></table></ScrollTable>
      </details>
    </>}
  </section>;
}

export function SalesHistory({start,end,pipelineId,compare,historyZone,includeArchived,owner}:Props){
  const [formError,setFormError]=useState("");
  const query=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!start||!end)return undefined;
    const startDate=parseDate(start),endDate=parseDate(end);
    if(!startDate||!endDate)throw Error("Укажите обе даты");
    return api.deals.getSalesHistory({period:{startDate,endDate},timeZone:historyZone,...(pipelineId?{pipelineId}:{}),comparePrevious:compare},{signal});
  },[start,end,pipelineId,historyZone,compare]));
  const data=query.data;
  return <section aria-label="Историческая аналитика продаж">
    <h2>Историческая аналитика продаж</h2>
    <p>По сохранённым сделкам и полным цепочкам событий. Все воронки, включая архивные, если не выбрана конкретная. Текущий ответственный не подменяет историческую атрибуцию. Удалённые сделки не восстанавливаются; неполная история явно исключается из достоверных показателей.</p>
    <p>Финальные закрытия — уникальные сделки, закрытые к концу наблюдения и в выбранном периоде. Эпизоды считаются отдельно; переоткрытие запускает новый открытый эпизод. Обычное изменение не сбрасывает таймер этапа. Когорта — сделки, созданные в периоде, наблюдаемые до его конца. Неполный период нельзя сравнивать как завершённый.</p>
    <FilterForm key={start+":"+end+":"+historyZone} aria-label="Период исторической аналитики" onSubmit={event=>{
      event.preventDefault();
      try{
        const form=new FormData(event.currentTarget),nextStart=String(form.get("start")??""),nextEnd=String(form.get("end")??""),zone=String(form.get("zone")??"");
        if(!parseDate(nextStart)||!parseDate(nextEnd)||!zone)throw Error("Укажите даты и часовой пояс");
        setFormError("");
        const href=intelligenceHref({start:nextStart,end:nextEnd,pipelineId,compare,historyZone:zone,includeArchived,owner});
        if(href===currentRouteHash())void query.reload().catch(()=>{});else navigateTo(href);
      }catch(error){setFormError(error instanceof Error?error.message:String(error));}
    }}>
      <TextField label="Начало исторического периода" type="date" name="start" defaultValue={start} required/>
      <TextField label="Конец исторического периода" type="date" name="end" defaultValue={end} required/>
      <TextField label="Часовой пояс истории" name="zone" defaultValue={historyZone} required/>
      <Button type="submit">Рассчитать историческую аналитику</Button>
    </FilterForm>
    {formError&&<p role="alert">{formError}</p>}
    {start&&end&&<QueryNotice query={query}/>}
    {query.error&&<Button type="button" variant="secondary" onClick={()=>void query.reload().catch(()=>{})}>Повторить исторический запрос</Button>}
    {data&&!query.stale&&!query.error&&data.current&&<>
      <p>Срез {instant(data.asOfUnixNanos)}. Границы периода рассчитаны Rust в {historyZone}.</p>
      <HistoryWindow window={data.current} title="История выбранного периода"/>
      {data.previous&&<><p>{data.comparisonComparable?"Полные периоды без обнаруженных пропусков.":"Периоды не сопоставимы: незавершённый период или неполная история."} Изменение Win rate: {data.winRateDeltaBasisPoints===undefined?"—":percent(data.winRateDeltaBasisPoints)+" п.п."}</p><HistoryWindow window={data.previous} title="История предыдущего периода"/></>}
    </>}
  </section>;
}

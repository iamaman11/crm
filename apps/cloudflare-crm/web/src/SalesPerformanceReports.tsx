import {useCallback,useState} from "react";
import {Button,TextField,SelectField} from "@ultimate-crm/ui";
import {SalesForecastDisposition} from "@ultimate-crm/client/cloudflare";
import type {SalesPerformanceReport,SalesQuota,SalesQuotaFactCursor,SalesForecastSource,PreviewSalesPerformanceReportResponse} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {useQuery,useReadAction,useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {decimalUnits,weightedMoneyText,percent} from "./money";
import type {CurrencyCatalog} from "./money";
import {inputDate} from "./calendarInput";
import {dealHref,salesPerformanceReportHref} from "./routes";
import {navigateTo} from "./navigation";
import {forecastCategoryText,probabilitySourceText} from "./DealRecord";

function ratio(value:string|undefined):string {
  if(value===undefined)return "Не применимо";
  const n=BigInt(value),abs=n<0n?-n:n;
  return (n<0n?"−":"")+(abs/100n).toString()+"."+(abs%100n).toString().padStart(2,"0")+"%";
}
function disposition(value:SalesForecastDisposition):string {
  switch(value){
    case SalesForecastDisposition.INCLUDED:return "В прогнозе";
    case SalesForecastDisposition.UNALLOCATED:return "Не распределена";
    case SalesForecastDisposition.UNDATED:return "Нет даты";
    case SalesForecastDisposition.OMITTED:return "Исключена";
    case SalesForecastDisposition.MISSING_AMOUNT:return "Нет суммы";
    case SalesForecastDisposition.OTHER_CURRENCY:return "Другая валюта";
    default:return "Неизвестная причина";
  }
}
function money(value:string,code:string,currencies:CurrencyCatalog|undefined):string {
  return currencies?decimalUnits(BigInt(value),code,currencies)+" "+code:value+" minor units "+code;
}
function weighted(value:string,code:string,currencies:CurrencyCatalog|undefined):string {
  return currencies?weightedMoneyText(value,code,currencies):value+" × 1/10000 minor units "+code;
}
function time(value:bigint):string {return new Date(Number(value/1000000n)).toISOString();}

function Summary({report,currencies}:{report:SalesPerformanceReport;currencies:CurrencyCatalog|undefined}){
  const a=report.actuals,f=report.forecast,c=report.quota?.configuration;
  if(!a||!f||!c?.target)return <p role="alert">Отсутствуют обязательные данные отчёта.</p>;
  const code=c.target.currencyCode;
  return <>
    <p>{c.name} · версия {report.quota?.version.toString()} · {inputDate(c.period?.startDate)} — {inputDate(c.period?.endDate)} · {c.period?.timeZone}</p>
    <p>Снимок: {time(report.capturedAtUnixNanos)} · последовательность {report.eventSequence.toString()} · формула v{report.formulaVersion} · {report.capturedBy}</p>
    <p>План: {money(c.target.minorUnits.toString(),code,currencies)} · Выигрыши: {money(a.grossMinorUnits,code,currencies)} · Сторно: {money(a.reversalMinorUnits,code,currencies)}</p>
    <p>Нетто: {money(a.netMinorUnits,code,currencies)} · Остаток: {money(a.remainingMinorUnits,code,currencies)} · Выполнение: {ratio(a.attainmentBasisPoints)}</p>
    <p>Выигрышей: {a.wonCount.toString()} · проигрышей: {a.lostCount.toString()} · сторно: {a.reversalCount.toString()} · без суммы: {a.missingAmountCount.toString()} · без исторического факта: {a.unknownHistoryCount.toString()} · не распределено: {a.unallocatedCount.toString()}</p>
    <h5>Прогнозное покрытие остатка цели</h5>
    <p>Открытый портфель: {money(f.openMinorUnits,code,currencies)} · Взвешенный: {weighted(f.weightedMinorUnitsTimesBasisPoints,code,currencies)}</p>
    <p>Покрытие остатка: {ratio(f.openCoverageBasisPoints)} · взвешенное покрытие: {ratio(f.weightedCoverageBasisPoints)}</p>
    <p>Включено: {f.includedCount.toString()} · не распределено: {f.unallocatedCount.toString()} · без даты: {f.undatedCount.toString()} · исключено: {f.omittedCount.toString()} · без суммы: {f.missingAmountCount.toString()} · другая валюта: {f.otherCurrencyCount.toString()}</p>
    <p>Причины исключения не пересекаются: неизвестная область → отсутствие даты → исключённая категория → сумма/валюта. Известные сделки другой области или вне периода не входят в снимок. Взвешенная сумма — оценка, не выручка; будущая явная атрибуция не предполагается.</p>
    <ul>{f.buckets.map(b=><li key={b.category}>{forecastCategoryText(b.category)} · {b.dealCount.toString()} · {money(b.openMinorUnits,code,currencies)} · {weighted(b.weightedMinorUnitsTimesBasisPoints,code,currencies)}</li>)}</ul>
  </>;
}
function SourceTable({sources,currencies}:{sources:SalesForecastSource[];currencies:CurrencyCatalog|undefined}){
  return <div role="region" aria-label="Исходные сделки прогноза" tabIndex={0} style={{maxWidth:"100%",overflowX:"auto"}}>
    <table><thead><tr><th scope="col">Сделка на момент снимка</th><th scope="col">Дата</th><th scope="col">Сумма</th><th scope="col">Прогноз</th><th scope="col">Атрибуция</th><th scope="col">Участие</th></tr></thead>
      <tbody>{sources.map(s=><tr key={s.dealId}><th scope="row"><a href={dealHref(s.dealId)}>{s.dealId}</a> · v{s.dealVersion.toString()}</th>
        <td>{inputDate(s.expectedCloseDate)||"—"}</td>
        <td>{s.amount?money(s.amount.minorUnits.toString(),s.amount.currencyCode,currencies):"—"}</td>
        <td>{forecastCategoryText(s.category)} · {percent(s.probabilityBasisPoints)}% · {probabilitySourceText(s.probabilitySource)}</td>
        <td>{s.creditedActor??"—"} · {s.creditedTeam?.name??"—"} · {s.creditedTerritory?.name??"—"}</td>
        <td>{disposition(s.disposition)}</td></tr>)}</tbody>
    </table>{sources.length===0&&<p>На этой странице нет исходных сделок.</p>}
  </div>;
}

export function SalesPerformanceReports({isAdmin,quota,reportId,currencies}:{isAdmin:boolean;quota:SalesQuota|undefined;reportId:string;currencies:CurrencyCatalog|undefined}){
  const [reportCursor,setReportCursor]=useState("");
  const [quotaOnly,setQuotaOnly]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const [name,setName]=useState("");
  const [opened,setOpened]=useState(reportId);
  const [actualAfter,setActualAfter]=useState<SalesQuotaFactCursor>();
  const [forecastAfter,setForecastAfter]=useState("");
  const [compareId,setCompareId]=useState("");
  const quotaId=quota?.quotaId,version=quota?.version;
  const ready=!!quota?.configuration?.published&&version!==undefined;
  const list=useQuery(useCallback((signal:AbortSignal)=>api.deals.listSalesPerformanceReports({
    page:{pageSize:25,pageToken:reportCursor},...(quotaOnly&&quotaId?{quotaId}:{})
  },{signal}),[reportCursor,quotaOnly,quotaId]));
  const saved=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!opened)return undefined;
    return api.deals.getSalesPerformanceReport({reportId:opened,pageSize:25,
      ...(actualAfter?{actualAfter}:{}),forecastAfter},{signal});
  },[opened,actualAfter,forecastAfter]));
  const compare=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!compareId||compareId===opened)return undefined;
    return api.deals.getSalesPerformanceReport({reportId:compareId,pageSize:1},{signal});
  },[compareId,opened]));
  const preview=useReadAction<{key:string;response:PreviewSalesPerformanceReportResponse}>();
  const key=quotaId+":"+version?.toString();
  const live=preview.data?.key===key?preview.data.response:undefined;
  const open=(id:string)=>{setOpened(id);setActualAfter(undefined);setForecastAfter("");setCompareId("");};
  const mutation=useMutation({
    storageKey:"crm.sales.performance.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.deals.getSalesPerformanceReport({reportId:resourceId,receiptIdempotencyKey:key,pageSize:1},{signal})).confirmedMutation,
    refresh:async marker=>{open(marker.resourceId);await list.reload();}
  });
  const pending=mutation.busy||mutation.pending;
  const report=saved.data?.report;
  const code=report?.quota?.configuration?.target?.currencyCode;
  const comparison=compare.data?.report;
  const sameQuota=!!report&&!!comparison&&report.quota?.quotaId===comparison.quota?.quotaId;
  return <section aria-label="Коммерческие отчёты и прогнозное покрытие">
    <h4>Коммерческие отчёты и прогнозное покрытие</h4>
    <p>Сохранённый отчёт — неизменяемый снимок версии цели, факта и текущего открытого портфеля. Сохранение фиксирует новый снимок, а не устаревший результат предварительного просмотра.</p>
    {ready&&quotaId&&version!==undefined&&<>
      <Button variant="secondary" disabled={preview.busy} onClick={()=>void preview.run(async signal=>({key,response:await api.deals.previewSalesPerformanceReport({quotaId,quotaVersion:version,pageSize:25},{signal})}))}>Предпросмотр покрытия цели</Button>
      <QueryNotice query={preview}/>
      {live?.report&&!preview.busy&&!preview.error&&<section aria-label="Текущий прогноз"><h5>Текущий прогноз (не сохранён)</h5>
        <Summary report={live.report} currencies={currencies}/><SourceTable sources={live.forecastSources} currencies={currencies}/>
        {live.moreForecastSources&&<p>Показана первая страница. Сохраните снимок для полной стабильной постраничной детализации.</p>}
      </section>}
      {isAdmin&&<form aria-label="Сохранение коммерческого отчёта" onSubmit={event=>{
        event.preventDefault();const id=draftId;
        void mutation.runIntent(()=>prepareMutation(api.deals.saveSalesPerformanceReport,{
          reportId:id,name:name.trim(),quotaId,quotaVersion:version,precondition:{idempotencyKey:mutation.key}
        },id),async()=>{setName("");setDraftId(newId());open(id);await list.reload();});
      }}>
        <TextField label="Название отчёта" value={name} disabled={pending} required onChange={event=>{setName(event.target.value);mutation.changed();}}/>
        <Button type="submit" disabled={pending||!name.trim()}>Сохранить коммерческий отчёт</Button>
      </form>}
    </>}
    <MutationNotice state={mutation}/>
    <p><label><input type="checkbox" checked={quotaOnly} disabled={!quotaId} onChange={event=>{setQuotaOnly(event.target.checked);setReportCursor("");}}/> Только выбранная цель</label></p>
    <QueryNotice query={list}/>
    <div role="region" aria-label="Сохранённые коммерческие отчёты" tabIndex={0} style={{maxWidth:"100%",overflowX:"auto"}}>
      <table><thead><tr><th scope="col">Отчёт</th><th scope="col">Цель</th><th scope="col">Снимок UTC</th><th scope="col">Выполнение</th><th scope="col">Взвешенное покрытие</th></tr></thead>
        <tbody>{list.data?.reports.map(r=><tr key={r.reportId}><th scope="row"><a href={salesPerformanceReportHref(r.reportId)} onClick={event=>{if(event.button===0&&!event.ctrlKey&&!event.metaKey&&!event.shiftKey&&!event.altKey){event.preventDefault();navigateTo(salesPerformanceReportHref(r.reportId));}}}>{r.name}</a></th>
          <td>{r.quota?.configuration?.name} · v{r.quota?.version.toString()}</td><td>{time(r.capturedAtUnixNanos)}</td><td>{ratio(r.actuals?.attainmentBasisPoints)}</td><td>{ratio(r.forecast?.weightedCoverageBasisPoints)}</td></tr>)}</tbody>
      </table>
      {list.data?.reports.length===0&&<p>Сохранённых отчётов нет.</p>}
    </div>
    {reportCursor&&<Button variant="secondary" disabled={list.busy} onClick={()=>setReportCursor("")}>Первая страница отчётов</Button>}
    {list.data?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={list.busy} onClick={()=>setReportCursor(list.data?.pageInfo?.nextPageToken??"")}>Следующая страница отчётов</Button>}
    <QueryNotice query={saved}/>
    {report&&code&&saved.data&&<section aria-label="Сохранённый коммерческий отчёт">
      <h5>{report.name}</h5><p><a href={salesPerformanceReportHref(report.reportId)}>Постоянная ссылка на отчёт</a></p>
      <Summary report={report} currencies={currencies}/>
      <h5>Исторические коммерческие факты</h5>
      {!saved.data.actualHistoryAvailable?<p role="alert">Исходная история больше не подтверждается. Сохранённые итоги не пересчитаны; детализация недоступна.</p>:
        <><ul>{saved.data.actualFacts.map(f=><li key={f.dealId+":"+f.dealVersion.toString()}><a href={dealHref(f.dealId)}>Сделка {f.dealId}</a> · v{f.dealVersion.toString()} · {f.reversal?"Сторно":"Закрытие"} · {f.signedMinorUnits===undefined?"Сумма неизвестна":money(f.signedMinorUnits,code,currencies)}</li>)}</ul>
        {actualAfter&&<Button variant="secondary" onClick={()=>setActualAfter(undefined)}>Первые факты отчёта</Button>}
        {saved.data.actualNext&&<Button variant="secondary" onClick={()=>setActualAfter(saved.data?.actualNext)}>Следующие факты отчёта</Button>}</>}
      <SourceTable sources={saved.data.forecastSources} currencies={currencies}/>
      {forecastAfter&&<Button variant="secondary" onClick={()=>setForecastAfter("")}>Первые сделки прогноза</Button>}
      {saved.data.forecastNext&&<Button variant="secondary" onClick={()=>setForecastAfter(saved.data?.forecastNext??"")}>Следующие сделки прогноза</Button>}
      <SelectField label="Сравнить с отчётом этой цели" value={compareId} onChange={event=>setCompareId(event.target.value)}>
        <option value="">Не сравнивать</option>
        {list.data?.reports.filter(r=>r.reportId!==report.reportId&&r.quota?.quotaId===report.quota?.quotaId).map(r=><option key={r.reportId} value={r.reportId}>{r.name}</option>)}
      </SelectField>
      <QueryNotice query={compare}/>
      {comparison&&sameQuota&&<section aria-label="Сравнение коммерческих отчётов"><h5>{comparison.name}</h5><Summary report={comparison} currencies={currencies}/></section>}
    </section>}
  </section>;
}

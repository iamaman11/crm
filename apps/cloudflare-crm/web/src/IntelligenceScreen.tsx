import {useCallback,useState} from "react";
import type {FormEvent} from "react";
import {Button,Card,PageHeader} from "@ultimate-crm/ui";
import type {SalesPipeline} from "@ultimate-crm/client/cloudflare";
import {DealStatus,ForecastCategory} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
import {useQuery,QueryNotice} from "./hooks";
import {parseDate} from "./calendarInput";
import {decimalUnits,weightedMoneyText,percent} from "./money";
import type {CurrencyCatalog} from "./money";
import {allPipelines} from "./salesPipelineCatalog";
import {salesPipelineHref,intelligenceHref,workdayHref} from "./routes";
import {navigateTo} from "./navigation";

// Infer from the ONE generated Connect client; never maintain a parallel DTO.
type GetPipelineMetricsResponse=Awaited<ReturnType<typeof api.deals.getPipelineMetrics>>;
type Slice={pipeline:SalesPipeline;current:GetPipelineMetricsResponse;previous?:GetPipelineMetricsResponse};
type Portfolio={slices:Slice[];currencies:CurrencyCatalog};
type Inputs={start:string;end:string;pipelineId:string;includeArchived:boolean;compare:boolean};
const MAX_CONCURRENT=4;
function n(value:bigint):string {return new Intl.NumberFormat("ru-RU").format(value);}
function signed(value:string):bigint {return BigInt(value);}
function previousRange(start:string,end:string):{start:string;end:string} {
  const begin=Date.parse(start+"T00:00:00Z"),finish=Date.parse(end+"T00:00:00Z");
  const length=Math.round((finish-begin)/86400000)+1;
  const day=(time:number)=>new Date(time).toISOString().slice(0,10);
  return {start:day(begin-length*86400000),end:day(begin-86400000)};
}
function statusTitle(status:DealStatus):string {
  if(status===DealStatus.OPEN)return "Открыто";
  if(status===DealStatus.WON)return "Выиграно";
  if(status===DealStatus.LOST)return "Проиграно";
  return "Неизвестно";
}
function categoryTitle(category:ForecastCategory):string {
  if(category===ForecastCategory.PIPELINE)return "Воронка";
  if(category===ForecastCategory.BEST_CASE)return "Лучший случай";
  if(category===ForecastCategory.COMMIT)return "Обязательство";
  if(category===ForecastCategory.OMITTED)return "Исключено";
  return "Не задано";
}
type CurrencyTotal={currency:string;minor:bigint;weighted:bigint;deals:bigint};
function currencyTotals(slices:Slice[],field:"forecastCurrencies"|"undatedForecastCurrencies"):
    CurrencyTotal[] {
  const totals=new Map<string,CurrencyTotal>();
  for(const slice of slices)for(const item of slice.current[field]){
    if(item.category===ForecastCategory.OMITTED)continue;
    const row=totals.get(item.currencyCode)??{
      currency:item.currencyCode,minor:0n,weighted:0n,deals:0n
    };
    row.minor+=signed(item.totalMinorUnits);
    row.weighted+=signed(item.weightedMinorUnitsTimesBasisPoints);
    row.deals+=item.amountDealCount;
    totals.set(item.currencyCode,row);
  }
  return [...totals.values()].sort((a,b)=>a.currency.localeCompare(b.currency));
}
function metricTotal(slices:Slice[],key:"overdueExpectedCloseCount"|
  "dueNext30DaysCount"|"noExpectedCloseCount"|"overdueLinkedTaskCount"):bigint {
  return slices.reduce((sum,s)=>sum+s.current.operational
    .filter(o=>o.status===DealStatus.OPEN).reduce((acc,row)=>acc+row[key],0n),0n);
}
function dealCount(slices:Slice[],status:DealStatus):bigint {
  return slices.reduce((sum,s)=>sum+s.current.statuses
    .filter(row=>row.status===status).reduce((acc,row)=>acc+row.dealCount,0n),0n);
}
function outcome(slices:Slice[],previous=false) {
  return slices.reduce((acc,s)=>{
    const facts=(previous?s.previous:s.current)?.closedOutcomes;
    if(facts){
      acc.won+=facts.wonCount;acc.lost+=facts.lostCount;
      acc.denominator+=facts.denominator;
      acc.unknown+=facts.unknownCloseCount;
    }
    return acc;
  },{won:0n,lost:0n,denominator:0n,unknown:0n});
}
function winRate({won,denominator}:{won:bigint;denominator:bigint}):string {
  return denominator>0n?percent(Number(won*10000n/denominator))+"%":"недостаточно закрытых сделок";
}
function CurrencyTable({title,rows,catalog}:{title:string;rows:CurrencyTotal[];
  catalog:CurrencyCatalog}) {
  return <section aria-label={title}>
    <h3>{title}</h3>
    {rows.length===0?<p>Указанных сумм нет.</p>:<table>
      <thead><tr><th scope="col">Валюта</th><th scope="col">Сумма</th>
        <th scope="col">Взвешено по вероятности</th><th scope="col">Сделок с суммой</th></tr></thead>
      <tbody>{rows.map(row=><tr key={row.currency}>
        <th scope="row">{row.currency}</th>
        <td>{decimalUnits(row.minor,row.currency,catalog)}</td>
        <td>{weightedMoneyText(row.weighted.toString(),row.currency,catalog)}</td>
        <td>{n(row.deals)}</td>
      </tr>)}</tbody>
    </table>}
  </section>;
}

/** SK-07: complete authorised native metrics, never partial Deal pages as analytics. */
export function IntelligenceScreen({start,end,pipelineId,includeArchived,compare}:Inputs) {
  const [draftStart,setDraftStart]=useState(start);
  const [draftEnd,setDraftEnd]=useState(end);
  const [formError,setFormError]=useState("");
  const query=useQuery(useCallback(async(signal:AbortSignal):Promise<Portfolio>=>{
    const catalog=await allPipelines(signal);
    // The owner is authoritative for the catalog. A guessed pipeline from the
    // URL must not produce a fabricated/partial portfolio.
    if(pipelineId&&!catalog.pipelines.some(p=>p.pipelineId===pipelineId))
      throw Error("Воронка по ссылке недоступна в текущем tenant");
    const eligible=catalog.pipelines.filter(p=>
      (!p.archived||includeArchived)&&(!pipelineId||p.pipelineId===pipelineId));
    const slices:Slice[]=[];
    const period=start&&end?{startDate:parseDate(start),endDate:parseDate(end)}:undefined;
    const prior=compare&&start&&end?previousRange(start,end):undefined;
    // Bounded parallelism, one whole, pageless aggregate per pipeline. If any
    // owner fails the entire portfolio fails closed: no misleading partial KPI.
    for(let i=0;i<eligible.length;i+=MAX_CONCURRENT) {
      const group=await Promise.all(eligible.slice(i,i+MAX_CONCURRENT).map(async pipeline=>{
        const current=await api.deals.getPipelineMetrics({
          pipelineId:pipeline.pipelineId,...(period?{analysisPeriod:period}:{})
        },{signal});
        const previous=prior?await api.deals.getPipelineMetrics({
          pipelineId:pipeline.pipelineId,analysisPeriod:{
            startDate:parseDate(prior.start),endDate:parseDate(prior.end)
          }
        },{signal}):undefined;
        return previous===undefined?{pipeline,current}:{pipeline,current,previous};
      }));
      slices.push(...group);
    }
    return {slices,currencies:catalog.currencies};
  },[start,end,pipelineId,includeArchived,compare]));
  const data=query.data,slices=data?.slices??[];
  const total=slices.reduce((sum,s)=>sum+s.current.totalDealCount,0n);
  const open=dealCount(slices,DealStatus.OPEN);
  const won=dealCount(slices,DealStatus.WON);
  const lost=dealCount(slices,DealStatus.LOST);
  const actual=outcome(slices),earlier=compare?outcome(slices,true):undefined;
  const currencies=data?currencyTotals(slices,"forecastCurrencies"):[];
  const undated=data?currencyTotals(slices,"undatedForecastCurrencies"):[];
  const perCategory=[ForecastCategory.PIPELINE,ForecastCategory.BEST_CASE,
    ForecastCategory.COMMIT,ForecastCategory.OMITTED].map(category=>({
      category,deals:slices.reduce((sum,s)=>sum+s.current.forecastCategories
        .filter(item=>item.category===category)
        .reduce((v,item)=>v+item.dealCount,0n),0n),
      unplanned:slices.reduce((sum,s)=>sum+s.current.undatedForecastCategories
        .filter(item=>item.category===category)
        .reduce((v,item)=>v+item.dealCount,0n),0n)
    }));
  const apply=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();
    try{
      if(Boolean(draftStart)!==Boolean(draftEnd))throw Error("Укажите обе границы периода");
      if(draftStart&&(!parseDate(draftStart)||!parseDate(draftEnd)||draftStart>draftEnd))
        throw Error("Некорректный период");
      setFormError("");
      navigateTo(intelligenceHref({
        start:draftStart,end:draftEnd,pipelineId,includeArchived,compare:compare&&!!draftStart
      }));
    }catch(error){setFormError(error instanceof Error?error.message:"Некорректные даты");}
  };
  return <>
    <PageHeader title="Аналитика бизнеса"
      description="Портфель всех авторизованных воронок, прогноз и причины операционных рисков. Полные показатели рассчитывает исходный Rust Sales owner, не неполные страницы сделок."/>
    <p><a href="#sales">Сделки и настройка воронок</a>{" · "}
      <a href={workdayHref()}>Рабочий день и действия</a></p>
    <form onSubmit={apply} aria-label="Фильтры аналитики">
      <label>Начало периода{" "}
        <input type="date" value={draftStart} onChange={e=>setDraftStart(e.target.value)}/></label>
      <label>Конец периода{" "}
        <input type="date" value={draftEnd} onChange={e=>setDraftEnd(e.target.value)}/></label>
      <Button type="submit">Применить период</Button>{" "}
      <Button type="button" variant="secondary" onClick={()=>{
        setDraftStart("");setDraftEnd("");setFormError("");
        navigateTo(intelligenceHref({pipelineId,includeArchived}));
      }}>Все даты</Button>
    </form>
    <p>Период (UTC, включительно) ограничивает прогноз по ожидаемой дате и
      фактическую конверсию по закрытию. Текущее число сделок и операционные риски
      показываются на момент запроса, независимо от периода. Это не исторический
      снимок состояния воронки.</p>
    {formError&&<p role="alert">{formError}</p>}
    <p>
      <label><input type="checkbox" checked={includeArchived} onChange={e=>
        navigateTo(intelligenceHref({start,end,pipelineId,compare,
          includeArchived:e.target.checked}))}/> Включить архивные воронки</label>{" · "}
      <label><input type="checkbox" checked={compare} disabled={!start||!end}
        onChange={e=>navigateTo(intelligenceHref({start,end,pipelineId,
          includeArchived,compare:e.target.checked}))}/>
        Сравнить с предыдущим периодом той же длительности</label>
    </p>
    <QueryNotice query={query}/>
    {query.error&&<Button type="button" variant="secondary"
      onClick={()=>void query.reload().catch(()=>{})}>Повторить полный запрос</Button>}
    {!query.busy&&!query.error&&data&&<div>
      <section aria-label="Общий портфель">
        <h2>Общий портфель</h2>
        <p>Воронок: {n(BigInt(slices.length))}{" · "}
          Сделок: {n(total)}{" · "}
          Открыто: {n(open)}{" · "}
          Выиграно: {n(won)}{" · "}
          Проиграно: {n(lost)}</p>
        <p>Показатели рассчитаны по всем записям выбранных воронок до любой
          пагинации. Каждый источник авторизуется по tenant. Неудачный запрос
          к одной воронке запрещает публикацию неполной общей суммы.</p>
        {slices.length===0&&<p>Доступных воронок нет. <a href="#sales">Создайте воронку</a>.</p>}
      </section>
      {slices.length>0&&<>
        <section aria-label="Операционные риски">
          <h2>Операционные риски открытых сделок</h2>
          <p>Просрочена ожидаемая дата закрытия: {n(metricTotal(slices,"overdueExpectedCloseCount"))}
            {" · "}Ожидается в ближайшие 30 дней: {n(metricTotal(slices,"dueNext30DaysCount"))}
            {" · "}Нет даты закрытия: {n(metricTotal(slices,"noExpectedCloseCount"))}
            {" · "}Есть связанная просроченная задача: {n(metricTotal(slices,"overdueLinkedTaskCount"))}</p>
          <p>Риск — факт из записей, не оценка AI. Сверяйте конкретные сделки
            и их задачи в исходной воронке.</p>
        </section>
        <section aria-label="Прогноз портфеля">
          <h2>Прогноз и категории</h2>
          <p>{start?start+" — "+end+" UTC":"Без ограничения по ожидаемой дате"}.
            Только открытые сделки. Категории не пересекаются; «Исключено»
            не входит в денежный прогноз. Взвешенная сумма = сумма × вероятность,
            это не статистический/AI-прогноз. Валюты не складываются друг с другом.</p>
          <table><thead><tr><th scope="col">Категория</th>
            <th scope="col">Сделок в периоде</th><th scope="col">Без даты</th></tr></thead>
            <tbody>{perCategory.map(cat=><tr key={cat.category}>
              <th scope="row">{categoryTitle(cat.category)}</th>
              <td>{n(cat.deals)}</td><td>{n(cat.unplanned)}</td>
            </tr>)}</tbody></table>
          <CurrencyTable title="Прогноз по валютам" rows={currencies} catalog={data.currencies}/>
          <CurrencyTable title={start?"Без даты закрытия (вне периода)":
            "Без даты закрытия (часть общего прогноза)"} rows={undated} catalog={data.currencies}/>
        </section>
        <section aria-label="Фактическая конверсия портфеля">
          <h2>Фактическая конверсия</h2>
          <p>Только сделки, которые сейчас закрыты и имеют подтверждённую дату
            закрытия в выбранном периоде; открытые снова исключены. Сравнение
            относится к датам закрытия, не к прошлым снимкам портфеля.</p>
          <p>Выиграно: {n(actual.won)}{" · "}Проиграно: {n(actual.lost)}
            {" · "}Знаменатель: {n(actual.denominator)}
            {" · "}Конверсия: {winRate(actual)}
            {" · "}Без подтверждённой даты: {n(actual.unknown)}</p>
          {earlier&&<p>Предыдущий период: {previousRange(start,end).start}
            {" — "}{previousRange(start,end).end} UTC.
            {" "}Выиграно: {n(earlier.won)}{" · "}Проиграно: {n(earlier.lost)}
            {" · "}Знаменатель: {n(earlier.denominator)}
            {" · "}Конверсия: {winRate(earlier)}</p>}
        </section>
        <section aria-label="Воронки и детализация">
          <h2>Воронки: сравнение и детализация</h2>
          <p>Переход открывает единственный исходный SalesScreen: этапы,
            карточки Deal, сохранённые представления и настройку под контролем
            Rust policy/CAS.</p>
          <table><thead><tr><th scope="col">Воронка</th>
            <th scope="col">Всего сделок</th><th scope="col">Открыто</th>
            <th scope="col">Просрочено закрытие</th>
            <th scope="col">Просроченные связанные задачи</th>
            <th scope="col">Перейти</th></tr></thead>
            <tbody>{slices.map(slice=>{
              const s=[slice],p=slice.pipeline;
              return <tr key={p.pipelineId}>
                <th scope="row">{p.name}{p.archived?" (архив)":""}</th>
                <td>{n(slice.current.totalDealCount)}</td>
                <td>{n(dealCount(s,DealStatus.OPEN))}</td>
                <td>{n(metricTotal(s,"overdueExpectedCloseCount"))}</td>
                <td>{n(metricTotal(s,"overdueLinkedTaskCount"))}</td>
                <td><a href={salesPipelineHref("",p.pipelineId)}>Открыть воронку {p.name}</a></td>
              </tr>;
            })}</tbody></table>
        </section>
        <section aria-label="Разрез по этапам">
          <h2>Разрез по этапам и статусам</h2>
          {slices.map(slice=><Card key={slice.pipeline.pipelineId} title={slice.pipeline.name}>
            <p>Время среза: {new Date(Number(slice.current.asOfUnixSeconds)*1000)
              .toISOString().slice(0,19)} UTC.{" "}
              <a href={salesPipelineHref("",slice.pipeline.pipelineId)}>
                Карточки и конфигурация воронки</a></p>
            <table><thead><tr><th scope="col">Этап</th><th scope="col">Статус</th>
              <th scope="col">Количество</th><th scope="col">Просрочено</th>
              <th scope="col">До 30 дней</th><th scope="col">Средний возраст этапа</th></tr></thead>
            <tbody>{slice.pipeline.stages.flatMap(stage=>
              [DealStatus.OPEN,DealStatus.WON,DealStatus.LOST].map(status=>{
                const cnt=slice.current.statuses.find(row=>row.stageId===stage.stageId&&row.status===status)?.dealCount??0n;
                const op=slice.current.operational.find(row=>row.stageId===stage.stageId&&row.status===status);
                if(cnt===0n)return null;
                return <tr key={stage.stageId+"/"+status}>
                  <th scope="row">{stage.name}{stage.archived?" (архив)":""}</th>
                  <td>{statusTitle(status)}</td><td>{n(cnt)}</td>
                  <td>{n(op?.overdueExpectedCloseCount??0n)}</td>
                  <td>{n(op?.dueNext30DaysCount??0n)}</td>
                  <td>{op&&op.knownStageAgeCount>0n
                    ?n(signed(op.totalStageAgeSeconds)/op.knownStageAgeCount/86400n)+" полных дней"
                    :"нет подтверждённой истории"}</td>
                </tr>;
              })
            )}</tbody></table>
          </Card>)}
        </section>
      </>}
      <section aria-label="Управление продажами">
        <h2>Управление структурой и представлениями</h2>
        <p>В одной действующей форме Sales owner доступны изменение и
          архивирование воронок/этапов, порядок стадий, вероятность,
          сохранённые представления, управление сделками и контроль версий.
          Роль и tenant проверяет сервер; этот аналитический экран только читает.</p>
        <p><a href="#sales">Настроить воронки и сохранённые представления</a>
          {" · "}<a href={workdayHref()}>Открыть рабочую очередь</a></p>
      </section>
    </div>}
  </>;
}

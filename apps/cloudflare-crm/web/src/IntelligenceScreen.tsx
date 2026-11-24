import {useCallback,useState} from "react";
import type {FormEvent} from "react";
import {Button,Card,PageHeader,SelectField,ScrollTable} from "@ultimate-crm/ui";
import {DealStatus,ForecastCategory} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
import {useQuery,QueryNotice} from "./hooks";
import {inputDate,parseDate} from "./calendarInput";
import {currencyCatalog,decimalUnits,weightedMoneyText,percent} from "./money";
import type {CurrencyCatalog} from "./money";
import {salesPipelineHref,intelligenceHref,workdayHref} from "./routes";
import {navigateTo} from "./navigation";
import {allOwners,ownerWire} from "./ownerCatalog";
import {SalesHistory} from "./SalesHistory";

// All portfolio facts are the generated Connect response. No browser aggregation.
type PortfolioResponse=Awaited<ReturnType<typeof api.deals.getSalesPortfolio>>;
type Portfolio={result:PortfolioResponse;currencies:CurrencyCatalog};
type CurrencyTotal=PortfolioResponse["forecastCurrencies"][number];
type Inputs={start:string;end:string;pipelineId:string;includeArchived:boolean;compare:boolean;owner?:string;historyZone?:string};
function n(value:bigint):string {return new Intl.NumberFormat("ru-RU").format(value);}
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
function winRate(value:NonNullable<PortfolioResponse["closedOutcomes"]>):string {
  return value.winRateBasisPoints===undefined?"недостаточно закрытых сделок":percent(value.winRateBasisPoints)+"%";
}

function CurrencyTable({title,rows,catalog}:{title:string;rows:CurrencyTotal[];
  catalog:CurrencyCatalog}) {
  return <section aria-label={title}>
    <h3>{title}</h3>
    {rows.length===0?<p>Указанных сумм нет.</p>:<ScrollTable label={title+" — таблица"}><table>
      <thead><tr><th scope="col">Валюта</th><th scope="col">Сумма</th>
        <th scope="col">Взвешено по вероятности</th><th scope="col">Сделок с суммой</th></tr></thead>
      <tbody>{rows.map(row=><tr key={row.currencyCode}>
        <th scope="row">{row.currencyCode}</th>
        <td>{decimalUnits(BigInt(row.totalMinorUnits),row.currencyCode,catalog)}</td>
        <td>{weightedMoneyText(row.weightedMinorUnitsTimesBasisPoints,row.currencyCode,catalog)}</td>
        <td>{n(row.amountDealCount)}</td>
      </tr>)}</tbody>
    </table></ScrollTable>}
  </section>;
}

/** SK-07: complete authorised native metrics, never partial Deal pages as analytics. */
export function IntelligenceScreen({start,end,pipelineId,includeArchived,compare,owner="",historyZone="UTC"}:Inputs) {
  const owners=useQuery(useCallback(allOwners,[]));
  const [draftStart,setDraftStart]=useState(start);
  const [draftEnd,setDraftEnd]=useState(end);
  const [formError,setFormError]=useState("");
  const query=useQuery(useCallback(async(signal:AbortSignal):Promise<Portfolio>=>{
    const analysisPeriod=start&&end?{startDate:parseDate(start),endDate:parseDate(end)}:undefined;
    const result=await api.deals.getSalesPortfolio({
      ...(pipelineId?{pipelineId}:{}),includeArchived,comparePrevious:compare,
      ...(analysisPeriod?{analysisPeriod}:{}),...(owner?{ownerFilter:ownerWire(owner)}:{})
    },{signal});
    if(!result.counts||!result.closedOutcomes||result.pipelines.some(p=>!p.pipeline||!p.counts)||
        (compare&&(!result.previousPeriod||!result.previousClosedOutcomes)))
      throw Error("Неполный типизированный ответ Sales owner");
    return {result,currencies:currencyCatalog(result.currencies)};
  },[start,end,pipelineId,includeArchived,compare,owner]));
  const data=query.data,slices=data?.result.pipelines??[];
  const counts=data?.result.counts,actual=data?.result.closedOutcomes;
  const earlier=data?.result.previousClosedOutcomes,prior=data?.result.previousPeriod;
  const apply=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();
    try{
      if(Boolean(draftStart)!==Boolean(draftEnd))throw Error("Укажите обе границы периода");
      if(draftStart&&(!parseDate(draftStart)||!parseDate(draftEnd)||draftStart>draftEnd))
        throw Error("Некорректный период");
      setFormError("");
      navigateTo(intelligenceHref({historyZone,
        start:draftStart,end:draftEnd,pipelineId,includeArchived,owner,compare:compare&&!!draftStart
      }));
    }catch(error){setFormError(error instanceof Error?error.message:"Некорректные даты");}
  };
  return <>
    <PageHeader title="Аналитика бизнеса"
      description="Портфель всех авторизованных воронок, прогноз и причины операционных рисков. Полные показатели рассчитывает исходный Rust Sales owner, не неполные страницы сделок."/>
    <p><a href="#sales">Сделки и настройка воронок</a>{" · "}
      <a href={workdayHref()}>Рабочий день и действия</a></p>
    <SalesHistory start={start} end={end} pipelineId={pipelineId} compare={compare} historyZone={historyZone} includeArchived={includeArchived} owner={owner}/>
    <QueryNotice query={owners}/>
    <SelectField label="Ответственный за портфель" value={owner} onChange={e=>navigateTo(intelligenceHref({historyZone,start,end,pipelineId,includeArchived,compare,owner:e.target.value}))}>
      <option value="">Все ответственные</option>{owners.data?.choices.map(c=><option key={c.value} value={c.value}>{c.label}</option>)}
    </SelectField>
    {owner&&<p>По текущему владельцу сделки, не историческая атрибуция выручки.</p>}
    <form onSubmit={apply} aria-label="Фильтры аналитики">
      <label>Начало периода{" "}
        <input type="date" value={draftStart} onChange={e=>setDraftStart(e.target.value)}/></label>
      <label>Конец периода{" "}
        <input type="date" value={draftEnd} onChange={e=>setDraftEnd(e.target.value)}/></label>
      <Button type="submit">Применить период</Button>{" "}
      <Button type="button" variant="secondary" onClick={()=>{
        setDraftStart("");setDraftEnd("");setFormError("");
        navigateTo(intelligenceHref({historyZone,pipelineId,includeArchived,owner}));
      }}>Все даты</Button>
    </form>
    <p>Период (UTC, включительно) ограничивает прогноз по ожидаемой дате и
      фактическую конверсию по закрытию. Текущее число сделок и операционные риски
      показываются на момент запроса, независимо от периода. Это не исторический
      снимок состояния воронки.</p>
    {formError&&<p role="alert">{formError}</p>}
    <p>
      <label><input type="checkbox" checked={includeArchived} onChange={e=>
        navigateTo(intelligenceHref({historyZone,start,end,pipelineId,compare,owner,
          includeArchived:e.target.checked}))}/> Включить архивные воронки</label>{" · "}
      <label><input type="checkbox" checked={compare} disabled={!start||!end}
        onChange={e=>navigateTo(intelligenceHref({historyZone,start,end,pipelineId,owner,
          includeArchived,compare:e.target.checked}))}/>
        Сравнить с предыдущим периодом той же длительности</label>
    </p>
    <QueryNotice query={query}/>
    {query.error&&<Button type="button" variant="secondary"
      onClick={()=>void query.reload().catch(()=>{})}>Повторить полный запрос</Button>}
    {!query.busy&&!query.error&&data&&counts&&actual&&<div>
      <section aria-label="Общий портфель">
        <h2>Общий портфель</h2>
        <p>Воронок: {n(counts.pipelineCount)}{" · "}
          Сделок: {n(counts.totalDealCount)}{" · "}
          Открыто: {n(counts.openCount)}{" · "}
          Выиграно: {n(counts.wonCount)}{" · "}
          Проиграно: {n(counts.lostCount)}</p>
        <p>Каталог, сделки, задачи, прогноз и сравнение рассчитаны Rust из одного
          снимка D1. Пагинация и браузер не участвуют в агрегации.
          Ошибка любого источника исключает публикацию неполного результата.</p>
        {slices.length===0&&<p>Доступных воронок нет. <a href="#sales">Создайте воронку</a>.</p>}
      </section>
      {slices.length>0&&<>
        <section aria-label="Операционные риски">
          <h2>Операционные риски открытых сделок</h2>
          <p>Просрочена ожидаемая дата закрытия: {n(counts.overdueExpectedCloseCount)}
            {" · "}Ожидается в ближайшие 30 дней: {n(counts.dueNext30DaysCount)}
            {" · "}Нет даты закрытия: {n(counts.noExpectedCloseCount)}
            {" · "}Есть связанная просроченная задача: {n(counts.overdueLinkedTaskCount)}</p>
          <p>Риск — факт из записей, не оценка AI. Сверяйте конкретные сделки
            и их задачи в исходной воронке.</p>
        </section>
        <section aria-label="Прогноз портфеля">
          <h2>Прогноз и категории</h2>
          <p>{start?start+" — "+end+" UTC":"Без ограничения по ожидаемой дате"}.
            Только открытые сделки. Категории не пересекаются; «Исключено»
            не входит в денежный прогноз. Взвешенная сумма = сумма × вероятность,
            это не статистический/AI-прогноз. Валюты не складываются друг с другом.</p>
          <ScrollTable label="Категории прогноза — таблица"><table><thead><tr><th scope="col">Категория</th>
            <th scope="col">Сделок в периоде</th><th scope="col">Без даты</th></tr></thead>
            <tbody>{data.result.forecastCategories.map(cat=><tr key={cat.category}>
              <th scope="row">{categoryTitle(cat.category)}</th>
              <td>{n(cat.dealCount)}</td><td>{n(cat.undatedDealCount)}</td>
            </tr>)}</tbody></table></ScrollTable>
          <CurrencyTable title="Прогноз по валютам" rows={data.result.forecastCurrencies} catalog={data.currencies}/>
          <CurrencyTable title={start?"Без даты закрытия (вне периода)":
            "Без даты закрытия (часть общего прогноза)"} rows={data.result.undatedForecastCurrencies} catalog={data.currencies}/>
        </section>
        <section aria-label="Фактическая конверсия портфеля">
          <h2>Фактическая конверсия</h2>
          <p>Только сделки, которые сейчас закрыты и имеют подтверждённую дату
            закрытия в выбранном периоде; открытые снова исключены. Сравнение
            относится к датам закрытия, не к прошлым снимкам портфеля.</p>
          <p>Выиграно: {n(actual.wonCount)}{" · "}Проиграно: {n(actual.lostCount)}
            {" · "}Знаменатель: {n(actual.denominator)}
            {" · "}Конверсия: {winRate(actual)}
            {" · "}Без подтверждённой даты: {n(actual.unknownCloseCount)}</p>
          {earlier&&prior&&<p>Предыдущий период: {inputDate(prior.startDate)}
            {" — "}{inputDate(prior.endDate)} UTC.
            {" "}Выиграно: {n(earlier.wonCount)}{" · "}Проиграно: {n(earlier.lostCount)}
            {" · "}Знаменатель: {n(earlier.denominator)}
            {" · "}Конверсия: {winRate(earlier)}</p>}
        </section>
        <section aria-label="Воронки и детализация">
          <h2>Воронки: сравнение и детализация</h2>
          <p>Переход открывает единственный исходный SalesScreen: этапы,
            карточки Deal, сохранённые представления и настройку под контролем
            Rust policy/CAS.</p>
          <ScrollTable label="Портфель воронок — таблица"><table><thead><tr><th scope="col">Воронка</th>
            <th scope="col">Всего сделок</th><th scope="col">Открыто</th>
            <th scope="col">Просрочено закрытие</th>
            <th scope="col">Просроченные связанные задачи</th>
            <th scope="col">Перейти</th></tr></thead>
            <tbody>{slices.map(slice=>{
              const p=slice.pipeline,c=slice.counts;
              if(!p||!c)return null;
              return <tr key={p.pipelineId}>
                <th scope="row">{p.name}{p.archived?" (архив)":""}</th>
                <td>{n(c.totalDealCount)}</td>
                <td>{n(c.openCount)}</td>
                <td>{n(c.overdueExpectedCloseCount)}</td>
                <td>{n(c.overdueLinkedTaskCount)}</td>
                <td><a href={salesPipelineHref("",p.pipelineId,"",owner)}>Открыть воронку {p.name}</a></td>
              </tr>;
            })}</tbody></table></ScrollTable>
        </section>
        <section aria-label="Разрез по этапам">
          <h2>Разрез по этапам и статусам</h2>
          {slices.map(slice=>{
            const p=slice.pipeline;
            if(!p)return null;
            return <Card key={p.pipelineId} title={p.name}>
              <p>Время среза: {new Date(Number(data.result.asOfUnixSeconds)*1000)
                .toISOString().slice(0,19)} UTC.{" "}
                <a href={salesPipelineHref("",p.pipelineId,"",owner)}>
                  Карточки и конфигурация воронки</a></p>
              <ScrollTable label={"Этапы "+p.name+" — таблица"}><table><thead><tr><th scope="col">Этап</th><th scope="col">Статус</th>
                <th scope="col">Количество</th><th scope="col">Просрочено</th>
                <th scope="col">До 30 дней</th><th scope="col">Средний возраст этапа</th></tr></thead>
                <tbody>{slice.stages.map(row=><tr key={row.stageId+"/"+row.status}>
                  <th scope="row">{row.stageName}{row.stageArchived?" (архив)":""}</th>
                  <td>{statusTitle(row.status)}</td><td>{n(row.dealCount)}</td>
                  <td>{n(row.overdueExpectedCloseCount)}</td>
                  <td>{n(row.dueNext30DaysCount)}</td>
                  <td>{row.averageStageAgeDays===undefined?"нет подтверждённой истории":
                    n(row.averageStageAgeDays)+" полных дней"}</td>
                </tr>)}</tbody></table></ScrollTable>
            </Card>;
          })}
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

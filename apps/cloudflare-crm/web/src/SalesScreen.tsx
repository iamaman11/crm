import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Deal,SalesPipeline,SalesDealView} from "@ultimate-crm/client/cloudflare";
import {DealStatus,ProbabilitySource,ForecastCategory} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice} from "./hooks";
import {AccountPicker} from "./Pickers";
import {DealContactPicker,DealDetails,inputDate,parseDate,forecastCategoryText,probabilitySourceText} from "./DealRecord";
import {dealHref} from "./routes";
import {minorUnits,decimalUnits,moneyText,basisPoints,percent,weightedMoneyText} from "./money";
import styles from "./App.module.css";

type EditorMode="create"|"edit"|"advance"|"close"|"reopen"|null;
type StageDraft={stageId:string;name:string;probability:string;archived:boolean};
function stageDrafts(pipeline?:SalesPipeline):StageDraft[] {
  return pipeline?.stages.map(stage=>({
    stageId:stage.stageId,name:stage.name,probability:percent(stage.probabilityBasisPoints),
    archived:stage.archived
  }))??[
    {stageId:newId(),name:"Первичный контакт",probability:"10",archived:false},
    {stageId:newId(),name:"Квалификация",probability:"40",archived:false}
  ];
}
async function allPipelines():Promise<SalesPipeline[]> {
  const all:SalesPipeline[]=[];
  let cursor="";
  const seen=new Set<string>();
  do {
    const page=await api.deals.listPipelines({page:{pageSize:100,pageToken:cursor},includeArchived:true});
    all.push(...page.pipelines);
    cursor=page.pageInfo?.nextPageToken??"";
    if(cursor&&seen.has(cursor))throw Error("Повторный курсор воронок");
    if(cursor)seen.add(cursor);
  } while(cursor);
  return all;
}
async function allSalesViews():Promise<SalesDealView[]> {
  const all:SalesDealView[]=[];
  let cursor="";
  const seen=new Set<string>();
  do {
    const page=await api.deals.listSalesViews({page:{pageSize:100,pageToken:cursor}});
    all.push(...page.views);
    cursor=page.pageInfo?.nextPageToken??"";
    if(cursor&&seen.has(cursor))throw Error("Повторный курсор представлений");
    if(cursor)seen.add(cursor);
  } while(cursor);
  return all;
}
export function SalesScreen({initialAccountId="",focusedDealId=""}:{
  initialAccountId?:string;focusedDealId?:string;
}) {
  const [stageCursors,setStageCursors]=useState<Record<string,string>>({});
  // Native DnD is presentation only: the existing confirmed advance form
  // remains the ONE mutation path (no optimistic Deal update).
  const [draggingDeal,setDraggingDeal]=useState<{
    dealId:string;pipelineId:string;stageId:string
  }|null>(null);
  const [droppedStageId,setDroppedStageId]=useState("");
  const [allDealsCursor,setAllDealsCursor]=useState("");
  const [status,setStatus]=useState("");
  const [accountId,setAccountId]=useState(initialAccountId);
  const [pipelineId,setPipelineId]=useState("");
  const [analysisStart,setAnalysisStart]=useState("");
  const [analysisEnd,setAnalysisEnd]=useState("");
  const [analysisPeriod,setAnalysisPeriod]=useState<{
    startDate:NonNullable<ReturnType<typeof parseDate>>;
    endDate:NonNullable<ReturnType<typeof parseDate>>;
  }>();
  const [analysisError,setAnalysisError]=useState("");
  const [selectedViewId,setSelectedViewId]=useState("");
  const [viewName,setViewName]=useState("");
  const [viewDraftId,setViewDraftId]=useState(newId);
  const [createViewOpen,setCreateViewOpen]=useState(false);
  const [selected,setSelected]=useState<Deal>();
  const [advance,setAdvance]=useState<Deal>();
  const [editorMode,setEditorMode]=useState<EditorMode>(null);
  const [draftId,setDraftId]=useState(newId);
  const [formAccountId,setFormAccountId]=useState("");
  const [probabilityChoice,setProbabilityChoice]=useState(String(ProbabilitySource.STAGE_DEFAULT));
  const [expandedDeal,setExpandedDeal]=useState("");
  const [detailRevision,setDetailRevision]=useState(0);
  const [pipelineEditor,setPipelineEditor]=useState<SalesPipeline|"new"|null>(null);
  const [pipelineDraftId,setPipelineDraftId]=useState<string>(newId);
  const [pipelineName,setPipelineName]=useState("");
  const [pipelineArchived,setPipelineArchived]=useState(false);
  const [pipelineDirty,setPipelineDirty]=useState(false);
  const [stageEdits,setStageEdits]=useState<StageDraft[]>([]);
  const mutation=useMutation();
  const pipelineMutation=useMutation();
  const viewMutation=useMutation();
  const savedViewsQuery=useQuery(useCallback(allSalesViews,[]));
  const savedViews=savedViewsQuery.data??[];
  const selectedView=savedViews.find(view=>view.viewId===selectedViewId);
  const viewStatus=()=>status?enumValue(status,
    [DealStatus.OPEN,DealStatus.WON,DealStatus.LOST],"Статус сделки"):undefined;
  const currentViewFilters=()=>{
    const selectedStatus=viewStatus();
    return {
      ...(pipelineId?{pipelineId}:{}),
      ...(selectedStatus!==undefined?{status:selectedStatus}:{}),
      ...(accountId?{accountId}:{})
    };
  };
  const applySavedView=(id:string)=>{
    const view=savedViews.find(entry=>entry.viewId===id);
    if(!view){setSelectedViewId("");return;}
    setSelectedViewId(view.viewId);
    setViewName(view.name);
    setPipelineId(view.pipelineId??"");
    setStatus(view.status===undefined?"":String(view.status));
    setAccountId(view.accountId??"");
    setStageCursors({});
    setAllDealsCursor("");
    viewMutation.changed();
    setCreateViewOpen(false);
  };
  const saveView=(archive=false,asNew=false)=>{
    const old=selectedView;
    void viewMutation.run(()=>asNew?
      api.deals.createSalesView({
        viewId:viewDraftId,name:viewName,...currentViewFilters(),
        precondition:{idempotencyKey:viewMutation.key}
      }):old?api.deals.updateSalesView({
        viewId:old.viewId,name:viewName,...currentViewFilters(),
        archived:archive,
        precondition:{idempotencyKey:viewMutation.key,expectedVersion:old.version}
      }):Promise.reject(Error("Представление не выбрано")),
      async()=>{
        const createdId=viewDraftId;
        await savedViewsQuery.reload();
        if(asNew){setSelectedViewId(createdId);setViewDraftId(newId());setCreateViewOpen(false);}
        if(archive){setSelectedViewId("");setViewName("");setCreateViewOpen(false);}
      }
    );
  };
  const pipelinesQuery=useQuery(useCallback(allPipelines,[]));
  const pipelines=pipelinesQuery.data??[];
  const activePipeline=pipelines.find(p=>p.pipelineId===pipelineId);
  const query=useQuery(useCallback(async()=>{
    if(!activePipeline)return [];
    const selectedStatus=status?enumValue(status,[DealStatus.OPEN,DealStatus.WON,DealStatus.LOST],"Статус сделки"):undefined;
    return Promise.all(activePipeline.stages.map(async stage=>{
      const result=await api.deals.listStageDeals({
        pipelineId:activePipeline.pipelineId,stageId:stage.stageId,
        page:{pageSize:25,pageToken:stageCursors[stage.stageId]??""},
        ...(selectedStatus!==undefined?{status:selectedStatus}:{}),
        ...(accountId?{accountId}:{})
      });
      return {stageId:stage.stageId,...result};
    }));
  },[activePipeline,stageCursors,status,accountId]));
  const metricsQuery=useQuery(useCallback(async()=>{
    if(!activePipeline)return undefined;
    const selectedStatus=status?enumValue(status,[DealStatus.OPEN,DealStatus.WON,DealStatus.LOST],"Статус сделки"):undefined;
    return api.deals.getPipelineMetrics({
      pipelineId:activePipeline.pipelineId,
      ...(accountId?{accountId}:{}),
      ...(selectedStatus!==undefined?{status:selectedStatus}:{}),
      ...(analysisPeriod?{analysisPeriod}:{})
    });
  },[activePipeline,accountId,status,analysisPeriod]));
  // The canonical ListDeals RPC already supports a tenant-wide cursor and
  // optional Account/status filters. It is the default all-pipelines overview;
  // the independent ListStageDeals query is only for a selected Kanban board.
  const allDealsQuery=useQuery(useCallback(async()=>{
    const selectedStatus=status?enumValue(status,[DealStatus.OPEN,DealStatus.WON,DealStatus.LOST],"Статус сделки"):undefined;
    return api.deals.listDeals({
      page:{pageSize:25,pageToken:allDealsCursor},
      ...(accountId?{accountId}:{}),
      ...(selectedStatus!==undefined?{status:selectedStatus}:{})
    });
  },[accountId,allDealsCursor,status]));
  const refreshSales=async()=>{
    await Promise.all([query.reload(),allDealsQuery.reload(),metricsQuery.reload()]);
  };
  const open=async(dealId:string,mode:"edit"|"advance"|"close"|"reopen",droppedOn="")=>{
    try {
      const value=(await api.deals.getDeal({dealId})).deal;
      if(!value)throw Error("Сделка не найдена");
      setSelected(mode==="edit"?value:undefined);
      if(mode==="edit")setProbabilityChoice(value.probabilitySource===ProbabilitySource.STAGE_DEFAULT?"keep":String(ProbabilitySource.MANUAL));
      setAdvance(mode==="edit"?undefined:value);
      setDroppedStageId(mode==="advance"?droppedOn:"");
      setEditorMode(mode);mutation.changed();
    } catch(error) {await mutation.run(()=>Promise.reject(error));}
  };
  const editPipeline=async(pipeline?:SalesPipeline)=>{
    try {
      // A catalog mutation may have committed before the React list re-renders.
      // Always open an existing editor with the current CAS version from Rust,
      // never with a stale render's SalesPipeline snapshot.
      const current=pipeline?(await api.deals.getPipeline({pipelineId:pipeline.pipelineId})).pipeline:undefined;
      if(pipeline&&!current)throw Error("Воронка не найдена");
      setPipelineEditor(current??"new");
      setPipelineDraftId(current?.pipelineId??newId());
      setPipelineName(current?.name??"");
      setPipelineArchived(current?.archived??false);
      setStageEdits(stageDrafts(current));
      setPipelineDirty(false);
      pipelineMutation.changed();
    } catch(error) {await pipelineMutation.run(()=>Promise.reject(error));}
  };
  const changeStage=(index:number,change:Partial<StageDraft>)=>{
    setStageEdits(before=>before.map((s,i)=>i===index?{...s,...change}:s));
    pipelineMutation.changed();setPipelineDirty(true);
  };
  const shiftStage=(index:number,offset:number)=>{
    setStageEdits(before=>{
      const target=index+offset;
      if(target<0||target>=before.length)return before;
      const next=[...before];
      const first=next[index],second=next[target];
      if(!first||!second)return before;
      next[index]=second;next[target]=first;
      return next;
    });pipelineMutation.changed();setPipelineDirty(true);
  };
  const columns=activePipeline?.stages??[];
  const advancePipeline=pipelines.find(p=>p.pipelineId===advance?.stage?.pipelineId);
  const nextStages=!advancePipeline?.archived?advancePipeline?.stages.filter(s=>!s.archived&&s.ordinal>(advance?.stage?.ordinal??0))??[]:[];
  const readyStage=!activePipeline?.archived?activePipeline?.stages.find(s=>!s.archived):undefined;
  return <>
    {focusedDealId?<section>
      <PageHeader title="Карточка сделки" actions={<a href="#sales">К списку сделок</a>}/>
      <DealDetails key={focusedDealId+":"+detailRevision} dealId={focusedDealId}
        onEdit={()=>void open(focusedDealId,"edit")}
        onClose={()=>void open(focusedDealId,"close")}
        onReopen={()=>void open(focusedDealId,"reopen")}/>
    </section>:<>
    <PageHeader title="Сделки" description="Сначала все сделки из всех воронок. Выберите воронку для Kanban: количество по этапам вычисляется на сервере, колонки имеют независимую пагинацию." actions={<>
      <Button variant="secondary" onClick={()=>void editPipeline()}>Создать воронку</Button>{" "}
      {activePipeline&&<Button variant="secondary" onClick={()=>void editPipeline(activePipeline)}>Настроить воронку</Button>}{" "}
      <Button disabled={!activePipeline||!readyStage} onClick={()=>{
        setSelected(undefined);setAdvance(undefined);setFormAccountId("");
        setProbabilityChoice(String(ProbabilitySource.STAGE_DEFAULT));
        setEditorMode("create");mutation.changed();
      }}>Новая сделка</Button>
    </>}/>
    <QueryNotice query={pipelinesQuery}/>
    <section aria-label="Сохранённые представления">
      <h2>Сохранённые представления</h2>
      <p>Именованные фильтры доступны в этом рабочем пространстве. Применение
        использует существующие запросы сделок и аналитики, без копирования данных.</p>
      <QueryNotice query={savedViewsQuery}/>
      <SelectField label="Сохранённое представление" value={selectedViewId}
        onChange={e=>applySavedView(e.target.value)}>
        <option value="">Ручные фильтры</option>
        {savedViews.map(view=><option value={view.viewId} key={view.viewId}>{view.name}</option>)}
      </SelectField>
      {" "}
      <Button variant="secondary" onClick={()=>{
        if(!selectedView)setViewName("");
        viewMutation.changed();setCreateViewOpen(true);
      }}>Сохранить как новое</Button>
      {selectedView&&<>
        {" "}
        <Button variant="secondary" disabled={viewMutation.busy}
          onClick={()=>saveView()}>Обновить представление</Button>
        {" "}
        <Button variant="secondary" disabled={viewMutation.busy}
          onClick={()=>saveView(true)}>Архивировать представление</Button>
      </>}
      {(selectedView||createViewOpen)&&<TextField label="Название представления"
        value={viewName} onChange={e=>{setViewName(e.target.value);viewMutation.changed();}}
        required/>}
      {createViewOpen&&<p>
        <Button disabled={viewMutation.busy||!viewName.trim()}
          onClick={()=>saveView(false,true)}>Сохранить новое представление</Button>{" "}
        <Button variant="secondary" onClick={()=>setCreateViewOpen(false)}>Отмена</Button>
      </p>}
      <MutationNotice state={viewMutation}/>
    </section>
    {!pipelinesQuery.busy&&pipelines.length===0&&
      <p>Сначала создайте воронку с этапами. Сделки нельзя создавать с произвольными кодами этапов.</p>}
    <SelectField label="Воронка продаж" value={pipelineId} onChange={e=>{
      setPipelineId(e.target.value);setStageCursors({});viewMutation.changed();
    }}>
      <option value="">Все воронки — обзор</option>
      {pipelines.map(p=><option value={p.pipelineId} key={p.pipelineId}>
        {p.name}{p.archived?" (архив)":""}
      </option>)}
    </SelectField>
    <TextField label="Фильтр по ID клиента" value={accountId} onChange={e=>{setAccountId(e.target.value);setStageCursors({});setAllDealsCursor("");viewMutation.changed();}}/>
    <SelectField label="Статус сделки" value={status} onChange={e=>{setStatus(e.target.value);setStageCursors({});setAllDealsCursor("");viewMutation.changed();}}>
      <option value="">Все</option><option value={DealStatus.OPEN}>Открытые</option>
      <option value={DealStatus.WON}>Выигранные</option><option value={DealStatus.LOST}>Проигранные</option>
    </SelectField>
    <QueryNotice query={query}/>
    {activePipeline&&<section aria-label="Аналитика воронки">
      <h2>Показатели всей воронки · {activePipeline.name}</h2>
      <p>Весь выбранный фильтр, включая сделки вне текущих страниц Kanban.
        Валюты и статусы не смешиваются. Взвешенная сумма = сумма × вероятность сделки;
        это расчёт, а не статистический прогноз. Просроченные даты, открытые связанные
        задачи и длительность текущего этапа учитываются только для открытых сделок.
        Время этапа начинается с реального события создания или перемещения.</p>
      <QueryNotice query={metricsQuery}/>
      {metricsQuery.data&&<>
        <p>
          <span>Всего сделок: {metricsQuery.data.totalDealCount.toString()}</span>
          <span>{" · срез по UTC: "}
            {new Date(Number(metricsQuery.data.asOfUnixSeconds)*1000).toISOString().slice(0,16)}
          </span>
        </p>
        {activePipeline.stages.map(stage=>{
          const groups=metricsQuery.data?.statuses.filter(m=>m.stageId===stage.stageId)??[];
          return <div key={stage.stageId}>
            <h3>{stage.name}{stage.archived?" (архив)":""}</h3>
            {groups.length===0?<p>Сделок нет</p>:groups.map(group=>{
              const title=group.status===DealStatus.OPEN?"Открытые":
                group.status===DealStatus.WON?"Выигранные":"Проигранные";
              const currencies=metricsQuery.data?.currencies.filter(m=>
                m.stageId===stage.stageId&&m.status===group.status)??[];
              const operational=metricsQuery.data?.operational.find(m=>
                m.stageId===stage.stageId&&m.status===group.status);
              return <div key={String(group.status)} role="group"
                aria-label={stage.name+" · "+title}>
                <p>{title}: {group.dealCount.toString()} сделок</p>
                {operational&&<div role="group" aria-label="Операционные показатели">
                  <p>Ожидаемое закрытие: просрочено {operational.overdueExpectedCloseCount.toString()}
                    {" · в ближайшие 30 дней "}{operational.dueNext30DaysCount.toString()}
                    {" · позднее "}{operational.dueLaterCount.toString()}
                    {" · дата не указана "}{operational.noExpectedCloseCount.toString()}
                  </p>
                  <p>Просроченная открытая связанная задача:
                    {" "}{operational.overdueLinkedTaskCount.toString()} сделок
                  </p>
                  <p>Среднее время на текущем этапе:
                    {" "}{operational.knownStageAgeCount>0n
                      ?(BigInt(operational.totalStageAgeSeconds)/
                          operational.knownStageAgeCount/86400n).toString()+" полных дней"
                      :"нет проверенной истории"}
                    {" · подтверждённая история у "}{operational.knownStageAgeCount.toString()}
                    {" из "}{group.dealCount.toString()} сделок
                  </p>
                </div>}
                {currencies.length===0?<p>Суммы не указаны</p>:currencies.map(total=>
                  <p key={total.currencyCode}>
                    {decimalUnits(BigInt(total.totalMinorUnits),total.currencyCode)} {total.currencyCode}
                    {" · взвешено: "}{weightedMoneyText(total.weightedMinorUnitsTimesBasisPoints,total.currencyCode)}
                    {" · сделок с суммой: "}{total.amountDealCount.toString()}
                  </p>)}
              </div>;
            })}
          </div>;
        })}
      </>}
    </section>}
    {activePipeline&&<section aria-label="Период аналитики">
      <h2>Период аналитики</h2>
      <form onSubmit={event=>{
        event.preventDefault();
        try {
          const startDate=parseDate(analysisStart),endDate=parseDate(analysisEnd);
          if(!startDate||!endDate||analysisStart>analysisEnd)throw Error("Укажите начало и конец периода в правильном порядке");
          setAnalysisPeriod({startDate,endDate});setAnalysisError("");
        } catch(error){setAnalysisError(error instanceof Error?error.message:"Неверный период");}
      }}>
        <TextField label="Начало периода" type="date" value={analysisStart}
          onChange={event=>setAnalysisStart(event.target.value)} required/>
        <TextField label="Конец периода" type="date" value={analysisEnd}
          onChange={event=>setAnalysisEnd(event.target.value)} required/>
        <Button type="submit">Применить период</Button>
        <Button type="button" variant="secondary" onClick={()=>{
          setAnalysisPeriod(undefined);setAnalysisStart("");setAnalysisEnd("");setAnalysisError("");
        }}>Без ограничения по сроку</Button>
      </form>
      {analysisError&&<p role="alert">{analysisError}</p>}
      <p>Период ограничивает прогноз и конверсию, не список и Kanban.
        Фильтры клиента, воронки и статуса остаются общими; период — отдельный параметр аналитики.</p>
    </section>}
    {activePipeline&&metricsQuery.data&&!metricsQuery.busy&&!metricsQuery.error&&<section aria-label="Прогноз продаж">
      <h2>Прогноз продаж · {activePipeline.name}</h2>
      <p>Только открытые сделки выбранного фильтра, {metricsQuery.data.analysisPeriod
        ?inputDate(metricsQuery.data.analysisPeriod.startDate)+" — "+inputDate(metricsQuery.data.analysisPeriod.endDate)+" (включительно)"
        :"без ограничения по сроку"}.
        Категории не пересекаются; исключённые показаны отдельно.
        Взвешивание — арифметическая оценка, а не статистический прогноз.</p>
      {[{key:"forecast",title:"По категориям",groups:metricsQuery.data.forecastCategories,currencies:metricsQuery.data.forecastCurrencies},
        {key:"undated",title:metricsQuery.data.analysisPeriod?"Без даты закрытия (вне периода)":"Без даты закрытия (часть общего прогноза)",
          groups:metricsQuery.data.undatedForecastCategories,currencies:metricsQuery.data.undatedForecastCurrencies}].map(bucket=>
        <div key={bucket.key} role="region" aria-label={bucket.title}>
          {bucket.key==="undated"&&<h3>{bucket.title}</h3>}
          {[ForecastCategory.PIPELINE,ForecastCategory.BEST_CASE,ForecastCategory.COMMIT,ForecastCategory.OMITTED].map(category=>{
            const group=bucket.groups.find(m=>m.category===category);
            const totals=bucket.currencies.filter(m=>m.category===category);
            return <div key={category} role="group" aria-label={bucket.key==="forecast"?forecastCategoryText(category):bucket.title+" · "+forecastCategoryText(category)}>
              <h3>{forecastCategoryText(category)}</h3>
              <p>Сделок: {(group?.dealCount??0n).toString()} · по этапу: {(group?.stageDefaultCount??0n).toString()} · вручную: {(group?.manualCount??0n).toString()}</p>
              {totals.length===0?<p>Суммы не указаны</p>:totals.map(total=><p key={total.currencyCode}>
                {decimalUnits(BigInt(total.totalMinorUnits),total.currencyCode)} {total.currencyCode}
                {" · взвешено: "}{weightedMoneyText(total.weightedMinorUnitsTimesBasisPoints,total.currencyCode)}
                {" · сделок с суммой: "}{total.amountDealCount.toString()}
              </p>)}
            </div>;
          })}
        </div>)}
    </section>}
    {activePipeline&&metricsQuery.data&&!metricsQuery.busy&&!metricsQuery.error&&<section aria-label="Фактическая конверсия">
      <h2>Фактическая конверсия · {activePipeline.name}</h2>
      <p>Доля выигранных среди сделок, которые сейчас закрыты и имеют фактическую дату закрытия в выбранном периоде (UTC).
        Повторно открытые сделки не входят в когорту; результат не является историей переходов.</p>
      {metricsQuery.data.closedOutcomes?<>
        <p>Выиграно: {metricsQuery.data.closedOutcomes.wonCount.toString()}
          {" · проиграно: "}{metricsQuery.data.closedOutcomes.lostCount.toString()}
          {" · знаменатель: "}{metricsQuery.data.closedOutcomes.denominator.toString()}</p>
        <p>Конверсия: {metricsQuery.data.closedOutcomes.winRateBasisPoints===undefined
          ?"Нет закрытых сделок с подтверждённой датой"
          :percent(metricsQuery.data.closedOutcomes.winRateBasisPoints)+"%"}</p>
        <p>Закрытые без подтверждённой даты (вне знаменателя): {metricsQuery.data.closedOutcomes.unknownCloseCount.toString()}</p>
      </>:<p>Для расчёта конверсии выберите все статусы сделок.</p>}
    </section>}
    {(!pipelineId||accountId)&&<section aria-label={accountId?"Сделки клиента во всех воронках":"Сделки во всех воронках"}>
      <h2>{accountId?"Все сделки клиента во всех воронках":"Все сделки во всех воронках"}</h2>
      <p>Показано {allDealsQuery.data?.deals.length??0} сделок на текущей странице. Это общий список, независимо от выбранной доски; полный итог не подменяется размером страницы.</p>
      <QueryNotice query={allDealsQuery}/>
      {allDealsQuery.data?.deals.map(deal=>{
        const pipeline=pipelines.find(p=>p.pipelineId===deal.stage?.pipelineId);
        const stage=pipeline?.stages.find(s=>s.stageId===deal.stage?.stageId);
        return <Card key={deal.dealId} title={deal.name}>
          <Button variant="secondary" onClick={()=>setExpandedDeal(old=>old===deal.dealId?"":deal.dealId)}>
            {expandedDeal===deal.dealId?"Свернуть":"Раскрыть карточку"}
          </Button>{" "}
          <a href={dealHref(deal.dealId)}>Полная страница</a>
          <p>Воронка: {pipeline?.name??deal.stage?.pipelineId??"неизвестна"} · Этап: {stage?.name??deal.stage?.stageId??"неизвестен"}</p>
          {pipeline&&<Button variant="secondary" onClick={()=>{setPipelineId(pipeline.pipelineId);setStageCursors({});}}>Открыть воронку</Button>}
          <p>{moneyText(deal.amount)} · {percent(deal.probabilityBasisPoints)}%</p>
          {deal.status===DealStatus.OPEN&&<p>{probabilitySourceText(deal.probabilitySource)} · {forecastCategoryText(deal.forecastCategory)}</p>}
          <p>{deal.status===DealStatus.OPEN?"Открыта":deal.status===DealStatus.WON?"Выиграна":"Проиграна"} · версия {deal.version.toString()}</p>
          {deal.status===DealStatus.OPEN?<>
            <Button variant="secondary" onClick={()=>void open(deal.dealId,"edit")}>Редактировать</Button>{" "}
            <Button variant="secondary" onClick={()=>void open(deal.dealId,"close")}>Завершить</Button>
          </>:<Button variant="secondary" onClick={()=>void open(deal.dealId,"reopen")}>Открыть повторно</Button>}
          {expandedDeal===deal.dealId&&<DealDetails key={deal.dealId+":"+detailRevision}
            dealId={deal.dealId} onEdit={()=>void open(deal.dealId,"edit")}
            onClose={()=>void open(deal.dealId,"close")}
             onReopen={()=>void open(deal.dealId,"reopen")}/>}
        </Card>;
      })}
      {!allDealsQuery.busy&&allDealsQuery.data?.deals.length===0&&
        <p>{accountId?"У клиента нет сделок для выбранного статуса.":"Сделок для выбранного статуса пока нет."}</p>}
      {allDealsQuery.data?.pageInfo?.nextPageToken&&
        <Button variant="secondary" disabled={allDealsQuery.busy}
          onClick={()=>setAllDealsCursor(allDealsQuery.data?.pageInfo?.nextPageToken??"")}>
          Следующие сделки
        </Button>}
      {allDealsCursor&&<Button variant="secondary" disabled={allDealsQuery.busy}
        onClick={()=>setAllDealsCursor("")}>К началу списка</Button>}
    </section>}
    {!activePipeline&&pipelines.length>0&&<p>Для работы с Kanban выберите конкретную воронку. Общий обзор показывает все сделки без выбора этапа.</p>}
    {activePipeline&&<>
      <p>Перетащите открытую сделку в следующий этап. Для клавиатуры или сенсорного экрана используйте «Изменить этап» и подтвердите переход в единой форме. Суммы по валютам рассчитаны сервером по всем подходящим сделкам этапа, независимо от страницы карточек.</p>
      <div className={styles.board} role="region"
        aria-label="Kanban — этапы и сделки" tabIndex={0}>
      {columns.map(stage=>{
        const boardPage=query.data?.find(entry=>entry.stageId===stage.stageId);
        const source=columns.find(s=>s.stageId===draggingDeal?.stageId);
        const acceptsDrop=!!draggingDeal&&draggingDeal.pipelineId===activePipeline.pipelineId&&
          !activePipeline.archived&&!stage.archived&&!!source&&
          stage.ordinal>source.ordinal;
        return <section key={stage.stageId} aria-label={"Этап "+stage.name}
          className={styles.boardColumn+(acceptsDrop?" "+styles.boardDropReady:"")}
          onDragOver={event=>{
            if(acceptsDrop){event.preventDefault();event.dataTransfer.dropEffect="move";}
          }}
          onDrop={event=>{
            const moving=draggingDeal;
            const dragId=event.dataTransfer.getData("application/x-crm-deal");
            setDraggingDeal(null);
            if(!acceptsDrop||!moving||dragId!==moving.dealId)return;
            event.preventDefault();
            void open(moving.dealId,"advance",stage.stageId);
          }}>
          <h2>{stage.name}{stage.archived?" (архив)":""} · {boardPage?.totalCount.toString()??"—"}</h2>
          {!stage.archived&&<div className={styles.boardStageDropTarget}
            role="group" aria-label={"Зона переноса в этап "+stage.name}>
            {acceptsDrop?"Отпустите сделку здесь":"Зона переноса"}
          </div>}
          {boardPage&&<div role="group" aria-label={"Итоги этапа "+stage.name}>
            {boardPage.currencyTotals.map(total=><p key={total.currencyCode}>
              {decimalUnits(BigInt(total.totalMinorUnits),total.currencyCode)} {total.currencyCode}
              {" · сделок с суммой: "}{total.amountDealCount.toString()}
            </p>)}
            {boardPage.currencyTotals.length===0&&<p>Суммы не указаны</p>}
          </div>}
          {boardPage?.deals.map(d=><Card key={d.dealId} title={d.name}>
            <Button variant="secondary" onClick={()=>setExpandedDeal(old=>old===d.dealId?"":d.dealId)}>
              {expandedDeal===d.dealId?"Свернуть":"Раскрыть карточку"}
            </Button>{" "}
            <a href={dealHref(d.dealId)}>Полная страница</a>
            {d.status===DealStatus.OPEN&&!activePipeline.archived&&
              <span className={styles.dealDragHandle} draggable
                aria-label={"Перетащить сделку "+d.name}
                title="Перетащить к следующему этапу; затем подтвердить изменение"
                onDragStart={event=>{
                  event.dataTransfer.effectAllowed="move";
                  event.dataTransfer.setData("application/x-crm-deal",d.dealId);
                  setDraggingDeal({dealId:d.dealId,
                    pipelineId:activePipeline.pipelineId,stageId:stage.stageId});
                }}
                onDragEnd={()=>setDraggingDeal(null)}>
                Перетащить
              </span>}
            <p>{moneyText(d.amount)} · {percent(d.probabilityBasisPoints)}%</p>
            {d.status===DealStatus.OPEN&&<p>{probabilitySourceText(d.probabilitySource)} · {forecastCategoryText(d.forecastCategory)}</p>}
            <p>Клиент: {d.accountId}</p>
            <p>{d.status===DealStatus.WON?"Выиграна":d.status===DealStatus.LOST?"Проиграна":
              d.status===DealStatus.OPEN?"Открыта":"Неизвестный статус"} · версия {d.version.toString()}</p>
            {d.status===DealStatus.OPEN?<>
              <Button variant="secondary" onClick={()=>void open(d.dealId,"edit")}>Изменить сделку</Button>{" "}
              <Button variant="secondary" onClick={()=>void open(d.dealId,"advance")}>Изменить этап</Button>{" "}
              <Button variant="secondary" onClick={()=>void open(d.dealId,"close")}>Завершить сделку</Button>
            </>:<Button variant="secondary" onClick={()=>void open(d.dealId,"reopen")}>Открыть повторно</Button>}
            {expandedDeal===d.dealId&&<DealDetails key={d.dealId+":"+detailRevision}
              dealId={d.dealId} onEdit={()=>void open(d.dealId,"edit")}
              onClose={()=>void open(d.dealId,"close")}
               onReopen={()=>void open(d.dealId,"reopen")}/>}
          </Card>)}
          {boardPage?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={query.busy}
            onClick={()=>setStageCursors(old=>({...old,[stage.stageId]:boardPage.pageInfo?.nextPageToken??""}))}>
            Следующие сделки этапа</Button>}
          {stageCursors[stage.stageId]&&<Button variant="secondary" disabled={query.busy}
            onClick={()=>setStageCursors(old=>({...old,[stage.stageId]:""}))}>
            К началу этапа</Button>}
        </section>;
      })}
      </div>
    </>}
    {activePipeline&&!query.busy&&query.data?.every(entry=>entry.totalCount===0n)&&<p>В выбранной воронке сделок пока нет.</p>}

    <RecordPanel open={pipelineEditor!==null} dirty={pipelineDirty}
      title={pipelineEditor==="new"?"Новая воронка":"Настройка воронки"}
      onClose={()=>{
        if(!pipelineDirty||window.confirm("Несохранённые изменения воронки будут потеряны. Закрыть?"))
          setPipelineEditor(null);
      }}>
      <form onChange={()=>{pipelineMutation.changed();setPipelineDirty(true);}} onSubmit={e=>{
        e.preventDefault();
        void pipelineMutation.run(()=>{
          const stages=stageEdits.map(stage=>({
            stageId:stage.stageId,name:stage.name,
            probabilityBasisPoints:basisPoints(stage.probability),archived:stage.archived
          }));
          return pipelineEditor==="new"?
            api.deals.createPipeline({
              pipelineId:pipelineDraftId,name:pipelineName,stages,
              precondition:{idempotencyKey:pipelineMutation.key}
            }):
            pipelineEditor?
              api.deals.updatePipeline({
                pipelineId:pipelineEditor.pipelineId,name:pipelineName,stages,
                archived:pipelineArchived,
                precondition:{idempotencyKey:pipelineMutation.key,
                  expectedVersion:pipelineEditor.version}
              }):
              Promise.reject(Error("Воронка не выбрана"));
        },async()=>{
          const destination=pipelineEditor==="new"?pipelineDraftId:pipelineEditor?.pipelineId;
          setPipelineDirty(false);setPipelineEditor(null);
          await pipelinesQuery.reload();
          if(destination)setPipelineId(destination);
          setStageCursors({});await refreshSales();
        });
      }}>
        <TextField label="Название воронки" value={pipelineName} onChange={e=>setPipelineName(e.target.value)} required/>
        {pipelineEditor!=="new"&&<label>
          <input type="checkbox" checked={pipelineArchived} onChange={e=>setPipelineArchived(e.target.checked)}/>
          Архивировать воронку
        </label>}
        <h3>Этапы</h3>
        {stageEdits.map((stage,index)=><Card key={stage.stageId} title={"Этап "+(index+1)}>
          <TextField label={"Название этапа "+(index+1)} value={stage.name}
            onChange={e=>changeStage(index,{name:e.target.value})} required/>
          <TextField label={"Вероятность этапа "+(index+1)+", %"} inputMode="decimal"
            value={stage.probability} onChange={e=>changeStage(index,{probability:e.target.value})} required/>
          <label><input type="checkbox" checked={stage.archived}
            onChange={e=>changeStage(index,{archived:e.target.checked})}/> Архивировать этап</label>
          <Button type="button" variant="secondary" disabled={index===0}
            onClick={()=>shiftStage(index,-1)}>Выше</Button>{" "}
          <Button type="button" variant="secondary" disabled={index===stageEdits.length-1}
            onClick={()=>shiftStage(index,1)}>Ниже</Button>{" "}
          <Button type="button" variant="secondary" disabled={stageEdits.length===1}
            onClick={()=>{setStageEdits(before=>before.filter((_,i)=>i!==index));pipelineMutation.changed();setPipelineDirty(true);}}>
            Удалить этап</Button>
        </Card>)}
        <Button type="button" variant="secondary" disabled={stageEdits.length>=24} onClick={()=>{
          setStageEdits(before=>[...before,{stageId:newId(),name:"",probability:"10",archived:false}]);
          pipelineMutation.changed();setPipelineDirty(true);
        }}>Добавить этап</Button>{" "}
        <Button type="submit" disabled={pipelineMutation.busy}>Сохранить воронку</Button>{" "}
        <Button type="button" variant="secondary" onClick={()=>{
          if(!pipelineDirty||window.confirm("Несохранённые изменения воронки будут потеряны. Закрыть?"))
            setPipelineEditor(null);
        }}>Отмена</Button>
      </form>
    </RecordPanel>
    <MutationNotice state={pipelineMutation}/>
    </>}

    <RecordPanel open={editorMode!==null}
      title={advance?(editorMode==="close"?"Завершить сделку: ":
         editorMode==="reopen"?"Повторно открыть сделку: ":"Этап сделки: ")+advance.name:
        selected?"Изменить сделку":"Новая сделка"}
      onClose={()=>{setEditorMode(null);setSelected(undefined);setAdvance(undefined);}}>
      {advance?<form key={advance.dealId+advance.version+droppedStageId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const precondition={idempotencyKey:mutation.key,expectedVersion:advance.version};
          if(editorMode==="reopen")return api.deals.reopenDeal({
            dealId:advance.dealId,reasonCode:field(data,"reason"),precondition
          });
          if(editorMode==="close")return api.deals.closeDeal({
            dealId:advance.dealId,
            outcome:enumValue(field(data,"targetStatus"),
              [DealStatus.WON,DealStatus.LOST],"Результат сделки"),
            reasonCode:field(data,"reason"),precondition
          });
          const currentStage=advance.stage;
          if(!currentStage)throw Error("У сделки отсутствует текущий этап");
          const next=nextStages.find(s=>s.stageId===field(data,"nextStage"));
          if(!next)throw Error("Выберите доступный следующий этап");
          return api.deals.advanceStage({
            dealId:advance.dealId,
            targetStage:{
              pipelineId:currentStage.pipelineId,stageId:next.stageId,
              ordinal:next.ordinal
            },
            targetStatus:DealStatus.OPEN,precondition
          });
        },async()=>{setEditorMode(null);setAdvance(undefined);setDetailRevision(v=>v+1);await refreshSales();});
      }}><FormGrid>{editorMode==="close"?<>
        <p>Текущий этап сохраняется: {advancePipeline?.stages.find(s=>s.stageId===advance.stage?.stageId)?.name??advance.stage?.stageId??"неизвестен"}.</p>
        <SelectField label="Результат сделки" name="targetStatus" defaultValue={DealStatus.WON}>
          <option value={DealStatus.WON}>Выиграна</option>
          <option value={DealStatus.LOST}>Проиграна</option>
        </SelectField>
        <TextField label="Причина закрытия" name="reason" required/>
      </>:editorMode==="reopen"?<>
        <p>Вернуть сделку в текущий активный этап воронки.</p>
        <TextField label="Причина повторного открытия" name="reason" required/>
      </>:<>
        <SelectField label="Следующий этап" name="nextStage"
          defaultValue={nextStages.some(s=>s.stageId===droppedStageId)?droppedStageId:nextStages[0]?.stageId??""} required>
          {nextStages.map(stage=><option value={stage.stageId} key={stage.stageId}>
            {stage.name}</option>)}
        </SelectField>
        {nextStages.length===0&&<p>Следующих этапов нет. Сделку можно завершить на текущем этапе.</p>}
      </>}</FormGrid>
        <Button type="submit" disabled={mutation.busy||editorMode==="advance"&&nextStages.length===0}>
          {editorMode==="close"?"Завершить сделку":
             editorMode==="reopen"?"Открыть повторно":"Сохранить этап"}
        </Button>{" "}
        <Button variant="secondary" type="button" onClick={()=>{setEditorMode(null);setAdvance(undefined);mutation.changed();}}>Отмена</Button>
      </form>:<form key={selected?selected.dealId+selected.version:draftId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const currencyCode=field(data,"currency"),amountText=field(data,"amount");
          const mode=selected?field(data,"amountMode"):"set";
          const amount=mode==="set"&&amountText?{
            minorUnits:minorUnits(amountText,currencyCode),currencyCode
          }:undefined;
          const sourceValue=field(data,"probabilitySource");
          const source=selected&&sourceValue==="keep"?undefined:enumValue(sourceValue,
            [ProbabilitySource.MANUAL,ProbabilitySource.STAGE_DEFAULT],"Источник вероятности");
          const forecastCategory=enumValue(field(data,"forecastCategory"),
            [ForecastCategory.PIPELINE,ForecastCategory.BEST_CASE,ForecastCategory.COMMIT,ForecastCategory.OMITTED],
            "Категория прогноза");
          const common={forecastCategory,...(source===ProbabilitySource.MANUAL?
            {probabilityBasisPoints:basisPoints(field(data,"probability"))}:{})};
          // A disabled native <select> is excluded from FormData while
          // Account/ContactPoint choices are loading. Keep the original
          // association on edit, rather than treating a pending read as Clear.
          // Rust crm-sales remains the authoritative contact/account validator.
          const contactField=data.get("primaryContactId");
          const contactId=typeof contactField==="string"?
            contactField:(selected?.primaryContactId??"");
          const expectedCloseDate=parseDate(field(data,"expectedCloseDate"));
          if(selected) {
            const dateBefore=inputDate(selected.expectedCloseDate);
            const dateNow=inputDate(expectedCloseDate);
            const contactPatch=contactId===selected.primaryContactId?{}:{
              primaryContactId:{operation:contactId?
                {case:"set" as const,value:contactId}:{case:"clear" as const,value:true}}
            };
            const datePatch=dateBefore===dateNow?{}:{
              expectedCloseDate:{operation:expectedCloseDate?
                {case:"set" as const,value:expectedCloseDate}:{case:"clear" as const,value:true}}
            };
            return api.deals.updateDeal({
              dealId:selected.dealId,
              name:{operation:{case:"set",value:field(data,"name")}},...common,
              ...(source===ProbabilitySource.STAGE_DEFAULT?{useStageProbability:true}:{}),
              ...contactPatch,...datePatch,
              ...(mode==="clear"?{amount:{operation:{case:"clear" as const,value:true}}}:
                mode==="set"?(amount?{amount:{operation:{case:"set" as const,value:amount}}}:
                  (()=>{throw Error("Введите сумму");})()):{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
            });
          }
          const stage=activePipeline?.stages.find(s=>s.stageId===field(data,"stageId"));
          if(!activePipeline||!stage||stage.archived)throw Error("Выберите существующий активный этап");
          return api.deals.createDeal({
            dealId:draftId,name:field(data,"name"),accountId:field(data,"accountId"),
            stage:{pipelineId:activePipeline.pipelineId,stageId:stage.stageId,ordinal:stage.ordinal},
            ...(amount?{amount}:{}),...common,
            ...(contactId?{primaryContactId:contactId}:{}),
            ...(expectedCloseDate?{expectedCloseDate}:{}),
            precondition:{idempotencyKey:mutation.key}
          });
        },async()=>{setEditorMode(null);setSelected(undefined);setDraftId(newId());setDetailRevision(v=>v+1);await refreshSales();});
      }}><FormGrid>
        <TextField label="Название сделки" name="name" defaultValue={selected?.name??""} required/>
        {!selected&&<>
          <AccountPicker onSelect={setFormAccountId}/>
          <SelectField label="Этап" name="stageId" defaultValue={readyStage?.stageId??""} required>
            {activePipeline?.stages.filter(stage=>!stage.archived).map(stage=>
              <option value={stage.stageId} key={stage.stageId}>{stage.name}</option>)}
          </SelectField>
        </>}
        <DealContactPicker key={selected?.dealId??formAccountId}
          accountId={selected?.accountId??formAccountId}
          initialContactId={selected?.primaryContactId??""}/>
        <TextField label="Ожидаемая дата закрытия" name="expectedCloseDate" type="date"
          defaultValue={inputDate(selected?.expectedCloseDate)}/>
        {selected&&<SelectField label="Изменение суммы" name="amountMode" defaultValue="keep">
          <option value="keep">Сохранить прежнюю</option><option value="set">Установить</option>
          <option value="clear">Убрать</option>
        </SelectField>}
        <TextField label="Сумма" name="amount" inputMode="decimal"
          defaultValue={selected?.amount?decimalUnits(selected.amount.minorUnits,selected.amount.currencyCode):""}/>
        <SelectField label="Валюта" name="currency" defaultValue={selected?.amount?.currencyCode??"USD"}>
          {["USD","EUR","PLN","JPY","KWD"].map(c=><option key={c} value={c}>{c}</option>)}
        </SelectField>
        <SelectField label="Источник вероятности" name="probabilitySource"
          value={probabilityChoice} onChange={e=>setProbabilityChoice(e.target.value)}>
          {selected&&<option value="keep">Сохранить текущую оценку</option>}
          <option value={ProbabilitySource.STAGE_DEFAULT}>По этапу (применить значение каталога)</option>
          <option value={ProbabilitySource.MANUAL}>Вручную</option>
        </SelectField>
        <TextField label="Вероятность, %" name="probability" inputMode="decimal"
          disabled={probabilityChoice!==String(ProbabilitySource.MANUAL)}
          defaultValue={selected?percent(selected.probabilityBasisPoints):"0"}
          required={probabilityChoice===String(ProbabilitySource.MANUAL)}/>
        <p>Значение этапа фиксируется при выборе и обновляется при переходе на новый этап.
          Изменение каталога не пересчитывает прежние оценки. Индивидуальная вероятность сохраняется при переходе.</p>
        <SelectField label="Категория прогноза" name="forecastCategory"
          defaultValue={selected?.forecastCategory??ForecastCategory.PIPELINE}>
          {[ForecastCategory.PIPELINE,ForecastCategory.BEST_CASE,ForecastCategory.COMMIT,ForecastCategory.OMITTED].map(category=>
            <option key={category} value={category}>{forecastCategoryText(category)}</option>)}
        </SelectField>
      </FormGrid>
        <Button type="submit" disabled={mutation.busy}>Сохранить сделку</Button>{" "}
        {selected&&<Button variant="secondary" type="button" onClick={()=>{
          setEditorMode(null);setSelected(undefined);mutation.changed();
        }}>Отмена</Button>}
      </form>}
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

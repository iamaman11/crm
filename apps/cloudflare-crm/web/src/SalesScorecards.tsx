import {useCallback,useState} from "react";
import {Button,SelectField,TextField,ScrollTable,FilterForm,MetricGrid,DivergingTrend} from "@ultimate-crm/ui";
import {SalesAnalyticsExportKind,SalesQuotaCadence,SalesScorecardAxis} from "@ultimate-crm/client/cloudflare";
import type {GetSalesScorecardResponse,SalesQuota,SalesScorecardCell,SalesQuotaFactCursor} from "@ultimate-crm/client/cloudflare";
import {api} from "./api";
import {inputDate,parseDate} from "./calendarInput";
import {decimalUnits} from "./money";
import {useQuery,QueryNotice} from "./hooks";
import {dealHref,salesScorecardHref} from "./routes";
import type {ScorecardFilters} from "./routes";
import {currentRouteHash,navigateTo} from "./navigation";
import {boundedInt,enumValue} from "./enumFields";
import {SalesExportButton} from "./SalesExportButton";
import type {allPipelines} from "./salesPipelineCatalog";

type Request=Extract<Parameters<typeof api.deals.getSalesScorecard>[0],{$typeName?:never}>;
type Detail=NonNullable<Request["detail"]>;
type Labels=ReadonlyArray<{value:string;label:string;eligible:boolean}>;
type Selection={organization:boolean;dimensionId?:string};
const org:Selection={organization:true};
const unallocated:Selection={organization:false};
function same(a:Selection,b:Selection){return a.organization===b.organization&&a.dimensionId===b.dimensionId;}
function key(s:Selection){return s.organization?"organization":s.dimensionId===undefined?"unallocated":"dimension:"+s.dimensionId;}
function ratio(value:string|number|undefined):string {
  if(value===undefined)return "—";
  const n=BigInt(value),abs=n<0n?-n:n;
  return (n<0n?"−":"")+(abs/100n).toString()+"."+(abs%100n).toString().padStart(2,"0")+"%";
}

export function SalesScorecards({catalog,labels,onQuota,isAdmin,request}:{request:ScorecardFilters|undefined;isAdmin:boolean;catalog:Awaited<ReturnType<typeof allPipelines>>;labels:Labels;onQuota:(quota:SalesQuota)=>void}){

  const [exported,setExported]=useState<{request:ScorecardFilters;data:GetSalesScorecardResponse}>();
  const [bucketIndex,setBucketIndex]=useState(0);
  const [selection,setSelection]=useState<Selection>(org);
  const [detail,setDetail]=useState<Detail>();
  const [formError,setFormError]=useState("");
  const score=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!request)return undefined;
    return api.deals.getSalesScorecard(request,{signal});
  },[request]));
  const facts=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!request||!detail)return undefined;
    return (await api.deals.getSalesScorecard({...request,detail},{signal})).detail;
  },[request,detail]));
  const data=exported&&exported.request===request?exported.data:score.data;
  const bucket=data?.buckets[bucketIndex];
  const axis=data?.axis;
  const code=data?.currencyCode??"";
  const money=(value:string|undefined)=>value===undefined?"—":catalog.currencies.has(code)?decimalUnits(BigInt(value),code,catalog.currencies)+" "+code:value+" minor units "+code;
  const label=(s:Selection)=>{
    if(s.organization)return "Вся организация";
    if(s.dimensionId===undefined)return "Без атрибуции";
    const prefix=axis===SalesScorecardAxis.ACTOR?"actor:":axis===SalesScorecardAxis.TEAM?"team:":"territory:";
    return labels.find(l=>l.value===prefix+s.dimensionId)?.label??s.dimensionId+" (исторический ID)";
  };
  const periodState=(start:bigint,end:bigint)=>{
    const now=data?.asOfUnixNanos;
    return now===undefined?"":start>now?"Будущий период":end>now?"Незавершённый период":"Завершённый период";
  };
  const dimensions:Selection[]=(bucket?.dimensions??[]).map(d=>d.dimensionId===undefined?unallocated:{organization:false,dimensionId:d.dimensionId});
  const cell=(b:NonNullable<typeof bucket>,s:Selection)=>s.organization?b.organization:b.dimensions.find(d=>d.dimensionId===s.dimensionId)?.cell;
  const selected=bucket&&cell(bucket,selection);
  const inspect=(s:Selection,c:SalesScorecardCell)=>{
    if(!data)return;
    setSelection(s);
    setDetail({bucketIndex,organization:s.organization,...(s.dimensionId===undefined?{}:{dimensionId:s.dimensionId}),
      asOfUnixNanos:data.asOfUnixNanos,eventSequence:data.eventSequence,expectedEventCount:c.sourceEventCount,pageSize:25});
  };
  const next=(cursor:SalesQuotaFactCursor)=>setDetail(old=>old?{...old,after:cursor}:old);
  const goal=(c:SalesScorecardCell)=>c.quota?.configuration?.target;
  const values=(c:SalesScorecardCell|undefined)=>c?<>
    <td>{goal(c)?money(goal(c)?.minorUnits.toString()):"Нет цели"}</td>
    <td>{money(c.grossMinorUnits)}</td><td>{money(c.reversalMinorUnits)}</td><td>{money(c.netMinorUnits)}</td>
    <td>{money(c.remainingMinorUnits)}</td><td>{ratio(c.attainmentBasisPoints)}</td>
    <td>{c.wonCount.toString()} / {c.lostCount.toString()} / {c.reversalCount.toString()}</td>
    <td>{ratio(c.winRateBasisPoints)}</td>
    <td>{c.missingAmountCount.toString()} / {c.unknownHistoryCount.toString()} / {c.unallocatedCount.toString()}</td>
  </>:<td colSpan={9}>Нет данных</td>;
  const headings=<><th scope="col">План</th><th scope="col">Выигрыши</th><th scope="col">Сторно</th><th scope="col">Нетто</th><th scope="col">Остаток</th><th scope="col">Выполнение</th><th scope="col">Выигр. / проигр. / сторно</th><th scope="col">Win rate</th><th scope="col">Без суммы / истории / атрибуции</th></>;
  return <section aria-label="Коммерческая эффективность">
    <h4>Коммерческая эффективность и динамика</h4>
    <p>Исторические закрытия и сторно, а не текущий владелец сделки. Цели подбираются только по точному совпадению области, валюты, воронки и периода. Используется последняя опубликованная версия цели, включая архив. Нет цели — не нулевой план.</p>
    <FilterForm aria-label="Настройка коммерческой эффективности" onSubmit={event=>{
      event.preventDefault();
      try{
        const form=new FormData(event.currentTarget);
        const firstDate=parseDate(String(form.get("first")??""));
        if(!firstDate)throw Error("Выберите начало периода");
        const pipeline=String(form.get("pipeline")??"");
        setFormError("");setDetail(undefined);setSelection(org);setBucketIndex(0);setExported(undefined);
        const next:ScorecardFilters={axis:enumValue(String(form.get("axis")),[SalesScorecardAxis.ORGANIZATION,SalesScorecardAxis.ACTOR,SalesScorecardAxis.TEAM,SalesScorecardAxis.TERRITORY],"Измерение"),
          currencyCode:String(form.get("currency")??""),...(pipeline?{pipelineId:pipeline}:{}),firstDate,timeZone:String(form.get("zone")??""),
          cadence:enumValue(String(form.get("cadence")),[SalesQuotaCadence.MONTHLY,SalesQuotaCadence.QUARTERLY],"Период"),
          periodCount:boundedInt(String(form.get("count")),1,24,"Число периодов")};
        const href=salesScorecardHref(next);
        if(href===currentRouteHash())void score.reload().catch(()=>{});else navigateTo(href);
      }catch(error){setDetail(undefined);setFormError(error instanceof Error?error.message:String(error));}
    }}>
      <SelectField label="Измерение эффективности" name="axis" defaultValue={request?.axis??SalesScorecardAxis.TEAM}>
        <option value={SalesScorecardAxis.ORGANIZATION}>Организация</option><option value={SalesScorecardAxis.ACTOR}>Сотрудники</option><option value={SalesScorecardAxis.TEAM}>Команды</option><option value={SalesScorecardAxis.TERRITORY}>Территории</option>
      </SelectField>
      <SelectField label="Валюта эффективности" name="currency" defaultValue={request?.currencyCode??"EUR"}>{Array.from(catalog.currencies.keys()).map(c=><option key={c} value={c}>{c}</option>)}</SelectField>
      <SelectField label="Воронка эффективности" name="pipeline" defaultValue={request?.pipelineId??""}><option value="">Все воронки</option>{catalog.pipelines.map(p=><option key={p.pipelineId} value={p.pipelineId}>{p.name}{p.archived?" (архив)":""}</option>)}</SelectField>
      <SelectField label="Шаг динамики" name="cadence" defaultValue={request?.cadence??SalesQuotaCadence.MONTHLY}><option value={SalesQuotaCadence.MONTHLY}>Месяц</option><option value={SalesQuotaCadence.QUARTERLY}>Квартал</option></SelectField>
      <TextField label="Первый день периода" name="first" type="date" defaultValue={request?.firstDate?inputDate({year:request.firstDate.year??0,month:request.firstDate.month??0,day:request.firstDate.day??0}):new Date().getUTCFullYear()+"-01-01"} required/>
      <TextField label="Число периодов" name="count" type="number" min={1} max={24} defaultValue={String(request?.periodCount??12)} required/>
      <TextField label="Часовой пояс эффективности" name="zone" defaultValue={request?.timeZone??"UTC"} required/>
      <Button type="submit" disabled={!!request&&score.busy}>Рассчитать эффективность</Button>
    </FilterForm>
    {formError&&<p role="alert">{formError}</p>}
    {request&&<QueryNotice query={score}/>}
    {data&&!score.stale&&!formError&&bucket&&<>
      <p>Срез: {new Date(Number(data.asOfUnixNanos/1000000n)).toISOString()} · последовательность {data.eventSequence.toString()} · {code}.</p>
      {isAdmin&&request&&<SalesExportButton key={data.eventSequence.toString()+":"+JSON.stringify([request.axis,request.currencyCode,request.pipelineId,request.timeZone,request.cadence,request.periodCount,request.firstDate])} label="Скачать Scorecard CSV" request={{kind:SalesAnalyticsExportKind.SCORECARD,scope:{case:"scorecard",value:request}}} onScorecard={snapshot=>{setExported({request,data:snapshot});setDetail(undefined);setSelection(org);setBucketIndex(0);}}/>}
      {exported?.request===request&&<p>Показан новый согласованный срез, использованный для выгрузки.</p>}
      <p>Win rate — выигранные среди известных событий закрытия, без сторно; это не доля уникальных сделок. Закрытия без валюты могут входить в счётчики разных валют, их нельзя складывать. Неизвестная история показана только на уровне организации. Разрезы не суммируются между собой.</p>
      {bucket.organization&&<MetricGrid>
        <div><dt>План</dt><dd>{goal(bucket.organization)?money(goal(bucket.organization)?.minorUnits.toString()):"Нет цели"}</dd></div>
        <div><dt>Нетто</dt><dd>{money(bucket.organization.netMinorUnits)}</dd></div>
        <div><dt>Выполнение</dt><dd>{ratio(bucket.organization.attainmentBasisPoints)}</dd></div>
        <div><dt>Win rate</dt><dd>{ratio(bucket.organization.winRateBasisPoints)}</dd></div>
      </MetricGrid>}
      <SelectField label="Период scorecard" value={bucketIndex} onChange={e=>{setBucketIndex(Number(e.target.value));setDetail(undefined);}}>
        {data.buckets.map((b,i)=><option key={b.startUnixNanos.toString()} value={i}>{inputDate(b.period?.startDate)} — {inputDate(b.period?.endDate)} · {periodState(b.startUnixNanos,b.endExclusiveUnixNanos)}</option>)}
      </SelectField>
      <ScrollTable label="Таблица коммерческой эффективности">
        <table><thead><tr><th scope="col">Область</th>{headings}<th scope="col">Действия</th></tr></thead>
          <tbody>{[org,...dimensions].map(s=>{
            const c=cell(bucket,s);
            return <tr key={key(s)}><th scope="row">{label(s)}</th>{values(c)}<td>{c&&<>
              <Button variant="secondary" onClick={()=>inspect(s,c)}>Исходные события</Button>{" "}
              {c.quota&&<Button variant="secondary" onClick={()=>{if(c.quota)onQuota(c.quota);}}>План-факт и прогноз · v{c.quota.version.toString()}{c.quota.configuration?.archived?" (архив)":""}</Button>}
            </>}</td></tr>;
          })}</tbody>
        </table>
      </ScrollTable>
      <SelectField label="Область динамики" value={key(selection)} onChange={e=>{const s=[org,...dimensions].find(item=>key(item)===e.target.value);if(s){setSelection(s);setDetail(undefined);}}}>
        {[org,...dimensions].map(s=><option key={key(s)} value={key(s)}>{label(s)}</option>)}
      </SelectField>
      <DivergingTrend label="График плана и нетто" title="План и нетто по периодам" description="План — серый, нетто — цветной. Отрицательное нетто показано слева от нуля. Точные значения в таблице." series={["План","Нетто"]} rows={data.buckets.map(b=>{const c=cell(b,selection),plan=c?.quota?.configuration?.target?.minorUnits,net=c?BigInt(c.netMinorUnits):undefined;return {label:inputDate(b.period?.startDate),values:[plan,net],formatted:[money(plan?.toString()),money(net?.toString())]};})}/>
      <ScrollTable label="Динамика коммерческой эффективности">
        <table><thead><tr><th scope="col">Период</th>{headings}</tr></thead>
          <tbody>{data.buckets.map(b=><tr key={b.startUnixNanos.toString()}><th scope="row">{inputDate(b.period?.startDate)} — {inputDate(b.period?.endDate)} · {periodState(b.startUnixNanos,b.endExclusiveUnixNanos)}</th>{values(cell(b,selection))}</tr>)}</tbody>
        </table>
      </ScrollTable>
      {detail&&selected&&same(selection,{organization:detail.organization??false,...(detail.dimensionId===undefined?{}:{dimensionId:detail.dimensionId})})&&<section aria-label="Исходные коммерческие события">
        <h5>{label(selection)} · {inputDate(bucket.period?.startDate)} — {inputDate(bucket.period?.endDate)}</h5>
        <QueryNotice query={facts}/>
        {facts.data&&!facts.stale&&<>
          <p>Всего событий в срезе: {facts.data.sourceEventCount.toString()}</p>
          {isAdmin&&request&&<SalesExportButton key={detail.asOfUnixNanos?.toString()+":"+detail.bucketIndex+":"+key(selection)} label="Скачать события Scorecard CSV" request={{kind:SalesAnalyticsExportKind.SCORECARD_DETAIL,scope:{case:"scorecard",value:{...request,detail}}}}/>}
          <ul>{facts.data.facts.map(f=><li key={f.dealId+":"+f.dealVersion.toString()}><a href={dealHref(f.dealId)}>Сделка {f.dealId}</a> · v{f.dealVersion.toString()} · {f.reversal?"Сторно":"Закрытие"} · {f.closeFact?(f.signedMinorUnits===undefined?"Без суммы":money(f.signedMinorUnits)):"Историческая атрибуция неизвестна"} · {f.closeFact?.creditedTeam?.name??"—"} · {f.closeFact?.creditedTerritory?.name??"—"}</li>)}</ul>
          {detail.after&&<Button variant="secondary" onClick={()=>inspect(selection,selected)}>Первая страница событий</Button>}
          {facts.data.next&&<Button variant="secondary" onClick={()=>{if(facts.data?.next)next(facts.data.next);}}>Следующая страница событий</Button>}
        </>}
      </section>}
    </>}
  </section>;
}


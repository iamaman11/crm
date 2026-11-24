import {useCallback,useState} from "react";
import {Button,SelectField,TextField} from "@ultimate-crm/ui";
import {SalesQuotaCadence} from "@ultimate-crm/client/cloudflare";
import type {SalesQuota,SalesQuotaConfiguration,SalesQuotaScope,SalesQuotaFactCursor,SalesTerritory} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {allPipelines} from "./salesPipelineCatalog";
import {ownerValue,ownerWire} from "./ownerCatalog";
import type {allOwners} from "./ownerCatalog";
import {decimalUnits,minorUnits} from "./money";
import {inputDate,parseDate} from "./calendarInput";
import {dealHref} from "./routes";

type Choices=Awaited<ReturnType<typeof allOwners>>["choices"];
function scopeValue(scope:SalesQuotaScope|undefined):string {
  switch(scope?.scope.case){
    case "owner":return ownerValue(scope.scope.value);
    case "territoryId":return "territory:"+scope.scope.value;
    default:return "tenant";
  }
}
function scopeWire(value:string):SalesQuotaConfiguration["scope"]|{scope:{case:"tenantWide";value:boolean}}|{scope:{case:"owner";value:NonNullable<ReturnType<typeof ownerWire>>}}|{scope:{case:"territoryId";value:string}} {
  if(value==="tenant")return {scope:{case:"tenantWide",value:true}};
  if(value.startsWith("territory:"))return {scope:{case:"territoryId",value:value.slice(10)}};
  const owner=ownerWire(value);if(!owner)throw Error("Выберите область цели");
  return {scope:{case:"owner",value:owner}};
}
async function allQuotas(signal:AbortSignal):Promise<SalesQuota[]> {
  const result:SalesQuota[]=[];let cursor="";const seen=new Set<string>();
  do{
    const page=await api.deals.listSalesQuotas({includeArchived:true,page:{pageSize:100,pageToken:cursor}},{signal});
    result.push(...page.quotas);cursor=page.pageInfo?.nextPageToken??"";
    if(cursor&&seen.has(cursor))throw Error("Повторный курсор целей");
    if(cursor)seen.add(cursor);
  }while(cursor);
  return result;
}
function ratio(value:string):string {
  const n=BigInt(value),abs=n<0n?-n:n;
  return (n<0n?"−":"")+(abs/100n).toString()+"."+(abs%100n).toString().padStart(2,"0")+"%";
}
export function SalesQuotaWorkspace({isAdmin,choices,ownersUnavailable,territories}:{isAdmin:boolean;choices:Choices;ownersUnavailable:boolean;territories:SalesTerritory[]}) {
  const quotas=useQuery(useCallback(allQuotas,[]));
  const catalog=useQuery(useCallback(allPipelines,[]));
  const [editing,setEditing]=useState<SalesQuota|"new">();
  const [id,setId]=useState<SalesQuota["quotaId"]>(newId);
  const [scope,setScope]=useState("tenant");
  const [currency,setCurrency]=useState("EUR");
  const [published,setPublished]=useState(false);
  const [archived,setArchived]=useState(false);
  const [selected,setSelected]=useState<SalesQuota>();
  const [version,setVersion]=useState<bigint>();
  const [asOf,setAsOf]=useState<bigint>();
  const [after,setAfter]=useState<SalesQuotaFactCursor>();
  const [includeArchived,setIncludeArchived]=useState(false);
  const performance=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!selected||version===undefined)return undefined;
    return api.deals.getSalesQuotaPerformance({quotaId:selected.quotaId,quotaVersion:version,...(asOf===undefined?{}:{asOfUnixNanos:asOf}),...(after?{after}:{}),pageSize:25},{signal});
  },[selected,version,asOf,after]));
  const mutation=useMutation({
    storageKey:"crm.sales.quota.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.deals.getSalesQuota({quotaId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{setEditing(undefined);await quotas.reload();}
  });
  const pending=mutation.busy||mutation.pending;
  const ready=!ownersUnavailable&&!catalog.busy&&!catalog.error&&!!catalog.data;
  const old=editing==="new"?undefined:editing,cfg=old?.configuration;
  const start=(quota?:SalesQuota)=>{
    setEditing(quota??"new");setId(quota?.quotaId??newId());
    setScope(scopeValue(quota?.configuration?.scope));setCurrency(quota?.configuration?.target?.currencyCode??"EUR");
    setPublished(quota?.configuration?.published??false);setArchived(false);mutation.changed();
  };
  const select=(quota:SalesQuota)=>{setSelected(quota);setVersion(quota.version);setAsOf(undefined);setAfter(undefined);};
  const labels=[{value:"tenant",label:"Вся организация",eligible:true},...choices,...territories.map(t=>({value:"territory:"+t.territoryId,label:"Территория: "+(t.configuration?.name??t.territoryId),eligible:!t.configuration?.archived}))];
  const money=(value:string,code:string)=>catalog.data?decimalUnits(BigInt(value),code,catalog.data.currencies)+" "+code:"—";
  const report=performance.data;
  const reportCurrency=report?.quota?.configuration?.target?.currencyCode;
  return <section aria-label="Коммерческие цели и план-факт">
    <h3>Коммерческие цели и план-факт</h3>
    <p>Факт — сумма выигранных сделок за период минус сторно в периоде. Это не бухгалтерская выручка. Цели разных областей и периодов не складываются.</p>
    <QueryNotice query={quotas}/><QueryNotice query={catalog}/>
    <p><label><input type="checkbox" checked={includeArchived} onChange={e=>setIncludeArchived(e.target.checked)}/> Показать архивные цели</label></p>
    {isAdmin&&<Button disabled={!ready||pending} onClick={()=>start()}>Создать коммерческую цель</Button>}
    <div role="region" aria-label="Коммерческие цели — таблица" tabIndex={0} style={{maxWidth:"100%",overflowX:"auto"}}>
      <table><thead><tr><th scope="col">Цель</th><th scope="col">Область</th><th scope="col">Период</th><th scope="col">План</th><th scope="col">Состояние</th><th scope="col">Действия</th></tr></thead>
      <tbody>{quotas.data?.filter(q=>includeArchived||!q.configuration?.archived).map(q=>{
        const c=q.configuration,p=c?.period,key=scopeValue(c?.scope);
        return <tr key={q.quotaId}><th scope="row">{c?.name} · v{q.version.toString()}</th><td>{labels.find(x=>x.value===key)?.label??key}</td><td>{inputDate(p?.startDate)} — {inputDate(p?.endDate)} · {p?.timeZone}</td><td>{c?.target?money(c.target.minorUnits.toString(),c.target.currencyCode):"—"}</td><td>{c?.archived?"Архив":c?.published?"Опубликована":"Черновик"}</td><td><Button variant="secondary" onClick={()=>select(q)}>План-факт</Button>{" "}{isAdmin&&!c?.archived&&<Button variant="secondary" disabled={pending||!ready} onClick={()=>start(q)}>Изменить цель</Button>}</td></tr>;
      })}</tbody></table>
    </div>
    {editing&&isAdmin&&catalog.data&&<form key={id} aria-label="Настройка коммерческой цели" onChange={mutation.changed} onSubmit={event=>{
      event.preventDefault();const data=new FormData(event.currentTarget);
      void mutation.runIntent(()=>{
        const currencies=catalog.data?.currencies;if(!currencies)throw Error("Нет справочника валют");
        const reason=field(data,"reason").trim(),pipeline=String(data.get("pipeline")??"");
        const startDate=parseDate(String(data.get("start")??"")),endDate=parseDate(String(data.get("end")??""));
        if(!cfg?.published&&(!startDate||!endDate))throw Error("Выберите период");
        const target={currencyCode:currency,minorUnits:minorUnits(field(data,"target"),currency,currencies)};
        const period=cfg?.published?cfg.period:{startDate,endDate,timeZone:field(data,"zone").trim(),cadence:Number(field(data,"cadence"))};
        const pipelineId=cfg?.published?cfg.pipelineId:pipeline;
        const configuration=archived&&cfg?{...cfg,archived:true,revisionReason:reason}:{
          name:field(data,"name").trim(),scope:scopeWire(scope),...(pipelineId?{pipelineId}:{}),
          period,target,published,archived:false,revisionReason:reason
        };
        return old?prepareMutation(api.deals.updateSalesQuota,{quotaId:id,configuration,precondition:{idempotencyKey:mutation.key,expectedVersion:old.version}},id):
          prepareMutation(api.deals.createSalesQuota,{quotaId:id,configuration,precondition:{idempotencyKey:mutation.key}},id);
      },async()=>{setEditing(undefined);await quotas.reload();});
    }}>
      <fieldset disabled={pending}>
        <TextField label="Название цели" name="name" defaultValue={cfg?.name??""} required/>
        <SelectField label="Область цели" value={scope} disabled={!!cfg?.published} onChange={e=>setScope(e.target.value)}>{labels.filter(x=>x.eligible||x.value===scope).map(x=><option key={x.value} value={x.value}>{x.label}</option>)}</SelectField>
        <SelectField label="Воронка цели" name="pipeline" defaultValue={cfg?.pipelineId??""} disabled={!!cfg?.published}><option value="">Все воронки</option>{catalog.data.pipelines.filter(p=>!p.archived||p.pipelineId===cfg?.pipelineId).map(p=><option key={p.pipelineId} value={p.pipelineId}>{p.name}</option>)}</SelectField>
        <SelectField label="Тип периода" name="cadence" disabled={!!cfg?.published} defaultValue={cfg?.period?.cadence??SalesQuotaCadence.MONTHLY}><option value={SalesQuotaCadence.MONTHLY}>Месяц</option><option value={SalesQuotaCadence.QUARTERLY}>Квартал</option><option value={SalesQuotaCadence.CUSTOM}>Произвольный</option></SelectField>
        <TextField label="Начало периода" name="start" type="date" disabled={!!cfg?.published} defaultValue={inputDate(cfg?.period?.startDate)} required/>
        <TextField label="Последний день периода" name="end" type="date" disabled={!!cfg?.published} defaultValue={inputDate(cfg?.period?.endDate)} required/>
        <TextField label="Часовой пояс IANA" name="zone" disabled={!!cfg?.published} defaultValue={cfg?.period?.timeZone??"UTC"} required/>
        <SelectField label="Валюта цели" value={currency} disabled={!!cfg?.published} onChange={e=>setCurrency(e.target.value)}>{Array.from(catalog.data.currencies.keys()).map(c=><option key={c} value={c}>{c}</option>)}</SelectField>
        <TextField key={currency} label="Плановая сумма" name="target" defaultValue={cfg?.target?.currencyCode===currency?decimalUnits(cfg.target.minorUnits,currency,catalog.data.currencies):""} required/>
        <TextField label="Причина версии" name="reason" required/>
        <p><label><input type="checkbox" checked={published} disabled={!!cfg?.published} onChange={e=>setPublished(e.target.checked)}/> Опубликовать цель</label></p>
        {old&&<p><label><input type="checkbox" checked={archived} onChange={e=>setArchived(e.target.checked)}/> Архивировать цель</label></p>}
      </fieldset>
      <Button type="submit" disabled={pending||!ready}>Сохранить цель</Button>{" "}<Button type="button" variant="secondary" disabled={pending} onClick={()=>setEditing(undefined)}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
    {selected&&<section aria-label="План-факт выбранной цели">
      <h4>{selected.configuration?.name}</h4>
      <SelectField label="Версия цели" value={version?.toString()??""} onChange={e=>{setVersion(BigInt(e.target.value));setAsOf(undefined);setAfter(undefined);}}>
        {Array.from({length:Number(selected.version)},(_,i)=>BigInt(i+1)).map(v=><option key={v.toString()} value={v.toString()}>Версия {v.toString()}</option>)}
      </SelectField>
      <Button variant="secondary" disabled={performance.busy} onClick={()=>{setAsOf(undefined);setAfter(undefined);void performance.reload();}}>Обновить факт</Button>
      <QueryNotice query={performance}/>
      {report&&reportCurrency&&<><p>План: {money(report.quota?.configuration?.target?.minorUnits.toString()??"0",reportCurrency)} · Выигрыши: {money(report.grossMinorUnits,reportCurrency)} · Сторно: {money(report.reversalMinorUnits,reportCurrency)}</p>
        <p>Нетто: {money(report.netMinorUnits,reportCurrency)} · Остаток плана: {money(report.remainingMinorUnits,reportCurrency)} · Выполнение: {ratio(report.attainmentBasisPoints)}</p>
        <p>Выигрышей: {report.wonCount.toString()} · проигрышей: {report.lostCount.toString()} · сторно: {report.reversalCount.toString()}</p>
        <p>Без суммы в области: {report.missingAmountCount.toString()} · исторические события без факта: {report.unknownHistoryCount.toString()} · не распределено по этому измерению: {report.unallocatedCount.toString()}</p>
        <ul>{report.facts.map(f=><li key={f.dealId+":"+f.dealVersion.toString()}><a href={dealHref(f.dealId)}>Сделка {f.dealId}</a> · v{f.dealVersion.toString()} · {f.reversal?"Сторно":"Закрытие"} · {f.signedMinorUnits===undefined?"Сумма неизвестна":money(f.signedMinorUnits,reportCurrency)}</li>)}</ul>
        {after&&<Button variant="secondary" onClick={()=>setAfter(undefined)}>Первая страница фактов</Button>}
        {report.next&&<Button variant="secondary" onClick={()=>{setAsOf(report.asOfUnixNanos);setAfter(report.next);}}>Следующая страница фактов</Button>}
      </>}
    </section>}
  </section>;
}

import {useCallback,useState} from "react";
import {Button,SelectField,TextField,ScrollTable} from "@ultimate-crm/ui";
import {SalesCapacityOverflow,SalesDispatchMode,SalesRoutingSource} from "@ultimate-crm/client/cloudflare";
import type {SalesRoutingDecision,SalesRoutingRule,SalesTerritory} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice} from "./hooks";
import {allPipelines} from "./salesPipelineCatalog";
import {decimalUnits,minorUnits} from "./money";
import {ownerValue,ownerWire,ownerLabel} from "./ownerCatalog";
import type {allOwners} from "./ownerCatalog";
type Choices=Awaited<ReturnType<typeof allOwners>>["choices"];

export function RoutingExplanation({decision,choices=[]}:{decision:SalesRoutingDecision;choices?:Choices}) {
  const source=decision.source===SalesRoutingSource.RULE?"Правило: "+decision.ruleName:
    decision.source===SalesRoutingSource.ACCOUNT?"Владелец клиентского портфеля":
    decision.source===SalesRoutingSource.TERRITORY?"Ответственный территории":
    decision.source===SalesRoutingSource.MANUAL?"Ручное назначение":
    decision.source===SalesRoutingSource.CREATOR?"Подписанный создатель":"Неизвестный источник";
  return <div aria-label="Причина назначения">
    <p>Источник назначения: {source}{decision.ruleVersion!==undefined?" · версия "+decision.ruleVersion.toString():""}</p>
    <p>Назначено: {ownerLabel(decision.selectedOwner,choices)}</p>
    {decision.territoryName&&<p>Территория при назначении: {decision.territoryName}</p>}
    {decision.assignmentVersion!==undefined&&<p>Версия клиентского портфеля: {decision.assignmentVersion.toString()}</p>}
    {decision.dispatch===SalesDispatchMode.LEAST_OPEN_DEALS&&<p>
      Команда: {ownerLabel(decision.target,choices)} · {decision.capacityOverflow?"Лимит достигнут — очередь команды":
        "Наименьшая нагрузка: "+(decision.selectedOpenDeals?.toString()??"—")+" открытых сделок"}
      {decision.maximumOpenDeals!==undefined?" · лимит "+decision.maximumOpenDeals:""}
    </p>}
  </div>;
}

async function allRules(signal:AbortSignal):Promise<SalesRoutingRule[]> {
  const items:SalesRoutingRule[]=[];
  let cursor="";
  const seen=new Set<string>();
  do {
    const page=await api.deals.listSalesRoutingRules({includeArchived:true,page:{pageSize:100,pageToken:cursor}},{signal});
    items.push(...page.rules);
    cursor=page.pageInfo?.nextPageToken??"";
    if(cursor&&seen.has(cursor))throw Error("Повторный курсор правил");
    if(cursor)seen.add(cursor);
  }while(cursor);
  return items.sort((a,b)=>(a.configuration?.priority??0)-(b.configuration?.priority??0)||a.ruleId.localeCompare(b.ruleId));
}
export function SalesRoutingWorkspace({isAdmin,choices,ownersUnavailable,territories}:{isAdmin:boolean;choices:Choices;ownersUnavailable:boolean;territories:SalesTerritory[]}) {
  const rules=useQuery(useCallback(allRules,[]));
  const catalog=useQuery(useCallback(allPipelines,[]));
  const [editing,setEditing]=useState<SalesRoutingRule|"new">();
  const [id,setId]=useState<string>(newId);
  const [pipeline,setPipeline]=useState("");
  const [target,setTarget]=useState("");
  const [dispatch,setDispatch]=useState(SalesDispatchMode.DIRECT);
  const [archived,setArchived]=useState(false);
  const [currency,setCurrency]=useState("");
  const [includeArchived,setIncludeArchived]=useState(false);
  const mutation=useMutation({
    storageKey:"crm.sales.routing-rule.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.deals.getSalesRoutingRule({ruleId:resourceId,receiptIdempotencyKey:key},{signal})).confirmedMutation,
    refresh:async()=>{setEditing(undefined);await rules.reload();}
  });
  const pending=mutation.busy||mutation.pending;
  const ready=!ownersUnavailable&&!catalog.busy&&!catalog.error&&!!catalog.data;
  const old=editing==="new"?undefined:editing;
  const cfg=old?.configuration;
  const criteria=cfg?.criteria;
  const start=(rule?:SalesRoutingRule)=>{
    setEditing(rule??"new");setId(rule?.ruleId??newId());
    setPipeline(rule?.configuration?.criteria?.pipelineId??"");
    setTarget(ownerValue(rule?.configuration?.target));
    setDispatch(rule?.configuration?.dispatch??SalesDispatchMode.DIRECT);
    setArchived(rule?.configuration?.archived??false);
    setCurrency(rule?.configuration?.criteria?.currencyCode??"");
    mutation.changed();
  };
  const activePipeline=catalog.data?.pipelines.find(p=>p.pipelineId===pipeline);
  const integer=(value:string,label:string)=>{
    const n=Number(value);
    if(!value||!Number.isInteger(n)||n<=0||n>4294967295)throw Error(label+": введите целое положительное число");
    return n;
  };
  return <section aria-label="Правила распределения продаж">
    <h3>Правила распределения продаж</h3>
    <p>Первое подходящее активное правило по приоритету. Если правила нет, применяется владелец портфеля, затем территории. При автоматическом назначении недоступный владелец не заменяется произвольно.</p>
    <QueryNotice query={rules}/><QueryNotice query={catalog}/>
    <p><label><input type="checkbox" checked={includeArchived} onChange={e=>setIncludeArchived(e.target.checked)}/> Показать архивные правила</label></p>
    {isAdmin&&<Button disabled={pending||!ready||rules.busy} onClick={()=>start()}>Создать правило распределения</Button>}
    <ScrollTable label="Правила распределения — таблица">
      <table><thead><tr><th scope="col">Правило</th><th scope="col">Приоритет</th><th scope="col">Условия</th><th scope="col">Назначение</th><th scope="col">Состояние</th><th scope="col">Управление</th></tr></thead>
        <tbody>{rules.data?.filter(r=>includeArchived||!r.configuration?.archived).map(r=>{
          const c=r.configuration;const q=c?.criteria;const p=catalog.data?.pipelines.find(p=>p.pipelineId===q?.pipelineId);
          const labels=[q?.territoryId?territories.find(t=>t.territoryId===q.territoryId)?.configuration?.name??q.territoryId:"",
            q?.pipelineId?p?.name??q.pipelineId:"",q?.stageId?p?.stages.find(s=>s.stageId===q.stageId)?.name??q.stageId:"",
            q?.currencyCode??"",q?.minimumMinorUnits!==undefined&&q.currencyCode&&catalog.data?"от "+decimalUnits(q.minimumMinorUnits,q.currencyCode,catalog.data.currencies):"",
            q?.maximumMinorUnits!==undefined&&q.currencyCode&&catalog.data?"до "+decimalUnits(q.maximumMinorUnits,q.currencyCode,catalog.data.currencies):""].filter(Boolean);
          return <tr key={r.ruleId}><th scope="row">{c?.name}</th><td>{c?.priority}</td><td>{labels.join(" · ")||"Все сделки"}</td>
            <td>{ownerLabel(c?.target,choices)} · {c?.dispatch===SalesDispatchMode.LEAST_OPEN_DEALS?"Наименьшая нагрузка":"Прямое назначение"}
              {c?.maximumOpenDeals!==undefined?" · лимит "+c.maximumOpenDeals:""}
              {c?.overflow===SalesCapacityOverflow.TEAM_QUEUE?" · переполнение в очередь":""}</td>
            <td>{c?.archived?"Архив":"Активно"}</td><td>{isAdmin&&<Button variant="secondary" disabled={pending||!ready} onClick={()=>start(r)}>Изменить правило</Button>}</td></tr>;
        })}</tbody></table>
    </ScrollTable>
    {editing&&isAdmin&&catalog.data&&<form key={id} aria-label="Настройка правила распределения" onChange={mutation.changed} onSubmit={event=>{
      event.preventDefault();const data=new FormData(event.currentTarget);
      void mutation.runIntent(()=>{
        const selected=ownerWire(target);if(!selected)throw Error("Выберите ответственного");
        const territoryId=String(data.get("territory")??""),stageId=String(data.get("stage")??""),min=String(data.get("minimum")??""),max=String(data.get("maximum")??""),cap=String(data.get("capacity")??"");
        const currencies=catalog.data?.currencies;if(!currencies)throw Error("Справочник валют не получен");
        if(!archived&&!territoryId&&!pipeline&&!currency&&!data.get("catchAll"))throw Error("Подтвердите правило для всех сделок");
        const configuration=archived&&cfg?{...cfg,archived:true}:{
          name:field(data,"name").trim(),priority:integer(field(data,"priority"),"Приоритет"),
          criteria:{...(territoryId?{territoryId}:{}),...(pipeline?{pipelineId:pipeline}:{}),...(stageId?{stageId}:{}),
            ...(currency?{currencyCode:currency}:{}),...(min?{minimumMinorUnits:minorUnits(min,currency,currencies)}:{}),
            ...(max?{maximumMinorUnits:minorUnits(max,currency,currencies)}:{})},
          target:selected,dispatch,...(dispatch===SalesDispatchMode.LEAST_OPEN_DEALS&&cap?{maximumOpenDeals:integer(cap,"Лимит")}:{}),
          overflow:dispatch===SalesDispatchMode.LEAST_OPEN_DEALS&&cap&&field(data,"overflow")==="queue"?SalesCapacityOverflow.TEAM_QUEUE:SalesCapacityOverflow.REJECT,
          archived:false
        };
        return old?prepareMutation(api.deals.updateSalesRoutingRule,{ruleId:id,configuration,precondition:{idempotencyKey:mutation.key,expectedVersion:old.version}},id):
          prepareMutation(api.deals.createSalesRoutingRule,{ruleId:id,configuration,precondition:{idempotencyKey:mutation.key}},id);
      },async()=>{setEditing(undefined);await rules.reload();});
    }}>
      <fieldset disabled={pending||archived}>
        <TextField label="Название правила" name="name" defaultValue={cfg?.name??""} required/>
        <TextField label="Приоритет правила" name="priority" type="number" defaultValue={cfg?.priority??10} required/>
        <SelectField label="Территория правила" name="territory" defaultValue={criteria?.territoryId??""}>
          <option value="">Любая территория</option>
          {territories.filter(t=>!t.configuration?.archived||t.territoryId===criteria?.territoryId).map(t=><option key={t.territoryId} value={t.territoryId} disabled={t.configuration?.archived}>{t.configuration?.name}</option>)}
        </SelectField>
        <SelectField label="Воронка правила" value={pipeline} onChange={e=>setPipeline(e.target.value)}>
          <option value="">Любая воронка</option>
          {catalog.data.pipelines.filter(p=>!p.archived||p.pipelineId===pipeline).map(p=><option key={p.pipelineId} value={p.pipelineId} disabled={p.archived}>{p.name}</option>)}
        </SelectField>
        <SelectField key={pipeline} label="Этап правила" name="stage" defaultValue={pipeline===criteria?.pipelineId?criteria?.stageId??"":""} disabled={!pipeline}>
          <option value="">Любой этап</option>
          {activePipeline?.stages.filter(s=>!s.archived||s.stageId===criteria?.stageId).map(s=><option key={s.stageId} value={s.stageId} disabled={s.archived}>{s.name}</option>)}
        </SelectField>
        <SelectField label="Валюта правила" value={currency} onChange={e=>setCurrency(e.target.value)}>
          <option value="">Любая валюта</option>
          {Array.from(catalog.data.currencies.keys()).map(c=><option key={c} value={c}>{c}</option>)}
        </SelectField>
        <TextField key={"min"+currency} label="Минимальная сумма правила" name="minimum" disabled={!currency} defaultValue={currency===criteria?.currencyCode&&criteria?.minimumMinorUnits!==undefined?decimalUnits(criteria.minimumMinorUnits,currency,catalog.data.currencies):""}/>
        <TextField key={"max"+currency} label="Максимальная сумма правила" name="maximum" disabled={!currency} defaultValue={currency===criteria?.currencyCode&&criteria?.maximumMinorUnits!==undefined?decimalUnits(criteria.maximumMinorUnits,currency,catalog.data.currencies):""}/>
        <p><label><input type="checkbox" name="catchAll" defaultChecked={!!cfg&&!criteria?.territoryId&&!criteria?.pipelineId&&!criteria?.currencyCode}/> Подтверждаю правило для всех сделок, если условия не заданы</label></p>
        <SelectField label="Получатель правила" value={target} onChange={e=>{setTarget(e.target.value);if(!e.target.value.startsWith("team:"))setDispatch(SalesDispatchMode.DIRECT);}}>
          <option value="">Выберите ответственного</option>
          {choices.filter(c=>c.eligible||c.value===target).map(c=><option key={c.value} value={c.value} disabled={!c.eligible}>{c.label}</option>)}
        </SelectField>
        <SelectField label="Способ распределения" value={dispatch} onChange={e=>setDispatch(e.target.value===String(SalesDispatchMode.LEAST_OPEN_DEALS)?SalesDispatchMode.LEAST_OPEN_DEALS:SalesDispatchMode.DIRECT)}>
          <option value={SalesDispatchMode.DIRECT}>Прямое назначение / очередь команды</option>
          {target.startsWith("team:")&&<option value={SalesDispatchMode.LEAST_OPEN_DEALS}>Сотрудник с наименьшим числом открытых сделок</option>}
        </SelectField>
        {dispatch===SalesDispatchMode.LEAST_OPEN_DEALS&&<>
          <TextField label="Лимит открытых сделок на сотрудника" name="capacity" type="number" defaultValue={cfg?.maximumOpenDeals??""}/>
          <SelectField label="При достижении лимита" name="overflow" defaultValue={cfg?.overflow===SalesCapacityOverflow.TEAM_QUEUE?"queue":"reject"}>
            <option value="reject">Отклонить назначение</option><option value="queue">Направить в очередь команды</option>
          </SelectField></>}
      </fieldset>
      {old&&<p><label><input type="checkbox" checked={archived} disabled={pending} onChange={e=>setArchived(e.target.checked)}/> Архивное правило</label></p>}
      <Button type="submit" disabled={pending||!ready||!target}>Сохранить правило</Button>{" "}
      <Button type="button" variant="secondary" disabled={pending} onClick={()=>setEditing(undefined)}>Отмена</Button>
    </form>}
    <MutationNotice state={mutation}/>
  </section>;
}

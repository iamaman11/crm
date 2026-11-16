import {useCallback,useEffect,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Deal,SalesPipeline} from "@ultimate-crm/client/cloudflare";
import {DealStatus} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice} from "./hooks";
import {AccountPicker} from "./Pickers";
import {minorUnits,decimalUnits,moneyText,basisPoints,percent} from "./money";
import styles from "./App.module.css";

type EditorMode="create"|"edit"|"advance"|"close"|null;
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
export function SalesScreen({initialAccountId=""}:{initialAccountId?:string}) {
  const [stageCursors,setStageCursors]=useState<Record<string,string>>({});
  const [accountCursor,setAccountCursor]=useState("");
  const [status,setStatus]=useState("");
  const [accountId,setAccountId]=useState(initialAccountId);
  const [pipelineId,setPipelineId]=useState("");
  const [selected,setSelected]=useState<Deal>();
  const [advance,setAdvance]=useState<Deal>();
  const [editorMode,setEditorMode]=useState<EditorMode>(null);
  const [draftId,setDraftId]=useState(newId);
  const [pipelineEditor,setPipelineEditor]=useState<SalesPipeline|"new"|null>(null);
  const [pipelineDraftId,setPipelineDraftId]=useState<string>(newId);
  const [pipelineName,setPipelineName]=useState("");
  const [pipelineArchived,setPipelineArchived]=useState(false);
  const [pipelineDirty,setPipelineDirty]=useState(false);
  const [stageEdits,setStageEdits]=useState<StageDraft[]>([]);
  const mutation=useMutation();
  const pipelineMutation=useMutation();
  const pipelinesQuery=useQuery(useCallback(allPipelines,[]));
  const pipelines=pipelinesQuery.data??[];
  useEffect(()=>{
    if(!pipelineId&&pipelines.length) {
      setPipelineId((pipelines.find(p=>!p.archived)??pipelines[0])?.pipelineId??"");
    }
  },[pipelineId,pipelines]);
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
  // Customer360 must include every Deal of the Account, not just the
  // currently selected Kanban pipeline. This is the existing typed ListDeals
  // read model with independent bounded keyset pagination, not another owner.
  const accountDealsQuery=useQuery(useCallback(async()=>{
    if(!accountId)return null;
    const selectedStatus=status?enumValue(status,[DealStatus.OPEN,DealStatus.WON,DealStatus.LOST],"Статус сделки"):undefined;
    return api.deals.listDeals({
      accountId,page:{pageSize:25,pageToken:accountCursor},
      ...(selectedStatus!==undefined?{status:selectedStatus}:{})
    });
  },[accountId,accountCursor,status]));
  const refreshSales=async()=>{
    await Promise.all([query.reload(),accountDealsQuery.reload()]);
  };
  const open=async(dealId:string,mode:"edit"|"advance"|"close")=>{
    try {
      const value=(await api.deals.getDeal({dealId})).deal;
      if(!value)throw Error("Сделка не найдена");
      setSelected(mode==="edit"?value:undefined);
      setAdvance(mode==="edit"?undefined:value);
      setEditorMode(mode);mutation.changed();
    } catch(error) {await mutation.run(()=>Promise.reject(error));}
  };
  const editPipeline=(pipeline?:SalesPipeline)=>{
    setPipelineEditor(pipeline??"new");
    setPipelineDraftId(pipeline?.pipelineId??newId());
    setPipelineName(pipeline?.name??"");
    setPipelineArchived(pipeline?.archived??false);
    setStageEdits(stageDrafts(pipeline));
    setPipelineDirty(false);
    pipelineMutation.changed();
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
    <PageHeader title="Сделки" description="Количество сделок по каждому этапу вычисляется на сервере. Колонки имеют независимую пагинацию." actions={<>
      <Button variant="secondary" onClick={()=>editPipeline()}>Создать воронку</Button>{" "}
      {activePipeline&&<Button variant="secondary" onClick={()=>editPipeline(activePipeline)}>Настроить воронку</Button>}{" "}
      <Button disabled={!activePipeline||!readyStage} onClick={()=>{
        setSelected(undefined);setAdvance(undefined);
        setEditorMode("create");mutation.changed();
      }}>Новая сделка</Button>
    </>}/>
    <QueryNotice query={pipelinesQuery}/>
    {!pipelinesQuery.busy&&pipelines.length===0&&
      <p>Сначала создайте воронку с этапами. Сделки нельзя создавать с произвольными кодами этапов.</p>}
    <SelectField label="Воронка продаж" value={pipelineId} onChange={e=>{
      setPipelineId(e.target.value);setStageCursors({});
    }}>
      {pipelines.map(p=><option value={p.pipelineId} key={p.pipelineId}>
        {p.name}{p.archived?" (архив)":""}
      </option>)}
    </SelectField>
    <TextField label="Фильтр по ID клиента" value={accountId} onChange={e=>{setAccountId(e.target.value);setStageCursors({});setAccountCursor("");}}/>
    <SelectField label="Статус сделки" value={status} onChange={e=>{setStatus(e.target.value);setStageCursors({});}}>
      <option value="">Все</option><option value={DealStatus.OPEN}>Открытые</option>
      <option value={DealStatus.WON}>Выигранные</option><option value={DealStatus.LOST}>Проигранные</option>
    </SelectField>
    <QueryNotice query={query}/>
    {accountId&&<section aria-label="Сделки клиента во всех воронках">
      <h2>Все сделки клиента во всех воронках</h2>
      <p>Независимый от выбранной доски список; показывает только текущую страницу, без выдуманного общего количества.</p>
      <QueryNotice query={accountDealsQuery}/>
      {accountDealsQuery.data?.deals.map(deal=>{
        const pipeline=pipelines.find(p=>p.pipelineId===deal.stage?.pipelineId);
        const stage=pipeline?.stages.find(s=>s.stageId===deal.stage?.stageId);
        return <Card key={deal.dealId} title={deal.name}>
          <p>Воронка: {pipeline?.name??deal.stage?.pipelineId??"неизвестна"} · Этап: {stage?.name??deal.stage?.stageId??"неизвестен"}</p>
          <p>{moneyText(deal.amount)} · {percent(deal.probabilityBasisPoints)}%</p>
          <p>{deal.status===DealStatus.OPEN?"Открыта":deal.status===DealStatus.WON?"Выиграна":"Проиграна"} · версия {deal.version.toString()}</p>
          {deal.status===DealStatus.OPEN&&<>
            <Button variant="secondary" onClick={()=>void open(deal.dealId,"edit")}>Редактировать</Button>{" "}
            <Button variant="secondary" onClick={()=>void open(deal.dealId,"close")}>Завершить</Button>
          </>}
        </Card>;
      })}
      {!accountDealsQuery.busy&&accountDealsQuery.data?.deals.length===0&&<p>У клиента нет сделок для выбранного статуса.</p>}
      {accountDealsQuery.data?.pageInfo?.nextPageToken&&
        <Button variant="secondary" disabled={accountDealsQuery.busy}
          onClick={()=>setAccountCursor(accountDealsQuery.data?.pageInfo?.nextPageToken??"")}>
          Следующие сделки клиента
        </Button>}
      {accountCursor&&<Button variant="secondary" disabled={accountDealsQuery.busy}
        onClick={()=>setAccountCursor("")}>К началу сделок клиента</Button>}
    </section>}
    {activePipeline&&<div className={styles.board}>
      {columns.map(stage=>{
        const boardPage=query.data?.find(entry=>entry.stageId===stage.stageId);
        return <section key={stage.stageId}>
          <h2>{stage.name}{stage.archived?" (архив)":""} · {boardPage?.totalCount.toString()??"—"}</h2>
          {boardPage?.deals.map(d=><Card key={d.dealId} title={d.name}>
            <p>{moneyText(d.amount)} · {percent(d.probabilityBasisPoints)}%</p>
            <p>Клиент: {d.accountId}</p>
            <p>{d.status===DealStatus.WON?"Выиграна":d.status===DealStatus.LOST?"Проиграна":
              d.status===DealStatus.OPEN?"Открыта":"Неизвестный статус"} · версия {d.version.toString()}</p>
            {d.status===DealStatus.OPEN&&<>
              <Button variant="secondary" onClick={()=>void open(d.dealId,"edit")}>Изменить сделку</Button>{" "}
              <Button variant="secondary" onClick={()=>void open(d.dealId,"advance")}>Изменить этап</Button>{" "}
              <Button variant="secondary" onClick={()=>void open(d.dealId,"close")}>Завершить сделку</Button>
            </>}
          </Card>)}
          {boardPage?.pageInfo?.nextPageToken&&<Button variant="secondary" disabled={query.busy}
            onClick={()=>setStageCursors(old=>({...old,[stage.stageId]:boardPage.pageInfo?.nextPageToken??""}))}>
            Следующие сделки этапа</Button>}
          {stageCursors[stage.stageId]&&<Button variant="secondary" disabled={query.busy}
            onClick={()=>setStageCursors(old=>({...old,[stage.stageId]:""}))}>
            К началу этапа</Button>}
        </section>;
      })}
    </div>}
    {!query.busy&&query.data?.every(entry=>entry.totalCount===0n)&&<p>Сделок пока нет.</p>}

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

    <RecordPanel open={editorMode!==null}
      title={advance?(editorMode==="close"?"Завершить сделку: ":"Этап сделки: ")+advance.name:
        selected?"Изменить сделку":"Новая сделка"}
      onClose={()=>{setEditorMode(null);setSelected(undefined);setAdvance(undefined);}}>
      {advance?<form key={advance.dealId+advance.version} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const closing=editorMode==="close";
          const currentStage=advance.stage;
          if(!currentStage)throw Error("У сделки отсутствует текущий этап");
          const next=nextStages.find(s=>s.stageId===field(data,"nextStage"));
          if(!closing&&!next)throw Error("Выберите доступный следующий этап");
          return api.deals.advanceStage({
            dealId:advance.dealId,
            targetStage:closing?currentStage:{
              pipelineId:currentStage.pipelineId,stageId:next?.stageId??"",
              ordinal:next?.ordinal??0
            },
            targetStatus:closing?enumValue(field(data,"targetStatus"),
              [DealStatus.WON,DealStatus.LOST],"Результат сделки"):DealStatus.OPEN,
            ...(closing?{closeReasonCode:field(data,"reason")}:{}),
            precondition:{idempotencyKey:mutation.key,expectedVersion:advance.version}
          });
        },async()=>{setEditorMode(null);setAdvance(undefined);await refreshSales();});
      }}><FormGrid>{editorMode==="close"?<>
        <p>Текущий этап сохраняется: {advancePipeline?.stages.find(s=>s.stageId===advance.stage?.stageId)?.name??advance.stage?.stageId??"неизвестен"}.</p>
        <SelectField label="Результат сделки" name="targetStatus" defaultValue={DealStatus.WON}>
          <option value={DealStatus.WON}>Выиграна</option>
          <option value={DealStatus.LOST}>Проиграна</option>
        </SelectField>
        <TextField label="Причина закрытия" name="reason" required/>
      </>:<>
        <SelectField label="Следующий этап" name="nextStage" required>
          {nextStages.map(stage=><option value={stage.stageId} key={stage.stageId}>
            {stage.name}</option>)}
        </SelectField>
        {nextStages.length===0&&<p>Следующих этапов нет. Сделку можно завершить на текущем этапе.</p>}
      </>}</FormGrid>
        <Button type="submit" disabled={mutation.busy||editorMode==="advance"&&nextStages.length===0}>
          {editorMode==="close"?"Завершить сделку":"Сохранить этап"}
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
          const common={probabilityBasisPoints:basisPoints(field(data,"probability"))};
          if(selected) {
            return api.deals.updateDeal({
              dealId:selected.dealId,
              name:{operation:{case:"set",value:field(data,"name")}},...common,
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
            precondition:{idempotencyKey:mutation.key}
          });
        },async()=>{setEditorMode(null);setSelected(undefined);setDraftId(newId());await query.reload();});
      }}><FormGrid>
        <TextField label="Название сделки" name="name" defaultValue={selected?.name??""} required/>
        {!selected&&<>
          <AccountPicker/>
          <SelectField label="Этап" name="stageId" defaultValue={readyStage?.stageId??""} required>
            {activePipeline?.stages.filter(stage=>!stage.archived).map(stage=>
              <option value={stage.stageId} key={stage.stageId}>{stage.name}</option>)}
          </SelectField>
        </>}
        {selected&&<SelectField label="Изменение суммы" name="amountMode" defaultValue="keep">
          <option value="keep">Сохранить прежнюю</option><option value="set">Установить</option>
          <option value="clear">Убрать</option>
        </SelectField>}
        <TextField label="Сумма" name="amount" inputMode="decimal"
          defaultValue={selected?.amount?decimalUnits(selected.amount.minorUnits,selected.amount.currencyCode):""}/>
        <SelectField label="Валюта" name="currency" defaultValue={selected?.amount?.currencyCode??"USD"}>
          {["USD","EUR","PLN","JPY","KWD"].map(c=><option key={c} value={c}>{c}</option>)}
        </SelectField>
        <TextField label="Вероятность, %" name="probability" inputMode="decimal"
          defaultValue={selected?percent(selected.probabilityBasisPoints):"0"} required/>
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

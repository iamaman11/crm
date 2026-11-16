import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Deal} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {DealStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue,boundedInt} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {AccountPicker} from "./Pickers";
import {minorUnits,decimalUnits,moneyText,basisPoints,percent} from "./money";
import styles from "./App.module.css";

export function SalesScreen({initialAccountId=""}:{initialAccountId?:string}) {
  const [cursor,setCursor]=useState("");
  const [status,setStatus]=useState("");
  const [accountId,setAccountId]=useState(initialAccountId);
  const [selected,setSelected]=useState<Deal>();
  const [advance,setAdvance]=useState<Deal>();
  const [editorMode,setEditorMode]=useState<"create"|"edit"|"advance"|"close"|null>(null);
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.deals.listDeals({page:{pageSize:25,pageToken:cursor},
    ...(accountId?{accountId}:{}),...(status?{status:enumValue(status,[DealStatus.OPEN,DealStatus.WON,DealStatus.LOST],"Статус сделки")}:{})}),[cursor,status,accountId]));
  const open=async(dealId:string,mode:"edit"|"advance"|"close")=>{
    try {const value=(await api.deals.getDeal({dealId})).deal;
      setSelected(mode==="edit"?value:undefined);setAdvance(mode==="edit"?undefined:value);setEditorMode(mode);mutation.changed();}
    catch(error){await mutation.run(()=>Promise.reject(error));}
  };
  const columns=[...new Set(query.data?.deals.map(d=>d.stage?.stageId??"Без этапа")??[])];
  return <>
    <PageHeader title="Сделки" description="Доска текущей страницы. Суммы передаются без потери точности." actions={<Button onClick={()=>{setSelected(undefined);setAdvance(undefined);setEditorMode("create");mutation.changed();}}>Новая сделка</Button>}/>
    <TextField label="Фильтр по ID клиента" value={accountId} onChange={e=>{setAccountId(e.target.value);setCursor("");}}/>
    <SelectField label="Статус сделки" value={status} onChange={e=>{setStatus(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value={DealStatus.OPEN}>Открытые</option><option value={DealStatus.WON}>Выигранные</option><option value={DealStatus.LOST}>Проигранные</option>
    </SelectField><QueryNotice query={query}/>
    <div className={styles.board}>{columns.map(stage=><section key={stage}><h2>{stage}</h2>
      {query.data?.deals.filter(d=>(d.stage?.stageId??"Без этапа")===stage).map(d=><Card key={d.dealId} title={d.name}>
        <p>{moneyText(d.amount)} · {percent(d.probabilityBasisPoints)}%</p><p>Клиент: {d.accountId}</p>
        <p>{d.status===DealStatus.WON?"Выиграна":d.status===DealStatus.LOST?"Проиграна":d.status===DealStatus.OPEN?"Открыта":"Неизвестный статус"} · версия {d.version.toString()}</p>
        {d.status===DealStatus.OPEN&&<>
          <Button variant="secondary" onClick={()=>void open(d.dealId,"edit")}>Изменить сделку</Button>{" "}
          <Button variant="secondary" onClick={()=>void open(d.dealId,"advance")}>Изменить этап</Button>{" "}
          <Button variant="secondary" onClick={()=>void open(d.dealId,"close")}>Завершить сделку</Button>
        </>}
      </Card>)}
    </section>)}</div>
    {!query.busy&&query.data?.deals.length===0&&<p>Сделок пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={editorMode!==null} title={advance?(editorMode==="close"?"Завершить сделку: ":"Этап сделки: ")+advance.name:selected?"Изменить сделку":"Новая сделка"} onClose={()=>{setEditorMode(null);setSelected(undefined);setAdvance(undefined);}}>{advance?<form key={advance.dealId+advance.version} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>{
        const closing=editorMode==="close";
        const currentStage=advance.stage;
        if(closing&&!currentStage)throw Error("У сделки отсутствует текущий этап");
        return api.deals.advanceStage({dealId:advance.dealId,
          targetStage:closing&&currentStage?currentStage:{
            pipelineId:field(data,"pipeline"),stageId:field(data,"stage"),
            ordinal:boundedInt(field(data,"ordinal"),1,65535,"Порядок этапа")},
          targetStatus:closing?enumValue(field(data,"targetStatus"),[DealStatus.WON,DealStatus.LOST],"Результат сделки"):DealStatus.OPEN,
          ...(closing?{closeReasonCode:field(data,"reason")}:{}),
          precondition:{idempotencyKey:mutation.key,expectedVersion:advance.version}
        });
      },async()=>{setEditorMode(null);setAdvance(undefined);await query.reload();});
    }}><FormGrid>{editorMode==="close"?<>
      <p>Текущий этап сохраняется: {advance.stage?.stageId??"неизвестен"}.</p>
      <SelectField label="Результат сделки" name="targetStatus" defaultValue={DealStatus.WON}>
        <option value={DealStatus.WON}>Выиграна</option><option value={DealStatus.LOST}>Проиграна</option>
      </SelectField>
      <TextField label="Причина закрытия" name="reason" required/>
    </>:<>
      <TextField label="Воронка" name="pipeline" defaultValue={advance.stage?.pipelineId??""} required/>
      <TextField label="Код следующего этапа" name="stage" required/>
      <TextField label="Порядок этапа" name="ordinal" type="number" min="1" max="65535" defaultValue={(advance.stage?.ordinal??0)+1} required/>
    </>}</FormGrid><Button type="submit" disabled={mutation.busy}>{editorMode==="close"?"Завершить сделку":"Сохранить этап"}</Button>{" "}
      <Button variant="secondary" onClick={()=>{setEditorMode(null);setAdvance(undefined);mutation.changed();}}>Отмена</Button>
    </form>:<>
      <form key={selected?selected.dealId+selected.version:draftId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const currencyCode=field(data,"currency"),amountText=field(data,"amount");
          const mode=selected?field(data,"amountMode"):"set";
          const amount=mode==="set"&&amountText?{minorUnits:minorUnits(amountText,currencyCode),currencyCode}:undefined;
          const common={probabilityBasisPoints:basisPoints(field(data,"probability"))};
          if(selected){
            return api.deals.updateDeal({dealId:selected.dealId,
              name:{operation:{case:"set",value:field(data,"name")}},...common,
              ...(mode==="clear"?{amount:{operation:{case:"clear" as const,value:true}}}:
                mode==="set"?(amount?{amount:{operation:{case:"set" as const,value:amount}}}:(()=>{throw Error("Введите сумму");})()):{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}});
          }
          return api.deals.createDeal({dealId:draftId,name:field(data,"name"),accountId:field(data,"accountId"),
            stage:{pipelineId:field(data,"pipeline"),stageId:field(data,"stage"),ordinal:boundedInt(field(data,"ordinal"),0,10000,"Порядок этапа")},
            ...(amount?{amount}:{}),...common,precondition:{idempotencyKey:mutation.key}});
        },async()=>{setEditorMode(null);setSelected(undefined);setDraftId(newId());await query.reload();});
      }}><FormGrid>
        <TextField label="Название сделки" name="name" defaultValue={selected?.name??""} required/>
        {!selected&&<><AccountPicker/><TextField label="Воронка" name="pipeline" defaultValue="pipeline.enterprise" required/>
          <TextField label="Этап" name="stage" defaultValue="lead" required/><TextField label="Порядок этапа" name="ordinal" type="number" min="1" max="65535" defaultValue="1" required/></>}
        {selected&&<SelectField label="Изменение суммы" name="amountMode" defaultValue="keep"><option value="keep">Сохранить прежнюю</option><option value="set">Установить</option><option value="clear">Убрать</option></SelectField>}
        <TextField label="Сумма" name="amount" inputMode="decimal" defaultValue={selected?.amount?decimalUnits(selected.amount.minorUnits,selected.amount.currencyCode):""}/>
        <SelectField label="Валюта" name="currency" defaultValue={selected?.amount?.currencyCode??"USD"}>
          {["USD","EUR","PLN","JPY","KWD"].map(c=><option key={c} value={c}>{c}</option>)}
        </SelectField><TextField label="Вероятность, %" name="probability" inputMode="decimal" defaultValue={selected?percent(selected.probabilityBasisPoints):"0"} required/>
      </FormGrid><Button type="submit" disabled={mutation.busy}>Сохранить сделку</Button>{" "}
      {selected&&<Button variant="secondary" onClick={()=>{setEditorMode(null);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
      </form></>}
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel><MutationNotice state={mutation}/>
  </>;
}

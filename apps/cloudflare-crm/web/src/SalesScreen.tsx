import {useCallback,useState} from "react";
import {Button,Card,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Deal} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {AccountPicker} from "./Pickers";
import {minorUnits,decimalUnits,moneyText,basisPoints,percent} from "./money";
import styles from "./App.module.css";

export function SalesScreen() {
  const [cursor,setCursor]=useState("");
  const [status,setStatus]=useState("");
  const [selected,setSelected]=useState<Deal>();
  const [advance,setAdvance]=useState<Deal>();
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.deals.listDeals({page:{pageSize:25,pageToken:cursor},
    ...(status?{status:Number(status)}:{})}),[cursor,status]));
  const open=async(dealId:string,mode:"edit"|"advance")=>{
    try {const value=(await api.deals.getDeal({dealId})).deal;
      setSelected(mode==="edit"?value:undefined);setAdvance(mode==="advance"?value:undefined);mutation.changed();}
    catch(error){await mutation.run(()=>Promise.reject(error));}
  };
  const columns=[...new Set(query.data?.deals.map(d=>d.stage?.stageId??"Без этапа")??[])];
  return <>
    <PageHeader title="Сделки" description="Доска текущей страницы. Суммы передаются без потери точности."/>
    <SelectField label="Статус сделки" value={status} onChange={e=>{setStatus(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value="1">Открытые</option><option value="2">Выигранные</option><option value="3">Проигранные</option>
    </SelectField><QueryNotice query={query}/>
    <div className={styles.board}>{columns.map(stage=><section key={stage}><h2>{stage}</h2>
      {query.data?.deals.filter(d=>(d.stage?.stageId??"Без этапа")===stage).map(d=><Card key={d.dealId} title={d.name}>
        <p>{moneyText(d.amount)} · {percent(d.probabilityBasisPoints)}%</p><p>Клиент: {d.accountId}</p>
        <p>{d.status===2?"Выиграна":d.status===3?"Проиграна":"Открыта"} · версия {d.version.toString()}</p>
        <Button variant="secondary" onClick={()=>void open(d.dealId,"edit")}>Изменить сделку</Button>{" "}
        {d.status===1&&<Button variant="secondary" onClick={()=>void open(d.dealId,"advance")}>Изменить этап</Button>}
      </Card>)}
    </section>)}</div>
    {!query.busy&&query.data?.deals.length===0&&<p>Сделок пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    {advance?<Card title={"Этап сделки: "+advance.name}><form key={advance.dealId+advance.version} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>api.deals.advanceStage({dealId:advance.dealId,
        targetStage:{pipelineId:field(data,"pipeline"),stageId:field(data,"stage"),ordinal:Number(field(data,"ordinal"))},
        targetStatus:Number(field(data,"targetStatus")),...(field(data,"reason")?{closeReasonCode:field(data,"reason")}:{}),
        precondition:{idempotencyKey:mutation.key,expectedVersion:advance.version}
      }),async()=>{setAdvance(undefined);await query.reload();});
    }}><FormGrid>
      <TextField label="Воронка" name="pipeline" defaultValue={advance.stage?.pipelineId??""} required/>
      <TextField label="Код следующего этапа" name="stage" required/>
      <TextField label="Порядок этапа" name="ordinal" type="number" min="1" max="65535" defaultValue={(advance.stage?.ordinal??0)+1} required/>
      <SelectField label="Результат" name="targetStatus" defaultValue="1"><option value="1">Продолжить</option><option value="2">Выиграна</option><option value="3">Проиграна</option></SelectField>
      <TextField label="Код причины закрытия" name="reason"/>
    </FormGrid><Button type="submit" disabled={mutation.busy}>Сохранить этап</Button>{" "}
      <Button variant="secondary" onClick={()=>{setAdvance(undefined);mutation.changed();}}>Отмена</Button>
    </form></Card>:<Card title={selected?"Изменить сделку":"Новая сделка"}>
      <form key={selected?selected.dealId+selected.version:draftId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const currencyCode=field(data,"currency"),amountText=field(data,"amount");
          const amount=amountText?{minorUnits:minorUnits(amountText,currencyCode),currencyCode}:undefined;
          const common={probabilityBasisPoints:basisPoints(field(data,"probability"))};
          if(selected){
            const mode=field(data,"amountMode");
            return api.deals.updateDeal({dealId:selected.dealId,
              name:{operation:{case:"set",value:field(data,"name")}},...common,
              ...(mode==="clear"?{amount:{operation:{case:"clear" as const,value:true}}}:
                mode==="set"?(amount?{amount:{operation:{case:"set" as const,value:amount}}}:(()=>{throw Error("Введите сумму");})()):{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}});
          }
          return api.deals.createDeal({dealId:draftId,name:field(data,"name"),accountId:field(data,"accountId"),
            stage:{pipelineId:field(data,"pipeline"),stageId:field(data,"stage"),ordinal:Number(field(data,"ordinal"))},
            ...(amount?{amount}:{}),...common,precondition:{idempotencyKey:mutation.key}});
        },async()=>{setSelected(undefined);setDraftId(newId());await query.reload();});
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
      {selected&&<Button variant="secondary" onClick={()=>{setSelected(undefined);mutation.changed();}}>Отмена</Button>}
      </form>
    </Card>}<MutationNotice state={mutation}/>
  </>;
}

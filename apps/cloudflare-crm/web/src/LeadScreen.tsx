import {useCallback,useState} from "react";
import {Button,Card,FormGrid,PageHeader,RecordPanel,SelectField,TextField} from "@ultimate-crm/ui";
import {LeadStatus} from "@ultimate-crm/client/cloudflare";
import type {Lead} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {MutationNotice,Paging,QueryNotice,prepareMutation,useMutation,useQuery} from "./hooks";
import {AccountPicker,PartyPicker} from "./Pickers";
import {dealHref,partyHref} from "./routes";
import {allPipelines} from "./salesPipelineCatalog";

const statusLabels:Record<LeadStatus,string>={
  [LeadStatus.UNSPECIFIED]:"Неизвестный статус",
  [LeadStatus.NEW]:"Новый",
  [LeadStatus.QUALIFIED]:"Квалифицирован",
  [LeadStatus.REJECTED]:"Отклонён",
  [LeadStatus.CONVERTED]:"Конвертирован",
};

type Editor={kind:"create";id:string}|{kind:"qualify"|"reject"|"convert";lead:Lead};

export function LeadScreen() {
  const [cursor,setCursor]=useState("");
  const [focused,setFocused]=useState("");
  const [editor,setEditor]=useState<Editor>();
  const query=useQuery(useCallback((signal:AbortSignal)=>
    api.leads.listLeads({page:{pageSize:25,pageToken:cursor}},{signal}),[cursor]));
  const detail=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!focused)return undefined;
    const [record,history]=await Promise.all([
      api.leads.getLead({leadId:focused},{signal}),
      api.leads.listLeadEvents({leadId:focused},{signal}),
    ]);
    return {entry:record.entry,events:history.events};
  },[focused]));
  const refresh=async()=>{setEditor(undefined);await Promise.all([query.reload(),detail.reload()]);};
  const mutation=useMutation({
    storageKey:"crm.lead.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.leads.getLead({
      leadId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async({resourceId})=>{setFocused(resourceId);await refresh();},
  });
  const locked=mutation.busy||mutation.pending;
  const open=(next:Editor)=>{mutation.changed();setEditor(next);};

  return <>
    <PageHeader title="Лиды" actions={query.data?.canCreate&&
      <Button disabled={locked} onClick={()=>open({kind:"create",id:newId()})}>Новый лид</Button>}/>
    <QueryNotice query={query}/>
    {query.data?.entries.map(({lead,capabilities})=>lead&&<Card title={lead.name} key={lead.leadId}>
      <p>{statusLabels[lead.status]}</p>
      <Button variant="secondary" onClick={()=>setFocused(lead.leadId)}>История лида</Button>{" "}
      {capabilities?.qualify&&<Button disabled={locked} onClick={()=>open({kind:"qualify",lead})}>Квалифицировать</Button>}{" "}
      {capabilities?.reject&&<Button variant="secondary" disabled={locked} onClick={()=>open({kind:"reject",lead})}>Отклонить</Button>}{" "}
      {capabilities?.convert&&<Button disabled={locked} onClick={()=>open({kind:"convert",lead})}>Создать сделку</Button>}{" "}
      {lead.dealId&&<a href={dealHref(lead.dealId)}>Открыть сделку</a>}
    </Card>)}
    {!query.busy&&!query.error&&query.data?.entries.length===0&&<p>Лидов пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
      onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    {focused&&<Card title="История лида">
      <QueryNotice query={detail}/>
      {detail.data?.entry?.lead&&<p>
        <a href={partyHref(detail.data.entry.lead.originalPartyId)}>Первоначальный участник</a>
      </p>}
      <ol>{detail.data?.events.map(({snapshot})=>snapshot&&<li key={snapshot.version.toString()}>
        {statusLabels[snapshot.status]}{snapshot.reason&&snapshot.status!==LeadStatus.CONVERTED&&" — "+snapshot.reason}
      </li>)}</ol>
      <Button variant="secondary" onClick={()=>setFocused("")}>Закрыть историю</Button>
    </Card>}
    {editor&&<RecordPanel open title={editorTitle(editor.kind)} onClose={()=>{if(!locked)setEditor(undefined);}}>
      <form onChange={mutation.changed} onSubmit={event=>{
        event.preventDefault();
        const data=new FormData(event.currentTarget);
        void mutation.runIntent(()=>{
          const idempotencyKey=mutation.key;
          if(editor.kind==="create")return prepareMutation(api.leads.createLead,{
            leadId:editor.id,partyId:field(data,"partyId"),name:field(data,"name"),
            precondition:{idempotencyKey},
          },editor.id);
          const {leadId,version,name}=editor.lead;
          const precondition={idempotencyKey,expectedVersion:version};
          if(editor.kind==="convert")return prepareMutation(api.leads.convertLead,{
            leadId,precondition,deal:{dealId:leadId,name,accountId:field(data,"accountId"),
              stage:{pipelineId:field(data,"pipelineId"),stageId:field(data,"stageId")}},
          },leadId);
          const intent={leadId,reason:field(data,"reason"),precondition};
          return editor.kind==="qualify"?prepareMutation(api.leads.qualifyLead,intent,leadId)
            :prepareMutation(api.leads.rejectLead,intent,leadId);
        },async()=>{setFocused(editor.kind==="create"?editor.id:editor.lead.leadId);await refresh();});
      }}>
        <FormGrid>
          {editor.kind==="create"?<>
            <PartyPicker/>
            <TextField label="Тема интереса" name="name" maxLength={200} required/>
          </>:editor.kind==="convert"?<ConversionFields/>:
            <TextField label="Причина" name="reason" maxLength={500} required/>}
        </FormGrid>
        <Button type="submit" disabled={locked}>Сохранить</Button>
      </form>
      <MutationNotice state={mutation}/>
    </RecordPanel>}
    {!editor&&<MutationNotice state={mutation}/>}
  </>;
}

function editorTitle(kind:Editor["kind"]):string {
  switch(kind){
    case "create":return "Новый лид";
    case "qualify":return "Квалифицировать лид";
    case "reject":return "Отклонить лид";
    case "convert":return "Создать сделку из лида";
  }
}

function ConversionFields() {
  const catalog=useQuery(useCallback(allPipelines,[]));
  const [pipelineId,setPipelineId]=useState("");
  const pipelines=catalog.data?.pipelines.filter(pipeline=>!pipeline.archived)??[];
  const stages=pipelines.find(pipeline=>pipeline.pipelineId===pipelineId)?.stages.filter(stage=>!stage.archived)??[];
  return <>
    <AccountPicker/>
    <SelectField label="Воронка" name="pipelineId" required value={pipelineId}
      onChange={event=>setPipelineId(event.target.value)}>
      <option value="">Выберите воронку</option>
      {pipelines.map(pipeline=><option value={pipeline.pipelineId} key={pipeline.pipelineId}>{pipeline.name}</option>)}
    </SelectField>
    <SelectField label="Этап" name="stageId" required key={pipelineId}>
      <option value="">Выберите этап</option>
      {stages.map(stage=><option value={stage.stageId} key={stage.stageId}>{stage.name}</option>)}
    </SelectField>
    <QueryNotice query={catalog}/>
  </>;
}

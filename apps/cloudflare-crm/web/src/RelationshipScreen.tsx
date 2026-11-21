import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {PartyRelationship} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {RelationshipDirectionality,RelationshipStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker,PartyName} from "./Pickers";
import {TimeFields} from "./TimeFields";
import {relationshipHref,scopeHref} from "./routes";
import {timePatch} from "./timePatch";

export function RelationshipScreen({initialPartyId="",focusedRelationshipId=""}:{
  initialPartyId?:string;focusedRelationshipId?:string;
}) {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState(initialPartyId);
  const [status,setStatus]=useState("");
  const [selected,setSelected]=useState<PartyRelationship>();
  const [editorOpen,setEditorOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const query=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(focusedRelationshipId)return undefined;
    return api.relationships.listPartyRelationships({page:{pageSize:25,pageToken:cursor},
      ...(partyId?{partyId}:{}),
      ...(status?{status:enumValue(status,[RelationshipStatus.ACTIVE,RelationshipStatus.INACTIVE],"Статус связи")}:{})
    },{signal});
  },[focusedRelationshipId,cursor,partyId,status]));
  const focused=useQuery(useCallback(async(signal:AbortSignal)=>{
    if(!focusedRelationshipId)return undefined;
    const relationship=(await api.relationships.getPartyRelationship({
      relationshipId:focusedRelationshipId
    },{signal})).relationship;
    // Both ends of a directional/reciprocal relation are legitimate Party roots.
    if(!relationship||initialPartyId&&relationship.fromPartyId!==initialPartyId&&
      relationship.toPartyId!==initialPartyId)
      throw Error("Связь не относится к выбранному участнику");
    return relationship;
  },[focusedRelationshipId,initialPartyId]));
  const refresh=async()=>{await Promise.all([query.reload(),focused.reload()]);};
  const visible=focusedRelationshipId?(focused.data?[focused.data]:[]):query.data?.relationships??[];
  const mutation=useMutation({
    storageKey:"crm.relationship.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.relationships.getPartyRelationship({
      relationshipId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh
  });
  return <>
    <PageHeader title={focusedRelationshipId?"Карточка связи":"Связи"}
      description="Отношения между людьми и организациями."
      actions={focusedRelationshipId
        ?<a href={scopeHref("relationships",initialPartyId||focused.data?.fromPartyId||"")}>К связям участника</a>
        :<Button disabled={mutation.busy||mutation.pending} onClick={()=>{setSelected(undefined);setEditorOpen(true);mutation.changed();}}>Создать связь</Button>}/>
    {!focusedRelationshipId&&<>
    <PartyPicker label="Фильтр по участнику" value={partyId} required={false}
      onSelect={id=>{setPartyId(id);setCursor("");}}/>
    <SelectField label="Статус связей" value={status} onChange={event=>{setStatus(event.target.value);setCursor("");}}>
      <option value="">Все статусы</option>
      <option value={RelationshipStatus.ACTIVE}>Активные</option>
      <option value={RelationshipStatus.INACTIVE}>Неактивные</option>
    </SelectField>
    </>}
    <QueryNotice query={focusedRelationshipId?focused:query}/>
    {visible.map(r=><Card key={r.relationshipId} title={r.type?.code??"Связь"}>
      <p>{r.type?.fromRole}: <PartyName partyId={r.fromPartyId}/> → {r.type?.toRole}: <PartyName partyId={r.toPartyId}/></p>
      <p>{r.status===RelationshipStatus.ACTIVE?"Активна":r.status===RelationshipStatus.INACTIVE?"Неактивна":"Неизвестный статус"} · с {dateText(r.validFrom)} до {dateText(r.validUntil)}</p>
      {!focusedRelationshipId&&<p><a href={relationshipHref(r.relationshipId,partyId||r.fromPartyId)}>Открыть карточку связи</a></p>}
      <Button variant="secondary" disabled={mutation.busy||mutation.pending} onClick={async()=>{
        try {setSelected((await api.relationships.getPartyRelationship({relationshipId:r.relationshipId})).relationship);setEditorOpen(true);mutation.changed();}
        catch(error){await mutation.reportError(error);}
      }}>Редактировать связь</Button>
    </Card>)}
    {!focusedRelationshipId&&!query.busy&&visible.length===0&&<p>Связей пока нет.</p>}
    {!focusedRelationshipId&&<Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
      onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>}
    <RecordPanel open={editorOpen} title={selected?"Изменить связь":"Новая связь"} onClose={()=>{if(mutation.busy||mutation.pending)return;setEditorOpen(false);setSelected(undefined);}}><form key={selected?selected.relationshipId+selected.version:draftId}
      onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.runIntent(()=>{
          if(selected){
            const validFrom=timePatch(data,"from"),validUntil=timePatch(data,"until");
            return prepareMutation(api.relationships.updatePartyRelationship,{relationshipId:selected.relationshipId,
              status:enumValue(field(data,"status"),[RelationshipStatus.ACTIVE,RelationshipStatus.INACTIVE],"Статус связи"),...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}},selected.relationshipId);
          }
          const validFrom=timestamp(field(data,"from")),validUntil=timestamp(field(data,"until"));
          return prepareMutation(api.relationships.createPartyRelationship,{relationshipId:draftId,
            fromPartyId:field(data,"fromParty"),toPartyId:field(data,"toParty"),
            type:{code:field(data,"code"),directionality:enumValue(field(data,"direction"),[RelationshipDirectionality.DIRECTIONAL,RelationshipDirectionality.RECIPROCAL],"Направление"),
              fromRole:field(data,"fromRole"),toRole:field(data,"toRole")},
            ...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
            precondition:{idempotencyKey:mutation.key}},draftId);
        },async()=>{setEditorOpen(false);setSelected(undefined);setDraftId(newId());await refresh();});
      }}><FormGrid>
      {!selected&&<><PartyPicker name="fromParty" label="Первый участник" initialPartyId={partyId}/><PartyPicker name="toParty" label="Второй участник"/>
        <TextField label="Код отношения" name="code" defaultValue="employment" required/>
        <TextField label="Роль первого" name="fromRole" defaultValue="employer" required/>
        <TextField label="Роль второго" name="toRole" defaultValue="employee" required/>
        <SelectField label="Направление" name="direction" defaultValue="1"><option value={RelationshipDirectionality.DIRECTIONAL}>Направленная</option><option value={RelationshipDirectionality.RECIPROCAL}>Взаимная</option></SelectField></>}
      {selected&&<SelectField label="Статус" name="status" defaultValue={selected.status}><option value={RelationshipStatus.ACTIVE}>Активна</option><option value={RelationshipStatus.INACTIVE}>Неактивна</option></SelectField>}
      <TimeFields name="from" label="Действует с" editing={!!selected}/><TimeFields name="until" label="Действует до" editing={!!selected}/>
    </FormGrid><Button type="submit" disabled={mutation.busy||mutation.pending}>Сохранить связь</Button>{" "}
    {selected&&<Button type="button" variant="secondary" disabled={mutation.busy||mutation.pending} onClick={()=>{setEditorOpen(false);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
    </form><MutationNotice state={mutation}/></RecordPanel>
    {!editorOpen&&<MutationNotice state={mutation}/>}
  </>;
}

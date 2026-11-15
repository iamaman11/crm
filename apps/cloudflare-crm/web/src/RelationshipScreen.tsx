import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {PartyRelationship} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {RelationshipDirectionality,RelationshipStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {TimeFields} from "./TimeFields";
import {timePatch} from "./timePatch";

export function RelationshipScreen({initialPartyId=""}:{initialPartyId?:string}) {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState(initialPartyId);
  const [selected,setSelected]=useState<PartyRelationship>();
  const [editorOpen,setEditorOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.relationships.listPartyRelationships({page:{pageSize:25,pageToken:cursor},
    ...(partyId?{partyId}:{})}),[cursor,partyId]));
  return <>
    <PageHeader title="Связи" description="Отношения между людьми и организациями." actions={<Button onClick={()=>{setSelected(undefined);setEditorOpen(true);mutation.changed();}}>Создать связь</Button>}/>
    <TextField label="Фильтр по ID участника" value={partyId} onChange={e=>{setPartyId(e.target.value);setCursor("");}}/>
    <QueryNotice query={query}/>
    {query.data?.relationships.map(r=><Card key={r.relationshipId} title={r.type?.code??"Связь"}>
      <p>{r.type?.fromRole}: {r.fromPartyId} → {r.type?.toRole}: {r.toPartyId}</p>
      <p>{r.status===RelationshipStatus.RELATIONSHIP_STATUS_ACTIVE?"Активна":r.status===RelationshipStatus.RELATIONSHIP_STATUS_INACTIVE?"Неактивна":"Неизвестный статус"} · с {dateText(r.validFrom)} до {dateText(r.validUntil)}</p>
      <Button variant="secondary" onClick={async()=>{
        try {setSelected((await api.relationships.getPartyRelationship({relationshipId:r.relationshipId})).relationship);setEditorOpen(true);mutation.changed();}
        catch(error){await mutation.run(()=>Promise.reject(error));}
      }}>Редактировать связь</Button>
    </Card>)}
    {!query.busy&&query.data?.relationships.length===0&&<p>Связей пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={editorOpen} title={selected?"Изменить связь":"Новая связь"} onClose={()=>{setEditorOpen(false);setSelected(undefined);}}><form key={selected?selected.relationshipId+selected.version:draftId}
      onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          if(selected){
            const validFrom=timePatch(data,"from"),validUntil=timePatch(data,"until");
            return api.relationships.updatePartyRelationship({relationshipId:selected.relationshipId,
              status:enumValue(field(data,"status"),[RelationshipStatus.RELATIONSHIP_STATUS_ACTIVE,RelationshipStatus.RELATIONSHIP_STATUS_INACTIVE],"Статус связи"),...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}});
          }
          const validFrom=timestamp(field(data,"from")),validUntil=timestamp(field(data,"until"));
          return api.relationships.createPartyRelationship({relationshipId:draftId,
            fromPartyId:field(data,"fromParty"),toPartyId:field(data,"toParty"),
            type:{code:field(data,"code"),directionality:enumValue(field(data,"direction"),[RelationshipDirectionality.RELATIONSHIP_DIRECTIONALITY_DIRECTIONAL,RelationshipDirectionality.RELATIONSHIP_DIRECTIONALITY_RECIPROCAL],"Направление"),
              fromRole:field(data,"fromRole"),toRole:field(data,"toRole")},
            ...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
            precondition:{idempotencyKey:mutation.key}});
        },async()=>{setEditorOpen(false);setSelected(undefined);setDraftId(newId());await query.reload();});
      }}><FormGrid>
      {!selected&&<><PartyPicker name="fromParty" label="Первый участник"/><PartyPicker name="toParty" label="Второй участник"/>
        <TextField label="Код отношения" name="code" defaultValue="employment" required/>
        <TextField label="Роль первого" name="fromRole" defaultValue="employer" required/>
        <TextField label="Роль второго" name="toRole" defaultValue="employee" required/>
        <SelectField label="Направление" name="direction" defaultValue="1"><option value="1">Направленная</option><option value="2">Взаимная</option></SelectField></>}
      {selected&&<SelectField label="Статус" name="status" defaultValue={selected.status}><option value="1">Активна</option><option value="2">Неактивна</option></SelectField>}
      <TimeFields name="from" label="Действует с" editing={!!selected}/><TimeFields name="until" label="Действует до" editing={!!selected}/>
    </FormGrid><Button type="submit" disabled={mutation.busy}>Сохранить связь</Button>{" "}
    {selected&&<Button variant="secondary" onClick={()=>{setEditorOpen(false);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
    </form>{mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}</RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

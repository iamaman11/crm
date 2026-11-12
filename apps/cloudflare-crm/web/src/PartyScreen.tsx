import {useCallback,useState} from "react";
import {Button,Card,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Party} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";

export function PartyScreen() {
  const [cursor,setCursor]=useState("");
  const [kind,setKind]=useState("");
  const [selected,setSelected]=useState<Party>();
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.partyRecords.listParties({
    page:{pageSize:25,pageToken:cursor},...(kind?{kind:Number(kind)}:{})
  }),[cursor,kind]));
  const edit=async(id:string)=>{
    try {const result=await api.partyRecords.getParty({partyId:id});setSelected(result.party);mutation.changed();}
    catch(error){await mutation.run(()=>Promise.reject(error));}
  };
  return <>
    <PageHeader title="Люди и организации" description="Исходные участники клиентских отношений."/>
    <SelectField label="Тип участника" value={kind} onChange={e=>{setKind(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value="1">Человек</option><option value="2">Организация</option>
    </SelectField>
    <QueryNotice query={query}/>
    <div>{query.data?.parties.map(p=><Card key={p.partyId} title={p.displayName}>
      <p>{p.kind===1?"Человек":"Организация"} · версия {p.version.toString()}</p>
      <p><small>ID: {p.partyId}</small></p>
      <Button variant="secondary" onClick={()=>void edit(p.partyId)}>Редактировать</Button>
    </Card>)}</div>
    {!query.busy&&query.data?.parties.length===0&&<p>Записей пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
      onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <Card title={selected?"Изменить участника":"Новый участник"}>
      <form key={selected?selected.partyId+selected.version: draftId} onChange={mutation.changed} onSubmit={event=>{
        event.preventDefault();const data=new FormData(event.currentTarget);
        void mutation.run(()=>selected?api.partyRecords.updateParty({
          partyId:selected.partyId,displayName:field(data,"name"),
          precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        }):api.partyRecords.createParty({
          partyId:draftId,displayName:field(data,"name"),kind:Number(field(data,"kind")),
          precondition:{idempotencyKey:mutation.key}
        }),async()=>{setSelected(undefined);setDraftId(newId());await query.reload();});
      }}>
        <FormGrid>
          <TextField label="Имя или название" name="name" defaultValue={selected?.displayName??""} required/>
          {!selected&&<SelectField label="Тип" name="kind" defaultValue="2"><option value="1">Человек</option><option value="2">Организация</option></SelectField>}
        </FormGrid>
        <Button type="submit" disabled={mutation.busy}>Сохранить участника</Button>{" "}
        {selected&&<Button variant="secondary" onClick={()=>{setSelected(undefined);mutation.changed();}}>Отмена</Button>}
      </form><MutationNotice state={mutation}/>
    </Card>
  </>;
}

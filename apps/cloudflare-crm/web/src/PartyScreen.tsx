import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Party} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {PartyKind} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {SearchField} from "./SearchField";

export function PartyScreen() {
  const [cursor,setCursor]=useState("");
  const [kind,setKind]=useState("");
  const [search,setSearch]=useState("");
  const [selected,setSelected]=useState<Party>();
  const [editorOpen,setEditorOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.partyRecords.listParties({
    page:{pageSize:25,pageToken:cursor},search,...(kind?{kind:enumValue(kind,[PartyKind.PARTY_KIND_PERSON,PartyKind.PARTY_KIND_ORGANIZATION],"Тип участника")}:{})
  }),[cursor,kind,search]));
  const edit=async(id:string)=>{
    try {const result=await api.partyRecords.getParty({partyId:id});setSelected(result.party);setEditorOpen(true);mutation.changed();}
    catch(error){await mutation.run(()=>Promise.reject(error));}
  };
  return <>
    <PageHeader title="Люди и организации" description="Исходные участники клиентских отношений." actions={<Button onClick={()=>{setSelected(undefined);setEditorOpen(true);mutation.changed();}}>Создать участника</Button>}/>
    <SearchField label="Поиск участников" onSearch={value=>{setSearch(value);setCursor("");}}/>
    <SelectField label="Тип участника" value={kind} onChange={e=>{setKind(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value={PartyKind.PARTY_KIND_PERSON}>Человек</option><option value={PartyKind.PARTY_KIND_ORGANIZATION}>Организация</option>
    </SelectField>
    <QueryNotice query={query}/>
    <div>{!query.busy&&!query.error&&query.data?.parties.map(p=><Card key={p.partyId} title={p.displayName}>
      <p>{p.kind===PartyKind.PARTY_KIND_PERSON?"Человек":p.kind===PartyKind.PARTY_KIND_ORGANIZATION?"Организация":"Неизвестный тип"} · версия {p.version.toString()}</p>
      <p><small>ID: {p.partyId}</small></p>
      <Button variant="secondary" onClick={()=>void edit(p.partyId)}>Редактировать</Button>
    </Card>)}</div>
    {!query.busy&&!query.error&&query.data?.parties.length===0&&<p>{search?"Участники не найдены.":"Записей пока нет."}</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
      onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={editorOpen} title={selected?"Изменить участника":"Новый участник"} onClose={()=>{setEditorOpen(false);setSelected(undefined);}}>
      <form key={selected?selected.partyId+selected.version: draftId} onChange={mutation.changed} onSubmit={event=>{
        event.preventDefault();const data=new FormData(event.currentTarget);
        void mutation.run(()=>selected?api.partyRecords.updateParty({
          partyId:selected.partyId,displayName:field(data,"name"),
          precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        }):api.partyRecords.createParty({
          partyId:draftId,displayName:field(data,"name"),kind:enumValue(field(data,"kind"),[PartyKind.PARTY_KIND_PERSON,PartyKind.PARTY_KIND_ORGANIZATION],"Тип участника"),
          precondition:{idempotencyKey:mutation.key}
        }),async()=>{setEditorOpen(false);setSelected(undefined);setDraftId(newId());await query.reload();});
      }}>
        <FormGrid>
          <TextField label="Имя или название" name="name" defaultValue={selected?.displayName??""} required/>
          {!selected&&<SelectField label="Тип" name="kind" defaultValue="2"><option value={PartyKind.PARTY_KIND_PERSON}>Человек</option><option value={PartyKind.PARTY_KIND_ORGANIZATION}>Организация</option></SelectField>}
        </FormGrid>
        <Button type="submit" disabled={mutation.busy}>Сохранить участника</Button>{" "}
        {selected&&<Button variant="secondary" onClick={()=>{setEditorOpen(false);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
      </form>
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

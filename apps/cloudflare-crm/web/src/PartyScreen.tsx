import {useCallback,useEffect,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Party} from "@ultimate-crm/client/cloudflare";
import {api,field,newId} from "./api";
import {PartyKind} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {SearchField} from "./SearchField";
import {openAccountFromParty,savedPartyListScroll} from "./navigation";
import {reviewHref} from "./routes";

export function PartyScreen({initialPartyId=""}:{initialPartyId?:string}) {
  const [cursor,setCursor]=useState("");
  const [kind,setKind]=useState("");
  const [search,setSearch]=useState("");
  const [selected,setSelected]=useState<Party>();
  const [editorOpen,setEditorOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const query=useQuery(useCallback(async(signal:AbortSignal)=>initialPartyId?undefined:api.partyRecords.listParties({
    page:{pageSize:25,pageToken:cursor},search,...(kind?{kind:enumValue(kind,[PartyKind.PERSON,PartyKind.ORGANIZATION],"Тип участника")}:{})
  },{signal}),[initialPartyId,cursor,kind,search]));
  // A Party permalink is an owner GetParty read, NOT an ID passed to the text search.
  const focused=useQuery(useCallback(async(signal:AbortSignal)=>initialPartyId?
    (await api.partyRecords.getParty({partyId:initialPartyId},{signal})).party:undefined,[initialPartyId]));
  const mutation=useMutation({
    storageKey:"crm.party.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.partyRecords.getParty({
      partyId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async()=>{await query.reload();await focused.reload();}
  });
  useEffect(()=>{
    if(!initialPartyId||focused.busy||!focused.data)return;
    const scroll=savedPartyListScroll();
    if(scroll!==null)requestAnimationFrame(()=>window.scrollTo(0,scroll));
  },[initialPartyId,focused.busy,focused.data]);
  const edit=async(id:string)=>{
    try {const result=await api.partyRecords.getParty({partyId:id});setSelected(result.party);setEditorOpen(true);mutation.changed();}
    catch(error){await mutation.reportError(error);}
  };
  return <>
    <PageHeader title="Люди и организации" description="Исходные участники клиентских отношений." actions={<Button disabled={mutation.busy||mutation.pending} onClick={()=>{setSelected(undefined);setEditorOpen(true);mutation.changed();}}>Создать участника</Button>}/>
    {initialPartyId?<><a href="#parties">К списку участников</a>{" · "}<a href={reviewHref(initialPartyId)}>Повторно проверить качество</a></>:<>
    <SearchField label="Поиск участников" submitted={search} onSearch={value=>{setSearch(value);setCursor("");}}/>
    <SelectField label="Тип участника" value={kind} onChange={e=>{setKind(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value={PartyKind.PERSON}>Человек</option><option value={PartyKind.ORGANIZATION}>Организация</option>
    </SelectField></>}
    <QueryNotice query={initialPartyId?focused:query}/>
    <div>{(initialPartyId?(focused.data?[focused.data]:[]):query.data?.parties??[]).map(p=><Card key={p.partyId} title={p.displayName}>
      <p>{p.kind===PartyKind.PERSON?"Человек":p.kind===PartyKind.ORGANIZATION?"Организация":"Неизвестный тип"} · версия {p.version.toString()}</p>
      <p><small>ID: {p.partyId}</small></p>
      <Button variant="secondary" disabled={mutation.busy||mutation.pending} onClick={()=>void edit(p.partyId)}>Редактировать</Button>{" "}
      <Button variant="secondary" onClick={()=>openAccountFromParty(p.partyId)}>Создать клиента</Button>
    </Card>)}</div>
    {!initialPartyId&&!query.busy&&!query.error&&query.data?.parties.length===0&&<p>{search?"Участники не найдены.":"Записей пока нет."}</p>}
    {initialPartyId&&!focused.busy&&!focused.error&&!focused.data&&<p>Участник не найден.</p>}
    {!initialPartyId&&<Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy}
      onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/> }
    <RecordPanel open={editorOpen} title={selected?"Изменить участника":"Новый участник"} onClose={()=>{if(mutation.busy||mutation.pending)return;setEditorOpen(false);setSelected(undefined);}}>
      <form key={selected?selected.partyId+selected.version: draftId} onChange={mutation.changed} onSubmit={event=>{
        event.preventDefault();const data=new FormData(event.currentTarget);
        void mutation.runIntent(()=>selected?prepareMutation(api.partyRecords.updateParty,{
          partyId:selected.partyId,displayName:field(data,"name"),
          precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        },selected.partyId):prepareMutation(api.partyRecords.createParty,{
          partyId:draftId,displayName:field(data,"name"),
          kind:enumValue(field(data,"kind"),[PartyKind.PERSON,PartyKind.ORGANIZATION],"Тип участника"),
          precondition:{idempotencyKey:mutation.key}
        },draftId),async()=>{setEditorOpen(false);setSelected(undefined);setDraftId(newId());await query.reload();await focused.reload();});
      }}>
        <FormGrid>
          <TextField label="Имя или название" name="name" defaultValue={selected?.displayName??""} required/>
          {!selected&&<SelectField label="Тип" name="kind" defaultValue="2"><option value={PartyKind.PERSON}>Человек</option><option value={PartyKind.ORGANIZATION}>Организация</option></SelectField>}
        </FormGrid>
        <Button type="submit" disabled={mutation.busy||mutation.pending}>Сохранить участника</Button>{" "}
        {selected&&<Button type="button" variant="secondary" disabled={mutation.busy||mutation.pending} onClick={()=>{setEditorOpen(false);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
      </form>
      <MutationNotice state={mutation}/>
    </RecordPanel>
    {!editorOpen&&<MutationNotice state={mutation}/>}
  </>;
}

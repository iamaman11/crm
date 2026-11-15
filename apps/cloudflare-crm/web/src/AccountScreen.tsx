import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {Account,AccountOverview} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,dateText} from "./api";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {SearchField} from "./SearchField";

function Overview({id}:{id:string}) {
  const query=useQuery(useCallback(()=>api.overview.getAccountOverview({accountId:id}),[id]));
  const card:AccountOverview|undefined=query.data?.overview;
  return <div><QueryNotice query={query}/>{card&&<>
    <h3>{card.account?.name}</h3><p>Основной участник: {card.primaryParty?.displayName}</p>
    <h3>Контакты</h3><ul>{card.contactPoints.map(p=><li key={p.contactPointId}>{p.displayValue}{p.verified?" · подтверждён":""}</li>)}</ul>
    {card.truncated?.contactPoints&&<p>Показаны первые {card.perCollectionLimit} контактов.</p>}
    {card.primaryParty&&<a href={"#contacts?partyId="+encodeURIComponent(card.primaryParty.partyId)}>Все контакты участника</a>}
    <h3>Связи</h3><ul>{card.relationships.map(r=><li key={r.relationshipId}>{r.type?.code}: {r.fromPartyId} → {r.toPartyId}</li>)}</ul>
    {card.truncated?.relationships&&<p>Показаны первые {card.perCollectionLimit} связей.</p>}
    {card.primaryParty&&<a href={"#relationships?partyId="+encodeURIComponent(card.primaryParty.partyId)}>Все связи участника</a>}
    <h3>Сделки</h3><ul>{card.deals.map(d=><li key={d.dealId}>{d.name} · {d.stage?.stageId}</li>)}</ul>
    {card.truncated?.deals&&<p>Показаны первые {card.perCollectionLimit} сделок.</p>}
    <a href={"#sales?accountId="+encodeURIComponent(id)}>Все сделки клиента</a>
    <h3>Задачи</h3><ul>{card.tasks.map(t=><li key={t.taskId}>{t.subject} · {t.status===2?"Выполнена":dateText(t.dueAt)}</li>)}</ul>
    {card.truncated?.tasks&&<p>Показаны первые {card.perCollectionLimit} задач.</p>}
    <a href={"#tasks?accountId="+encodeURIComponent(id)}>Все задачи клиента</a>
  </>}</div>;
}
export function AccountScreen() {
  const [cursor,setCursor]=useState("");
  const [status,setStatus]=useState("");
  const [search,setSearch]=useState("");
  const [selected,setSelected]=useState<Account>();
  const [editorOpen,setEditorOpen]=useState(false);
  const [overview,setOverview]=useState("");
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.accounts.listAccounts({
    page:{pageSize:25,pageToken:cursor},search,...(status?{status:Number(status)}:{})
  }),[cursor,status,search]));
  const edit=async(accountId:string)=>{
    try {setSelected((await api.accounts.getAccount({accountId})).account);setOverview("");setEditorOpen(true);mutation.changed();}
    catch(error){await mutation.run(()=>Promise.reject(error));}
  };
  return <>
    <PageHeader title="Клиенты" description="Клиент связывается с человеком или организацией из раздела участников." actions={<Button onClick={()=>{setSelected(undefined);setOverview("");setEditorOpen(true);mutation.changed();}}>Создать клиента</Button>}/>
    <SearchField label="Поиск клиентов" onSearch={value=>{setSearch(value);setCursor("");}}/>
    <SelectField label="Статус клиента" value={status} onChange={e=>{setStatus(e.target.value);setCursor("");}}>
      <option value="">Все</option><option value="1">Активные</option><option value="2">Неактивные</option>
    </SelectField><QueryNotice query={query}/>
    {!query.busy&&!query.error&&query.data?.accounts.map(a=><Card key={a.accountId} title={a.name}>
      <p>{a.status===1?"Активен":"Неактивен"} · версия {a.version.toString()}</p><p><small>ID: {a.accountId}</small></p>
      <Button variant="secondary" onClick={()=>void edit(a.accountId)}>Редактировать</Button>{" "}
      <Button variant="secondary" onClick={()=>{setEditorOpen(false);setSelected(undefined);setOverview(a.accountId);}}>Открыть карточку</Button>
    </Card>)}
    {!query.busy&&!query.error&&query.data?.accounts.length===0&&<p>{search?"Клиенты не найдены.":"Клиентов пока нет."}</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")}
      onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={!!overview} title="Карточка клиента" onClose={()=>setOverview("")}>{overview&&<Overview id={overview}/>}</RecordPanel>
    <RecordPanel open={editorOpen} title={selected?"Изменить клиента":"Новый клиент"} onClose={()=>{setEditorOpen(false);setSelected(undefined);}}>
      <form key={selected?selected.accountId+selected.version:draftId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>selected?api.accounts.updateAccount({
          accountId:selected.accountId,name:field(data,"name"),status:Number(field(data,"status")),
          precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        }):api.accounts.createAccount({accountId:draftId,name:field(data,"name"),primaryPartyId:field(data,"partyId"),
          precondition:{idempotencyKey:mutation.key}}),
          async()=>{setEditorOpen(false);setSelected(undefined);setDraftId(newId());await query.reload();});
      }}>
        <FormGrid><TextField label="Название клиента" name="name" defaultValue={selected?.name??""} required/>
          {selected?<SelectField label="Статус" name="status" defaultValue={selected.status}><option value="1">Активен</option><option value="2">Неактивен</option></SelectField>:<PartyPicker label="Основной участник"/>}
        </FormGrid><Button type="submit" disabled={mutation.busy}>Сохранить клиента</Button>{" "}
        {selected&&<Button variant="secondary" onClick={()=>{setEditorOpen(false);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
      </form>
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

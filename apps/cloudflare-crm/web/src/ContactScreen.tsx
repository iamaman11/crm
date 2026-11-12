import {useCallback,useState} from "react";
import {Button,Card,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {ContactPoint} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {TimeFields} from "./TimeFields";
import {timePatch} from "./timePatch";

export function ContactScreen() {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState("");
  const [selected,setSelected]=useState<ContactPoint>();
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.contactPoints.listContactPoints({page:{pageSize:25,pageToken:cursor},
    ...(partyId?{partyId}:{})}),[cursor,partyId]));
  return <>
    <PageHeader title="Контакты" description="Email, телефоны и другие способы связи с участниками."/>
    <TextField label="Фильтр по ID участника" value={partyId} onChange={e=>{setPartyId(e.target.value);setCursor("");}}/>
    <QueryNotice query={query}/>
    {query.data?.contactPoints.map(p=><Card key={p.contactPointId} title={p.displayValue}>
      <p>Участник: {p.partyId} · {p.status===1?"Активен":"Неактивен"}{p.preferred?" · предпочтительный":""}</p>
      <p>{p.verified?"Подтверждён доверенными данными":"Не подтверждён"} · {p.normalizedValue}</p>
      <p>С {dateText(p.validFrom)} до {dateText(p.validUntil)}</p>
      <Button variant="secondary" onClick={async()=>{
        try {setSelected((await api.contactPoints.getContactPoint({contactPointId:p.contactPointId})).contactPoint);mutation.changed();}
        catch(error){await mutation.run(()=>Promise.reject(error));}
      }}>Редактировать контакт</Button>
    </Card>)}
    {!query.busy&&query.data?.contactPoints.length===0&&<p>Контактов пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <Card title={selected?"Изменить контакт":"Новый контакт"}><form key={selected?selected.contactPointId+selected.version:draftId}
      onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const common={value:field(data,"value"),preferred:data.get("preferred")==="on"};
          if(selected){
            const validFrom=timePatch(data,"from"),validUntil=timePatch(data,"until");
            return api.contactPoints.updateContactPoint({contactPointId:selected.contactPointId,...common,
              status:Number(field(data,"status")),...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}});
          }
          const validFrom=timestamp(field(data,"from")),validUntil=timestamp(field(data,"until"));
          return api.contactPoints.createContactPoint({contactPointId:draftId,partyId:field(data,"partyId"),
            kind:Number(field(data,"kind")),...common,...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
            precondition:{idempotencyKey:mutation.key}});
        },async()=>{setSelected(undefined);setDraftId(newId());await query.reload();});
      }}><FormGrid>
        {!selected&&<><PartyPicker/><SelectField label="Тип контакта" name="kind" defaultValue="1">
          <option value="1">Email</option><option value="2">Телефон</option><option value="3">Почтовый адрес</option><option value="4">Сайт</option><option value="5">Мессенджер</option>
        </SelectField></>}
        <TextField label="Значение контакта" name="value" defaultValue={selected?.displayValue??""} required/>
        <TextField label="Предпочтительный" name="preferred" type="checkbox" defaultChecked={selected?.preferred??false}/>
        {selected&&<SelectField label="Статус" name="status" defaultValue={selected.status}><option value="1">Активен</option><option value="2">Неактивен</option></SelectField>}
        <TimeFields name="from" label="Действует с" editing={!!selected}/><TimeFields name="until" label="Действует до" editing={!!selected}/>
      </FormGrid><Button type="submit" disabled={mutation.busy}>Сохранить контакт</Button>{" "}
      {selected&&<Button variant="secondary" onClick={()=>{setSelected(undefined);mutation.changed();}}>Отмена</Button>}
    </form><MutationNotice state={mutation}/></Card>
  </>;
}

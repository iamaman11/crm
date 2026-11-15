import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import type {ContactPoint} from "@ultimate-crm/client/cloudflare";
import {api,field,newId,timestamp,dateText} from "./api";
import {ContactPointKind,ContactPointStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {TimeFields} from "./TimeFields";
import {timePatch} from "./timePatch";

export function ContactScreen({initialPartyId=""}:{initialPartyId?:string}) {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState(initialPartyId);
  const [selected,setSelected]=useState<ContactPoint>();
  const [editorOpen,setEditorOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.contactPoints.listContactPoints({page:{pageSize:25,pageToken:cursor},
    ...(partyId?{partyId}:{})}),[cursor,partyId]));
  return <>
    <PageHeader title="Контакты" description="Email, телефоны и другие способы связи. Email подтверждает владелец через Access или тестовый администратор в dev." actions={<Button onClick={()=>{setSelected(undefined);setEditorOpen(true);mutation.changed();}}>Создать контакт</Button>}/>
    <TextField label="Фильтр по ID участника" value={partyId} onChange={e=>{setPartyId(e.target.value);setCursor("");}}/>
    <QueryNotice query={query}/>
    {query.data?.contactPoints.map(p=><Card key={p.contactPointId} title={p.displayValue}>
      <p>Участник: {p.partyId} · {p.status===ContactPointStatus.ACTIVE?"Активен":p.status===ContactPointStatus.INACTIVE?"Неактивен":"Неизвестный статус"}{p.preferred?" · предпочтительный":""}</p>
      <p>{p.verified?"Подтверждён доверенными данными":"Не подтверждён"} · {p.normalizedValue}</p>
      <p>ID контакта: {p.contactPointId}</p>
      {p.kind===ContactPointKind.EMAIL&&!p.verified&&<Button variant="secondary" disabled={mutation.busy}
        onClick={()=>void mutation.run(()=>api.contactPoints.verifyOwnEmailContactPoint({
          contactPointId:p.contactPointId,
          precondition:{idempotencyKey:mutation.key,expectedVersion:p.version}
        }),query.reload)}>Подтвердить email (Access / тестовый администратор)</Button>}{" "}
      <p>С {dateText(p.validFrom)} до {dateText(p.validUntil)}</p>
      <Button variant="secondary" onClick={async()=>{
        try {setSelected((await api.contactPoints.getContactPoint({contactPointId:p.contactPointId})).contactPoint);setEditorOpen(true);mutation.changed();}
        catch(error){await mutation.run(()=>Promise.reject(error));}
      }}>Редактировать контакт</Button>
    </Card>)}
    {!query.busy&&query.data?.contactPoints.length===0&&<p>Контактов пока нет.</p>}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={editorOpen} title={selected?"Изменить контакт":"Новый контакт"} onClose={()=>{setEditorOpen(false);setSelected(undefined);}}><form key={selected?selected.contactPointId+selected.version:draftId}
      onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>{
          const common={value:field(data,"value"),preferred:data.get("preferred")==="on"};
          if(selected){
            const validFrom=timePatch(data,"from"),validUntil=timePatch(data,"until");
            return api.contactPoints.updateContactPoint({contactPointId:selected.contactPointId,...common,
              status:enumValue(field(data,"status"),[ContactPointStatus.ACTIVE,ContactPointStatus.INACTIVE],"Статус контакта"),...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
              precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}});
          }
          const validFrom=timestamp(field(data,"from")),validUntil=timestamp(field(data,"until"));
          return api.contactPoints.createContactPoint({contactPointId:draftId,partyId:field(data,"partyId"),
            kind:enumValue(field(data,"kind"),[ContactPointKind.EMAIL,ContactPointKind.PHONE,ContactPointKind.POSTAL,ContactPointKind.WEB,ContactPointKind.MESSAGING],"Тип контакта"),...common,...(validFrom?{validFrom}:{}),...(validUntil?{validUntil}:{}),
            precondition:{idempotencyKey:mutation.key}});
        },async()=>{setEditorOpen(false);setSelected(undefined);setDraftId(newId());await query.reload();});
      }}><FormGrid>
        {!selected&&<><PartyPicker/><SelectField label="Тип контакта" name="kind" defaultValue="1">
          <option value={ContactPointKind.EMAIL}>Email</option><option value={ContactPointKind.PHONE}>Телефон</option><option value={ContactPointKind.POSTAL}>Почтовый адрес</option><option value={ContactPointKind.WEB}>Сайт</option><option value={ContactPointKind.MESSAGING}>Мессенджер</option>
        </SelectField></>}
        <TextField label="Значение контакта" name="value" defaultValue={selected?.displayValue??""} required/>
        <TextField label="Предпочтительный" name="preferred" type="checkbox" defaultChecked={selected?.preferred??false}/>
        {selected&&<SelectField label="Статус" name="status" defaultValue={selected.status}><option value={ContactPointStatus.ACTIVE}>Активен</option><option value={ContactPointStatus.INACTIVE}>Неактивен</option></SelectField>}
        <TimeFields name="from" label="Действует с" editing={!!selected}/><TimeFields name="until" label="Действует до" editing={!!selected}/>
      </FormGrid><Button type="submit" disabled={mutation.busy}>Сохранить контакт</Button>{" "}
      {selected&&<Button variant="secondary" onClick={()=>{setEditorOpen(false);setSelected(undefined);mutation.changed();}}>Отмена</Button>}
    </form>{mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}</RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

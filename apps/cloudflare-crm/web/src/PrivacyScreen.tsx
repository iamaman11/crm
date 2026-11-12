import {useCallback,useState} from "react";
import {Button,Card,FormGrid,PageHeader,SelectField} from "@ultimate-crm/ui";
import type {PrivacyCase} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
const kinds=["Не указан","Доступ к данным","Переносимость данных","Ограничение обработки","Удаление данных"];
const statuses=["Не указан","Черновик","Подано","Личность подтверждена","Определение состава","Состав определён","Запланировано","Ожидание согласования","Исполнение","Согласование результатов","Пересмотр состава","Повторяемая ошибка","Завершено","Частично завершено","Отказано","Отменено","Окончательная ошибка"];
export function PrivacyScreen() {
  const [cursor,setCursor]=useState("");
  const [selected,setSelected]=useState<PrivacyCase>();
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.privacy.listPrivacyCases({page:{pageSize:25,pageToken:cursor}}),[cursor]));
  return <>
    <PageHeader title="Запросы о данных" description="Создание, подача и отмена обращений. Проверка личности и исполнение запросов пока недоступны."/>
    <QueryNotice query={query}/>
    {query.data?.cases.map(c=><Card key={c.caseId} title={kinds[c.kind]??"Запрос"}>
      <p>{statuses[c.status]??"Неизвестный статус"} · версия {c.version.toString()}</p>
      <p><small>ID: {c.caseId}</small></p><Button variant="secondary" onClick={async()=>{
        try {setSelected((await api.privacy.getPrivacyCase({caseId:c.caseId})).privacyCase);mutation.changed();}
        catch(error){await mutation.run(()=>Promise.reject(error));}
      }}>Открыть запрос</Button>
    </Card>)}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    {selected&&<Card title={"Обращение "+selected.caseId}>
      <p>{statuses[selected.status]} · {selected.subjectVerified?"Личность подтверждена":"Личность не подтверждена"}</p>
      {selected.status===1&&<Button disabled={mutation.busy} onClick={()=>void mutation.run(()=>api.privacy.submitPrivacyCase({
        caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
      }),async()=>{setSelected(undefined);await query.reload();})}>Подать запрос</Button>}{" "}
      {[1,2].includes(selected.status)&&<Button variant="danger" disabled={mutation.busy} onClick={()=>void mutation.run(()=>api.privacy.cancelPrivacyCase({
        caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
      }),async()=>{setSelected(undefined);await query.reload();})}>Отменить запрос</Button>}
    </Card>}
    <Card title="Новое обращение"><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>api.privacy.createPrivacyCase({caseId:draftId,kind:Number(data.get("kind")),
        precondition:{idempotencyKey:mutation.key}}),async()=>{setDraftId(newId());await query.reload();});
    }}><FormGrid><SelectField label="Вид обращения" name="kind" defaultValue="1">
      {kinds.slice(1).map((k,i)=><option key={k} value={i+1}>{k}</option>)}
    </SelectField></FormGrid><Button type="submit" disabled={mutation.busy}>Создать обращение</Button></form></Card>
    <MutationNotice state={mutation}/>
  </>;
}

import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField} from "@ultimate-crm/ui";
import type {PrivacyCase} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {PrivacyCaseKind,PrivacyCaseStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
const kinds:Partial<Record<PrivacyCaseKind,string>>={
  [PrivacyCaseKind.PRIVACY_CASE_KIND_UNSPECIFIED]:"Не указан",
  [PrivacyCaseKind.PRIVACY_CASE_KIND_ACCESS]:"Доступ к данным",
  [PrivacyCaseKind.PRIVACY_CASE_KIND_PORTABILITY_EXPORT]:"Переносимость данных",
  [PrivacyCaseKind.PRIVACY_CASE_KIND_RESTRICT_PROCESSING]:"Ограничение обработки",
  [PrivacyCaseKind.PRIVACY_CASE_KIND_ERASURE]:"Удаление данных",
};
const statuses:Partial<Record<PrivacyCaseStatus,string>>={
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_UNSPECIFIED]:"Не указан",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_DRAFT]:"Черновик",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_SUBMITTED]:"Подано",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_SUBJECT_VERIFIED]:"Личность подтверждена",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_SCOPING]:"Определение состава",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_SCOPED]:"Состав определён",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_PLANNED]:"Запланировано",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_AWAITING_APPROVAL]:"Ожидание согласования",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_EXECUTING]:"Исполнение",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_CONVERGING]:"Согласование результатов",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_RESCOPE_REQUIRED]:"Пересмотр состава",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_FAILED_RETRYABLE]:"Повторяемая ошибка",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_COMPLETED]:"Завершено",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_PARTIALLY_COMPLETED]:"Частично завершено",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_DENIED]:"Отказано",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_CANCELLED]:"Отменено",
  [PrivacyCaseStatus.PRIVACY_CASE_STATUS_FAILED_TERMINAL]:"Окончательная ошибка",
};
export function PrivacyScreen() {
  const [cursor,setCursor]=useState("");
  const [selected,setSelected]=useState<PrivacyCase>();
  const [createOpen,setCreateOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const query=useQuery(useCallback(()=>api.privacy.listPrivacyCases({page:{pageSize:25,pageToken:cursor}}),[cursor]));
  return <>
    <PageHeader title="Запросы о данных" description="Создание, подача и отмена обращений. Проверка личности и исполнение запросов пока недоступны." actions={<Button onClick={()=>{setSelected(undefined);setCreateOpen(true);mutation.changed();}}>Новое обращение</Button>}/>
    <QueryNotice query={query}/>
    {query.data?.cases.map(c=><Card key={c.caseId} title={kinds[c.kind]??"Запрос"}>
      <p>{statuses[c.status]??"Неизвестный статус"} · версия {c.version.toString()}</p>
      <p><small>ID: {c.caseId}</small></p><Button variant="secondary" onClick={async()=>{
        try {setSelected((await api.privacy.getPrivacyCase({caseId:c.caseId})).privacyCase);mutation.changed();}
        catch(error){await mutation.run(()=>Promise.reject(error));}
      }}>Открыть запрос</Button>
    </Card>)}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={!!selected} title={selected?"Обращение "+selected.caseId:"Обращение"} onClose={()=>setSelected(undefined)}>{selected&&<>
      <p>{statuses[selected.status]??"Неизвестный статус"} · {selected.subjectVerified?"Личность подтверждена":"Личность не подтверждена"}</p>
      {selected.status===PrivacyCaseStatus.PRIVACY_CASE_STATUS_DRAFT&&<Button disabled={mutation.busy} onClick={()=>void mutation.run(()=>api.privacy.submitPrivacyCase({
        caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
      }),async()=>{setSelected(undefined);await query.reload();})}>Подать запрос</Button>}{" "}
      {[PrivacyCaseStatus.PRIVACY_CASE_STATUS_DRAFT,PrivacyCaseStatus.PRIVACY_CASE_STATUS_SUBMITTED].includes(selected.status)&&<Button variant="danger" disabled={mutation.busy} onClick={()=>void mutation.run(()=>api.privacy.cancelPrivacyCase({
        caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
      }),async()=>{setSelected(undefined);await query.reload();})}>Отменить запрос</Button>}
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </>}</RecordPanel>
    <RecordPanel open={createOpen} title="Новое обращение" onClose={()=>setCreateOpen(false)}><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>api.privacy.createPrivacyCase({caseId:draftId,kind:enumValue(String(data.get("kind")??""),[PrivacyCaseKind.PRIVACY_CASE_KIND_ACCESS,PrivacyCaseKind.PRIVACY_CASE_KIND_PORTABILITY_EXPORT,PrivacyCaseKind.PRIVACY_CASE_KIND_RESTRICT_PROCESSING,PrivacyCaseKind.PRIVACY_CASE_KIND_ERASURE],"Вид обращения"),
        precondition:{idempotencyKey:mutation.key}}),async()=>{setCreateOpen(false);setDraftId(newId());await query.reload();});
    }}><FormGrid><SelectField label="Вид обращения" name="kind" defaultValue="1">
      {[PrivacyCaseKind.PRIVACY_CASE_KIND_ACCESS,PrivacyCaseKind.PRIVACY_CASE_KIND_PORTABILITY_EXPORT,PrivacyCaseKind.PRIVACY_CASE_KIND_RESTRICT_PROCESSING,PrivacyCaseKind.PRIVACY_CASE_KIND_ERASURE].map(kind=><option key={kind} value={kind}>{kinds[kind]}</option>)}
    </SelectField></FormGrid><Button type="submit" disabled={mutation.busy}>Создать обращение</Button></form>{mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}</RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

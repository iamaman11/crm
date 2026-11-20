import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField} from "@ultimate-crm/ui";
import type {PrivacyCase} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {PrivacyCaseKind,PrivacyCaseStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
const kinds:Partial<Record<PrivacyCaseKind,string>>={
  [PrivacyCaseKind.UNSPECIFIED]:"Не указан",
  [PrivacyCaseKind.ACCESS]:"Доступ к данным",
  [PrivacyCaseKind.PORTABILITY_EXPORT]:"Переносимость данных",
  [PrivacyCaseKind.RESTRICT_PROCESSING]:"Ограничение обработки",
  [PrivacyCaseKind.ERASURE]:"Удаление данных",
};
const statuses:Partial<Record<PrivacyCaseStatus,string>>={
  [PrivacyCaseStatus.UNSPECIFIED]:"Не указан",
  [PrivacyCaseStatus.DRAFT]:"Черновик",
  [PrivacyCaseStatus.SUBMITTED]:"Подано",
  [PrivacyCaseStatus.SUBJECT_VERIFIED]:"Личность подтверждена",
  [PrivacyCaseStatus.SCOPING]:"Определение состава",
  [PrivacyCaseStatus.SCOPED]:"Состав определён",
  [PrivacyCaseStatus.PLANNED]:"Запланировано",
  [PrivacyCaseStatus.AWAITING_APPROVAL]:"Ожидание согласования",
  [PrivacyCaseStatus.EXECUTING]:"Исполнение",
  [PrivacyCaseStatus.CONVERGING]:"Согласование результатов",
  [PrivacyCaseStatus.RESCOPE_REQUIRED]:"Пересмотр состава",
  [PrivacyCaseStatus.FAILED_RETRYABLE]:"Повторяемая ошибка",
  [PrivacyCaseStatus.COMPLETED]:"Завершено",
  [PrivacyCaseStatus.PARTIALLY_COMPLETED]:"Частично завершено",
  [PrivacyCaseStatus.DENIED]:"Отказано",
  [PrivacyCaseStatus.CANCELLED]:"Отменено",
  [PrivacyCaseStatus.FAILED_TERMINAL]:"Окончательная ошибка",
};
export function PrivacyScreen() {
  const [cursor,setCursor]=useState("");
  const [selected,setSelected]=useState<PrivacyCase>();
  const [createOpen,setCreateOpen]=useState(false);
  const [draftId,setDraftId]=useState(newId);
  const query=useQuery(useCallback((signal:AbortSignal)=>api.privacy.listPrivacyCases({page:{pageSize:25,pageToken:cursor}},{signal}),[cursor]));
  const mutation=useMutation({
    storageKey:"crm.privacy.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.privacy.getPrivacyCase({
      caseId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:query.reload
  });
  return <>
    <PageHeader title="Запросы о данных" description="Создание, подача и отмена обращений. Проверка личности и исполнение запросов пока недоступны." actions={<Button disabled={mutation.busy||mutation.pending} onClick={()=>{setSelected(undefined);setCreateOpen(true);mutation.changed();}}>Новое обращение</Button>}/>
    <QueryNotice query={query}/>
    {query.data?.cases.map(c=><Card key={c.caseId} title={kinds[c.kind]??"Запрос"}>
      <p>{statuses[c.status]??"Неизвестный статус"} · версия {c.version.toString()}</p>
      <p><small>ID: {c.caseId}</small></p><Button variant="secondary" disabled={mutation.busy||mutation.pending} onClick={async()=>{
        try {setSelected((await api.privacy.getPrivacyCase({caseId:c.caseId})).privacyCase);mutation.changed();}
        catch(error){await mutation.reportError(error);}
      }}>Открыть запрос</Button>
    </Card>)}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={!!selected} title={selected?"Обращение "+selected.caseId:"Обращение"} onClose={()=>{if(!mutation.busy&&!mutation.pending)setSelected(undefined);}}>{selected&&<>
      <p>{statuses[selected.status]??"Неизвестный статус"} · {selected.subjectVerified?"Личность подтверждена":"Личность не подтверждена"}</p>
      {selected.status===PrivacyCaseStatus.DRAFT&&<Button disabled={mutation.busy||mutation.pending} onClick={()=>void mutation.runIntent(
        ()=>prepareMutation(api.privacy.submitPrivacyCase,{
          caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        },selected.caseId),async()=>{setSelected(undefined);await query.reload();}
      )}>Подать запрос</Button>}{" "}
      {[PrivacyCaseStatus.DRAFT,PrivacyCaseStatus.SUBMITTED].includes(selected.status)&&<Button variant="danger" disabled={mutation.busy||mutation.pending} onClick={()=>void mutation.runIntent(
        ()=>prepareMutation(api.privacy.cancelPrivacyCase,{
          caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        },selected.caseId),async()=>{setSelected(undefined);await query.reload();}
      )}>Отменить запрос</Button>}
      <MutationNotice state={mutation}/>
    </>}</RecordPanel>
    <RecordPanel open={createOpen} title="Новое обращение" onClose={()=>{if(!mutation.busy&&!mutation.pending)setCreateOpen(false);}}><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.runIntent(()=>prepareMutation(api.privacy.createPrivacyCase,{
        caseId:draftId,
        kind:enumValue(String(data.get("kind")??""),[PrivacyCaseKind.ACCESS,PrivacyCaseKind.PORTABILITY_EXPORT,PrivacyCaseKind.RESTRICT_PROCESSING,PrivacyCaseKind.ERASURE],"Вид обращения"),
        precondition:{idempotencyKey:mutation.key}
      },draftId),async()=>{setCreateOpen(false);setDraftId(newId());await query.reload();});
    }}><FormGrid><SelectField label="Вид обращения" name="kind" defaultValue="1">
      {[PrivacyCaseKind.ACCESS,PrivacyCaseKind.PORTABILITY_EXPORT,PrivacyCaseKind.RESTRICT_PROCESSING,PrivacyCaseKind.ERASURE].map(kind=><option key={kind} value={kind}>{kinds[kind]}</option>)}
    </SelectField></FormGrid><Button type="submit" disabled={mutation.busy||mutation.pending}>Создать обращение</Button></form><MutationNotice state={mutation}/></RecordPanel>
    {!createOpen&&!selected&&<MutationNotice state={mutation}/>}
  </>;
}

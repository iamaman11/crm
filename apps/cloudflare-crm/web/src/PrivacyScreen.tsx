import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {PartyPicker} from "./Pickers";
import type {PrivacyCase} from "@ultimate-crm/client/cloudflare";
import {api,newId} from "./api";
import {PrivacyCaseKind,PrivacyCaseStatus} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useMutation,useReadAction,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
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
  const scope=useReadAction<Awaited<ReturnType<typeof api.privacy.previewOwnPrivacyScope>>>();
  const mutation=useMutation({
    storageKey:"crm.privacy.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.privacy.getPrivacyCase({
      caseId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:query.reload
  });
  return <>
    <PageHeader title="Запросы о данных" description="Создание и подтверждение собственного обращения. Доступен неполный предварительный состав данных; экспорт и исполнение пока недоступны." actions={<Button disabled={mutation.busy||mutation.pending} onClick={()=>{setSelected(undefined);setCreateOpen(true);mutation.changed();}}>Новое обращение</Button>}/>
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
      {selected.status===PrivacyCaseStatus.SUBMITTED&&<form
        onChange={mutation.changed}
        onSubmit={e=>{
          e.preventDefault();
          const form=new FormData(e.currentTarget);
          void mutation.runIntent(()=>prepareMutation(api.privacy.verifyOwnPrivacyCaseSubject,{
            caseId:selected.caseId,
            partyId:String(form.get("partyId")??""),
            contactPointId:String(form.get("contactPointId")??""),
            precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
          },selected.caseId),async()=>{setSelected(undefined);await query.reload();});
        }}>
        <p>Подтверждение доступно только создателю обращения для собственного Person с активным email,
          уже проверенным по подписанной Cloudflare Access identity.</p>
        <FormGrid>
          <PartyPicker label="Мой участник (Person)" name="partyId"/>
          <TextField label="ID проверенного email ContactPoint" name="contactPointId" required/>
        </FormGrid>
        <Button type="submit" disabled={mutation.busy||mutation.pending}>Подтвердить свою личность</Button>
      </form>}
      {selected.status===PrivacyCaseStatus.SUBJECT_VERIFIED&&<>
        <Button variant="secondary" disabled={scope.busy} onClick={()=>void scope.run(
          signal=>api.privacy.previewOwnPrivacyScope({caseId:selected.caseId},{signal})
        )}>Посмотреть предварительный состав моих данных</Button>
        {scope.error&&<p role="alert">{scope.error}</p>}
        {scope.data?.caseId===selected.caseId&&scope.data.caseVersion===selected.version&&
          <Card title="Неполный предварительный состав">
            <p>Данные читаются из действующей CRM, но не сохраняются как окончательный Privacy scope.
              До охвата всех владельцев экспорт и исполнение запрещены.</p>
            <p>Проверено владельцев: {scope.data.owners.length} из {scope.data.owners.length+scope.data.missingOwnerModuleIds.length}.
              Данных: {scope.data.resources.length}.</p>
            <ul>{scope.data.resources.map(r=>
              <li key={r.ownerModuleId+":"+r.resourceId}>
                {r.resourceType}: {r.resourceId} · версия {r.resourceVersion.toString()}
              </li>)}</ul>
            <p>Ещё не охвачены: {scope.data.missingOwnerModuleIds.join(", ")}.</p>
          </Card>}
      </>}
      {selected.status===PrivacyCaseStatus.DRAFT&&<Button disabled={mutation.busy||mutation.pending} onClick={()=>void mutation.runIntent(
        ()=>prepareMutation(api.privacy.submitPrivacyCase,{
          caseId:selected.caseId,precondition:{idempotencyKey:mutation.key,expectedVersion:selected.version}
        },selected.caseId),async()=>{setSelected(undefined);await query.reload();}
      )}>Подать запрос</Button>}{" "}
      {[PrivacyCaseStatus.DRAFT,PrivacyCaseStatus.SUBMITTED,PrivacyCaseStatus.SUBJECT_VERIFIED].includes(selected.status)&&<Button variant="danger" disabled={mutation.busy||mutation.pending} onClick={()=>void mutation.runIntent(
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

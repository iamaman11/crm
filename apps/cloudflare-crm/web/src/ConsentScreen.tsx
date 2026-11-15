import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,RecordActions,FeedbackPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,newId} from "./api";
import {CommunicationChannel,ConsentEffect,ConsentStatus,EvaluationReason} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useReadAction,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
const reasonLabels:Partial<Record<EvaluationReason,string>>={
  [EvaluationReason.UNSPECIFIED]:"Не указано",
  [EvaluationReason.ACTIVE_GRANT]:"Действующее согласие",
  [EvaluationReason.ACTIVE_DENY]:"Действующий запрет",
  [EvaluationReason.WITHDRAWN]:"Согласие отозвано",
  [EvaluationReason.NO_APPLICABLE_GRANT]:"Подходящего согласия нет"
};
function ScopeFields() {
  return <><PartyPicker/><TextField label="ID контакта (необязательно)" name="contact"/>
    <TextField label="Цель" name="purpose" defaultValue="marketing" required/>
    <SelectField label="Канал" name="channel" defaultValue="1">
      <option value={CommunicationChannel.EMAIL}>Email</option><option value={CommunicationChannel.PHONE}>Телефон</option><option value={CommunicationChannel.SMS}>SMS</option>
      <option value={CommunicationChannel.POSTAL}>Почта</option><option value={CommunicationChannel.MESSAGING}>Мессенджер</option><option value={CommunicationChannel.PUSH}>Push</option>
    </SelectField></>;
}
function scope(data:FormData) {
  const contact=field(data,"contact");
  return {partyId:field(data,"partyId"),purpose:field(data,"purpose"),channel:enumValue(field(data,"channel"),[CommunicationChannel.EMAIL,CommunicationChannel.PHONE,CommunicationChannel.SMS,CommunicationChannel.POSTAL,CommunicationChannel.MESSAGING,CommunicationChannel.PUSH],"Канал"),
    ...(contact?{contactPointId:contact}:{})};
}
export function ConsentScreen() {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState("");
  const [draftId,setDraftId]=useState(newId);
  const [composer,setComposer]=useState<"grant"|"suppress"|null>(null);
  const mutation=useMutation();
  const evaluation=useReadAction<Awaited<ReturnType<typeof api.consents.evaluateConsent>>>();
  const query=useQuery(useCallback(()=>api.consents.listConsentAuthorizations({page:{pageSize:25,pageToken:cursor},
    ...(partyId?{partyId}:{})}),[cursor,partyId]));
  return <>
    <PageHeader title="Коммуникации" description="Проверка разрешений, отзыв и запрет. Подписанный Access-владелец выдаёт личное согласие; сервисный администратор dev может проводить отдельный тестовый сценарий." actions={<RecordActions><Button onClick={()=>{setComposer("grant");mutation.changed();}}>Выдать согласие</Button><Button variant="secondary" onClick={()=>{setComposer("suppress");mutation.changed();}}>Запретить коммуникацию</Button></RecordActions>}/>
    <Card title="Проверить возможность контакта"><form onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void evaluation.run(()=>api.consents.evaluateConsent(scope(data)));
    }}><FormGrid><ScopeFields/></FormGrid><Button type="submit" disabled={evaluation.busy}>Проверить разрешение</Button></form>
      {evaluation.error&&<FeedbackPanel tone="danger" title={evaluation.error}/>}
      {evaluation.data&&<FeedbackPanel tone={evaluation.data.allowed?"success":"warning"} title={evaluation.data.allowed?"Контакт разрешён":"Контакт не разрешён"}>
        {reasonLabels[evaluation.data.reason]??"Неизвестная причина"}
      </FeedbackPanel>}
    </Card>
    <TextField label="Фильтр по ID участника" value={partyId} onChange={e=>{setPartyId(e.target.value);setCursor("");}}/>
    <QueryNotice query={query}/>
    {query.data?.authorizations.map(a=><Card key={a.authorizationId} title={a.effect===ConsentEffect.DENY?"Запрет":a.effect===ConsentEffect.GRANT?"Согласие":"Неизвестный тип"}>
      <p>{a.partyId} · {a.purpose} · {a.jurisdiction}</p><p>{a.status===ConsentStatus.ACTIVE?"Действует":a.status===ConsentStatus.WITHDRAWN?"Отозвано":"Неизвестный статус"} · версия {a.version.toString()}</p>
      {a.effect===ConsentEffect.GRANT&&a.status===ConsentStatus.ACTIVE&&<Button variant="danger" disabled={mutation.busy}
        onClick={()=>void mutation.run(()=>api.consents.withdrawConsentAuthorization({
          authorizationId:a.authorizationId,
          precondition:{idempotencyKey:mutation.key,expectedVersion:a.version}
        }),query.reload)}>Отозвать согласие</Button>}
    </Card>)}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={composer==="grant"} title="Согласие для подтверждённого email" onClose={()=>setComposer(null)}>
      <p>Сначала подтвердите email в разделе «Контакты». Сервисный администратор проверяет только тестовые контакты и записывается в аудит как агент, не как человек.</p>
      <form key={"grant-"+draftId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.run(()=>api.consents.grantVerifiedEmailConsent({
          authorizationId:draftId,partyId:field(data,"partyId"),
          contactPointId:field(data,"verifiedContactId"),purpose:field(data,"grantPurpose"),
          jurisdiction:field(data,"grantJurisdiction"),
          precondition:{idempotencyKey:mutation.key}
        }),async()=>{setComposer(null);setDraftId(newId());await query.reload();});
      }}>
        <FormGrid>
          <PartyPicker/>
          <TextField label="ID подтверждённого email-контакта" name="verifiedContactId" required/>
          <TextField label="Цель согласия" name="grantPurpose" defaultValue="marketing" required/>
          <TextField label="Юрисдикция согласия" name="grantJurisdiction" defaultValue="EU" required/>
        </FormGrid>
        <Button type="submit" disabled={mutation.busy}>Выдать согласие на email-коммуникацию</Button>
      </form>
      {mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}
    </RecordPanel>
    <RecordPanel open={composer==="suppress"} title="Запретить коммуникацию" onClose={()=>setComposer(null)}><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>api.consents.suppressCommunication({authorizationId:draftId,...scope(data),
        jurisdiction:field(data,"jurisdiction"),precondition:{idempotencyKey:mutation.key}}),
        async()=>{setComposer(null);setDraftId(newId());await query.reload();});
    }}><FormGrid><ScopeFields/><TextField label="Юрисдикция" name="jurisdiction" defaultValue="EU" required/></FormGrid>
      <Button type="submit" disabled={mutation.busy} variant="danger">Запретить контакт</Button>
    </form>{mutation.notice?.tone==="danger"&&<MutationNotice state={mutation}/>}</RecordPanel>
    <MutationNotice state={mutation}/>
  </>;
}

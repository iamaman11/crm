import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,RecordActions,FeedbackPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,newId} from "./api";
import {CommunicationChannel,ConsentEffect,ConsentStatus,EvaluationReason} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useQuery,useReadAction,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker,PartyName,ContactPointPicker} from "./Pickers";
const reasonLabels:Partial<Record<EvaluationReason,string>>={
  [EvaluationReason.UNSPECIFIED]:"Не указано",
  [EvaluationReason.ACTIVE_GRANT]:"Действующее согласие",
  [EvaluationReason.ACTIVE_DENY]:"Действующий запрет",
  [EvaluationReason.WITHDRAWN]:"Согласие отозвано",
  [EvaluationReason.NO_APPLICABLE_GRANT]:"Подходящего согласия нет"
};
function ScopeFields() {
  const [selectedPartyId,setSelectedPartyId]=useState("");
  return <><PartyPicker onSelect={setSelectedPartyId}/><ContactPointPicker key={selectedPartyId} partyId={selectedPartyId} label="Контакт (необязательно)"/>
    <TextField label="Цель" name="purpose" defaultValue="marketing" required/>
    <SelectField label="Канал" name="channel" defaultValue="1">
      <option value={CommunicationChannel.EMAIL}>Email</option><option value={CommunicationChannel.PHONE}>Телефон</option><option value={CommunicationChannel.SMS}>SMS</option>
      <option value={CommunicationChannel.POSTAL}>Почта</option><option value={CommunicationChannel.MESSAGING}>Мессенджер</option><option value={CommunicationChannel.PUSH}>Push</option>
    </SelectField></>;
}
function scope(data:FormData) {
  const contact=data.get("contact");
  return {partyId:field(data,"partyId"),purpose:field(data,"purpose"),channel:enumValue(field(data,"channel"),[CommunicationChannel.EMAIL,CommunicationChannel.PHONE,CommunicationChannel.SMS,CommunicationChannel.POSTAL,CommunicationChannel.MESSAGING,CommunicationChannel.PUSH],"Канал"),
    ...(typeof contact==="string"&&contact?{contactPointId:contact}:{})};
}
function ConsentDecision({data}:{data:Awaited<ReturnType<typeof api.consents.evaluateConsent>>}) {
  return <FeedbackPanel tone={data.allowed?"success":"warning"}
    title={data.allowed?"Контакт разрешён":"Контакт не разрешён"}>
    {reasonLabels[data.reason]??"Неизвестная причина"}
  </FeedbackPanel>;
}
export function ConsentScreen() {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState("");
  const [grantPartyId,setGrantPartyId]=useState("");
  const [grantContactId,setGrantContactId]=useState("");
  const [evaluatedAuthorizationId,setEvaluatedAuthorizationId]=useState("");
  const [draftId,setDraftId]=useState(newId);
  const [composer,setComposer]=useState<"grant"|"suppress"|null>(null);
  const evaluation=useReadAction<Awaited<ReturnType<typeof api.consents.evaluateConsent>>>();
  const query=useQuery(useCallback((signal:AbortSignal)=>api.consents.listConsentAuthorizations({page:{pageSize:25,pageToken:cursor},
    ...(partyId?{partyId}:{})},{signal}),[cursor,partyId]));
  const mutation=useMutation({
    storageKey:"crm.consents.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.consents.getConsentAuthorization({
      authorizationId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:query.reload
  });
  return <>
    <PageHeader title="Коммуникации" description="Проверка разрешений, отзыв и запрет. Подписанный Access-владелец выдаёт личное согласие; сервисный администратор dev может проводить отдельный тестовый сценарий." actions={<RecordActions><Button disabled={mutation.busy||mutation.pending} onClick={()=>{setComposer("grant");mutation.changed();}}>Выдать согласие</Button><Button disabled={mutation.busy||mutation.pending} variant="secondary" onClick={()=>{setComposer("suppress");mutation.changed();}}>Запретить коммуникацию</Button></RecordActions>}/>
    <Card title="Проверить возможность контакта"><form onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      setEvaluatedAuthorizationId("");
      void evaluation.run(signal=>api.consents.evaluateConsent(scope(data),{signal}));
    }}><FormGrid><ScopeFields/></FormGrid><Button type="submit" disabled={evaluation.busy}>Проверить разрешение</Button></form>
      {!evaluatedAuthorizationId&&!evaluation.busy&&evaluation.error&&<FeedbackPanel tone="danger" title={evaluation.error}/>}
      {!evaluatedAuthorizationId&&!evaluation.busy&&evaluation.data&&<ConsentDecision data={evaluation.data}/>}
    </Card>
    <PartyPicker label="Фильтр по участнику" required={false} value={partyId} onSelect={id=>{setPartyId(id);setCursor("");}}/>
    <QueryNotice query={query}/>
    {query.data?.authorizations.map(a=><Card key={a.authorizationId} title={a.effect===ConsentEffect.DENY?"Запрет":a.effect===ConsentEffect.GRANT?"Согласие":"Неизвестный тип"}>
      <p><PartyName partyId={a.partyId}/> · {a.purpose} · {a.jurisdiction}</p><p>{a.status===ConsentStatus.ACTIVE?"Действует":a.status===ConsentStatus.WITHDRAWN?"Отозвано":"Неизвестный статус"} · версия {a.version.toString()}</p>
      <Button variant="secondary" disabled={evaluation.busy} onClick={()=>{
        setEvaluatedAuthorizationId(a.authorizationId);
        void evaluation.run(signal=>api.consents.evaluateConsent({partyId:a.partyId,
          ...(a.contactPointId?{contactPointId:a.contactPointId}:{}),
          purpose:a.purpose,channel:a.channel},{signal}));
      }}>Проверить разрешение</Button>
      {evaluatedAuthorizationId===a.authorizationId&&!evaluation.busy&&evaluation.error&&
        <FeedbackPanel tone="danger" title={evaluation.error}/>}
      {evaluatedAuthorizationId===a.authorizationId&&!evaluation.busy&&evaluation.data&&
        <ConsentDecision data={evaluation.data}/>}
      {a.effect===ConsentEffect.GRANT&&a.status===ConsentStatus.ACTIVE&&<Button variant="danger" disabled={mutation.busy||mutation.pending}
        onClick={()=>void mutation.runIntent(
          ()=>prepareMutation(api.consents.withdrawConsentAuthorization,{
            authorizationId:a.authorizationId,
            precondition:{idempotencyKey:mutation.key,expectedVersion:a.version}
          },a.authorizationId),query.reload
        )}>Отозвать согласие</Button>}
    </Card>)}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <RecordPanel open={composer==="grant"} title="Согласие для подтверждённого email" onClose={()=>{if(!mutation.busy&&!mutation.pending)setComposer(null);}}>
      <p>Сначала подтвердите email в разделе «Контакты». Сервисный администратор проверяет только тестовые контакты и записывается в аудит как агент, не как человек.</p>
      <form key={"grant-"+draftId} onChange={mutation.changed} onSubmit={e=>{
        e.preventDefault();const data=new FormData(e.currentTarget);
        void mutation.runIntent(()=>prepareMutation(api.consents.grantVerifiedEmailConsent,{
          authorizationId:draftId,partyId:field(data,"partyId"),
          contactPointId:field(data,"verifiedContactId"),purpose:field(data,"grantPurpose"),
          jurisdiction:field(data,"grantJurisdiction"),
          precondition:{idempotencyKey:mutation.key}
        },draftId),async()=>{setComposer(null);setGrantPartyId("");setGrantContactId("");setDraftId(newId());await query.reload();});
      }}>
        <FormGrid>
          <PartyPicker onSelect={id=>{setGrantPartyId(id);setGrantContactId("");}}/>
          <ContactPointPicker key={grantPartyId} partyId={grantPartyId} name="verifiedContactId"
            label="Подтверждённый email" onSelect={setGrantContactId} emailOnly verifiedOnly required/>
          <TextField label="Цель согласия" name="grantPurpose" defaultValue="marketing" required/>
          <TextField label="Юрисдикция согласия" name="grantJurisdiction" defaultValue="EU" required/>
        </FormGrid>
        <Button type="submit" disabled={!grantPartyId||!grantContactId||mutation.busy||mutation.pending}>Выдать согласие на email-коммуникацию</Button>
      </form>
      <MutationNotice state={mutation}/>
    </RecordPanel>
    <RecordPanel open={composer==="suppress"} title="Запретить коммуникацию" onClose={()=>{if(!mutation.busy&&!mutation.pending)setComposer(null);}}><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.runIntent(()=>prepareMutation(api.consents.suppressCommunication,{
        authorizationId:draftId,...scope(data),
        jurisdiction:field(data,"jurisdiction"),precondition:{idempotencyKey:mutation.key}
      },draftId),async()=>{setComposer(null);setDraftId(newId());await query.reload();});
    }}><FormGrid><ScopeFields/><TextField label="Юрисдикция" name="jurisdiction" defaultValue="EU" required/></FormGrid>
      <Button type="submit" disabled={mutation.busy||mutation.pending} variant="danger">Запретить контакт</Button>
    </form><MutationNotice state={mutation}/></RecordPanel>
    {composer===null&&<MutationNotice state={mutation}/>}
  </>;
}

import {useCallback,useState} from "react";
import {Button,Card,FeedbackPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,newId} from "./api";
import {useQuery,useReadAction,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
function ScopeFields() {
  return <><PartyPicker/><TextField label="ID контакта (необязательно)" name="contact"/>
    <TextField label="Цель" name="purpose" defaultValue="marketing" required/>
    <SelectField label="Канал" name="channel" defaultValue="1">
      <option value="1">Email</option><option value="2">Телефон</option><option value="3">SMS</option>
      <option value="4">Почта</option><option value="5">Мессенджер</option><option value="6">Push</option>
    </SelectField></>;
}
function scope(data:FormData) {
  const contact=field(data,"contact");
  return {partyId:field(data,"partyId"),purpose:field(data,"purpose"),channel:Number(field(data,"channel")),
    ...(contact?{contactPointId:contact}:{})};
}
export function ConsentScreen() {
  const [cursor,setCursor]=useState("");
  const [partyId,setPartyId]=useState("");
  const [draftId,setDraftId]=useState(newId);
  const mutation=useMutation();
  const evaluation=useReadAction<Awaited<ReturnType<typeof api.consents.evaluateConsent>>>();
  const query=useQuery(useCallback(()=>api.consents.listConsentAuthorizations({page:{pageSize:25,pageToken:cursor},
    ...(partyId?{partyId}:{})}),[cursor,partyId]));
  return <>
    <PageHeader title="Коммуникации" description="Проверка разрешений и запрет контакта оператором. Выдать согласие без подтверждающих данных здесь нельзя."/>
    <Card title="Проверить возможность контакта"><form onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void evaluation.run(()=>api.consents.evaluateConsent(scope(data)));
    }}><FormGrid><ScopeFields/></FormGrid><Button type="submit" disabled={evaluation.busy}>Проверить разрешение</Button></form>
      {evaluation.error&&<FeedbackPanel tone="danger" title={evaluation.error}/>}
      {evaluation.data&&<FeedbackPanel tone={evaluation.data.allowed?"success":"warning"} title={evaluation.data.allowed?"Контакт разрешён":"Контакт не разрешён"}>
        {["Не указано","Действующее согласие","Действующий запрет","Согласие отозвано","Подходящего согласия нет"][evaluation.data.reason]??"Неизвестная причина"}
      </FeedbackPanel>}
    </Card>
    <TextField label="Фильтр по ID участника" value={partyId} onChange={e=>{setPartyId(e.target.value);setCursor("");}}/>
    <QueryNotice query={query}/>
    {query.data?.authorizations.map(a=><Card key={a.authorizationId} title={a.effect===2?"Запрет":"Согласие"}>
      <p>{a.partyId} · {a.purpose} · {a.jurisdiction}</p><p>{a.status===1?"Действует":"Отозвано"} · версия {a.version.toString()}</p>
    </Card>)}
    <Paging next={query.data?.pageInfo?.nextPageToken??""} busy={query.busy} onFirst={()=>setCursor("")} onNext={()=>setCursor(query.data?.pageInfo?.nextPageToken??"")}/>
    <Card title="Запретить коммуникацию"><form key={draftId} onChange={mutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mutation.run(()=>api.consents.suppressCommunication({authorizationId:draftId,...scope(data),
        jurisdiction:field(data,"jurisdiction"),precondition:{idempotencyKey:mutation.key}}),
        async()=>{setDraftId(newId());await query.reload();});
    }}><FormGrid><ScopeFields/><TextField label="Юрисдикция" name="jurisdiction" defaultValue="EU" required/></FormGrid>
      <Button type="submit" disabled={mutation.busy} variant="danger">Запретить контакт</Button>
    </form><MutationNotice state={mutation}/></Card>
  </>;
}

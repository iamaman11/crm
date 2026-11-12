import {useCallback,useState} from "react";
import {Button,Card,FeedbackPanel,FormGrid,PageHeader} from "@ultimate-crm/ui";
import {api,field} from "./api";
import {useQuery,useReadAction,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";

export function ReviewScreen() {
  const [cursor,setCursor]=useState("");
  const quality=useReadAction<Awaited<ReturnType<typeof api.dataQuality.evaluatePartyName>>>();
  const comparison=useReadAction<Awaited<ReturnType<typeof api.identityResolution.compareParties>>>();
  const candidates=useQuery(useCallback(()=>api.identityResolution.findSharedEmailCandidates({page:{pageSize:25,pageToken:cursor}}),[cursor]));
  return <>
    <PageHeader title="Качество и совпадения" description="Проверки помогают заметить проблему. Совпадение email не подтверждает личность и не объединяет записи."/>
    <Card title="Проверить имя"><form onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void quality.run(()=>api.dataQuality.evaluatePartyName({partyId:field(data,"partyId")}));
    }}><FormGrid><PartyPicker/></FormGrid><Button type="submit" disabled={quality.busy}>Проверить качество</Button></form>
      {quality.error&&<FeedbackPanel tone="danger" title={quality.error}/>}
      {quality.data&&<><h3>{quality.data.party?.displayName}</h3><p>Не пройдено: {quality.data.failedCount}</p>
        <ul>{quality.data.checks.map(c=><li key={c.ruleKey}>{c.passed?"✓":"!"} {c.title}{!c.passed&&" — "+c.remediation}</li>)}</ul></>}
    </Card>
    <Card title="Сравнить участников"><form onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void comparison.run(()=>api.identityResolution.compareParties({firstPartyId:field(data,"first"),secondPartyId:field(data,"second")}));
    }}><FormGrid><PartyPicker name="first" label="Первый участник"/><PartyPicker name="second" label="Второй участник"/></FormGrid>
      <Button type="submit" disabled={comparison.busy}>Сравнить</Button></form>
      {comparison.error&&<FeedbackPanel tone="danger" title={comparison.error}/>}
      {comparison.data&&<p>{comparison.data.firstParty?.displayName} / {comparison.data.secondParty?.displayName}:
        {comparison.data.samePartyId?" одна запись":" разные записи"}; {comparison.data.identityConfirmed?"личность подтверждена":"личность не подтверждена"}.</p>}
    </Card>
    <Card title="Совпадающие email"><QueryNotice query={candidates}/>
      {candidates.data?.candidates.map(c=><p key={c.firstContactPointId+":"+c.secondContactPointId}>
        {c.firstParty?.displayName} ↔ {c.secondParty?.displayName} · {c.bothContactsVerified?"оба контакта подтверждены":"есть неподтверждённые контакты"}
      </p>)}
      {candidates.data?.candidates.length===0&&<p>Совпадений не найдено.</p>}
      <Paging next={candidates.data?.pageInfo?.nextPageToken??""} busy={candidates.busy} onFirst={()=>setCursor("")}
        onNext={()=>setCursor(candidates.data?.pageInfo?.nextPageToken??"")}/>
    </Card>
  </>;
}

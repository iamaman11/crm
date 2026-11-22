import {useCallback,useState} from "react";
import {Button,Card,FeedbackPanel,FormGrid,PageHeader} from "@ultimate-crm/ui";
import {api,field} from "./api";
import {useQuery,useReadAction,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {partyHref} from "./routes";

export function ReviewScreen({initialPartyId=""}:{initialPartyId?:string}) {
  const [cursor,setCursor]=useState("");
  const [selectedCandidate,setSelectedCandidate]=useState("");
  const quality=useReadAction<Awaited<ReturnType<typeof api.dataQuality.evaluatePartyName>>>();
  const comparison=useReadAction<Awaited<ReturnType<typeof api.identityResolution.compareParties>>>();
  const candidates=useQuery(useCallback((signal:AbortSignal)=>api.identityResolution.findSharedEmailCandidates({page:{pageSize:25,pageToken:cursor}},{signal}),[cursor]));
  const comparisonResult=comparison.data&&<p>
    {comparison.data.firstParty&&<a href={partyHref(comparison.data.firstParty.partyId)}>{comparison.data.firstParty.displayName}</a>}
    {" / "}
    {comparison.data.secondParty&&<a href={partyHref(comparison.data.secondParty.partyId)}>{comparison.data.secondParty.displayName}</a>}:
    {comparison.data.samePartyId?" одна запись":" разные записи"}; {comparison.data.identityConfirmed?"личность подтверждена":"личность не подтверждена"}.
  </p>;
  return <>
    <PageHeader title="Качество и совпадения" description="Проверки помогают заметить проблему. Совпадение email не подтверждает личность и не объединяет записи."/>
    <Card title="Проверить имя"><form onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void quality.run(signal=>api.dataQuality.evaluatePartyName({partyId:field(data,"partyId")},{signal}));
    }}><FormGrid><PartyPicker initialPartyId={initialPartyId}/></FormGrid><Button type="submit" disabled={quality.busy}>Проверить качество</Button></form>
      {quality.error&&<FeedbackPanel tone="danger" title={quality.error}/>}
      {quality.data&&<><h3>{quality.data.party?.displayName}</h3><p>Не пройдено: {quality.data.failedCount}</p>
        <ul>{quality.data.checks.map(c=><li key={c.ruleKey}>{c.passed?"✓":"!"} {c.title}{!c.passed&&" — "+c.remediation}</li>)}</ul>
        {quality.data.failedCount>0&&quality.data.party&&<p><a href={partyHref(quality.data.party.partyId)}>Открыть карточку и исправить</a></p>}</>}
    </Card>
    <Card title="Сравнить участников"><form onSubmit={e=>{
      e.preventDefault();setSelectedCandidate("");const data=new FormData(e.currentTarget);
      void comparison.run(signal=>api.identityResolution.compareParties({firstPartyId:field(data,"first"),secondPartyId:field(data,"second")},{signal}));
    }}><FormGrid><PartyPicker name="first" label="Первый участник"/><PartyPicker name="second" label="Второй участник"/></FormGrid>
      <Button type="submit" disabled={comparison.busy}>Сравнить</Button></form>
      {!selectedCandidate&&comparison.error&&<FeedbackPanel tone="danger" title={comparison.error}/>}
      {!selectedCandidate&&comparisonResult}
    </Card>
    <Card title="Совпадающие email"><QueryNotice query={candidates}/>
      <ul>{candidates.data?.candidates.map(c=>{
        const key=c.firstContactPointId+":"+c.secondContactPointId;
        return <li key={key}>
          {c.firstParty&&<a href={partyHref(c.firstParty.partyId)}>{c.firstParty.displayName}</a>}
          {" ↔ "}
          {c.secondParty&&<a href={partyHref(c.secondParty.partyId)}>{c.secondParty.displayName}</a>}
          {" · "}{c.bothContactsVerified?"оба контакта подтверждены":"есть неподтверждённые контакты"}{" "}
          {c.firstParty&&c.secondParty&&<Button variant="secondary" disabled={comparison.busy}
            onClick={()=>{
              setSelectedCandidate(key);
              void comparison.run(signal=>api.identityResolution.compareParties({
                firstPartyId:c.firstParty!.partyId,secondPartyId:c.secondParty!.partyId
              },{signal}));
            }}>Сравнить пару</Button>}
          {selectedCandidate===key&&!comparison.busy&&comparison.error&&<FeedbackPanel tone="danger" title={comparison.error}/>}
          {selectedCandidate===key&&!comparison.busy&&comparisonResult}
        </li>;
      })}</ul>
      {candidates.data?.candidates.length===0&&<p>Совпадений не найдено.</p>
      <Paging next={candidates.data?.pageInfo?.nextPageToken??""} busy={candidates.busy} onFirst={()=>setCursor("")}
        onNext={()=>setCursor(candidates.data?.pageInfo?.nextPageToken??"")}/>
    </Card>
  </>;
}

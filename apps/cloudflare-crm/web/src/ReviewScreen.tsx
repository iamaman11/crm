import {useCallback,useState} from "react";
import {Button,Card,FeedbackPanel,FormGrid,PageHeader,SelectField} from "@ultimate-crm/ui";
import {api,field,newId} from "./api";
import {useQuery,useReadAction,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
import {PartyPicker} from "./Pickers";
import {partyHref} from "./routes";

export function ReviewScreen({initialPartyId=""}:{initialPartyId?:string}) {
  const [cursor,setCursor]=useState("");
  const [selectedCandidate,setSelectedCandidate]=useState("");
  const [mergeSource,setMergeSource]=useState("");
  const [confirmedIdentity,setConfirmedIdentity]=useState(false);
  const [mergeId,setMergeId]=useState(newId);
  const [unmergeReason,setUnmergeReason]=useState("incorrect_merge");
  const quality=useReadAction<Awaited<ReturnType<typeof api.dataQuality.evaluatePartyName>>>();
  const comparison=useReadAction<Awaited<ReturnType<typeof api.identityResolution.compareParties>>>();
  const candidates=useQuery(useCallback((signal:AbortSignal)=>api.identityResolution.findSharedEmailCandidates({page:{pageSize:25,pageToken:cursor}},{signal}),[cursor]));
  const focusPartyId=comparison.data?.firstParty?.partyId??initialPartyId;
  const lineage=useQuery(useCallback(async(signal:AbortSignal)=>focusPartyId?
    api.identityResolution.listMergeOperationsByParty({partyId:focusPartyId,page:{pageSize:50}},{signal}):
    undefined,[focusPartyId]));
  const currentCanonical=useQuery(useCallback(async(signal:AbortSignal)=>focusPartyId?
    api.identityResolution.resolveCanonicalParty({partyId:focusPartyId},{signal}):undefined,[focusPartyId]));
  const mergeMutation=useMutation({
    storageKey:"crm.identity.merge.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.identityResolution.getMergeOperation({
      operationId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async()=>{await lineage.reload();await currentCanonical.reload();}
  });
  const unmergeMutation=useMutation({
    storageKey:"crm.identity.unmerge.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.identityResolution.getMergeOperation({
      operationId:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async()=>{await lineage.reload();await currentCanonical.reload();}
  });
  const source=comparison.data?.firstParty?.partyId===mergeSource?
    comparison.data.firstParty:comparison.data?.secondParty?.partyId===mergeSource?
    comparison.data.secondParty:undefined;
  const survivor=source?.partyId===comparison.data?.firstParty?.partyId?
    comparison.data?.secondParty:source?comparison.data?.firstParty:undefined;
  const recordMerge=()=>void mergeMutation.runIntent(()=> {
    if(!confirmedIdentity||!source||!survivor)throw Error("Необходимо подтвердить обе записи и выбрать исходную.");
    return prepareMutation(api.identityResolution.mergeParty,{
      operationId:mergeId,
      sourcePartyId:source.partyId,sourcePartyVersion:source.version,
      survivorPartyId:survivor.partyId,survivorPartyVersion:survivor.version,
      reason:"manual_identity_review",
      precondition:{idempotencyKey:mergeMutation.key}
    },mergeId);
  },async()=>{
    setConfirmedIdentity(false);setMergeSource("");setMergeId(newId());
    await lineage.reload();await currentCanonical.reload();
  });
  const undoMerge=async(operation:{
    operationId:string;sourcePartyId:string;survivorPartyId:string;version:bigint;
  })=>{
    try {
      const [sourceRecord,survivorRecord]=await Promise.all([
        api.partyRecords.getParty({partyId:operation.sourcePartyId}),
        api.partyRecords.getParty({partyId:operation.survivorPartyId})
      ]);
      if(!sourceRecord.party||!survivorRecord.party)throw Error("Исходная запись недоступна.");
      void unmergeMutation.runIntent(()=>prepareMutation(api.identityResolution.unmergeParty,{
        operationId:operation.operationId,
        sourcePartyVersion:sourceRecord.party!.version,
        survivorPartyVersion:survivorRecord.party!.version,
        reason:unmergeReason,
        precondition:{expectedVersion:operation.version,idempotencyKey:unmergeMutation.key}
      },operation.operationId),async()=>{
        await lineage.reload();await currentCanonical.reload();
      });
    }catch(error){await unmergeMutation.reportError(error);}
  };
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
      setMergeSource("");setConfirmedIdentity(false);void comparison.run(signal=>api.identityResolution.compareParties({firstPartyId:field(data,"first"),secondPartyId:field(data,"second")},{signal}));
    }}><FormGrid><PartyPicker name="first" label="Первый участник"/><PartyPicker name="second" label="Второй участник"/></FormGrid>
      <Button type="submit" disabled={comparison.busy}>Сравнить</Button></form>
      {!selectedCandidate&&comparison.error&&<FeedbackPanel tone="danger" title={comparison.error}/>}
      {!selectedCandidate&&comparisonResult}
    </Card>

    {(comparison.data?.firstParty&&comparison.data?.secondParty||initialPartyId)&&<Card title="Каноническая запись и история объединений">
      <p><strong>Без переноса данных:</strong> объединение помечает каноническую запись.
        Обе исходные карточки, контакты, связи, клиенты, сделки и задачи сохраняют свои ID,
        остаются доступными, а объединение можно отменить.</p>
      <QueryNotice query={lineage}/><QueryNotice query={currentCanonical}/>
      {currentCanonical.data&&currentCanonical.data.canonicalPartyId!==currentCanonical.data.requestedPartyId&&
        <p>Каноническая запись: <a href={partyHref(currentCanonical.data.canonicalPartyId)}>
          Открыть выбранного участника</a>.</p>}
      {!!comparison.data?.firstParty&&!!comparison.data.secondParty&&<>
        <SelectField label="Запись-дубликат (неосновная)" value={mergeSource}
          onChange={e=>{setMergeSource(e.target.value);setConfirmedIdentity(false);mergeMutation.changed();}}>
          <option value="">Выберите запись</option>
          <option value={comparison.data.firstParty.partyId}>{comparison.data.firstParty.displayName}</option>
          <option value={comparison.data.secondParty.partyId}>{comparison.data.secondParty.displayName}</option>
        </SelectField>
        {survivor&&<p>Основная запись останется: <a href={partyHref(survivor.partyId)}>{survivor.displayName}</a>.</p>}
        <label><input type="checkbox" checked={confirmedIdentity}
          onChange={e=>{setConfirmedIdentity(e.target.checked);mergeMutation.changed();}}/>
          Я вручную проверил личность/организацию и подтверждаю, что это дубликат; совпадение email не является достаточным основанием.</label>{" "}
        <Button disabled={!confirmedIdentity||!source||!survivor||mergeMutation.busy||mergeMutation.pending}
          onClick={recordMerge}>Подтвердить объединение</Button>
      </>}
      <MutationNotice state={mergeMutation}/>
      <h3>История решений</h3>
      {lineage.data?.operations.length===0&&<p>Объединений пока нет.</p>}
      <ul>{lineage.data?.operations.map(operation=><li key={operation.operationId}>
        {operation.active?"Активно":"Отменено"}:{" "}
        <a href={partyHref(operation.sourcePartyId)}>Исходная запись</a>{" → "}
        <a href={partyHref(operation.survivorPartyId)}>каноническая запись</a>
        {" · "}{operation.reason}
        {operation.active&&<>
          <SelectField label="Причина отмены" value={unmergeReason}
            onChange={e=>{setUnmergeReason(e.target.value);unmergeMutation.changed();}}>
            <option value="incorrect_merge">Ошибочное объединение</option>
            <option value="identity_changed">Уточнены сведения</option>
          </SelectField>
          <Button variant="secondary" disabled={unmergeMutation.busy||unmergeMutation.pending}
            onClick={()=>void undoMerge(operation)}>Отменить объединение</Button>
        </>}
      </li>)}</ul>
      <MutationNotice state={unmergeMutation}/>
    </Card>}
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
              setSelectedCandidate(key);setMergeSource("");setConfirmedIdentity(false);
              void comparison.run(signal=>api.identityResolution.compareParties({
                firstPartyId:c.firstParty!.partyId,secondPartyId:c.secondParty!.partyId
              },{signal}));
            }}>Сравнить пару</Button>}
          {selectedCandidate===key&&!comparison.busy&&comparison.error&&<FeedbackPanel tone="danger" title={comparison.error}/>}
          {selectedCandidate===key&&!comparison.busy&&comparisonResult}
        </li>;
      })}</ul>
      {candidates.data?.candidates.length===0&&<p>Совпадений не найдено.</p>}
      <Paging next={candidates.data?.pageInfo?.nextPageToken??""} busy={candidates.busy} onFirst={()=>setCursor("")}
        onNext={()=>setCursor(candidates.data?.pageInfo?.nextPageToken??"")}/>
    </Card>
  </>;
}

import {useCallback,useState} from "react";
import {Button,Card,RecordPanel,RecordActions,FeedbackPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,timestamp} from "./api";
import {RawPayloadPolicy} from "@ultimate-crm/client/cloudflare";
import {enumValue,boundedInt} from "./enumFields";
import {useQuery,useReadAction,useMutation,prepareMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
export function EnrichmentScreen() {
  const [profileCursor,setProfileCursor]=useState("");
  const [mappingCursor,setMappingCursor]=useState("");
  const profiles=useQuery(useCallback((signal:AbortSignal)=>api.enrichment.listProviderProfiles({page:{pageSize:25,pageToken:profileCursor}},{signal}),[profileCursor]));
  const mappings=useQuery(useCallback((signal:AbortSignal)=>api.enrichment.listMappings({page:{pageSize:25,pageToken:mappingCursor}},{signal}),[mappingCursor]));
  const profileDetail=useReadAction<Awaited<ReturnType<typeof api.enrichment.getProviderProfile>>>();
  const mappingDetail=useReadAction<Awaited<ReturnType<typeof api.enrichment.getMapping>>>();
  const profileMutation=useMutation({
    storageKey:"crm.enrichment.profile.pending.v1",
    lookup:async(marker,signal)=>{
      const result=await api.enrichment.getProviderProfile({
        profileVersionId:"",receiptIdempotencyKey:marker.key
      },{signal});
      return result.profile?.providerKey===marker.resourceId?result.confirmedMutation:undefined;
    },
    refresh:profiles.reload
  });
  const mappingMutation=useMutation({
    storageKey:"crm.enrichment.mapping.pending.v1",
    lookup:async(marker,signal)=>{
      const result=await api.enrichment.getMapping({
        mappingVersionId:"",receiptIdempotencyKey:marker.key
      },{signal});
      return result.mapping?.mappingKey===marker.resourceId?result.confirmedMutation:undefined;
    },
    refresh:mappings.reload
  });
  const [publishMode,setPublishMode]=useState<"profile"|"mapping"|null>(null);
  const [detailMode,setDetailMode]=useState<"profile"|"mapping"|null>(null);
  return <>
    <PageHeader title="Обогащение данных" description="Неизменяемые версии настроек провайдеров и сопоставлений. Запросы к провайдерам и изменение данных участников пока недоступны." actions={<RecordActions><Button disabled={profileMutation.busy||profileMutation.pending} onClick={()=>{setPublishMode("profile");profileMutation.changed();}}>Новый профиль</Button><Button variant="secondary" disabled={mappingMutation.busy||mappingMutation.pending} onClick={()=>{setPublishMode("mapping");mappingMutation.changed();}}>Новое сопоставление</Button></RecordActions>}/>
    <Card title="Профили провайдеров"><QueryNotice query={profiles}/>
      {profiles.data?.profiles.map(p=><p key={p.profileVersionId}>
        <Button variant="secondary" onClick={()=>{setDetailMode("profile");void profileDetail.run(signal=>api.enrichment.getProviderProfile({profileVersionId:p.profileVersionId},{signal}));}}>{p.providerKey} · {p.adapterContractVersion}</Button>
      </p>)}
      <Paging next={profiles.data?.pageInfo?.nextPageToken??""} busy={profiles.busy} onFirst={()=>setProfileCursor("")} onNext={()=>setProfileCursor(profiles.data?.pageInfo?.nextPageToken??"")}/>
      </Card>
    <RecordPanel open={detailMode==="profile"} title="Профиль провайдера" onClose={()=>setDetailMode(null)}>
      {profileDetail.busy&&<p role="status">Загрузка…</p>}
      {profileDetail.error&&<FeedbackPanel tone="danger" title={profileDetail.error}/>}
      {profileDetail.data?.profile&&<><p>ID: {profileDetail.data.profile.profileVersionId}</p>
        <p>Лицензия: {profileDetail.data.profile.licenseId}; регион: {profileDetail.data.profile.residencyRegion}; хранение: {profileDetail.data.profile.retentionDays} дней.</p></>}
    </RecordPanel>
    <RecordPanel open={publishMode==="profile"} title="Опубликовать профиль" onClose={()=>{if(!profileMutation.busy&&!profileMutation.pending)setPublishMode(null);}}><form onChange={profileMutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void profileMutation.runIntent(()=>{
        const effectiveAt=timestamp(field(data,"effective"));
        if(!effectiveAt)throw Error("Укажите начало действия");
        const expiresAt=timestamp(field(data,"expires"));
        const providerKey=field(data,"provider");
        return prepareMutation(api.enrichment.publishProviderProfile,{
          providerKey,adapterKind:field(data,"adapter"),adapterContractVersion:field(data,"version"),
          purposeCodes:field(data,"purposes").split(",").map(x=>x.trim()).filter(Boolean),
          licenseId:field(data,"license"),permittedUseClass:field(data,"use"),residencyRegion:field(data,"region"),
          retentionDays:boundedInt(field(data,"retention"),0,3650,"Срок хранения"),rawPayloadPolicy:enumValue(field(data,"raw"),[RawPayloadPolicy.DIGEST_ONLY,RawPayloadPolicy.GOVERNED_PROTECTED_EVIDENCE],"Политика исходных данных"),
          credentialHandleAliases:field(data,"handles").split(",").map(x=>x.trim()).filter(Boolean),
          effectiveAt,...(expiresAt?{expiresAt}:{}),precondition:{idempotencyKey:profileMutation.key}
        },providerKey);
      },async()=>{setPublishMode(null);await profiles.reload();});
    }}><FormGrid>
      <TextField label="Ключ провайдера" name="provider" defaultValue="company_registry" required/>
      <TextField label="Вид адаптера" name="adapter" defaultValue="registry_http_v1" required/>
      <TextField label="Версия контракта адаптера" name="version" defaultValue="1.0.0" required/>
      <TextField label="Цели через запятую" name="purposes" defaultValue="customer_profile_enrichment" required/>
      <TextField label="Лицензия" name="license" required/>
      <TextField label="Класс разрешённого использования" name="use" defaultValue="customer_master_review" required/>
      <TextField label="Регион хранения" name="region" defaultValue="eu" required/>
      <TextField label="Срок хранения, дней" name="retention" type="number" min="0" max="3650" defaultValue="30" required/>
      <SelectField label="Исходные данные" name="raw" defaultValue="1"><option value={RawPayloadPolicy.DIGEST_ONLY}>Только хеш</option><option value={RawPayloadPolicy.GOVERNED_PROTECTED_EVIDENCE}>Защищённое доказательство</option></SelectField>
      <TextField label="Имена ссылок на учётные данные (не секреты)" name="handles"/>
      <TextField label="Начало действия" name="effective" type="datetime-local" step="1" required/>
      <TextField label="Окончание действия" name="expires" type="datetime-local" step="1"/>
    </FormGrid><Button type="submit" disabled={profileMutation.busy||profileMutation.pending}>Опубликовать профиль</Button></form><MutationNotice state={profileMutation}/></RecordPanel>
    {publishMode!=="profile"&&<MutationNotice state={profileMutation}/>}
    <Card title="Сопоставления"><QueryNotice query={mappings}/>
      {mappings.data?.mappings.map(m=><p key={m.mappingVersionId}><Button variant="secondary"
        onClick={()=>{setDetailMode("mapping");void mappingDetail.run(signal=>api.enrichment.getMapping({mappingVersionId:m.mappingVersionId},{signal}));}}>{m.mappingKey}</Button></p>)}
      <Paging next={mappings.data?.pageInfo?.nextPageToken??""} busy={mappings.busy} onFirst={()=>setMappingCursor("")} onNext={()=>setMappingCursor(mappings.data?.pageInfo?.nextPageToken??"")}/>
      </Card>
    <RecordPanel open={detailMode==="mapping"} title="Сопоставление" onClose={()=>setDetailMode(null)}>
      {mappingDetail.busy&&<p role="status">Загрузка…</p>}
      {mappingDetail.error&&<FeedbackPanel tone="danger" title={mappingDetail.error}/>}
      {mappingDetail.data?.mapping&&<p>ID: {mappingDetail.data.mapping.mappingVersionId}; поле: {mappingDetail.data.mapping.providerResponseFieldPath}; профиль: {mappingDetail.data.mapping.providerProfileVersionId}</p>}
    </RecordPanel>
    <RecordPanel open={publishMode==="mapping"} title="Опубликовать сопоставление" onClose={()=>{if(!mappingMutation.busy&&!mappingMutation.pending)setPublishMode(null);}}><form onChange={mappingMutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      const mappingKey=field(data,"key");
      void mappingMutation.runIntent(()=>prepareMutation(api.enrichment.publishMapping,{
        mappingKey,providerProfileVersionId:field(data,"profile"),
        providerResponseFieldPath:field(data,"path"),maximumSuggestionsPerResponse:boundedInt(field(data,"maximum"),1,32,"Максимум предложений"),
        confidenceRequired:data.get("confidence")==="on",precondition:{idempotencyKey:mappingMutation.key}
      },mappingKey),async()=>{setPublishMode(null);await mappings.reload();});
    }}><FormGrid>
      <TextField label="Ключ сопоставления" name="key" defaultValue="party_display_name" required/>
      <SelectField label="Версия профиля" name="profile" required defaultValue="">
        <option value="">Выберите профиль из текущей страницы</option>
        {profiles.data?.profiles.map(p=><option value={p.profileVersionId} key={p.profileVersionId}>{p.providerKey} · {p.profileVersionId}</option>)}
      </SelectField>
      <TextField label="Поле ответа провайдера" name="path" defaultValue="organization.legal_name" required/>
      <TextField label="Максимум предложений" name="maximum" type="number" min="1" max="32" defaultValue="1" required/>
      <TextField label="Требовать оценку уверенности" name="confidence" type="checkbox" defaultChecked/>
    </FormGrid><Button type="submit" disabled={mappingMutation.busy||mappingMutation.pending}>Опубликовать сопоставление</Button></form><MutationNotice state={mappingMutation}/></RecordPanel>
    {publishMode!=="mapping"&&<MutationNotice state={mappingMutation}/>}
  </>;
}

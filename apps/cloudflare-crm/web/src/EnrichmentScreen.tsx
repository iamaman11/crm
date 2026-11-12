import {useCallback,useState} from "react";
import {Button,Card,FeedbackPanel,FormGrid,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api,field,timestamp} from "./api";
import {useQuery,useReadAction,useMutation,MutationNotice,QueryNotice,Paging} from "./hooks";
export function EnrichmentScreen() {
  const [profileCursor,setProfileCursor]=useState("");
  const [mappingCursor,setMappingCursor]=useState("");
  const profiles=useQuery(useCallback(()=>api.enrichment.listProviderProfiles({page:{pageSize:25,pageToken:profileCursor}}),[profileCursor]));
  const mappings=useQuery(useCallback(()=>api.enrichment.listMappings({page:{pageSize:25,pageToken:mappingCursor}}),[mappingCursor]));
  const profileDetail=useReadAction<Awaited<ReturnType<typeof api.enrichment.getProviderProfile>>>();
  const mappingDetail=useReadAction<Awaited<ReturnType<typeof api.enrichment.getMapping>>>();
  const profileMutation=useMutation(),mappingMutation=useMutation();
  return <>
    <PageHeader title="Обогащение данных" description="Неизменяемые версии настроек провайдеров и сопоставлений. Запросы к провайдерам и изменение данных участников пока недоступны."/>
    <Card title="Профили провайдеров"><QueryNotice query={profiles}/>
      {profiles.data?.profiles.map(p=><p key={p.profileVersionId}>
        <Button variant="secondary" onClick={()=>void profileDetail.run(()=>api.enrichment.getProviderProfile({profileVersionId:p.profileVersionId}))}>{p.providerKey} · {p.adapterContractVersion}</Button>
      </p>)}
      <Paging next={profiles.data?.pageInfo?.nextPageToken??""} busy={profiles.busy} onFirst={()=>setProfileCursor("")} onNext={()=>setProfileCursor(profiles.data?.pageInfo?.nextPageToken??"")}/>
      {profileDetail.error&&<FeedbackPanel tone="danger" title={profileDetail.error}/>}
      {profileDetail.data?.profile&&<><p>ID: {profileDetail.data.profile.profileVersionId}</p>
        <p>Лицензия: {profileDetail.data.profile.licenseId}; регион: {profileDetail.data.profile.residencyRegion}; хранение: {profileDetail.data.profile.retentionDays} дней.</p></>}
    </Card>
    <Card title="Опубликовать профиль"><form onChange={profileMutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void profileMutation.run(()=>{
        const effectiveAt=timestamp(field(data,"effective"));
        if(!effectiveAt)throw Error("Укажите начало действия");
        const expiresAt=timestamp(field(data,"expires"));
        return api.enrichment.publishProviderProfile({
          providerKey:field(data,"provider"),adapterKind:field(data,"adapter"),adapterContractVersion:field(data,"version"),
          purposeCodes:field(data,"purposes").split(",").map(x=>x.trim()).filter(Boolean),
          licenseId:field(data,"license"),permittedUseClass:field(data,"use"),residencyRegion:field(data,"region"),
          retentionDays:Number(field(data,"retention")),rawPayloadPolicy:Number(field(data,"raw")),
          credentialHandleAliases:field(data,"handles").split(",").map(x=>x.trim()).filter(Boolean),
          effectiveAt,...(expiresAt?{expiresAt}:{}),precondition:{idempotencyKey:profileMutation.key}
        });
      },profiles.reload);
    }}><FormGrid>
      <TextField label="Ключ провайдера" name="provider" defaultValue="company_registry" required/>
      <TextField label="Вид адаптера" name="adapter" defaultValue="registry_http_v1" required/>
      <TextField label="Версия контракта адаптера" name="version" defaultValue="1.0.0" required/>
      <TextField label="Цели через запятую" name="purposes" defaultValue="customer_profile_enrichment" required/>
      <TextField label="Лицензия" name="license" required/>
      <TextField label="Класс разрешённого использования" name="use" defaultValue="customer_master_review" required/>
      <TextField label="Регион хранения" name="region" defaultValue="eu" required/>
      <TextField label="Срок хранения, дней" name="retention" type="number" min="0" max="3650" defaultValue="30" required/>
      <SelectField label="Исходные данные" name="raw" defaultValue="1"><option value="1">Только хеш</option><option value="2">Защищённое доказательство</option></SelectField>
      <TextField label="Имена ссылок на учётные данные (не секреты)" name="handles"/>
      <TextField label="Начало действия" name="effective" type="datetime-local" step="1" required/>
      <TextField label="Окончание действия" name="expires" type="datetime-local" step="1"/>
    </FormGrid><Button type="submit" disabled={profileMutation.busy}>Опубликовать профиль</Button></form><MutationNotice state={profileMutation}/></Card>
    <Card title="Сопоставления"><QueryNotice query={mappings}/>
      {mappings.data?.mappings.map(m=><p key={m.mappingVersionId}><Button variant="secondary"
        onClick={()=>void mappingDetail.run(()=>api.enrichment.getMapping({mappingVersionId:m.mappingVersionId}))}>{m.mappingKey}</Button></p>)}
      <Paging next={mappings.data?.pageInfo?.nextPageToken??""} busy={mappings.busy} onFirst={()=>setMappingCursor("")} onNext={()=>setMappingCursor(mappings.data?.pageInfo?.nextPageToken??"")}/>
      {mappingDetail.error&&<FeedbackPanel tone="danger" title={mappingDetail.error}/>}
      {mappingDetail.data?.mapping&&<p>ID: {mappingDetail.data.mapping.mappingVersionId}; поле: {mappingDetail.data.mapping.providerResponseFieldPath}; профиль: {mappingDetail.data.mapping.providerProfileVersionId}</p>}
    </Card>
    <Card title="Опубликовать сопоставление"><form onChange={mappingMutation.changed} onSubmit={e=>{
      e.preventDefault();const data=new FormData(e.currentTarget);
      void mappingMutation.run(()=>api.enrichment.publishMapping({
        mappingKey:field(data,"key"),providerProfileVersionId:field(data,"profile"),
        providerResponseFieldPath:field(data,"path"),maximumSuggestionsPerResponse:Number(field(data,"maximum")),
        confidenceRequired:data.get("confidence")==="on",precondition:{idempotencyKey:mappingMutation.key}
      }),mappings.reload);
    }}><FormGrid>
      <TextField label="Ключ сопоставления" name="key" defaultValue="party_display_name" required/>
      <SelectField label="Версия профиля" name="profile" required defaultValue="">
        <option value="">Выберите профиль из текущей страницы</option>
        {profiles.data?.profiles.map(p=><option value={p.profileVersionId} key={p.profileVersionId}>{p.providerKey} · {p.profileVersionId}</option>)}
      </SelectField>
      <TextField label="Поле ответа провайдера" name="path" defaultValue="organization.legal_name" required/>
      <TextField label="Максимум предложений" name="maximum" type="number" min="1" max="32" defaultValue="1" required/>
      <TextField label="Требовать оценку уверенности" name="confidence" type="checkbox" defaultChecked/>
    </FormGrid><Button type="submit" disabled={mappingMutation.busy}>Опубликовать сопоставление</Button></form><MutationNotice state={mappingMutation}/></Card>
  </>;
}

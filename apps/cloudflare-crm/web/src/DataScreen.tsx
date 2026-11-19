import {useRef,useState} from "react";
import {Button,Card,FeedbackPanel,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api} from "./api";
import {PartyKind} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useReadAction,useMutation,prepareMutation,MutationNotice} from "./hooks";

export function DataScreen() {
  // New file selection supersedes all prior asynchronous file and preview results.
  const fileRevision=useRef(0);
  const [bytes,setBytes]=useState<Uint8Array>();
  const [fileName,setFileName]=useState("");
  const [fileError,setFileError]=useState("");
  const [kind,setKind]=useState("");
  const [preview,setPreview]=useState<Awaited<ReturnType<typeof api.csv.previewPartyImport>>>();
  const [previewBusy,setPreviewBusy]=useState(false);
  const exported=useReadAction<Awaited<ReturnType<typeof api.csv.previewPartyExport>>>();
  const mutation=useMutation({
    storageKey:"crm.csv-import.pending.v1",
    lookup:async({resourceId,key},signal)=>(await api.csv.getPartyImportResult({
      sourceSha256Hex:resourceId,receiptIdempotencyKey:key
    },{signal})).confirmedMutation,
    refresh:async()=>{}
  });
  return <>
    <PageHeader title="Импорт и экспорт" description="Доступно администратору. CSV-импорт создаёт новых участников; ID исходной системы не заменяют ID CRM."/>
    <Card title="Экспорт участников">
      <SelectField label="Тип экспорта" value={kind} onChange={e=>setKind(e.target.value===""?"":String(enumValue(e.target.value,[PartyKind.PERSON,PartyKind.ORGANIZATION],"Тип экспорта")))}>
        <option value="">Все</option><option value={PartyKind.PERSON}>Люди</option><option value={PartyKind.ORGANIZATION}>Организации</option>
      </SelectField>
      <p><Button disabled={exported.busy} onClick={()=>void exported.run(signal=>api.csv.previewPartyExport(kind?{kind:enumValue(kind,[PartyKind.PERSON,PartyKind.ORGANIZATION],"Тип экспорта")}:{},{signal}))}>Проверить экспорт</Button></p>
      {exported.error&&<FeedbackPanel tone="danger" title={exported.error}/>}
      {exported.data&&<p>Записей: {exported.data.count}. Предел: {exported.data.maximumTenantParties} участников в организации.</p>}
      <a href={api.csvDownloadUrl+(kind?"?kind="+(kind===String(PartyKind.PERSON)?"person":"organization"):"")}>Скачать CSV</a>
      <p>Файл содержит party_id, kind, display_name, resource_version.</p>
    </Card>
    <Card title="Импорт участников">
      <TextField label="CSV-файл" type="file" accept=".csv,text/csv" disabled={mutation.busy||mutation.pending||previewBusy} onChange={async e=>{
        const file=e.target.files?.[0],revision=++fileRevision.current;
        setBytes(undefined);setPreview(undefined);setPreviewBusy(false);
        setFileError("");mutation.changed();
        if(!file){setFileName("");return;}
        setFileName(file.name);
        if(file.size>16*1024){setFileError("Максимальный размер — 16 КиБ.");return;}
        try {
          const selectedBytes=new Uint8Array(await file.arrayBuffer());
          if(revision===fileRevision.current)setBytes(selectedBytes);
        } catch(error) {
          if(revision===fileRevision.current)setFileError(api.describeError(error));
        }
      }}/>
      <p>До 20 строк за операцию. Сначала проверьте файл, затем подтвердите создание.</p>
      <Button disabled={!bytes||previewBusy||mutation.busy||mutation.pending} onClick={async()=>{
        if(!bytes)return;
        const revision=fileRevision.current,selectedBytes=bytes;
        setPreviewBusy(true);setFileError("");
        try {
          const candidate=await api.csv.previewPartyImport({csv:selectedBytes});
          if(revision===fileRevision.current)setPreview(candidate);
        } catch(error) {
          if(revision===fileRevision.current)setFileError(api.describeError(error));
        } finally {
          if(revision===fileRevision.current)setPreviewBusy(false);
        }
      }}>Проверить импорт</Button>
      {fileError&&<FeedbackPanel tone="danger" title={fileError}/>}
      {preview&&<><p>{fileName}: строк {preview.count}, уже импортировано {preview.previouslyImported}.</p>
        <details><summary>Создаваемые ID</summary><ul>{preview.targetPartyIds.map(id=><li key={id}>{id}</li>)}</ul></details>
        <p><Button disabled={mutation.busy||mutation.pending||!bytes} onClick={()=>{
          const selectedBytes=bytes,selectedPreview=preview;
          if(!selectedBytes||!selectedPreview)return;
          const revision=fileRevision.current;
          void mutation.runIntent(()=>prepareMutation(api.csv.applyPartyImport,{
            csv:selectedBytes,expectedSha256Hex:selectedPreview.csvSha256Hex,
            precondition:{idempotencyKey:mutation.key}
          },selectedPreview.csvSha256Hex),async()=>{
            const refreshed=await api.csv.previewPartyImport({csv:selectedBytes});
            if(revision===fileRevision.current)setPreview(refreshed);
          });
        }}>Подтвердить импорт</Button></p>
      </>}<MutationNotice state={mutation}/>
    </Card>
  </>;
}

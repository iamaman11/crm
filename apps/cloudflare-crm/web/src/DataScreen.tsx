import {useState} from "react";
import {Button,Card,FeedbackPanel,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api} from "./api";
import {useReadAction,useMutation,MutationNotice} from "./hooks";

export function DataScreen() {
  const [bytes,setBytes]=useState<Uint8Array>();
  const [fileName,setFileName]=useState("");
  const [fileError,setFileError]=useState("");
  const [kind,setKind]=useState("");
  const [preview,setPreview]=useState<Awaited<ReturnType<typeof api.csv.previewPartyImport>>>();
  const [previewBusy,setPreviewBusy]=useState(false);
  const exported=useReadAction<Awaited<ReturnType<typeof api.csv.previewPartyExport>>>();
  const mutation=useMutation();
  return <>
    <PageHeader title="Импорт и экспорт" description="Доступно администратору. CSV-импорт создаёт новых участников; ID исходной системы не заменяют ID CRM."/>
    <Card title="Экспорт участников">
      <SelectField label="Тип экспорта" value={kind} onChange={e=>setKind(e.target.value)}>
        <option value="">Все</option><option value="1">Люди</option><option value="2">Организации</option>
      </SelectField>
      <p><Button disabled={exported.busy} onClick={()=>void exported.run(()=>api.csv.previewPartyExport(kind?{kind:Number(kind)}:{}))}>Проверить экспорт</Button></p>
      {exported.error&&<FeedbackPanel tone="danger" title={exported.error}/>}
      {exported.data&&<p>Записей: {exported.data.count}. Предел: {exported.data.maximumTenantParties} участников в организации.</p>}
      <a href={api.csvDownloadUrl+(kind?"?kind="+(kind==="1"?"person":"organization"):"")}>Скачать CSV</a>
      <p>Файл содержит party_id, kind, display_name, resource_version.</p>
    </Card>
    <Card title="Импорт участников">
      <TextField label="CSV-файл" type="file" accept=".csv,text/csv" disabled={mutation.busy||previewBusy} onChange={async e=>{
        const file=e.target.files?.[0];
        setBytes(undefined);setPreview(undefined);setFileError("");mutation.changed();
        if(!file){setFileName("");return;}
        setFileName(file.name);
        if(file.size>16*1024){setFileError("Максимальный размер — 16 КиБ.");return;}
        try {setBytes(new Uint8Array(await file.arrayBuffer()));}
        catch(error){setFileError(api.describeError(error));}
      }}/>
      <p>До 20 строк за операцию. Сначала проверьте файл, затем подтвердите создание.</p>
      <Button disabled={!bytes||previewBusy||mutation.busy} onClick={async()=>{
        if(!bytes)return;setPreviewBusy(true);setFileError("");
        try {setPreview(await api.csv.previewPartyImport({csv:bytes}));}
        catch(error){setFileError(api.describeError(error));}
        finally {setPreviewBusy(false);}
      }}>Проверить импорт</Button>
      {fileError&&<FeedbackPanel tone="danger" title={fileError}/>}
      {preview&&<><p>{fileName}: строк {preview.count}, уже импортировано {preview.previouslyImported}.</p>
        <details><summary>Создаваемые ID</summary><ul>{preview.targetPartyIds.map(id=><li key={id}>{id}</li>)}</ul></details>
        <p><Button disabled={mutation.busy||!bytes} onClick={()=>void mutation.run(()=>{
          if(!bytes)throw Error("Выберите файл");
          return api.csv.applyPartyImport({csv:bytes,expectedSha256Hex:preview.csvSha256Hex,
            precondition:{idempotencyKey:mutation.key}});
        },async()=>{if(bytes)setPreview(await api.csv.previewPartyImport({csv:bytes}));})}>Подтвердить импорт</Button></p>
      </>}<MutationNotice state={mutation}/>
    </Card>
  </>;
}

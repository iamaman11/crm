import {useRef,useState} from "react";
import {Button,Card,FeedbackPanel,PageHeader,SelectField,TextField} from "@ultimate-crm/ui";
import {api} from "./api";
import {PartyKind} from "@ultimate-crm/client/cloudflare";
import {enumValue} from "./enumFields";
import {useReadAction,useMutation,prepareMutation,MutationNotice} from "./hooks";

// Presentation only. Validation and error codes originate from the one Rust owner.
const csvColumnLabels:Record<string,string>={
  party_id:"ID исходной системы",kind:"Тип участника",
  display_name:"Имя или название",resource_version:"Версия исходной записи"
};
const csvErrorLabels:Record<string,string>={
  IMPORT_SOURCE_VERSION_INVALID:"Укажите положительное целое число.",
  IMPORT_DUPLICATE_OR_EMPTY_SOURCE_ID:"ID пустой или повторяется в этом файле.",
  IMPORT_KIND_INVALID:"Допустимы только person и organization.",
  IMPORT_ROW_ID_INVALID:"Недопустимый ID исходной записи.",
  IMPORT_TARGET_ID_INVALID:"Не удалось сформировать ID участника CRM.",
  IMPORT_PARTY_INVALID:"Недопустимое или пустое имя/название.",
  IMPORT_NAME_NONCANONICAL:"Уберите лишние пробелы и используйте нормализованное имя.",
  IMPORT_TARGET_PARTY_CONFLICT:"Участник с этим целевым ID уже существует и не совпадает с импортом."
};

export function DataScreen() {
  // New file selection supersedes all prior asynchronous file and preview results.
  const fileRevision=useRef(0);
  const [bytes,setBytes]=useState<Uint8Array>();
  const [fileName,setFileName]=useState("");
  const [fileError,setFileError]=useState("");
  const [kind,setKind]=useState("");
  const [preview,setPreview]=useState<Awaited<ReturnType<typeof api.csv.previewPartyImport>>>();
  const [previewBusy,setPreviewBusy]=useState(false);
  const [fileInputRevision,setFileInputRevision]=useState(0);
  const exported=useReadAction<Awaited<ReturnType<typeof api.csv.previewPartyExport>>>();
  const clearSelection=async()=>{
    fileRevision.current++;
    setFileInputRevision(revision=>revision+1);
    setBytes(undefined);setFileName("");setPreview(undefined);setFileError("");
  };
  const mutation=useMutation({
    storageKey:"crm.data.csv-import.pending.v1",
    lookup:async(marker,signal)=>{
      const result=await api.csv.previewPartyImport({
        csv:new Uint8Array(),receiptIdempotencyKey:marker.key
      },{signal});
      return result.csvSha256Hex===marker.resourceId?result.confirmedMutation:undefined;
    },
    refresh:clearSelection
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
      <TextField key={fileInputRevision} label="CSV-файл" type="file" accept=".csv,text/csv" disabled={mutation.busy||mutation.pending||previewBusy} onChange={async e=>{
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
      {preview&&<>
        <p>{fileName}: записей {preview.count}, ранее импортировано {preview.previouslyImported}.</p>
        {preview.rowIssues.length>0?<section aria-label="Ошибки CSV">
          <p role="status">Найдено ошибок: {preview.rowIssues.length}. Импорт всего файла запрещён. Исправьте CSV и проверьте повторно; участники не созданы.</p>
          <ol>{preview.rowIssues.map((issue,index)=><li key={issue.rowPosition+"-"+issue.field+"-"+index}>
            Строка данных {issue.rowPosition} · {csvColumnLabels[issue.field]??issue.field}: {csvErrorLabels[issue.code]??"Некорректное значение ("+issue.code+")"}
          </li>)}</ol>
        </section>:<>
          <details><summary>Создаваемые ID</summary><ul>{preview.targetPartyIds.map(id=><li key={id}>{id}</li>)}</ul></details>
          <p><Button disabled={mutation.busy||mutation.pending||!bytes} onClick={()=>void mutation.runIntent(()=>{
            if(!bytes)throw Error("Выберите файл");
            return prepareMutation(api.csv.applyPartyImport,{csv:bytes,expectedSha256Hex:preview.csvSha256Hex,
              precondition:{idempotencyKey:mutation.key}},preview.csvSha256Hex);
          },clearSelection)}>Подтвердить импорт</Button></p>
        </>}
      </>}<MutationNotice state={mutation}/>
    </Card>
  </>;
}

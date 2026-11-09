import React, {useState} from "react";

// The server owns data selection, canonical CSV encoding and permissions.
// This page does NOT build its own export, track shadow Party state or store files.
export default function DataExport({load,writable,loading,onSaved}){
  const [kind,setKind]=useState("all");
  const [preview,setPreview]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  const [file,setFile]=useState(null);
  const [importPreview,setImportPreview]=useState(null);
  const query="customer-data-operations/";
  async function run(action){
    if(busy||!writable)return;
    setBusy(true);setError("");setNotice("");
    try{
      if(action==="preview"){
        setPreview(await load(query+"preview?kind="+kind));
      }else{
        const {blob,headers}=await load(query+"parties.csv?kind="+kind,{responseType:"download"});
        if(preview && preview.csv_sha256!==headers.get("x-crm-export-sha256"))
          throw Error("Данные изменились после предварительного просмотра. Проверьте заново.");
        const objectUrl=URL.createObjectURL(blob);
        const a=document.createElement("a");
        a.href=objectUrl;a.download="crm-parties-"+kind+".csv";
        document.body.appendChild(a);a.click();a.remove();
        setTimeout(()=>URL.revokeObjectURL(objectUrl),1000);
        setNotice("CSV подготовлен и отправлен на скачивание.");
      }
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  async function sendImport(action){
    if(!file||busy||!writable)return;
    setBusy(true);setError("");setNotice("");
    try{
      const content=await file.arrayBuffer();
      const path=query+(action==="preview"?"import-preview":"import-apply");
      const result=await load(path,{method:"POST",headers:{
        "content-type":"text/csv",
        ...(action==="apply"&&importPreview?{"x-crm-import-sha256":importPreview.file_sha256}:{})
      },body:content});
      if(action==="preview")setImportPreview(result);
      else {await onSaved();setImportPreview(null);setNotice("Импорт выполнен. Создано: "+result.created+
        (result.replayed?" · Повтор подтверждён: "+result.replayed:""));}
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  return <section aria-label="Экспорт данных">
    <h2>Экспорт лиц и организаций</h2>
    <p>Скачать CSV из текущей D1. Доступно только администратору.
      Не более 100 записей в tenant; при превышении сервер не выдаёт неполный экспорт.</p>
    {!writable?<p>Для экспорта необходима роль администратора.</p>:<>
      <div className="field"><label htmlFor="export-kind">Тип записей</label>
        <select id="export-kind" value={kind} disabled={busy||loading}
          onChange={e=>{setKind(e.target.value);setPreview(null);setNotice("");setError("");}}>
          <option value="all">Все лица и организации</option>
          <option value="person">Только физические лица</option>
          <option value="organization">Только организации</option>
        </select></div>
      <div className="row-actions">
        <button disabled={busy||loading} onClick={()=>run("preview")}>
          Проверить экспорт</button>
        <button disabled={busy||loading||!preview} onClick={()=>run("download")}>
          Скачать CSV</button>
      </div>
      {preview&&<section aria-label="Предпросмотр экспорта">
        <h3>Подготовлено записей: {preview.count}</h3>
        <p>Поля: {preview.fields.join(", ")}</p>
        <p>Версия спецификации: <code>{preview.specification_version}</code></p>
        <p>SHA-256 CSV: <code>{preview.csv_sha256}</code></p>
      </section>}
    </>}
    {writable&&<section aria-label="Импорт данных">
      <h2>Импорт новых лиц и организаций</h2>
      <p>Только новые Party. CSV: party_id,kind,display_name,resource_version.
        До 20 строк; создание атомарное. Версия относится к источнику:
        существующие Party не изменяются. Внешний party_id — только ID источника;
        CRM создаёт собственный идентификатор. Повтор того же файла безопасен.</p>
      <div className="field"><label htmlFor="import-csv">CSV-файл</label>
        <input id="import-csv" type="file" accept=".csv,text/csv"
          disabled={busy||loading} onChange={e=>{
            setFile(e.target.files?.[0]||null);setImportPreview(null);setError("");setNotice("");
          }}/></div>
      <div className="row-actions">
        <button disabled={!file||busy||loading}
          onClick={()=>sendImport("preview")}>Проверить CSV</button>
        <button disabled={!importPreview||busy||loading}
          onClick={()=>sendImport("apply")}>Импортировать новые записи</button>
      </div>
      {importPreview&&<p role="status">Файл проверен. Строк: {importPreview.count}.
        SHA-256: <code>{importPreview.file_sha256}</code></p>}
    </section>}
    {error&&<p className="notice error" role="alert">{error}</p>}
    {notice&&<p className="notice" role="status">{notice}</p>}
    <p>Экспорт формируется непосредственно из действующих Rust Party.
      Долгие задания экспорта и сохраняемые задания импорта ещё не подключены.</p>
  </section>;
}

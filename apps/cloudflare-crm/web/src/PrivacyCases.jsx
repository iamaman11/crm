import React,{useEffect,useState} from "react";

// Privacy request intake, NOT subject verification or destructive execution.
export default function PrivacyCases({load,writable,loading}){
  const [items,setItems]=useState([]);
  const [kind,setKind]=useState("access");
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  async function refresh(){setItems((await load("customer-privacy/cases")).items);}
  // A successful write is definitive; a subsequent list-read failure is not a write failure.
  async function refreshAfterMutation(message){
    setNotice(message);
    try {await refresh();}
    catch(e){setError("Заявка сохранена, но список не обновился. "+
      "Показанные данные могут быть устаревшими. Нажмите «Обновить заявки». "+e.message);}
  }
  useEffect(()=>{refresh().catch(e=>setError(e.message));},[]);
  async function create(){
    if(busy||!writable)return;
    setBusy(true);setError("");setNotice("");
    try{
      await load("customer-privacy/cases",{method:"POST",
        headers:{"idempotency-key":crypto.randomUUID()},
        body:JSON.stringify({id:crypto.randomUUID(),kind})});
      await refreshAfterMutation("Черновик заявки зарегистрирован");
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  async function move(item,action){
    if(busy||!writable)return;
    setBusy(true);setError("");setNotice("");
    try{
      await load("customer-privacy/cases/"+item.id+"/"+action,{
        method:"POST",headers:{"idempotency-key":crypto.randomUUID()},
        body:JSON.stringify({expected_version:item.version})});
      await refreshAfterMutation(action==="submit"?"Заявка отправлена":"Заявка отменена");
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  return <section aria-label="Заявки на приватность">
    <h2>Заявки на приватность</h2>
    <p>Начальная регистрация запросов. Личность субъекта ещё не проверяется;
      отправка заявки не запускает удаление, экспорт или ограничения обработки.</p>
    {writable&&<div className="row-actions">
      <div className="field"><label htmlFor="privacy-kind">Тип заявки</label>
        <select id="privacy-kind" value={kind} disabled={busy||loading}
          onChange={e=>setKind(e.target.value)}>
          <option value="access">Запрос доступа</option>
          <option value="portability_export">Запрос переноса данных</option>
          <option value="restrict_processing">Запрос ограничения обработки</option>
          <option value="erasure">Запрос удаления</option>
        </select></div>
      <button disabled={busy||loading} onClick={create}>Создать заявку</button>
    </div>}
    <button className="ghost" disabled={busy||loading}
      onClick={()=>{setError("");refresh().catch(e=>
        setError("Не удалось обновить заявки. Показанные данные могут быть устаревшими. "+
          e.message));}}>Обновить заявки</button>
    {error&&<p role="alert" className="notice error">{error}</p>}
    {notice&&<p role="status" className="notice">{notice}</p>}
    <div className="table-scroll"><table><thead><tr>
      <th>Заявка</th><th>Тип</th><th>Статус</th><th>Действия</th>
    </tr></thead><tbody>{items.map(item=><tr key={item.id}>
      <td>{item.id}</td><td>{item.kind}</td><td>{item.status}</td>
      <td>{writable&&<>
        {item.status==="draft"&&<button disabled={busy||loading}
          onClick={()=>move(item,"submit")}>Отправить заявку</button>}
        {["draft","submitted"].includes(item.status)&&
          <button disabled={busy||loading}
            onClick={()=>move(item,"cancel")}>Отменить заявку</button>}
      </>}</td>
    </tr>)}</tbody></table></div>
    <p>Subject verification, scope planning, restriction enforcement и
      erasure execution — NOT_CONNECTED.</p>
  </section>;
}

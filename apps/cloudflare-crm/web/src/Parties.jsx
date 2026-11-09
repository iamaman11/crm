import React, {useEffect,useState} from "react";

// Original crm-parties Rust module owns identity and all its business rules.
// The React component handles only user input and HTTP transport.
const newForm=()=>({id:crypto.randomUUID(),key:crypto.randomUUID(),
  kind:"organization",name:"",expected_version:0});

export default function Parties({load,writable=true,onSaved}) {
  const [items,setItems]=useState([]);
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [form,setForm]=useState(null);
  const [query,setQuery]=useState("");
  const [notice,setNotice]=useState("");
  const [error,setError]=useState("");

  async function reload(){
    setLoading(true);
    try {
      const result=await load("parties");
      setItems(result.items);
      setError("");
    } catch(e) {setError(e.message);}
    finally {setLoading(false);}
  }
  useEffect(()=>{reload();},[]);
  function edit(item){
    setForm({id:item.id,key:crypto.randomUUID(),name:item.name,
      kind:item.kind,expected_version:item.version});
    setNotice("");setError("");
  }
  async function submit(event){
    event.preventDefault();
    if(busy)return;
    setBusy(true);setError("");setNotice("");
    try {
      const updating=form.expected_version>0;
      const path="parties"+(updating?"/"+form.id:"");
      const data=updating?{name:form.name,expected_version:form.expected_version}:
        {id:form.id,name:form.name,kind:form.kind};
      const result=await load(path,{method:updating?"PATCH":"POST",
        headers:{"idempotency-key":form.key},body:JSON.stringify(data)});
      setForm(null);
      await reload();
      if(onSaved)await onSaved();
      // The success message is the UI consistency barrier: the parent
      // Account/Contact selectors must already contain this Party revision.
      setNotice(result.replayed?"Повтор выполнен без дублирования":"Сохранено через Rust / D1");
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  const filtered=items.filter(x=>
    (x.name+" "+x.kind).toLocaleLowerCase("ru").includes(query.toLocaleLowerCase("ru")));
  return <section aria-label="Лица и организации (Rust)">
    <div className="toolbar"><input aria-label="Поиск лиц и организаций"
      placeholder="Найти человека или компанию" value={query}
      onChange={e=>setQuery(e.target.value)}/>
      {writable&&<button onClick={()=>{setForm(newForm());setError("");}}>
        + Добавить лицо / организацию
      </button>}
      <button className="ghost" onClick={reload} disabled={loading}>Обновить</button>
    </div>
    <p>Лица и организации обрабатываются оригинальным Rust-модулем. Клиентские аккаунты связываются с ними в разделе «Клиенты».</p>
    {notice&&<div role="status" className="notice">{notice}</div>}
    {error&&<div role="alert" className="notice error">{error}</div>}
    {loading?<p>Загрузка…</p>:filtered.length===0?<p className="empty">Записей нет</p>:
      <div className="table-scroll"><table><thead><tr>
        <th>Имя / название</th><th>Тип</th><th>Версия</th><th>Действия</th>
      </tr></thead><tbody>{filtered.map(item=><tr key={item.id}>
        <td>{item.name}</td>
        <td>{item.kind==="organization"?"Компания":"Частное лицо"}</td>
        <td>{item.version}</td>
        <td>{writable&&<button className="ghost" onClick={()=>edit(item)}>Изменить</button>}</td>
      </tr>)}</tbody></table></div>}
    {form&&<div className="shade" onClick={e=>{
      if(e.currentTarget===e.target&&!busy)setForm(null);
    }}><form className="dialog" onSubmit={submit}>
      <h2>{form.expected_version?"Изменить":"Добавить"} лицо / организацию</h2>
      {!form.expected_version&&<div className="field"><label htmlFor="rust-kind">Тип</label>
        <select id="rust-kind" value={form.kind}
          onChange={e=>setForm({...form,kind:e.target.value})}>
          <option value="organization">Компания</option>
          <option value="person">Частное лицо</option>
        </select></div>}
      <div className="field"><label htmlFor="rust-name">Имя / название</label>
        <input id="rust-name" required value={form.name}
          onChange={e=>setForm({...form,name:e.target.value})}/></div>
      <footer><button type="button" className="ghost" disabled={busy}
          onClick={()=>setForm(null)}>Отмена</button>
        <button disabled={busy}>{busy?"Сохранение…":"Сохранить"}</button>
      </footer>
    </form></div>}
  </section>;
}

import React, {useState} from "react";

// Original crm-party-relationships owns rules and versions; this screen owns
// only form input and display. App is the single owner of the D1 list.
const fresh=()=>({
  id:crypto.randomUUID(),key:crypto.randomUUID(),expected_version:0,
  from_party_id:"",to_party_id:"",code:"employment",directionality:"directional",
  from_role:"employer",to_role:"employee",status:"active",
  valid_from_unix_nanos:null,valid_until_unix_nanos:null
});

export default function Relationships({items,parties,load,onSaved,writable,loading}){
  const [form,setForm]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  const [query,setQuery]=useState("");
  const partyName=id=>parties.find(p=>p.id===id)?.name||id;
  const label=item=>partyName(item.from_party_id)+" → "+partyName(item.to_party_id);
  const change=(name,value)=>setForm(f=>({...f,[name]:value}));
  const start=(item=null)=>{
    setError("");setNotice("");
    setForm(item?{
      id:item.id,key:crypto.randomUUID(),expected_version:item.version,
      from_party_id:item.from_party_id,to_party_id:item.to_party_id,
      code:item.type.code,directionality:item.type.directionality,
      from_role:item.type.from_role,to_role:item.type.to_role,status:item.status,
      valid_from_unix_nanos:item.valid_from_unix_nanos,
      valid_until_unix_nanos:item.valid_until_unix_nanos
    }:fresh());
  };
  async function refresh(){
    setError("");
    try{await onSaved();}
    catch(e){setError(e.message);}
  }
  async function submit(event){
    event.preventDefault();
    if(busy||!writable||!form)return;
    setBusy(true);setError("");setNotice("");
    try{
      const updating=form.expected_version>0;
      const payload=updating?
        {expected_version:form.expected_version,status:form.status,
          valid_from_unix_nanos:form.valid_from_unix_nanos,
          valid_until_unix_nanos:form.valid_until_unix_nanos}:
        {id:form.id,from_party_id:form.from_party_id,to_party_id:form.to_party_id,
          code:form.code,directionality:form.directionality,
          from_role:form.from_role,to_role:form.to_role,
          valid_from_unix_nanos:null,valid_until_unix_nanos:null};
      const result=await load("party-relationships"+(updating?"/"+form.id:""),{
        method:updating?"PATCH":"POST",
        headers:{"idempotency-key":form.key},body:JSON.stringify(payload)
      });
      // Never show a green success with a stale Party/Relationship projection.
      await onSaved();
      setForm(null);
      setNotice(result.replayed?"Повтор выполнен без дублирования":"Сохранено");
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  const filtered=items.filter(item=>
    (label(item)+" "+item.type.code+" "+item.status)
      .toLocaleLowerCase("ru").includes(query.toLocaleLowerCase("ru")));
  const field=(title,name,options)=> <div className="field" key={name}>
    <label htmlFor={"rel-"+name}>{title}</label>
    {options?<select id={"rel-"+name} required value={form[name]}
      onChange={e=>change(name,e.target.value)}>
      {options.map(([value,title])=><option key={value} value={value}>{title}</option>)}
    </select>:<input id={"rel-"+name} required value={form[name]}
      onChange={e=>change(name,e.target.value)}/>}
  </div>;
  const choices=[["","Выберите лицо / организацию"],
    ...parties.map(p=>[p.id,p.name])];
  const describe=item=><>
    <p>Тип: {item.type.code} ({item.type.from_role} → {item.type.to_role})</p>
    <p>Направление: {item.type.directionality==="directional"?"Направленная":"Взаимная"}</p>
    <p>Статус: {item.status==="active"?"Активна":"Неактивна"} · Версия {item.version}</p>
  </>;
  return <section aria-label="Связи лиц и организаций">
    <div className="toolbar">
      <input aria-label="Поиск связей" placeholder="Найти связь" value={query}
        onChange={e=>setQuery(e.target.value)}/>
      {writable&&<button disabled={parties.length<2||busy}
        onClick={()=>start()}>+ Добавить связь</button>}
      <button className="ghost" disabled={loading||busy} onClick={refresh}>Обновить</button>
    </div>
    {parties.length<2&&<p>Для связи нужны два лица или организации из раздела «Лица и организации».</p>}
    {notice&&<div role="status" className="notice">{notice}</div>}
    {error&&<div role="alert" className="notice error">{error}</div>}
    {loading?<p className="empty">Загрузка…</p>:filtered.length===0?
      <p className="empty">Пока связей нет</p>:<>
      <div className="table-scroll"><table><thead><tr>
        <th>Участники</th><th>Тип и роли</th><th>Статус</th><th>Действия</th>
      </tr></thead><tbody>{filtered.map(item=><tr key={item.id}>
        <td>{label(item)}</td>
        <td>{item.type.code}: {item.type.from_role} → {item.type.to_role}</td>
        <td>{item.status==="active"?"Активна":"Неактивна"} · v{item.version}</td>
        <td>{writable&&<button className="ghost" onClick={()=>start(item)}>Изменить</button>}</td>
      </tr>)}</tbody></table></div>
      <div className="mobile">{filtered.map(item=><article key={item.id}>
        <strong>{label(item)}</strong>{describe(item)}
        {writable&&<button className="ghost" onClick={()=>start(item)}>Изменить</button>}
      </article>)}</div>
    </>}
    {form&&<div className="shade" onClick={e=>{
      if(e.currentTarget===e.target&&!busy)setForm(null);
    }}><form className="dialog" onSubmit={submit}>
      <h2>{form.expected_version?"Изменить связь":"Добавить связь"}</h2>
      {!form.expected_version?<>
        {field("От кого","from_party_id",choices)}
        {field("К кому","to_party_id",choices)}
        {field("Код связи","code")}
        {field("Направление","directionality",[
          ["directional","Направленная"],["reciprocal","Взаимная"]])}
        {field("Роль первого участника","from_role")}
        {field("Роль второго участника","to_role")}
      </>:<>
        <p>{partyName(form.from_party_id)} → {partyName(form.to_party_id)}</p>
        {field("Статус связи","status",[
          ["active","Активна"],["inactive","Неактивна"]])}
      </>}
      <footer>
        <button type="button" className="ghost" disabled={busy}
          onClick={()=>setForm(null)}>Отмена</button>
        <button disabled={busy}>{busy?"Сохранение…":"Сохранить"}</button>
      </footer>
    </form></div>}
  </section>;
}

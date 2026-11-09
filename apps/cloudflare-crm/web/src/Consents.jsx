import React, {useState} from "react";

// Only UI and transport live here. Original crm-consents Rust owns authorization.
// A verified operator may record a DENY, never invent a subject GRANT.
const initial=()=>({party_id:"",contact_point_id:"",purpose:"marketing.newsletter",
  channel:"email",jurisdiction:"eu"});
const channels=[["email","Электронная почта"],["phone","Телефон"],
  ["sms","SMS"],["postal","Почта"],["messaging","Мессенджер"],["push","Push"]];
const reasons={
  active_grant:"Есть подтверждённое согласие",
  active_deny:"Есть действующий запрет",
  withdrawn:"Согласие отозвано",
  no_applicable_grant:"Нет подтверждённого согласия"
};
const payload=s=>({
  party_id:s.party_id,contact_point_id:s.contact_point_id||null,
  purpose:s.purpose.trim(),channel:s.channel
});

export default function Consents({items,parties,contacts,load,onSaved,writable,loading}){
  const [scope,setScope]=useState(initial);
  const [decision,setDecision]=useState(null);
  const [pending,setPending]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  const [query,setQuery]=useState("");
  const partyName=id=>parties.find(p=>p.id===id)?.name||id;
  const contactName=id=>contacts.find(c=>c.id===id)?.value||id||"Все контакты";
  const related=contacts.filter(c=>c.party_id===scope.party_id);
  const change=(name,value)=>{
    setScope(s=>name==="party_id"?
      {...s,party_id:value,contact_point_id:""}:{...s,[name]:value});
    setDecision(null);setNotice("");setError("");
  };
  async function check(value=scope){
    const result=await load("consents/evaluate",{
      method:"POST",body:JSON.stringify(payload(value))
    });
    setDecision(result);
    return result;
  }
  async function evaluate(event){
    event.preventDefault();
    if(!scope.party_id||!scope.purpose.trim()||busy)return;
    setBusy(true);setError("");setNotice("");setDecision(null);
    try{await check();}
    catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  function confirm(){
    if(!writable||busy||!scope.party_id||!scope.purpose.trim())return;
    setPending({...scope,id:crypto.randomUUID(),key:crypto.randomUUID()});
    setError("");setNotice("");
  }
  async function suppress(event){
    event.preventDefault();
    if(!writable||!pending||busy)return;
    setBusy(true);setError("");setNotice("");
    try{
      const result=await load("consents/suppress",{
        method:"POST",headers:{"idempotency-key":pending.key},
        body:JSON.stringify({...payload(pending),id:pending.id,
          jurisdiction:pending.jurisdiction})
      });
      if(result.item?.effect!=="deny"||
        (!result.replayed&&result.subject_consent_evidence!==false)||
        result.subject_consent_evidence===true)
        throw Error("Некорректное подтверждение запрета оператором");
      await onSaved();
      await check(pending);
      setPending(null);
      setNotice(result.replayed?
        "Запрет уже был сохранён; повтор не создал запись":"Запрет сохранён");
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  async function refresh(){
    setError("");
    try{await onSaved();setDecision(null);}
    catch(e){setError(e.message);}
  }
  const filtered=items.filter(item=>
    (partyName(item.party_id)+" "+contactName(item.contact_point_id)+" "+
      item.purpose+" "+item.channel+" "+item.status)
      .toLocaleLowerCase("ru").includes(query.toLocaleLowerCase("ru")));
  const select=(title,name,options,required=true)=><div className="field" key={name}>
    <label htmlFor={"consent-"+name}>{title}</label>
    <select id={"consent-"+name} required={required} value={scope[name]}
      onChange={e=>change(name,e.target.value)}>
      {options.map(([value,label])=><option key={value} value={value}>{label}</option>)}
    </select>
  </div>;
  return <section aria-label="Разрешения коммуникации">
    <p>Проверка выполняется оригинальным Rust-модулем. При отсутствии достоверного
      согласия коммуникация запрещена. Оператор может зафиксировать запрет,
      но не может выдать согласие за человека.</p>
    <form onSubmit={evaluate}>
      <div className="consent-scope">
        {select("Лицо / организация","party_id",[
          ["","Выберите лицо / организацию"],...parties.map(p=>[p.id,p.name])])}
        {select("Контакт (необязательно)","contact_point_id",[
          ["","Все контакты"],...related.map(c=>[c.id,c.value])],false)}
        <div className="field">
          <label htmlFor="consent-purpose">Цель коммуникации</label>
          <input id="consent-purpose" required value={scope.purpose}
            onChange={e=>change("purpose",e.target.value)}/>
        </div>
        {select("Канал","channel",channels)}
        {writable&&<div className="field">
          <label htmlFor="consent-jurisdiction">Юрисдикция запрета</label>
          <input id="consent-jurisdiction" required value={scope.jurisdiction}
            onChange={e=>change("jurisdiction",e.target.value)}/>
        </div>}
      </div>
      <div className="row-actions">
        <button disabled={busy||loading||!scope.party_id||!scope.purpose.trim()}>
          Проверить разрешение</button>
        {writable&&<button type="button" className="ghost"
          disabled={busy||loading||!scope.party_id||!scope.purpose.trim()}
          onClick={confirm}>Запретить коммуникацию</button>}
        <button type="button" className="ghost" disabled={busy||loading}
          onClick={refresh}>Обновить</button>
      </div>
    </form>
    {decision&&<div role="status" className={decision.allowed?"notice":"notice error"}>
      Результат проверки: {decision.allowed?"Разрешено":"Запрещено"}.
      {" "+(reasons[decision.reason]||decision.reason)}
      {decision.grant_ingress==="NOT_CONNECTED"&&
        " · Источник подтверждённых согласий ещё не подключён."}
    </div>}
    {notice&&<div role="status" className="notice">{notice}</div>}
    {error&&<div role="alert" className="notice error">{error}</div>}
    <div className="toolbar">
      <input aria-label="Поиск запретов" placeholder="Поиск запретов"
        value={query} onChange={e=>setQuery(e.target.value)}/>
    </div>
    <h2>Зарегистрированные решения</h2>
    {loading?<p className="empty">Загрузка…</p>:filtered.length===0?
      <p className="empty">Записей нет</p>:<>
      <div className="table-scroll"><table><thead><tr>
        <th>Лицо / организация</th><th>Контакт</th><th>Цель / канал</th><th>Решение</th>
      </tr></thead><tbody>{filtered.map(item=><tr key={item.id}>
        <td>{partyName(item.party_id)}</td>
        <td>{contactName(item.contact_point_id)}</td>
        <td>{item.purpose} · {item.channel}</td>
        <td>{item.effect==="deny"?"Запрет":"Согласие"} · {item.status}</td>
      </tr>)}</tbody></table></div>
      <div className="mobile">{filtered.map(item=><article key={item.id}>
        <strong>{partyName(item.party_id)}</strong>
        <p>{contactName(item.contact_point_id)}</p>
        <p>{item.purpose} · {item.channel}</p>
        <p>{item.effect==="deny"?"Запрет":"Согласие"} · {item.status}</p>
      </article>)}</div>
    </>}
    {pending&&<div className="shade" onClick={e=>{
      if(e.currentTarget===e.target&&!busy)setPending(null);
    }}><form className="dialog" onSubmit={suppress}>
      <h2>Подтвердить запрет</h2>
      <p>Запретить коммуникацию: {partyName(pending.party_id)}
        {pending.contact_point_id?" · "+contactName(pending.contact_point_id):""},
        {" "+pending.purpose} / {pending.channel}?</p>
      <p>Это действие оператора, а не подтверждённое согласие человека.</p>
      <footer>
        <button type="button" className="ghost" disabled={busy}
          onClick={()=>setPending(null)}>Отмена</button>
        <button disabled={busy}>{busy?"Сохранение…":"Подтвердить запрет"}</button>
      </footer>
    </form></div>}
  </section>;
}

import React, {useEffect, useState} from "react";
import {createRoot} from "react-dom/client";
import "./styles.css";
import Customer360 from "./Customer360.jsx";
import Parties from "./Parties.jsx";
import Relationships from "./Relationships.jsx";
import Consents from "./Consents.jsx";
import IdentityResolution from "./IdentityResolution.jsx";
import DataQuality from "./DataQuality.jsx";
import DataExport from "./DataExport.jsx";

// A single React UI for ORIGINAL Rust owners. No pilot CRUD, JS business rules,
// shadow entities or client-side authorization. All commands are typed at the
// native Worker boundary, and all state survives via the single Cloudflare D1.
const sections = {
 accounts: {title:"Клиенты",one:"клиента",route:"accounts"},
 contacts: {title:"Контакты",one:"контакт",route:"contact-points"},
 deals: {title:"Сделки",one:"сделку",route:"sales-deals"},
 tasks: {title:"Задачи",one:"задачу",route:"activity-tasks"}
};
const stages=[["lead","Новая",1],["qualified","В работе",2],
  ["proposal","Предложение",3],["won","Выиграна",4],["lost","Проиграна",4]];
const stageTitle=id=>stages.find(s=>s[0]===id)?.[1]||id;
const id=()=>crypto.randomUUID();
const money=x=>x?x.minor_units+" "+x.currency:"—";
const taskAccount=task=>task?.related_resources?.find(x=>x.resource_type==="crm.customer-accounts")?.resource_id;
const keyHeaders=key=>({"idempotency-key":key});
const limitMoney=value=>{
  if(!/^(0|[1-9][0-9]*)$/.test(value)||!Number.isSafeInteger(Number(value)))
    throw Error("Сумма должна быть целым количеством минимальных денежных единиц");
  return Number(value);
};
async function api(path,options={}){
  const {responseType,...fetchOptions}=options;
  const response=await fetch("/api/"+path,{...fetchOptions,headers:{
    "content-type":"application/json",...(fetchOptions.headers||{})}});
  if(!response.ok){
    const body=await response.json().catch(()=>({}));
    const code=response.headers.get("x-crm-error-code")||body.code||"HTTP_"+response.status;
    const requestId=response.headers.get("x-request-id");
    throw Error((body.error||"Ошибка "+response.status)+" ["+code+(requestId?"; запрос "+requestId:"")+"]");
  }
  if(responseType==="download")return {blob:await response.blob(),headers:response.headers};
  return response.status===204?null:response.json();
}
function App(){
  const [tab,setTab]=useState("accounts");
  const [items,setItems]=useState({parties:[],relationships:[],consents:[],accounts:[],contacts:[],deals:[],tasks:[]});
  const [role,setRole]=useState(null);
  const [form,setForm]=useState(null);
  const [card,setCard]=useState(null);
  const [search,setSearch]=useState("");
  const [view,setView]=useState("table");
  const [busy,setBusy]=useState(false);
  const [loading,setLoading]=useState(true);
  const [notice,setNotice]=useState("");
  const [error,setError]=useState("");
  const writable=role==="admin";
  async function reload(){
    const [parties,relationships,consents,accounts,contacts,deals,tasks,session]=await Promise.all([
      api("parties"),api("party-relationships"),api("consents"),api("accounts"),
      api("contact-points"),api("sales-deals"),api("activity-tasks"),api("session")]);
    setItems({parties:parties.items,relationships:relationships.items,consents:consents.items,
      accounts:accounts.items,contacts:contacts.items,deals:deals.items,tasks:tasks.items});
    setRole(session.role);
  }
  useEffect(()=>{reload().catch(e=>setError(e.message)).finally(()=>setLoading(false));},[]);
  const partyName=value=>items.parties.find(p=>p.id===value)?.name||"—";
  const accountName=value=>items.accounts.find(a=>a.id===value)?.name||"—";
  const contactsParty=account=>account?.parties?.find(p=>p.role==="primary")?.id;
  const itemName=(kind,row)=>kind==="contacts"?row.value:
    kind==="tasks"?row.subject:row.name;
  function begin(kind,record=null,mode="edit"){
    setTab(kind);setCard(null);setNotice("");setError("");
    const f={kind,mode:record?mode:"create",id:record?.id||id(),key:id(),
      expected_version:record?.version||0};
    if(kind==="accounts")Object.assign(f,{name:record?.name||"",party_id:contactsParty(record)||"",
      status:record?.status||"active"});
    if(kind==="contacts")Object.assign(f,{party_id:record?.party_id||"",value:record?.value||"",
      contact_kind:record?.kind||"email",status:record?.status||"active",
      preferred:record?.preferred||false});
    if(kind==="deals")Object.assign(f,{name:record?.name||"",account_id:record?.account_id||"",
      stage_id:mode==="advance"?stages.find(stage=>stage[2]===(record?.stage?.ordinal||0)+1)?.[0]||record?.stage?.stage_id:"lead",
      amount_minor_units:record?.amount?.minor_units||"",
      currency:record?.amount?.currency||"USD",
      probability_bps:record?.probability_bps??2500,
      close_reason_code:""});
    if(kind==="tasks")Object.assign(f,{subject:record?.subject||"",
      account_id:taskAccount(record)||"",priority:record?.priority||"normal",
      due_at:record?.due_at_unix_nanos
        ?new Date(Number(record.due_at_unix_nanos)/1e6).toISOString().slice(0,10):""});
    setForm(f);
  }
  const change=(name,value)=>setForm(f=>({...f,[name]:value}));
  async function submit(event){
    event.preventDefault();if(!form||busy||!writable)return;
    setBusy(true);setError("");setNotice("");
    try{
      let path=sections[form.kind].route,method="POST",data;
      if(form.kind==="accounts"){
        if(form.mode==="create")data={id:form.id,name:form.name,party_id:form.party_id};
        else{path+="/"+form.id;method="PATCH";
          data={name:form.name,status:form.status,expected_version:form.expected_version};}
      }else if(form.kind==="contacts"){
        if(form.mode==="create")data={id:form.id,party_id:form.party_id,
          kind:form.contact_kind,value:form.value,preferred:form.preferred,
          valid_from_unix_nanos:null,valid_until_unix_nanos:null};
        else{path+="/"+form.id;method="PATCH";
          data={expected_version:form.expected_version,value:form.value,
            status:form.status,preferred:form.preferred,
            valid_from_unix_nanos:null,valid_until_unix_nanos:null};}
      }else if(form.kind==="deals"){
        if(form.mode==="advance"){
          const stage=stages.find(s=>s[0]===form.stage_id);
          path+="/"+form.id+"/advance";
          data={expected_version:form.expected_version,
            pipeline_id:"pipeline.enterprise",stage_id:stage[0],ordinal:stage[2],
            status:["won","lost"].includes(stage[0])?stage[0]:"open",
            close_reason_code:["won","lost"].includes(stage[0])?form.close_reason_code:null};
        }else if(form.mode==="create"){
          data={id:form.id,name:form.name,account_id:form.account_id,
            pipeline_id:"pipeline.enterprise",stage_id:"lead",ordinal:1,
            amount_minor_units:form.amount_minor_units?limitMoney(form.amount_minor_units):null,
            currency:form.amount_minor_units?form.currency:null,
            probability_bps:Number(form.probability_bps)};
        }else{
          path+="/"+form.id;method="PATCH";
          data={expected_version:form.expected_version,name:form.name,
            probability_bps:Number(form.probability_bps),
            ...(form.amount_minor_units?{
              amount_minor_units:limitMoney(form.amount_minor_units),currency:form.currency
            }:{clear_amount:true})};
        }
      }else{
        if(form.mode==="complete"){path+="/"+form.id+"/complete";
          data={expected_version:form.expected_version};}
        else data={id:form.id,subject:form.subject,description:null,
          account_id:form.account_id,priority:form.priority,
          due_at_unix_nanos:form.due_at?Date.parse(form.due_at+"T00:00:00Z")*1e6:null,
          reminder_at_unix_nanos:null};
      }
      const saved=await api(path,{method,headers:keyHeaders(form.key),body:JSON.stringify(data)});
      await reload();
      setForm(null);
      setNotice(saved.replayed?"Повтор выполнен без дублирования":"Сохранено");
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  function navigate(kind){
    setTab(kind);setCard(null);setSearch("");setForm(null);setError("");setNotice("");
  }
  const kind=sections[tab],rows=(items[tab]||[]).filter(row=>
    JSON.stringify(row).toLocaleLowerCase("ru").includes(search.toLocaleLowerCase("ru")));
  function actions(row){
    return <span className="row-actions">
      {tab==="accounts"&&<button className="ghost" onClick={()=>setCard(row.id)}>Карточка</button>}
      {writable&&tab!=="tasks"&&<button className="ghost" onClick={()=>begin(tab,row)}>Изменить</button>}
      {writable&&tab==="deals"&&row.status==="open"&&
        <button className="ghost" onClick={()=>begin(tab,row,"advance")}>Этап сделки</button>}
      {writable&&tab==="tasks"&&row.status==="open"&&
        <button className="ghost" onClick={()=>begin(tab,row,"complete")}>Выполнить</button>}
    </span>;
  }
  const columns={
    accounts:[["name","Название",row=>row.name],
      ["party","Лицо / организация",row=>partyName(contactsParty(row))],
      ["status","Статус",row=>row.status]],
    contacts:[["value","Контакт",row=>row.value],
      ["kind","Тип",row=>row.kind],["party","Лицо / организация",row=>partyName(row.party_id)],
      ["verified","Подтверждён",row=>row.verified?"Да":"Нет"]],
    deals:[["name","Сделка",row=>row.name],
      ["account","Клиент",row=>accountName(row.account_id)],
      ["stage","Этап",row=>stageTitle(row.stage.stage_id)],
      ["amount","Сумма",row=>money(row.amount)]],
    tasks:[["subject","Задача",row=>row.subject],
      ["account","Клиент",row=>accountName(taskAccount(row))],
      ["priority","Приоритет",row=>row.priority],
      ["status","Статус",row=>row.status==="completed"?"Готово":"В работе"]]
  };
  const field=(label,name,type="text",choices)=> <div className="field" key={name}>
    <label htmlFor={"native-"+name}>{label}</label>
    {choices?<select id={"native-"+name} required value={form[name]}
      onChange={e=>change(name,e.target.value)}>
      {choices.map(([key,title])=><option value={key} key={key}>{title}</option>)}
    </select>:<input id={"native-"+name} type={type} required={name!=="amount_minor_units"}
      value={form[name]} onChange={e=>change(name,e.target.value)}/>}
  </div>;
  const partyChoices=[["","Выберите лицо / организацию"],
    ...items.parties.map(p=>[p.id,p.name])];
  const accountChoices=[["","Выберите клиента"],
    ...items.accounts.map(a=>[a.id,a.name])];
  function fields(){
    if(form.kind==="accounts")return <>
      {field("Название","name")}
      {form.mode==="create"?field("Лицо / организация","party_id","text",partyChoices):
        field("Статус","status","text",[["active","Активный"],["inactive","Неактивный"]])}
    </>;
    if(form.kind==="contacts")return <>
      {form.mode==="create"&&field("Лицо / организация","party_id","text",partyChoices)}
      {form.mode==="create"&&field("Тип контакта","contact_kind","text",
        [["email","Электронная почта"],["phone","Телефон"],
          ["web","Сайт"],["postal","Адрес"],["messaging","Мессенджер"]])}
      {field("Значение","value")}
      <label><input type="checkbox" checked={form.preferred}
        onChange={e=>change("preferred",e.target.checked)}/> Предпочтительный</label>
      {form.mode!=="create"&&field("Статус","status","text",
        [["active","Активный"],["inactive","Неактивный"]])}
    </>;
    if(form.kind==="deals"){
      if(form.mode==="advance")return <>
        <p>Текущая стадия: {stageTitle(items.deals.find(d=>d.id===form.id)?.stage.stage_id)}</p>
        {field("Новый этап","stage_id","text",stages.map(s=>[s[0],s[1]]))}
        {["won","lost"].includes(form.stage_id)&&field("Причина закрытия (код)","close_reason_code")}
      </>;
      return <>{field("Название сделки","name")}
        {form.mode==="create"&&field("Клиент","account_id","text",accountChoices)}
        {field("Сумма (минимальные единицы)","amount_minor_units","text")}
        {field("Валюта","currency","text",[["USD","USD"],["EUR","EUR"],["PLN","PLN"]])}
        {field("Вероятность (0–10000)","probability_bps","number")}
      </>;
    }
    if(form.mode==="complete")return <p>Отметить задачу выполненной?</p>;
    return <>{field("Что сделать","subject")}
      {field("Клиент","account_id","text",accountChoices)}
      {field("Приоритет","priority","text",
        [["low","Низкий"],["normal","Обычный"],["high","Высокий"],["urgent","Срочный"]])}
      <div className="field"><label htmlFor="native-due">Срок (необязательно)</label>
        <input id="native-due" type="date" value={form.due_at}
          onChange={e=>change("due_at",e.target.value)}/></div>
    </>;
  }
  return <><header><strong>CRM</strong><span>Один Rust Worker · одна D1</span></header>
    <main><div className="heading"><div><h1>Моя CRM</h1>
      <p>Клиенты, контакты, сделки и задачи — оригинальные модули Rust</p></div>
      <button className="ghost" disabled={loading} onClick={()=>reload().then(()=>{
        setNotice("Обновлено");setError("");
      }).catch(e=>setError(e.message))}>Обновить</button></div>
      <nav aria-label="Разделы">{Object.entries(sections).map(([key,section])=>
        <button key={key} className={tab===key?"active":"ghost"}
          onClick={()=>navigate(key)}>{section.title} <small>{items[key].length}</small></button>)}
        <button className={tab==="parties"?"active":"ghost"} onClick={()=>navigate("parties")}>
          Лица и организации <small>{items.parties.length}</small></button>
        <button className={tab==="relationships"?"active":"ghost"}
          onClick={()=>navigate("relationships")}>
          Связи <small>{items.relationships.length}</small></button>
        <button className={tab==="consents"?"active":"ghost"}
          onClick={()=>navigate("consents")}>
          Разрешения <small>{items.consents.length}</small></button>
        <button className={tab==="identity-resolution"?"active":"ghost"}
          onClick={()=>navigate("identity-resolution")}>Сравнение записей</button>
        <button className={tab==="data-quality"?"active":"ghost"}
          onClick={()=>navigate("data-quality")}>Качество данных</button>
        <button className={tab==="data-export"?"active":"ghost"}
          onClick={()=>navigate("data-export")}>Экспорт данных</button></nav>
      {notice&&<div role="status" className="notice">{notice}</div>}
      {error&&<div role="alert" className="notice error">{error}</div>}
      <section className="panel">{tab==="parties"?
        <Parties items={items.parties} load={api} writable={writable} loading={loading} onSaved={reload}/>:
        tab==="relationships"?<Relationships items={items.relationships}
          parties={items.parties} load={api} writable={writable} loading={loading}
          onSaved={reload}/>:
        tab==="identity-resolution"?<IdentityResolution parties={items.parties}
          load={api} loading={loading}/>:
        tab==="data-quality"?<DataQuality parties={items.parties}
          load={api} loading={loading}/>:
        tab==="data-export"?<DataExport load={api} writable={writable} loading={loading} onSaved={reload}/>:
        tab==="consents"?<Consents items={items.consents}
          parties={items.parties} contacts={items.contacts}
          load={api} writable={writable} loading={loading} onSaved={reload}/>:
        card?<Customer360 accountId={card} load={api} writable={writable} onBack={()=>setCard(null)}
          onNavigate={navigate} onEdit={(kind,row)=>begin(kind,row)}/>:
        <>
          <div className="toolbar">
            <input aria-label="Поиск" placeholder="Поиск" value={search}
              onChange={e=>setSearch(e.target.value)}/>
            {writable&&<button disabled={
              tab==="accounts"?!items.parties.length:
              tab==="contacts"?!items.parties.length:
              !items.accounts.length} onClick={()=>begin(tab)}>
              + Добавить {kind.one}</button>}
          </div>
          {writable&&tab==="accounts"&&!items.parties.length&&
            <p>Сначала создайте лицо или организацию.</p>}
          {writable&&["deals","tasks"].includes(tab)&&!items.accounts.length&&
            <p>Сначала добавьте клиента.</p>}
          {tab==="deals"&&<div className="view-switch" role="group" aria-label="Вид сделок">
            <button className={view==="table"?"active":"ghost"} onClick={()=>setView("table")}>Таблица</button>
            <button className={view==="board"?"active":"ghost"} onClick={()=>setView("board")}>Воронка</button>
          </div>}
          {loading?<p className="empty">Загрузка…</p>:rows.length===0?
            <p className="empty">{search?"Ничего не найдено":"Пока записей нет"}</p>:
            tab==="deals"&&view==="board"?
              <div className="deal-board">{stages.map(stage=><section className="deal-column" key={stage[0]}>
                <h3>{stage[1]}</h3>{rows.filter(row=>row.stage.stage_id===stage[0]).map(row=>
                  <article className="deal-card" key={row.id}><strong>{row.name}</strong>
                    <p>{accountName(row.account_id)}</p><p>{money(row.amount)}</p>{actions(row)}
                  </article>)}</section>)}</div>:
              <><div className="table-scroll"><table><thead><tr>
                {columns[tab].map(([key,title])=><th key={key}>{title}</th>)}
                <th>Действия</th></tr></thead><tbody>{rows.map(row=><tr key={row.id}>
                  {columns[tab].map(([key,,render])=><td key={key}>{render(row)}</td>)}
                  <td>{actions(row)}</td></tr>)}</tbody></table></div>
                <div className="mobile">{rows.map(row=><article key={row.id}>
                  <strong>{itemName(tab,row)}</strong>
                  {columns[tab].map(([key,title,render])=><p key={key}>{title}: {render(row)}</p>)}
                  {actions(row)}</article>)}</div></>}
        </>}
      </section>
    </main>
    {form&&<div className="shade" onClick={e=>{
      if(e.target===e.currentTarget&&!busy)setForm(null);
    }}><form className="dialog" onSubmit={submit}>
      <h2>{form.mode==="create"?"Добавить":form.mode==="advance"?"Изменить этап":
        form.mode==="complete"?"Выполнить":"Изменить"} {sections[form.kind].one}</h2>
      {fields()}
      <footer><button type="button" className="ghost" disabled={busy}
        onClick={()=>setForm(null)}>Отмена</button>
        <button disabled={busy}>{busy?"Сохранение…":"Сохранить"}</button></footer>
    </form></div>}
  </>;
}
createRoot(document.getElementById("root")).render(<App/>);

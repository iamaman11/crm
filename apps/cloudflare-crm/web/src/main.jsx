import React, {useEffect,useState} from "react";
import {createRoot} from "react-dom/client";
import "./styles.css";
import Customer360 from "./Customer360.jsx";

import { modules as features, labels as names } from "../../shared/catalog.mjs";

const dealStages=["new","qualified","proposal","won","lost"];
const number=value=>new Intl.NumberFormat("ru-RU",{maximumFractionDigits:2}).format(Number(value)||0);

async function api(url, options={}) {
 const response=await fetch("/api/"+url,{...options,headers:{"content-type":"application/json"}});
 if(!response.ok){const body=await response.json().catch(()=>({}));
  const code=response.headers.get("x-crm-error-code")||body.code||"HTTP_"+response.status;
  const requestId=response.headers.get("x-request-id");
  throw Error((body.error||"Ошибка "+response.status)+" ["+code+(requestId?"; запрос "+requestId:"")+"]");
 }
 return response.status===204?null:response.json();
}
function App(){
 const [tab,setTab]=useState("customers");
 const [dealView,setDealView]=useState("table");
 const [selectedCustomer,setSelectedCustomer]=useState(null);
 const [items,setItems]=useState({customers:[],contacts:[],deals:[],tasks:[]});
 const [search,setSearch]=useState("");
 const [form,setForm]=useState(null);
 const [editing,setEditing]=useState(null);
 const [busy,setBusy]=useState(false);
 const [loading,setLoading]=useState(true);
 const [message,setMessage]=useState("");
 const [error,setError]=useState(false);
 const feature=features[tab];
 async function reload(){
  const keys=Object.keys(features),results=await Promise.all(keys.map(k=>api(k)));
  setItems(Object.fromEntries(keys.map((k,i)=>[k,results[i].items])));
 }
 useEffect(()=>{reload().catch(e=>{setError(true);setMessage(e.message)}).finally(()=>setLoading(false));},[]);
 function open(item=null, forTab=tab){
  const defaults={kind:"organization",stage:"new",status:"open",amount:0};
  setTab(forTab);setSelectedCustomer(null);setSearch("");
  setEditing(item?.id||null);
  setForm(item?{...item}:Object.fromEntries(features[forTab].fields.map(([k])=>[k,defaults[k]??""])));
  setMessage("");
 }
 async function submit(event){
  event.preventDefault();setBusy(true);
  try{
   await api(tab+(editing?"/"+editing:""),{method:editing?"PATCH":"POST",body:JSON.stringify(form)});
   setForm(null);await reload();setMessage("Сохранено");setError(false);
  }catch(e){setMessage(e.message);setError(true);}
  finally{setBusy(false);}
 }
 async function changeDealStage(item,stage){
  if(stage===item.stage)return;
  setBusy(true);
  try{
   await api("deals/"+item.id,{method:"PATCH",body:JSON.stringify({
    customer_id:item.customer_id,title:item.title,stage,amount:item.amount
   })});
   await reload();setMessage("Этап обновлён");setError(false);
  }catch(e){setMessage(e.message);setError(true);}
  finally{setBusy(false);}
 }
 async function remove(item){
  if(!confirm("Удалить «"+item[feature.primary]+"»?"))return;
  try{await api(tab+"/"+item.id,{method:"DELETE"});await reload();setMessage("Удалено");setError(false);}
  catch(e){setMessage(e.message);setError(true);}
 }
 const customerName=id=>items.customers.find(c=>c.id===id)?.name||"—";
 const format=(key,value)=>key==="customer_id"?customerName(value):
  ["kind","stage","status"].includes(key)?names[value]||String(value??"—"):String(value??"—");
 const rows=items[tab].filter(row=>JSON.stringify(row).toLowerCase().includes(search.toLowerCase()));
 const buttons=row=><span className="row-actions">{tab==="customers"&&
  <button className="ghost" onClick={()=>{setSelectedCustomer(row.id);setSearch("");}}>Карточка</button>}
  <button className="ghost" onClick={()=>open(row)}>Изменить</button>
  <button className="delete" onClick={()=>remove(row)}>Удалить</button></span>;
 const field=([key,label,type,options])=><div className="field" key={key}><label htmlFor={"crm-"+key}>{label}</label>
  {type==="customer"?<select id={"crm-"+key} required value={form[key]||""} onChange={e=>setForm({...form,[key]:e.target.value})}>
   <option value="">Выберите клиента</option>{items.customers.map(c=><option value={c.id} key={c.id}>{c.name}</option>)}</select>
  :type==="select"?<select id={"crm-"+key} value={form[key]??""} onChange={e=>setForm({...form,[key]:e.target.value})}>
   {options.map(v=><option value={v} key={v}>{names[v]||v}</option>)}</select>
  :<input id={"crm-"+key} type={type} required={["name","title"].includes(key)} min={type==="number"?0:undefined}
   step={type==="number"?"0.01":undefined} value={form[key]??""}
   onChange={e=>setForm({...form,[key]:e.target.value})}/>}</div>;
 return <><header><strong>CRM</strong><span>Тестовое рабочее пространство</span></header>
  <main><div className="heading"><div><h1>Моя CRM</h1><p>Клиенты, контакты, сделки и задачи — в одном месте</p></div>
   <button className="ghost" onClick={()=>reload().then(()=>{setMessage("Обновлено");setError(false)}).catch(e=>{setMessage(e.message);setError(true)})}>Обновить</button></div>
   <nav aria-label="Разделы">{Object.entries(features).map(([key,v])=>
    <button key={key} className={tab===key?"active":"ghost"} onClick={()=>{setTab(key);setSelectedCustomer(null);setSearch("");setMessage("")}}>{v.title} <small>{items[key].length}</small></button>)}</nav>
   {message&&<div role="status" className={error?"notice error":"notice"}>{message}</div>}
   <section className="panel">{selectedCustomer?
    <Customer360 customerId={selectedCustomer} load={api}
      onBack={()=>{setSelectedCustomer(null);setTab("customers");setSearch("");}}
      onNavigate={section=>{setTab(section);setSearch(selectedCustomer);setSelectedCustomer(null);}}
      onEdit={(section,item)=>open(item,section)}/>:
    <><div className="toolbar"><input aria-label="Поиск" placeholder="Поиск" value={search} onChange={e=>setSearch(e.target.value)}/>
    <button disabled={tab!=="customers"&&!items.customers.length} onClick={()=>open()}>+ Добавить {feature.one}</button></div>
    {tab==="deals"&&<div className="view-switch" role="group" aria-label="Вид сделок">
     <button className={dealView==="table"?"active":"ghost"} aria-pressed={dealView==="table"} onClick={()=>setDealView("table")}>Таблица</button>
     <button className={dealView==="board"?"active":"ghost"} aria-pressed={dealView==="board"} onClick={()=>setDealView("board")}>Воронка</button>
    </div>}
    {tab!=="customers"&&!items.customers.length&&<p>Сначала добавьте клиента.</p>}
    {loading?<p className="empty">Загрузка…</p>:!rows.length?<p className="empty">{search?"Ничего не найдено":"Пока записей нет"}</p>:
     tab==="deals"&&dealView==="board"?<div className="deal-board" aria-label="Воронка продаж">
      {dealStages.map(stage=>{
       const deals=rows.filter(row=>row.stage===stage);
       return <section key={stage} className="deal-column" aria-label={names[stage]}>
        <h3>{names[stage]} <small>{deals.length}</small></h3>
        <p className="deal-total">Сумма: {number(deals.reduce((sum,deal)=>sum+Number(deal.amount),0))}</p>
        {deals.map(deal=><article key={deal.id} className="deal-card">
         <strong>{deal.title}</strong>
         <p>{customerName(deal.customer_id)}</p>
         <p className="deal-amount">{number(deal.amount)}</p>
         <label>Этап сделки <select aria-label="Этап сделки" disabled={busy} value={deal.stage}
          onChange={e=>changeDealStage(deal,e.target.value)}>
          {dealStages.map(value=><option key={value} value={value}>{names[value]}</option>)}
         </select></label>
         {buttons(deal)}
        </article>)}
       </section>;
      })}
     </div>:
     <><div className="table-scroll"><table><thead><tr>{feature.fields.map(([key,label])=><th key={key}>{label}</th>)}<th>Действия</th></tr></thead>
      <tbody>{rows.map(row=><tr key={row.id}>{feature.fields.map(([key])=><td key={key}>{format(key,row[key])}</td>)}<td>{buttons(row)}</td></tr>)}</tbody></table></div>
      <div className="mobile">{rows.map(row=><article key={row.id}><b>{row[feature.primary]}</b>
       {feature.fields.filter(([k])=>k!==feature.primary).map(([k,label])=><p key={k}>{label}: {format(k,row[k])}</p>)}{buttons(row)}</article>)}</div></>}
   </>}
   </section>
  </main>
  {form&&<div className="shade" onClick={e=>{if(e.target===e.currentTarget)setForm(null)}}><form className="dialog" onSubmit={submit}>
   <h2>{editing?"Изменить":"Добавить"} {feature.one}</h2>{feature.fields.map(field)}
   <footer><button type="button" className="ghost" onClick={()=>setForm(null)}>Отмена</button><button disabled={busy}>{busy?"Сохранение…":"Сохранить"}</button></footer>
  </form></div>}</>;
}
createRoot(document.getElementById("root")).render(<App/>);

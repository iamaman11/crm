import React,{useEffect,useState} from "react";

// Read-only composition of ORIGINAL owner views, not a second Customer domain.
// No shadow Customer360 table: one tenant-scoped Account overview from Rust/D1.
import {formatDealMoney} from "./deal-presentation.mjs";
export default function Customer360({accountId,load,writable,onBack,onNavigate,onEdit}){
  const [state,setState]=useState({loading:true});
  async function refresh(){
    setState({loading:true});
    try{const result=await load("accounts/"+accountId+"/overview");
      setState({loading:false,data:result});}
    catch(e){setState({loading:false,error:e.message});}
  }
  useEffect(()=>{refresh();},[accountId]);
  const {data,loading,error}=state;
  const sections=[
    ["contact_points","Контакты","contacts",p=>p.value,p=>p.kind+
      (p.verified?" · Подтверждён":" · Не подтверждён")],
    ["deals","Сделки","deals",d=>d.name,d=>d.stage.stage_id+" · "+formatDealMoney(d.amount)],
    ["tasks","Задачи","tasks",t=>t.subject,t=>t.status==="completed"?"Готово":"В работе"],
    ["party_relationships","Связи","parties",r=>r.type?.code||r.id,r=>r.status]
  ];
  return <div className="customer360" aria-label="Карточка клиента">
    <div className="customer360-header">
      <button className="ghost" onClick={onBack}>← К списку клиентов</button>
      <button className="ghost" onClick={refresh} disabled={loading}>Обновить карточку</button>
    </div>
    {loading?<p role="status">Загрузка карточки…</p>:
      error?<div role="alert"><p>{error}</p><button onClick={refresh}>Повторить</button></div>:
      <><h2>{data.account.name}</h2>
        <p>Лицо / организация: {data.primary_party.name} · {data.account.status}</p>
        <div className="customer360-stats" aria-label="Сводка клиента">
          <div><strong>{data.contact_points.length}</strong><span>Контактов</span></div>
          <div><strong>{data.deals.length}</strong><span>Сделок</span></div>
          <div><strong>{data.tasks.filter(t=>t.status==="open").length}</strong>
            <span>Открытых задач</span></div>
          <div><strong>{data.party_relationships.length}</strong><span>Связей</span></div>
          <div><strong>{data.account.version}</strong><span>Версия аккаунта</span></div>
        </div>
        {Object.values(data.truncated).some(Boolean)&&<p className="customer360-asof">
          Показаны первые 100 записей в ограниченных разделах.</p>}
        {sections.map(([key,title,route,primary,secondary])=>
          <section key={key} className="customer360-section" aria-label={title+" клиента"}>
            <div className="customer360-section-head">
              <h3>{title} <small>{data[key].length}{data.truncated[key]?"+":""}</small></h3>
              <button className="ghost" onClick={()=>onNavigate(route)}>Все {title.toLowerCase()}</button>
            </div>
            {!data[key].length?<p>Пока нет записей</p>:
              <div className="customer360-list">{data[key].map(row=><article key={row.id}>
                <div><strong>{primary(row)}</strong><p>{secondary(row)}</p></div>
                {writable&&route!=="parties"&&route!=="tasks"&&
                  <button className="ghost" onClick={()=>onEdit(route,row)}>Изменить</button>}
              </article>)}</div>}
          </section>)}
      </>}
  </div>;
}

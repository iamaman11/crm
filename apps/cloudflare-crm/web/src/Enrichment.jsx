import React,{useEffect,useState} from "react";

const blankProfile=()=>({
  provider_key:"",adapter_kind:"",adapter_contract_version:"1.0.0",
  purpose_codes:"",license_id:"",permitted_use_class:"",residency_region:"",
  retention_days:"0"
});
const blankMapping=()=>({
  mapping_key:"",provider_profile_version_id:"",
  provider_response_field_path:"",maximum_suggestions_per_response:"1",
  confidence_required:true
});
export default function Enrichment({load,writable,loading}){
  const [profiles,setProfiles]=useState([]);
  const [mappings,setMappings]=useState([]);
  const [form,setForm]=useState(null);
  const [profile,setProfile]=useState(blankProfile);
  const [mapping,setMapping]=useState(blankMapping);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  async function refresh(){
    const [p,m]=await Promise.all([
      load("customer-enrichment/profiles"),load("customer-enrichment/mappings")
    ]);
    setProfiles(p.items);setMappings(m.items);
  }
  useEffect(()=>{refresh().catch(e=>setError(e.message));},[]);
  function begin(kind){
    setError("");setNotice("");
    setProfile(blankProfile());setMapping(blankMapping());
    setForm(kind);
  }
  async function submit(e){
    e.preventDefault();
    if(busy||!writable)return;
    setBusy(true);setError("");setNotice("");
    const isProfile=form==="profile";
    const path="customer-enrichment/"+(isProfile?"profiles":"mappings");
    const body=isProfile?{
      ...profile,
      purpose_codes:profile.purpose_codes.split(",").map(x=>x.trim()).filter(Boolean),
      retention_days:Number(profile.retention_days),
      raw_payload_policy:"digest_only",
      credential_handle_aliases:[],
      effective_at_unix_ms:Date.now(),
      expires_at_unix_ms:null
    }:{
      ...mapping,maximum_suggestions_per_response:Number(mapping.maximum_suggestions_per_response)
    };
    try{
      const result=await load(path,{method:"POST",
        headers:{"idempotency-key":crypto.randomUUID()},
        body:JSON.stringify(body)});
      setForm(null);
      setNotice(result.replayed?"Повтор выполнен без дублирования":
        isProfile?"Профиль сохранён":"Сопоставление сохранено");
      try{await refresh();}
      catch(err){setError("Конфигурация сохранена, но список не обновился. "+
        "Показанные данные могут быть устаревшими. Нажмите «Обновить настройки». "+err.message);}
    }catch(err){setError(err.message);}
    finally{setBusy(false);}
  }
  const changeProfile=(key,value)=>setProfile(p=>({...p,[key]:value}));
  const changeMapping=(key,value)=>setMapping(m=>({...m,[key]:value}));
  return <section aria-label="Обогащение данных">
    <h2>Обогащение данных</h2>
    <p>Настройка источника и сопоставления данных. Сохранённые версии
      неизменяемы. Вызовы провайдеров, получение предложений и изменение
      данных клиентов пока не подключены. Настройки лицензии указывает
      оператор — приложение не подтверждает действительность лицензии.</p>
    {writable&&<div className="row-actions">
      <button disabled={busy||loading} onClick={()=>begin("profile")}>
        Добавить профиль провайдера</button>
      <button disabled={busy||loading||!profiles.length}
        onClick={()=>begin("mapping")}>Добавить сопоставление</button>
    </div>}
    <button className="ghost" disabled={busy||loading}
      onClick={()=>{setError("");refresh().then(()=>setNotice("Настройки обновлены"))
        .catch(e=>setError("Не удалось обновить настройки. "+e.message));}}>
      Обновить настройки</button>
    {error&&<p role="alert" className="notice error">{error}</p>}
    {notice&&<p role="status" className="notice">{notice}</p>}
    {form&&<form onSubmit={submit} aria-label={form==="profile"?
      "Форма профиля провайдера":"Форма сопоставления"}>
      <h3>{form==="profile"?"Новый профиль провайдера":"Новое сопоставление"}</h3>
      {form==="profile"?<>
        {[
          ["provider_key","Ключ провайдера","example_provider"],
          ["adapter_kind","Тип адаптера","manual"],
          ["adapter_contract_version","Версия контракта","1.0.0"],
          ["purpose_codes","Цели использования (через запятую)","research"],
          ["license_id","Лицензия / основание","Укажите действующее основание"],
          ["permitted_use_class","Класс разрешённого использования","internal"],
          ["residency_region","Регион хранения","eu"]
        ].map(([key,title,placeholder])=>
          <div className="field" key={key}>
            <label htmlFor={"enrichment-"+key}>{title}</label>
            <input id={"enrichment-"+key} required placeholder={placeholder}
              value={profile[key]} disabled={busy}
              onChange={e=>changeProfile(key,e.target.value)}/>
          </div>)}
        <div className="field">
          <label htmlFor="enrichment-retention">Срок хранения (дней, до 3650)</label>
          <input id="enrichment-retention" type="number" min="0" max="3650"
            required value={profile.retention_days} disabled={busy}
            onChange={e=>changeProfile("retention_days",e.target.value)}/>
        </div>
        <p>Допустимое поле: имя Party. Только digest исходного ответа,
          без секретов и вызовов внешнего провайдера.</p>
      </>:<>
        <div className="field"><label htmlFor="enrichment-provider-version">
          Профиль провайдера</label>
          <select id="enrichment-provider-version" required
            value={mapping.provider_profile_version_id} disabled={busy}
            onChange={e=>changeMapping("provider_profile_version_id",e.target.value)}>
            <option value="">Выберите профиль</option>
            {profiles.map(p=><option key={p.id} value={p.id}>
              {p.provider_key} · {p.adapter_contract_version}</option>)}
          </select></div>
        <div className="field"><label htmlFor="enrichment-mapping-key">
          Ключ сопоставления</label>
          <input id="enrichment-mapping-key" required value={mapping.mapping_key}
            disabled={busy} onChange={e=>changeMapping("mapping_key",e.target.value)}/>
        </div>
        <div className="field"><label htmlFor="enrichment-source-path">
          Поле ответа провайдера</label>
          <input id="enrichment-source-path" required placeholder="company.name"
            value={mapping.provider_response_field_path} disabled={busy}
            onChange={e=>changeMapping("provider_response_field_path",e.target.value)}/>
        </div>
        <div className="field"><label htmlFor="enrichment-max">
          Максимум предложений (1–32)</label>
          <input id="enrichment-max" type="number" min="1" max="32" required
            value={mapping.maximum_suggestions_per_response} disabled={busy}
            onChange={e=>changeMapping("maximum_suggestions_per_response",e.target.value)}/>
        </div>
        <label><input type="checkbox" checked={mapping.confidence_required}
          disabled={busy}
          onChange={e=>changeMapping("confidence_required",e.target.checked)}/>
          Требовать confidence от провайдера</label>
        <p>Цель сопоставления: Party.display_name. Это описание правила,
          а не получение данных или разрешение их автоматически применять.</p>
      </>}
      <div className="row-actions">
        <button type="button" className="ghost" disabled={busy}
          onClick={()=>setForm(null)}>Отмена</button>
        <button disabled={busy}>{busy?"Сохранение…":form==="profile"?
          "Сохранить профиль":"Сохранить сопоставление"}</button>
      </div>
    </form>}
    <h3>Профили провайдеров ({profiles.length})</h3>
    {!profiles.length?<p>Пока нет настроенных профилей.</p>:
      <div className="table-scroll"><table><thead><tr>
        <th>Провайдер</th><th>Контракт</th><th>Назначение</th><th>Идентификатор версии</th>
      </tr></thead><tbody>{profiles.map(p=><tr key={p.id}>
        <td>{p.provider_key}</td>
        <td>{p.adapter_kind} · {p.adapter_contract_version}</td>
        <td>{p.purpose_codes.join(", ")}</td><td><code>{p.id}</code></td>
      </tr>)}</tbody></table></div>}
    <h3>Сопоставления ({mappings.length})</h3>
    {!mappings.length?<p>Сопоставлений пока нет.</p>:
      <div className="table-scroll"><table><thead><tr>
        <th>Правило</th><th>Поле источника</th><th>Целевое поле</th><th>Версия профиля</th>
      </tr></thead><tbody>{mappings.map(m=><tr key={m.id}>
        <td>{m.mapping_key}</td><td>{m.provider_response_field_path}</td>
        <td>Party.display_name</td><td><code>{m.provider_profile_version_id}</code></td>
      </tr>)}</tbody></table></div>}
  </section>;
}

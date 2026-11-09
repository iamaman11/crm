import React, {useState} from "react";

// A single read-only quality review. The original Rust domain owns all rules;
// App owns the Party list. This screen has no shadow Party data or scoring.
export default function DataQuality({parties,load,loading}){
  const [partyId,setPartyId]=useState("");
  const [result,setResult]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const current=parties.find(x=>x.id===partyId);
  async function evaluate(event){
    event.preventDefault();if(busy||!partyId)return;
    setBusy(true);setError("");setResult(null);
    try{setResult(await load("data-quality/parties/"+encodeURIComponent(partyId)+"/evaluate"));}
    catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  function reset(){setPartyId("");setResult(null);setError("");}
  return <section aria-label="Качество данных">
    <h2>Качество данных</h2>
    <p>Проверка названия лица или организации по двум встроенным правилам
      оригинального Rust-модуля. Это не полная оценка качества клиента.</p>
    <form onSubmit={evaluate}>
      <div className="field"><label htmlFor="quality-party">Лицо или организация для проверки</label>
        <select id="quality-party" required value={partyId}
          onChange={e=>{setPartyId(e.target.value);setResult(null);setError("");}}>
          <option value="">Выберите запись</option>
          {parties.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}
        </select>
      </div>
      <div className="row-actions">
        <button disabled={loading||busy||!partyId}>Проверить качество</button>
        <button className="ghost" type="button" disabled={busy} onClick={reset}>Сбросить</button>
      </div>
    </form>
    {error&&<p className="notice error" role="alert">{error}</p>}
    {result&&<div role="status">
      {current&&current.version!==result.party.version?
        <p>Запись изменилась. Запустите проверку повторно.</p>:
        <p>{result.failed?"Есть замечания: "+result.failed:"Замечаний нет"}
          {" · Правил проверено: "+result.checked}</p>}
      <p>Название: {result.party.name} · Версия записи: {result.party.version}</p>
      <div className="table-scroll"><table><thead><tr>
        <th>Правило</th><th>Результат</th><th>Что делать</th>
      </tr></thead><tbody>
        {result.checks.map(c=><tr key={c.rule_key}>
          <td>{c.title}</td>
          <td>{c.passed?"Пройдено":"Замечание"}</td>
          <td>{c.passed?"—":c.remediation}</td>
        </tr>)}
      </tbody></table></div>
      <p>Результат вычислен по текущей версии Party и не записан в D1.
        Findings и общая оценка полноты пока не подключены.</p>
    </div>}
  </section>;
}

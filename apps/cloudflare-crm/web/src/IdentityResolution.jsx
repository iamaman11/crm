import React, {useState} from "react";

// Honest first Identity Resolution read path: no duplicate guesses, invented
// match score, client-side business validation or shadow Party list.
export default function IdentityResolution({parties,load,loading}){
  const [first,setFirst]=useState("");
  const [second,setSecond]=useState("");
  const [comparison,setComparison]=useState(null);
  const [candidates,setCandidates]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  function reset(){setFirst("");setSecond("");setComparison(null);setError("");}
  async function discover(){
    if(busy)return;
    setBusy(true);setError("");setCandidates(null);
    try{setCandidates((await load("identity-resolution/candidates")).items);}
    catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  async function compare(e){
    e.preventDefault();
    if(busy)return;
    setBusy(true);setComparison(null);setError("");
    try{
      const result=await load("identity-resolution/compare/"+
        encodeURIComponent(first)+"/"+encodeURIComponent(second));
      setComparison(result);
    }catch(e){setError(e.message);}
    finally{setBusy(false);}
  }
  return <section aria-label="Сравнение записей">
    <h2>Сравнение записей</h2>
    <p>Поиск совпадающих сохранённых email проверяет только реальные,
      действующие записи контактов. Совпадение email не доказывает, что
      люди или организации одинаковы; email не считается подтверждённым
      без независимого подтверждения.</p>
    <button disabled={loading||busy} onClick={discover}>Найти совпадения email</button>
    {candidates&&<section aria-label="Наблюдаемые совпадения">
      <h3>Возможные совпадения: {candidates.length}</h3>
      {candidates.length===0?<p>Совпадающих email не найдено.</p>:
        <div className="table-scroll"><table><thead><tr>
          <th>Первая запись</th><th>Вторая запись</th><th>Источник</th>
        </tr></thead><tbody>
          {candidates.map(c=><tr key={c.first.id+"|"+c.second.id}>
            <td>{c.first.name}</td><td>{c.second.name}</td>
            <td>{c.both_contacts_verified?"Оба контакта подтверждены":"Контакты не подтверждены с обеих сторон"}</td>
          </tr>)}
        </tbody></table></div>}
    </section>}
    <p>Выберите два существующих лица или организации. Сравнение показывает
      данные и версии оригинальных Rust-записей, но не устанавливает дубликат.</p>
    <form onSubmit={compare}>
      <div className="field"><label htmlFor="identity-first">Первая запись</label>
        <select id="identity-first" required value={first}
          onChange={e=>{setFirst(e.target.value);setComparison(null);setError("");}}>
          <option value="">Выберите запись</option>
          {parties.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}
        </select></div>
      <div className="field"><label htmlFor="identity-second">Вторая запись</label>
        <select id="identity-second" required value={second}
          onChange={e=>{setSecond(e.target.value);setComparison(null);setError("");}}>
          <option value="">Выберите запись</option>
          {parties.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}
        </select></div>
      <div className="row-actions">
        <button disabled={loading||busy||parties.length<2}>Сравнить</button>
        <button className="ghost" type="button" disabled={busy} onClick={reset}>Сбросить</button>
      </div>
    </form>
    {error&&<p role="alert" className="notice error">{error}</p>}
    {comparison&&<div role="status">
      <p>Сравнение выполнено без изменения записей.</p>
      <div className="table-scroll"><table><thead><tr>
        <th>Поле</th><th>Первая запись</th><th>Вторая запись</th>
      </tr></thead><tbody>
        {[
          ["Имя",comparison.first.name,comparison.second.name],
          ["Тип",comparison.first.kind,comparison.second.kind],
          ["Версия",comparison.first.version,comparison.second.version],
          ["ID",comparison.first.id,comparison.second.id]
        ].map(([name,a,b])=><tr key={name}><th>{name}</th><td>{a}</td><td>{b}</td></tr>)}
      </tbody></table></div>
    </div>}
    <p>Найденные пары — наблюдения по одинаковому сохранённому email,
      а не решения о дубликатах. Score, сохранение дел о дубликатах,
      подтверждение личности и объединение пока не подключены.</p>
  </section>;
}

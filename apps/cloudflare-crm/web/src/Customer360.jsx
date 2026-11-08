import React, { useEffect, useState } from "react";
import { labels } from "../../shared/catalog.mjs";

const formatNumber = value => new Intl.NumberFormat("ru-RU",
  { maximumFractionDigits: 2 }).format(Number(value) || 0);
export default function Customer360({ customerId, load, onBack, onEdit, onNavigate }) {
  const [state, setState] = useState({ loading: true });
  async function refresh() {
    setState({ loading: true });
    try {
      const result = await load("customer360/" + customerId);
      setState({ loading: false, data: result });
    } catch (error) {
      setState({ loading: false, error: error.message });
    }
  }
  useEffect(() => { refresh(); }, [customerId]);
  const { data, loading, error } = state;
  const groups = [
    ["contacts", "Контакты", row => row.name, row => [row.email, row.phone].filter(Boolean).join(" · ")],
    ["deals", "Сделки", row => row.title, row => (labels[row.stage] || row.stage) +
      " · " + formatNumber(row.amount)],
    ["tasks", "Задачи", row => row.title, row => (labels[row.status] || row.status) +
      (row.due_date ? " · " + row.due_date : "")],
  ];
  return <div className="customer360" aria-label="Карточка клиента">
    <div className="customer360-header">
      <button className="ghost" onClick={onBack}>← К списку клиентов</button>
      <button className="ghost" onClick={refresh} disabled={loading}>Обновить карточку</button>
    </div>
    {loading ? <p role="status">Загрузка карточки…</p> :
      error ? <div role="alert"><p>{error}</p><button onClick={refresh}>Повторить</button></div> :
      <><h2>{data.customer.name}</h2>
        <p>{labels[data.customer.kind] || data.customer.kind}</p>
        <div className="customer360-stats" aria-label="Сводка клиента">
          <div><strong>{data.summary.contacts}</strong><span>Контактов</span></div>
          <div><strong>{data.summary.deals}</strong><span>Сделок</span></div>
          <div><strong>{data.summary.open_tasks ?? "—"}</strong><span>Открытых задач</span></div>
          <div><strong>{data.summary.overdue_tasks ?? "—"}</strong><span>Просрочено</span></div>
          <div><strong>{data.summary.deal_amount === null ? "—" :
            formatNumber(data.summary.deal_amount)}</strong><span>Сумма сделок</span></div>
        </div>
        <p className="customer360-asof">Просрочка по дате UTC: {data.as_of}.
          {groups.some(([key]) => data.related[key].total > data.related[key].items.length) ?
            " Некоторые списки ограничены первыми 100 записями." : ""}</p>
        {groups.map(([key, title, primary, secondary]) => {
          const group = data.related[key];
          return <section key={key} className="customer360-section" aria-label={title + " клиента"}>
            <div className="customer360-section-head">
              <h3>{title} <small>{group.total}</small></h3>
              <button className="ghost" onClick={() => onNavigate(key)}>Все {title.toLowerCase()}</button>
            </div>
            {!group.items.length ? <p>Пока нет записей</p> :
              <div className="customer360-list">{group.items.map(item =>
                <article key={item.id}>
                  <div><strong>{primary(item)}</strong><p>{secondary(item)}</p></div>
                  <button className="ghost" onClick={() => onEdit(key, item)}>Изменить</button>
                </article>)}</div>}
          </section>;
        })}
      </>}
  </div>;
}

// Customer 360 is a bounded read-only composition of existing tenant-owned rows.
// No duplicate customer master, new D1 table, service, or mutations.
export const CUSTOMER360_LIMIT = 100;
export async function readCustomer360(db, tenant, customerId, asOf = new Date().toISOString().slice(0, 10)) {
  const customer = await db.prepare(
    "SELECT id, name, kind, updated_at FROM crm_customers WHERE id = ? AND tenant_id = ?"
  ).bind(customerId, tenant).first();
  if (!customer) return null;
  const names = { contacts: "crm_contacts", deals: "crm_deals", tasks: "crm_tasks" };
  const groups = await Promise.all(Object.entries(names).map(async ([key, table]) => {
    const [count, result] = await Promise.all([
      db.prepare("SELECT COUNT(*) AS total FROM " + table + " WHERE tenant_id = ? AND customer_id = ?")
        .bind(tenant, customerId).first(),
      db.prepare("SELECT * FROM " + table +
        " WHERE tenant_id = ? AND customer_id = ? ORDER BY updated_at DESC LIMIT ?")
        .bind(tenant, customerId, CUSTOMER360_LIMIT).all(),
    ]);
    return [key, { total: Number(count?.total || 0), items: result.results || [],
      limit: CUSTOMER360_LIMIT }];
  }));
  const related = Object.fromEntries(groups);
  const open = related.tasks.items.filter(task => task.status === "open");
  return { customer, related, as_of: asOf,
    summary: {
      contacts: related.contacts.total,
      deals: related.deals.total,
      tasks: related.tasks.total,
      // Do not claim accurate full metrics if the returned window is truncated.
      open_tasks: related.tasks.total <= CUSTOMER360_LIMIT ? open.length : null,
      overdue_tasks: related.tasks.total <= CUSTOMER360_LIMIT ?
        open.filter(task => task.due_date && task.due_date < asOf).length : null,
      deal_amount: related.deals.total <= CUSTOMER360_LIMIT ?
        related.deals.items.reduce((sum, deal) => sum + Number(deal.amount || 0), 0) : null,
    } };
}

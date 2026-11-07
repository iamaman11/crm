import { useEffect, useRef, useState } from "react";
import { GovernedCrmClient, type Deal, type Party, type SessionState, type Task } from "@ultimate-crm/client";
import { FeedbackPanel, PageHeader } from "@ultimate-crm/ui";
import { crmMessage } from "./CrmPageUtils";

export function TasksPage({ client, session, capabilities }: {
  client: GovernedCrmClient;
  session: Extract<SessionState, { status: "authenticated" }>;
  capabilities: ReadonlySet<string>;
}) {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [parties, setParties] = useState<Party[]>([]);
  const [deals, setDeals] = useState<Deal[]>([]);
  const [cursor, setCursor] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [subject, setSubject] = useState("");
  const [related, setRelated] = useState("");
  const [newId, setNewId] = useState(() => crypto.randomUUID());
  const [editSubject, setEditSubject] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const current = tasks.find((task) => task.taskId === selectedId);
  const canCreate = capabilities.has("activities.task.create");
  const canEdit = capabilities.has("activities.task.update");
  const canComplete = capabilities.has("activities.task.complete");
  useEffect(() => {
    if (!selectedId) return;
    let active = true;
    client.getTask(selectedId).then((task) => {
      if (active && task) setTasks((items) => items.map((item) => item.taskId === selectedId ? task : item));
    }).catch((caught) => { if (active) setError(crmMessage(caught)); });
    return () => { active = false; };
  }, [client, selectedId]);

  async function reload(after = "") {
    setLoading(true); setError("");
    try {
      const page = await client.listTasks(after);
      setTasks((items) => after ? [...items, ...page.items] : page.items);
      setCursor(page.next);
      const [customers, opportunities] = await Promise.all([
        capabilities.has("parties.party.list") ? client.listParties() : Promise.resolve({ items: [] as Party[], next: "" }),
        capabilities.has("sales.deal.list") ? client.listDeals() : Promise.resolve({ items: [] as Deal[], next: "" }),
      ]);
      setParties(customers.items); setDeals(opportunities.items);
    } catch (caught) { setError(crmMessage(caught)); }
    finally { setLoading(false); }
  }
  useEffect(() => { void reload(); }, [client]);
  async function run(action: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setNotice(""); setError("");
    try { await action(); }
    catch (caught) { setError(crmMessage(caught)); }
    finally { lock.current = false; setBusy(false); }
  }

  async function add(event: React.FormEvent) {
    event.preventDefault();
    if (!session.actorId || !related || !subject.trim()) return;
    const [kind, reference] = related.split(":");
    if ((kind !== "parties.party" && kind !== "sales.deal") || !reference) return;
    await run(async () => {
      const created = await client.createTask(newId, subject, session.actorId!, session.tenantId, kind, reference);
      if (!created) throw new Error("Missing saved task");
      setTasks((items) => [created, ...items.filter((item) => item.taskId !== created.taskId)]);
      setSelectedId(created.taskId); setEditSubject(created.subject);
      setNewId(crypto.randomUUID()); setSubject("");
      setNotice("Task saved.");
    });
  }
  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!current) return;
    await run(async () => {
      const saved = await client.updateTask(current, editSubject);
      if (saved) setTasks((items) => items.map((item) => item.taskId === saved.taskId ? saved : item));
      setNotice("Task updated.");
    });
  }
  async function complete() {
    if (!current) return;
    await run(async () => {
      const saved = await client.completeTask(current);
      if (saved) setTasks((items) => items.map((item) => item.taskId === saved.taskId ? saved : item));
      setNotice("Task completed.");
    });
  }
  function relatedLabel(task: Task): string {
    const ref = task.relatedResources[0];
    if (!ref) return "No related record";
    if (ref.resourceType === "sales.deal") return deals.find((item) => item.dealId === ref.resourceId)?.name ?? "Deal";
    return parties.find((item) => item.partyRef?.partyId === ref.resourceId)?.displayName ?? "Customer";
  }
  return <div>
    <PageHeader eyebrow="Activities" title="Tasks" description="Create, edit and complete work linked to customers or deals." />
    {error ? <FeedbackPanel tone="danger" title="Request unavailable">{error}{" "}
      <button type="button" onClick={() => { void reload(); }}>Reload</button>
    </FeedbackPanel> : null}
    {notice ? <p role="status">{notice}</p> : null}
    {canCreate ? <form onSubmit={(event) => { void add(event); }}>
      <h2>New task</h2>
      <label htmlFor="task-subject">Subject</label>{" "}
      <input id="task-subject" className="crm-input" required maxLength={200} disabled={busy}
        value={subject} onChange={(event) => { setSubject(event.target.value); setNewId(crypto.randomUUID()); }} />
      <label htmlFor="task-related">Related to</label>{" "}
      <select id="task-related" value={related} required disabled={busy}
        onChange={(event) => { setRelated(event.target.value); setNewId(crypto.randomUUID()); }}>
        <option value="">Select customer or deal</option>
        {parties.map((party) => <option key={party.partyRef?.partyId} value={"parties.party:" + party.partyRef?.partyId}>
          Customer: {party.displayName}
        </option>)}
        {deals.map((deal) => <option key={deal.dealId} value={"sales.deal:" + deal.dealId}>Deal: {deal.name}</option>)}
      </select>{" "}
      <button className="crm-button crm-button-primary" disabled={busy || !subject.trim() || !related || !session.actorId}>
        Save task
      </button>
      {!session.actorId ? <p role="status">The signed-in operator could not be verified for assigning this task.</p> : null}
    </form> : null}
    <h2>Task list</h2>
    {loading ? <FeedbackPanel tone="neutral" title="Loading tasks" busy /> : null}
    {!loading && tasks.length === 0 ? <FeedbackPanel tone="neutral" title="No tasks yet">Create a task for a customer or deal.</FeedbackPanel> : null}
    <ul>{tasks.map((task) => <li key={task.taskId}>
      <button type="button" disabled={busy} aria-pressed={selectedId === task.taskId}
        onClick={() => { setSelectedId(task.taskId); setEditSubject(task.subject); }}>
        {task.subject} · {relatedLabel(task)} · {task.status === 2 ? "Completed" : "Open"}
      </button>
    </li>)}</ul>
    {cursor ? <button type="button" disabled={loading} onClick={() => { void reload(cursor); }}>More tasks</button> : null}
    {current ? <section aria-label="Task card">
      <h2>{current.subject}</h2>
      <p>Related to: {relatedLabel(current)}</p>
      <p>Status: {current.status === 2 ? "Completed" : "Open"}</p>
      {canEdit && current.status !== 2 ? <form onSubmit={(event) => { void save(event); }}>
        <label htmlFor="edit-task">Subject</label>{" "}
        <input id="edit-task" className="crm-input" required maxLength={200} value={editSubject}
          disabled={busy} onChange={(event) => setEditSubject(event.target.value)} />
        <button disabled={busy || !editSubject.trim()}>Save changes</button>
      </form> : null}
      {canComplete && current.status !== 2 ? <button type="button" disabled={busy}
        onClick={() => { void complete(); }}>Complete task</button> : null}
    </section> : null}
  </div>;
}

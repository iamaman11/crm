import { useEffect, useRef, useState } from "react";
import { GovernedCrmClient, type Account, type Deal, type SessionState } from "@ultimate-crm/client";
import { FeedbackPanel, PageHeader } from "@ultimate-crm/ui";
import { crmMessage } from "./CrmPageUtils";

export function DealsPage({ client, session, capabilities }: {
  client: GovernedCrmClient;
  session: Extract<SessionState, { status: "authenticated" }>;
  capabilities: ReadonlySet<string>;
}) {
  const [deals, setDeals] = useState<Deal[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [cursor, setCursor] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [title, setTitle] = useState("");
  const [accountId, setAccountId] = useState("");
  const [newId, setNewId] = useState(() => crypto.randomUUID());
  const [editing, setEditing] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const selected = deals.find((deal) => deal.dealId === selectedId);
  const canCreate = capabilities.has("sales.deal.create");
  const canEdit = capabilities.has("sales.deal.update");
  const canStage = capabilities.has("sales.deal.advance_stage");
  const activeActorId = session.actorId;
  useEffect(() => {
    if (!selectedId) return;
    let active = true;
    client.getDeal(selectedId).then((deal) => {
      if (active && deal) setDeals((items) => items.map((item) => item.dealId === selectedId ? deal : item));
    }).catch((caught) => { if (active) setError(crmMessage(caught)); });
    return () => { active = false; };
  }, [client, selectedId]);

  async function reload(after = "") {
    setLoading(true); setError("");
    try {
      const result = await client.listDeals(after);
      setDeals((current) => after ? [...current, ...result.items] : result.items);
      setCursor(result.next);
      if (capabilities.has("accounts.account.list")) {
        const list = await client.listAccounts();
        setAccounts(list.items);
      }
    } catch (caught) { setError(crmMessage(caught)); }
    finally { setLoading(false); }
  }
  useEffect(() => { void reload(); }, [client]);

  async function run(action: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(""); setNotice("");
    try { await action(); }
    catch (caught) { setError(crmMessage(caught)); }
    finally { lock.current = false; setBusy(false); }
  }

  async function add(event: React.FormEvent) {
    event.preventDefault();
    if (!activeActorId || !accountId || !title.trim()) return;
    await run(async () => {
      const result = await client.createDeal(newId, title, activeActorId, session.tenantId, accountId);
      if (!result) throw new Error("Missing saved deal");
      setDeals((current) => [result, ...current.filter((deal) => deal.dealId !== result.dealId)]);
      setSelectedId(result.dealId);
      setEditing(result.name);
      setTitle(""); setNewId(crypto.randomUUID());
      setNotice("Deal saved.");
    });
  }
  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!selected) return;
    await run(async () => {
      const result = await client.updateDeal(selected, editing);
      if (result) setDeals((current) => current.map((deal) => deal.dealId === result.dealId ? result : deal));
      setNotice("Deal updated.");
    });
  }
  async function advance() {
    if (!selected) return;
    const next = selected.stageDetails?.ordinal === 1 ? "qualified"
      : selected.stageDetails?.ordinal === 2 ? "proposal"
      : selected.stageDetails?.ordinal === 3 ? "won" : null;
    if (!next) return;
    await run(async () => {
      const result = await client.advanceDeal(selected, next);
      if (result) setDeals((current) => current.map((deal) => deal.dealId === result.dealId ? result : deal));
      setNotice("Deal stage updated.");
    });
  }
  const linkedAccountName = (deal: Deal) => accounts.find((account) =>
    account.accountRef?.accountId === deal.account?.resourceId)?.name ?? "Customer account";

  return <div>
    <PageHeader eyebrow="Sales" title="Deals" description="Track customer opportunities and move them through stages." />
    {error ? <FeedbackPanel tone="danger" title="Request unavailable">{error}{" "}
      <button type="button" onClick={() => { void reload(); }}>Reload</button>
    </FeedbackPanel> : null}
    {notice ? <p role="status">{notice}</p> : null}
    {canCreate ? <form onSubmit={(event) => { void add(event); }}>
      <h2>New deal</h2>
      <label htmlFor="deal-name">Deal name</label>{" "}
      <input id="deal-name" required maxLength={200} className="crm-input" disabled={busy}
        value={title} onChange={(event) => { setTitle(event.target.value); setNewId(crypto.randomUUID()); }} />
      <label htmlFor="deal-customer">Customer</label>{" "}
      <select id="deal-customer" required value={accountId} disabled={busy}
        onChange={(event) => { setAccountId(event.target.value); setNewId(crypto.randomUUID()); }}>
        <option value="">Select customer</option>
        {accounts.map((account) => <option key={account.accountRef?.accountId}
          value={account.accountRef?.accountId}>{account.name}</option>)}
      </select>{" "}
      <button className="crm-button crm-button-primary" disabled={busy || !title.trim() || !accountId || !activeActorId}>
        Save deal
      </button>
      {!activeActorId ? <p role="status">The signed-in operator could not be verified for assigning this deal.</p> : null}
      {!accounts.length ? <p>Add a linked customer account in Customers first.</p> : null}
    </form> : null}
    <h2>Deal list</h2>
    {loading ? <FeedbackPanel tone="neutral" title="Loading deals" busy /> : null}
    {!loading && deals.length === 0 ? <FeedbackPanel tone="neutral" title="No deals yet">Create a deal for an existing customer.</FeedbackPanel> : null}
    <ul>{deals.map((deal) => <li key={deal.dealId}>
      <button type="button" disabled={busy} aria-pressed={selectedId === deal.dealId}
        onClick={() => { setSelectedId(deal.dealId); setEditing(deal.name); }}>
        {deal.name} · {linkedAccountName(deal)} · {deal.stageDetails?.stageId ?? "Open"}
      </button>
    </li>)}</ul>
    {cursor ? <button type="button" disabled={loading} onClick={() => { void reload(cursor); }}>More deals</button> : null}
    {selected ? <section aria-label="Deal card">
      <h2>{selected.name}</h2>
      <p>Customer: {linkedAccountName(selected)}</p>
      <p>Stage: {selected.stageDetails?.stageId ?? "Open"}</p>
      {canEdit ? <form onSubmit={(event) => { void save(event); }}>
        <label htmlFor="edit-deal-name">Deal name</label>{" "}
        <input id="edit-deal-name" className="crm-input" required maxLength={200} disabled={busy}
          value={editing} onChange={(event) => setEditing(event.target.value)} />
        <button disabled={busy || !editing.trim()}>Save changes</button>
      </form> : null}
      {canStage && selected.stageDetails?.ordinal && selected.stageDetails.ordinal < 4
        ? <button type="button" disabled={busy} onClick={() => { void advance(); }}>
          Advance to {selected.stageDetails.ordinal === 1 ? "Qualified" : selected.stageDetails.ordinal === 2 ? "Proposal" : "Won"}
        </button> : null}
    </section> : null}
  </div>;
}

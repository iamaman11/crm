import { useEffect, useRef, useState } from "react";
import {
  ContactPointKind, GovernedCrmClient, PartyKind,
  type Account, type ContactPoint, type Party,
} from "@ultimate-crm/client";
import { FeedbackPanel, PageHeader } from "@ultimate-crm/ui";
import { crmMessage, validContact } from "./CrmPageUtils";

type Props = { client: GovernedCrmClient; capabilities: ReadonlySet<string> };

export function CustomersPage({ client, capabilities }: Props) {
  const [items, setItems] = useState<Party[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [next, setNext] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [contacts, setContacts] = useState<ContactPoint[]>([]);
  const [search, setSearch] = useState("");
  const [newKind, setNewKind] = useState<PartyKind>(PartyKind.ORGANIZATION);
  const [newName, setNewName] = useState("");
  const [newId, setNewId] = useState(() => crypto.randomUUID());
  const [editingName, setEditingName] = useState("");
  const [contactKind, setContactKind] = useState<"email" | "phone">("email");
  const [contactValue, setContactValue] = useState("");
  const [contactId, setContactId] = useState(() => crypto.randomUUID());
  const [editingContact, setEditingContact] = useState("");
  const [editContactValue, setEditContactValue] = useState("");
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const [loading, setLoading] = useState(true);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const canCreate = capabilities.has("parties.party.create");
  const canEdit = capabilities.has("parties.party.update");
  const canContact = capabilities.has("contact-points.contact-point.create");
  const canEditContact = capabilities.has("contact-points.contact-point.update");
  const canAccounts = capabilities.has("accounts.account.create");
  const selected = items.find((party) => party.partyRef?.partyId === selectedId);
  const account = accounts.find((entry) =>
    entry.partyAssociations.some((a) => a.partyRef?.partyId === selectedId));

  async function reload(cursor = "") {
    setLoading(true); setError("");
    try {
      const result = await client.listParties(cursor);
      setItems((current) => cursor ? [...current, ...result.items] : result.items);
      setNext(result.next);
      if (capabilities.has("accounts.account.list")) {
        const list = await client.listAccounts();
        setAccounts(list.items);
      }
    } catch (caught) { setError(crmMessage(caught)); }
    finally { setLoading(false); }
  }

  useEffect(() => { void reload(); }, [client]);

  useEffect(() => {
    if (!selectedId) { setContacts([]); return; }
    let current = true;
    setLoadingDetail(true);
    Promise.all([client.getParty(selectedId), client.listContacts(selectedId)])
      .then(([party, result]) => {
        if (!current) return;
        if (party) {
          setItems((list) => list.map((item) =>
            item.partyRef?.partyId === selectedId ? party : item));
          setEditingName(party.displayName);
        }
        setContacts(result.items);
        setError("");
      })
      .catch((caught) => { if (current) setError(crmMessage(caught)); })
      .finally(() => { if (current) setLoadingDetail(false); });
    return () => { current = false; };
  }, [client, selectedId]);

  async function run(action: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setNotice(""); setError("");
    try { await action(); }
    catch (caught) { setError(crmMessage(caught)); }
    finally { lock.current = false; setBusy(false); }
  }

  async function addCustomer(event: React.FormEvent) {
    event.preventDefault();
    if (!newName.trim()) return;
    await run(async () => {
      const party = await client.createParty(newId, newKind, newName);
      if (!party?.partyRef?.partyId) throw new Error("Missing saved customer");
      setItems((list) => [party, ...list.filter((item) => item.partyRef?.partyId !== newId)]);
      setSelectedId(newId);
      setEditingName(party.displayName);
      setNotice("Customer saved.");
      const savedId = newId;
      setNewId(crypto.randomUUID()); setNewName("");
      if (canAccounts) {
        try {
          const linked = await client.createAccount("account-" + savedId, savedId, party.displayName);
          if (linked) setAccounts((existing) => [linked, ...existing]);
        } catch {
          setNotice("Customer saved. Account link not completed; use the retry button in the customer card.");
        }
      }
    });
  }

  async function linkAccount() {
    if (!selectedId || !selected) return;
    await run(async () => {
      const linked = await client.createAccount("account-" + selectedId, selectedId, selected.displayName);
      if (linked) setAccounts((existing) => [linked, ...existing]);
      setNotice("Customer account linked for deals.");
    });
  }

  async function saveCustomer(event: React.FormEvent) {
    event.preventDefault();
    if (!selected) return;
    await run(async () => {
      const result = await client.updateParty(selected, editingName);
      if (result) setItems((list) => list.map((entry) =>
        entry.partyRef?.partyId === selectedId ? result : entry));
      setNotice("Customer updated.");
    });
  }

  async function addContact(event: React.FormEvent) {
    event.preventDefault();
    if (!selectedId || !validContact(contactKind, contactValue)) {
      setError("Enter a valid email address or phone number.");
      return;
    }
    await run(async () => {
      const saved = await client.createContact(
        selectedId, contactId,
        contactKind === "email" ? ContactPointKind.EMAIL : ContactPointKind.PHONE, contactValue,
      );
      if (saved) setContacts((list) => [...list, saved]);
      setContactValue(""); setContactId(crypto.randomUUID());
      setNotice("Contact saved.");
    });
  }

  async function saveContact(event: React.FormEvent) {
    event.preventDefault();
    const contact = contacts.find((item) => item.contactPointRef?.contactPointId === editingContact);
    if (!contact) return;
    const kind = contact.kind === ContactPointKind.EMAIL ? "email" : "phone";
    if (!validContact(kind, editContactValue)) { setError("Enter a valid contact."); return; }
    await run(async () => {
      const saved = await client.updateContact(contact, editContactValue);
      if (saved) setContacts((list) => list.map((item) =>
        item.contactPointRef?.contactPointId === editingContact ? saved : item));
      setEditingContact(""); setNotice("Contact updated.");
    });
  }

  const visible = items.filter((party) =>
    party.displayName.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  return (
    <div>
      <PageHeader eyebrow="Customers" title="Customers" description="People and organizations, with their contact details." />
      {error ? <FeedbackPanel tone="danger" title="Request unavailable">{error}{" "}
        <button type="button" onClick={() => { void reload(); }}>Reload</button>
      </FeedbackPanel> : null}
      {notice ? <p role="status">{notice}</p> : null}
      {canCreate ? <form onSubmit={(event) => { void addCustomer(event); }}>
        <h2>Add customer</h2>
        <label htmlFor="customer-kind">Type</label>{" "}
        <select id="customer-kind" value={newKind} disabled={busy}
          onChange={(event) => setNewKind(Number(event.target.value) as PartyKind)}>
          <option value={PartyKind.ORGANIZATION}>Organization</option>
          <option value={PartyKind.PERSON}>Person</option>
        </select>{" "}
        <label htmlFor="customer-name">Name</label>{" "}
        <input id="customer-name" className="crm-input" required maxLength={200} value={newName}
          disabled={busy} onChange={(event) => { setNewName(event.target.value); setNewId(crypto.randomUUID()); }} />
        <button className="crm-button crm-button-primary" disabled={busy || !newName.trim()}>Save customer</button>
      </form> : null}
      <h2>Customer list</h2>
      <label htmlFor="customer-search">Find by name</label>{" "}
      <input id="customer-search" className="crm-input" value={search} onChange={(event) => setSearch(event.target.value)} />
      {loading ? <FeedbackPanel tone="neutral" title="Loading customers" busy /> : null}
      {!loading && items.length === 0 ? <FeedbackPanel tone="neutral" title="No customers yet">Add the first person or organization.</FeedbackPanel> : null}
      <ul>
        {visible.map((party) => <li key={party.partyRef?.partyId}>
          <button type="button" disabled={busy}
            aria-pressed={selectedId === party.partyRef?.partyId}
            onClick={() => setSelectedId(party.partyRef?.partyId ?? "")}>
            {party.displayName} · {party.kind === PartyKind.PERSON ? "Person" : "Organization"}
          </button>
        </li>)}
      </ul>
      {next ? <button type="button" disabled={loading} onClick={() => { void reload(next); }}>More customers</button> : null}
      {selected ? <section aria-label="Customer card">
        <h2>{selected.displayName}</h2>
        {loadingDetail ? <p role="status">Loading customer contacts…</p> : null}
        {canEdit ? <form onSubmit={(event) => { void saveCustomer(event); }}>
          <label htmlFor="edit-customer-name">Customer name</label>{" "}
          <input id="edit-customer-name" className="crm-input" required value={editingName}
            disabled={busy} maxLength={200} onChange={(event) => setEditingName(event.target.value)} />
          <button type="submit" disabled={busy || !editingName.trim()}>Save changes</button>
        </form> : null}
        {!account && canAccounts ? <p>No linked customer account.{" "}
          <button type="button" disabled={busy} onClick={() => { void linkAccount(); }}>Create account for deals</button>
        </p> : null}
        {account ? <p>Commercial customer account is linked.</p> : null}
        <h3>Contacts</h3>
        {!loadingDetail && contacts.length === 0 ? <p>No email or phone yet.</p> : null}
        <ul>{contacts.map((entry) => <li key={entry.contactPointRef?.contactPointId}>
          {entry.kind === ContactPointKind.EMAIL ? "Email" : "Phone"}: {entry.displayValue || entry.normalizedValue}{" "}
          {canEditContact ? <button type="button" disabled={busy} onClick={() => {
            setEditingContact(entry.contactPointRef?.contactPointId ?? "");
            setEditContactValue(entry.displayValue || entry.normalizedValue);
          }}>Edit</button> : null}
        </li>)}</ul>
        {editingContact ? <form onSubmit={(event) => { void saveContact(event); }}>
          <label htmlFor="edit-contact-value">Contact detail</label>{" "}
          <input id="edit-contact-value" className="crm-input" required value={editContactValue}
            disabled={busy} onChange={(event) => setEditContactValue(event.target.value)} />
          <button disabled={busy}>Save contact</button>
          <button type="button" onClick={() => setEditingContact("")}>Cancel</button>
        </form> : null}
        {canContact ? <form onSubmit={(event) => { void addContact(event); }}>
          <h4>Add contact</h4>
          <label htmlFor="contact-kind">Method</label>{" "}
          <select id="contact-kind" value={contactKind} disabled={busy}
            onChange={(event) => setContactKind(event.target.value as "email" | "phone")}>
            <option value="email">Email</option><option value="phone">Phone</option>
          </select>{" "}
          <label htmlFor="contact-value">Email or phone</label>{" "}
          <input id="contact-value" className="crm-input" value={contactValue} disabled={busy}
            required onChange={(event) => { setContactValue(event.target.value); setContactId(crypto.randomUUID()); }} />
          <button disabled={busy || !contactValue.trim()}>Save contact</button>
        </form> : null}
      </section> : null}
    </div>
  );
}

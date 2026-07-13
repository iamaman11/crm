# Phase 8A.3d — Customer 360 Composition Architecture

Status: **Normative delivery boundary for #110**  
Parent packet: #100  
Customer-master program: #28

## 1. Purpose

Customer 360 is a permission-aware, rebuildable read composition over canonical customer-master owners. It exists to make the current customer context useful to product surfaces and downstream governed readers without creating a second identity or customer master.

The composition root is a canonical `PartyRef`. Every surfaced Account, Contact Point or Party Relationship remains identifiable by its owning module's stable reference and source version.

## 2. Authoritative ownership

Customer 360 owns **no authoritative mutable business state**.

Authoritative state remains with:

- `crm.parties` — person and organization identity;
- `crm.customer-accounts` — Account lifecycle and typed Party associations;
- `crm.contact-points` — canonical endpoint, lifecycle, preference and verification state;
- `crm.party-relationships` — typed Party-to-Party relationship lifecycle and validity.

Customer 360 must never:

- issue an owner-domain mutation on behalf of a read request;
- expose private persistence rows as a public composition contract;
- treat denormalized projection fields as an alternate mutation target;
- infer consent, legal basis or send authorization from preferred or verified Contact Points;
- merge identities or perform survivorship;
- become generic relationship storage;
- give any pure owner module SQL, transport dependencies or direct cross-owner storage access.

## 3. Composition model

### 3.1 Root identity

One Customer 360 document is rooted by one canonical `crm.customer.v1.PartyRef`.

The root identity is stable even when derived sections are empty or temporarily behind. No Account, Contact Point or Party Relationship may create a competing Customer 360 identity.

### 3.2 Section references

The durable read model may denormalize bounded display fields for query efficiency, but every section entry must retain:

- source owner module id;
- canonical source resource reference;
- exact source resource version;
- source event identity or equivalent deterministic projection lineage where available.

Required initial sections:

- Party summary;
- Accounts associated with the root Party;
- Contact Points attached to the root Party;
- Party Relationships where the root Party is an endpoint.

A section item is not authoritative merely because it is materialized in a Customer 360 document.

### 3.3 Deterministic ordering

All repeated sections require deterministic ordering independent of database row order.

Initial ordering rules are defined by stable owner-domain references and explicit typed sort keys. Locale-dependent display ordering is not allowed to determine canonical projection identity.

## 4. Projection architecture

### 4.1 Event inputs

The composition consumes only exact published owner events for the initial slice:

- Party created/updated;
- Account created/updated;
- Contact Point created/updated/verified;
- Party Relationship created/updated.

Every consumed delivery must validate:

- tenant identity;
- source owner module;
- exact event type and event version;
- aggregate record type and id;
- aggregate version consistency;
- payload owner/schema/version/descriptor hash;
- data class and encoding.

Unknown or inconsistent deliveries fail projection processing rather than being silently accepted.

### 4.2 Rebuildability

The existing projection runtime remains the durable checkpoint/replay/rebuild authority.

Customer 360 projection documents are disposable. Deleting them and rebuilding from authoritative event history must reproduce the same logical documents for the same event history and projection version.

Projection rebuild operations must not modify:

- owner records;
- owner optimistic versions;
- mutation idempotency records;
- authoritative owner outbox history;
- owner audit evidence.

### 4.3 Fan-out and affected roots

A source event may affect one or more Party-rooted Customer 360 documents:

- Party event → that Party root;
- Contact Point event → referenced Party root;
- Account event → every associated Party root in the authoritative event payload;
- Party Relationship event → both endpoint Party roots.

The affected-root set must be deterministic and duplicate-free.

### 4.4 Freshness metadata

Each Customer 360 response must make projection state explicit rather than pretending synchronous cross-owner consistency.

The initial contract must expose bounded freshness metadata sufficient to identify:

- Customer 360 projection version;
- last applied projection/event position or equivalent deterministic checkpoint identity;
- source resource versions represented in each returned item.

The exact public shape is finalized with the v1 contract. Wall-clock timestamps alone are not sufficient as a correctness boundary.

## 5. Authorization model

Projection storage is a candidate/read optimization, not an authorization oracle.

A Customer 360 query must:

1. authenticate and resolve tenant/actor through the existing query gateway;
2. validate the exact Customer 360 query contract;
3. load the tenant-scoped projected candidate document;
4. perform live visibility checks against every source resource represented in the response;
5. apply field-level redaction for every visible source resource;
6. omit source resources that are no longer visible;
7. return the same safe non-disclosing result for a missing root and a root hidden by authorization where the existing query model requires non-disclosure.

A stale projection may delay visibility of newly created data, but it must never preserve disclosure after live authorization has been revoked.

## 6. Public query boundary

The initial public surface is read-only.

Expected first capability:

- `customer-360.customer.get@1.0.0`

The exact package/service/schema names are finalized additively before publication. There is no Customer 360 mutation capability.

The query contract must use canonical customer references and typed section/resource metadata. It must not expose:

- raw projection table keys;
- owner persistence envelopes;
- internal authorization decisions;
- generic untyped JSON maps as the primary business contract.

## 7. Application composition

The production composition belongs outside owner modules:

```text
owner mutations
→ immutable owner events
→ durable event delivery
→ Customer 360 projection handler
→ tenant-scoped rebuildable projection documents
→ governed Customer 360 query adapter
→ live source resource/field authorization
→ typed Customer 360 response
```

`crm-application-runtime` wires the projection worker/query adapter into the deployable `crm-api` process. Pure owner modules remain unchanged except for already-published contracts/events they own.

## 8. Failure semantics

- invalid source-event contract identity → projection failure/poison handling through the existing projection runtime;
- missing projected root → safe not-found behavior;
- hidden Party root → safe non-disclosure equivalent to not found;
- hidden section resource → omit that section item;
- hidden field → redact only that field according to the source resource visibility decision;
- projection lag → explicit freshness metadata, never fabricated synchronous consistency;
- projection deletion/corruption → rebuild, never owner-state reconstruction from the projection.

## 9. Acceptance gate

Fresh PostgreSQL plus a real `crm-api` process must prove:

1. governed prerequisite Party creation;
2. Account, Contact Point and Party Relationship creation only through existing governed capabilities;
3. deterministic convergence into one Party-rooted Customer 360 response;
4. exact stable source references and versions in every section;
5. owner updates converge without creating duplicate Customer 360 roots;
6. Contact Point verification and canonical-value reset behavior propagate from owner events;
7. Party Relationship lifecycle/validity changes propagate from owner events;
8. deterministic section ordering;
9. live source-resource and field authorization before disclosure;
10. unauthenticated rejection and cross-tenant non-disclosure;
11. explicit projection/source-version freshness metadata;
12. replay and rebuild produce the same logical Customer 360 documents;
13. deleting Customer 360 projection documents and rebuilding restores them;
14. rebuild leaves authoritative owner records, versions, outbox and audit evidence unchanged;
15. independent fresh-database process proof;
16. all applicable CI workflows green together on one exact final head SHA.

## 10. Non-goals

This packet does not deliver:

- consent or communication authorization;
- probabilistic identity matching or duplicate-candidate review;
- merge/unmerge, survivorship or field provenance;
- Account, Contact Point or Party Relationship mutations through Customer 360;
- provider delivery state or omnichannel conversation ownership;
- marketing segmentation or service-domain ownership.

Those remain explicit later owner/composition packets.

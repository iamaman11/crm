# Phase 8A.1 — Canonical Customer Identity and Reference Contracts

Issue: #92  
Parent program: #28  
Phase: 8A

## Objective

Establish stable cross-domain customer references and explicit owner-module identities before implementing the first Party production vertical slice.

The packet deliberately separates **wire compatibility** from **private domain implementation**:

- stable cross-module/process/language boundaries use versioned Protobuf;
- aggregate internals, matching algorithms, survivorship rules, repository rows and adapter implementation types remain native and private until intentionally published.

## Ownership map

### `crm.parties`

Owns canonical person and organization identity.

Published in 8A.1:

- `crm.customer.v1.PartyRef`;
- `crm.parties.v1.PartyKind`;
- minimal public `Party` representation;
- `CreatePartyRequest/Response`;
- `GetPartyRequest/Response`;
- `PartyCreatedEvent`.

Production aggregate behavior, persistence and governed application adapters are Phase 8A.2.

### `crm.customer-accounts`

Owns the customer/commercial relationship anchored to one or more Parties.

Published in 8A.1:

- immutable module identity;
- `crm.customer.v1.AccountRef`.

Full Account behavior is Phase 8A.3.

### `crm.contact-points`

Owns email, phone, postal and messaging endpoints plus future verification, validity and preference state.

Published in 8A.1:

- immutable module identity;
- `crm.customer.v1.ContactPointRef`.

Full Contact Point behavior is Phase 8A.3.

## Shared `crm.customer.v1` package

`crm.customer.v1` is a **contract namespace, not a business module**. It owns no mutable state, database tables, lifecycle or capabilities.

It contains only cross-owner primitives that downstream modules need without importing owner internals:

- `PartyRef`;
- `AccountRef`;
- `ContactPointRef`;
- `CustomerResourceVersion`.

References intentionally omit tenant identity. Public execution derives tenant and actor from the authenticated governed execution context; accepting caller-controlled tenant IDs in these customer references would weaken the tenant boundary.

## Party contract evolution rules

`PartyKind` uses an explicit zero `UNSPECIFIED` sentinel and additive numeric values. Unknown future enum numbers remain wire-preservable so older generated consumers do not silently rewrite a future kind into a known value.

The first `Party` message is intentionally minimal: identity reference, typed kind, display name and public version metadata. Structured person/organization profiles, provenance and identity-resolution evidence are additive follow-on contracts rather than a dump of private persisted state.

## Downstream rule

Sales, Service, Marketing, Billing, Projects and AI tools may hold stable customer references and domain-owned snapshots where explicitly justified. They may not:

- create a competing Party, Account or Contact Point master;
- mutate customer-master storage directly;
- import another owner module's internals;
- treat a revision hash, resource ID or reference as authorization;
- bypass governed capability/query paths.

Existing generic resource references remain compatible legacy fields until a downstream vertical slice can adopt canonical customer references without publishing wire fields that the runtime does not yet honor.

## Acceptance

The packet is complete only when:

1. Protobuf compilation, lint and binding validation succeed;
2. module manifests and contract bindings agree exactly;
3. descriptor identity tests cover the new messages;
4. generated Rust/browser contracts and contract hashes are synchronized;
5. new owner modules compile as infrastructure-free Foundations;
6. Governance and Rust architecture checks remain green;
7. one exact final SHA passes every applicable gate before merge.

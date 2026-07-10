# ADR-031: Exact CRM domain primitives and first owner modules

Status: Accepted

## Context

Phase 6 is the first point where the platform must prove that it can host recognizable modern CRM behavior without collapsing domain semantics into untyped JSON, generic scripts or cross-module table access. Sales and Activities are intentionally separate owner modules. The platform also needs common value objects for money, percentages, calendar dates, explicit patch semantics and cursor pagination so that every module does not invent incompatible representations.

The first slice establishes deterministic in-memory domain contracts and invariants. PostgreSQL planners, public capability composition, link-module event consumption and rebuildable projections remain separate reviewable slices under issue #9.

## Decision

### Shared exact primitives

`crm-core-contracts` owns:

- ISO-style three-letter uppercase `CurrencyCode`;
- exact `Money` expressed as integer minor units (`i128`), never binary floating point;
- bounded `BasisPoints` in the range 0..=10,000;
- validated Gregorian `CalendarDate` without implicit time zone;
- `Patch<T>` with distinct keep, set and clear semantics;
- bounded opaque cursor pagination with a maximum page size.

These are value objects only. Public command, event and RPC serialization remains governed by versioned Protobuf contracts in the later contract-publication slice. No hash or wire identity is derived from Rust debug/display output.

### Sales ownership

`crm-sales` exclusively owns the deal aggregate. Its first typed contract includes:

- immutable deal identity;
- actor/team ownership;
- optional stable account/contact resource references;
- versioned pipeline/stage identity and ordinal;
- exact amount/currency, expected close date and probability;
- open/won/lost lifecycle with mandatory close reason;
- optimistic versioning and monotonic mutation time;
- explicit transition policy for regression and skipped stages;
- bounded cursor list query contracts.

Closed deals cannot advance stage. A stage transition cannot silently change pipeline. Regressions require an explicit policy input. Terminal outcomes require a typed reason code.

### Activities ownership

`crm-activities` exclusively owns the task aggregate. Its first typed contract includes:

- subject, optional description, actor/team owner and priority;
- bounded unique related-resource references;
- due, reminder and completion times;
- optimistic versioning and monotonic mutation time;
- idempotent completion and identical reminder scheduling at the current version;
- prohibition on scheduling reminders after completion;
- bounded cursor list query contracts.

Completing a task clears its reminder. A reminder must be in the future relative to the scheduling command and must not be later than the due time.

### Independence

Neither owner module imports the other. Both depend only on `crm-core-contracts` and `crm-module-sdk`. Cross-domain behavior will be implemented by `crm-sales-activities-link` through a versioned event and `CapabilityClient`, never through source imports or shared tables.

## Consequences

- Standard CRM money, date, patch and pagination semantics now have one authoritative Rust value-object representation.
- Domain invariants can be tested without PostgreSQL or transport infrastructure.
- Module manifests declare independent versioned capabilities and events.
- The Sales manifest advances to module version `0.2.0`; existing capability/event versions retain their meaning.
- Activities is introduced as an independently buildable module.
- This ADR does not claim that wire contracts, persistence, projections or the full expert Sales/Activities product are complete.

## Follow-up gates

Before issue #9 can close:

1. publish mechanically checked versioned Protobuf contracts for commands, responses and events;
2. bind capability definitions and planners to the PostgreSQL transactional executor;
3. add the link module with deterministic event-delivery deduplication;
4. add tenant-scoped rebuildable projections;
5. prove the complete public API path, cross-tenant isolation and replay/rollback behavior against PostgreSQL.

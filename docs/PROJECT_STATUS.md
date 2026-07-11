# Ultimate CRM — Project Status

Status date: 2026-07-11

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`.

## Current position

The repository has completed the platform foundation and the first production Sales/Activities mutation and query paths through Phase 6H. The optional Sales–Activities link module core and its canonical Protobuf contract adapter are merged; production event-delivery composition is now the active Phase 6I work in long-lived draft PR #54.

Completed:

- repository governance and executable architecture rules;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module lifecycle and registry runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundation;
- capability execution gateway and authenticated HTTP/gRPC mutation ingress;
- independent Sales Deal and Activities Task owner-domain slices;
- generated Protobuf contract runtime and validated persisted-state codecs;
- production Sales/Activities mutation composition through PostgreSQL;
- permission-bound Deal/Task get/list queries with stable opaque cursor pagination;
- authenticated HTTP/gRPC query ingress with query-only execution context;
- optional `crm.sales-activities-link` pure module core;
- canonical Sales event / Activities command Protobuf adapter for the link module.

Current phase: **Phase 6 — first independent modular proof**.

Current active delivery packet: **PR #54 — Phase 6 runtime completion**.

Current active slice: **6I — production event delivery, lifecycle gating and PostgreSQL acceptance for the optional Sales–Activities link**.

## Phase 6 progress

| Slice | Result | State |
|---|---|---|
| 6A | Typed Sales/Activities domain contracts | Complete |
| 6B | Publication-compatible Protobuf contracts | Complete |
| 6C | Transactional audit materialization | Complete |
| 6D | Transaction-aware aggregate execution | Complete |
| 6E | Persisted codecs and generated contract runtime | Complete |
| 6F | Production Sales/Activities capability adapters | Complete |
| 6G | Authenticated production PostgreSQL mutations | Complete |
| 6H | Permission-bound production queries | Complete |
| 6I | Optional Sales–Activities link module | In progress |
| 6J | Rebuildable deal timeline and task-status projections | Planned in PR #54 delivery wave |
| 6K | Production `crm-api` application composition root | Planned in PR #54 delivery wave |
| 6L | Complete Phase 6 production E2E and closure | Planned in PR #54 delivery wave |

Phase 6 is not complete until 6I–6L are merged and the complete acceptance path is green.

## Active 6I implementation

Already merged to `main`:

- governed inbound `EventDelivery` contract;
- independently installable `crm.sales-activities-link` module core;
- deterministic delivery identity and link-owned receipt model;
- canonical Protobuf decoder for `sales.deal.stage_changed@1.0.0`;
- canonical encoder for `activities.task.create@1.0.0`;
- target invocation only through `CapabilityClient`.

Being completed in PR #54:

- generic host-side `EventDeliveryRuntime` and consumer lifecycle gate;
- FORCE-RLS PostgreSQL event reader over authoritative outbox evidence;
- immutable source module, actor, event version, correlation and trace lineage in outbox storage;
- gateway-backed in-process `CapabilityClient` that enters the same production `CapabilityGateway` as public mutations;
- durable PostgreSQL `ModuleStateStore` host adapter;
- target capability idempotency as the exactly-once business-effect anchor;
- process-level PostgreSQL acceptance for denial, retry, duplicate delivery, tenant isolation, suspend and uninstall behavior.

## Product readiness summary

The architecture and backend platform are substantially ahead of the breadth of end-user CRM functionality.

### Implemented business owner modules

- `crm.sales` — production Deal slice; broader Sales expert functionality remains planned.
- `crm.activities` — production Task slice; broader activity/calendar/productivity functionality remains planned.

### Integration module

- `crm.sales-activities-link` — optional link module; pure core and contract adapter are merged, production delivery composition is in progress.

### Important distinction

The workspace contains many technical crates, but those are platform components, not CRM business modules. Module counting follows `MODULE_CATALOG.md`.

### Not yet complete

- production completion of the optional Sales–Activities link module;
- rebuildable first projections;
- real deployable `crm-api` production composition root;
- complete first vertical-slice E2E closure;
- search and Admin Studio;
- web/mobile product shell and product-quality frontend;
- canonical customer master, identity resolution and consent;
- catalog, pricing, CPQ, quote-to-revenue lifecycle;
- communications, marketing, support/service and other expert domains;
- AI-native layer;
- signed marketplace/WASM sandbox;
- enterprise operational proof, restore/failover/security/SLO drills.

## Immediate delivery sequence

1. Complete 6I in PR #54 with real outbox delivery, active-installation checks, gateway-only target invocation, durable receipt recovery and PostgreSQL acceptance.
2. Continue the same coherent Phase 6 delivery wave with 6J rebuildable projections: tenant checkpoints, retries, replay, poison handling and deletion/rebuild equivalence.
3. Continue with 6K by turning `services/crm-api` from a skeleton into the production composition root without moving business rules into the service.
4. Complete 6L with one process-level production acceptance path covering independent modules, link enabled/disabled behavior, duplicate event delivery, tenant denial, stale conflict, rollback and projection rebuild.
5. Begin Phase 7 with search/projection platform expansion plus Admin Studio and frontend shell foundations.

## Delivery workflow

Development is organized as **coherent reviewable delivery packets**, not a branch or PR for every mechanical change. A long-lived implementation branch may contain multiple internal checkpoint commits while one architectural wave is being built. Strict Contract, Governance, Rust and Database gates still protect merge boundaries.

For `agent/**` development branches, the permanent Agent Checkpoint workflow synchronizes `cargo fmt` and the resolved `Cargo.lock` mechanically. Temporary lockfile/format helper workflows must not be created for ordinary development.

## Documentation hygiene rule

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.

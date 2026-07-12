# Ultimate CRM — Project Status

Status date: 2026-07-12

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`.

## Current position

**Phase 6 is complete.** PR #63 was merged into `main` as merge commit `82910fa17f21074b1e091615a4251092cfa8ab2f` after the final one-commit review head passed all required gates.

The repository now contains a complete first production-composed modular CRM proof:

- repository governance and executable architecture rules;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module lifecycle and registry runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundation;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task owner-domain vertical slices;
- governed inbound `EventDelivery` and restart-safe event lineage;
- independently governed `crm.sales-activities-link` module;
- durable consumer delivery ledger with lease, retry, recovery and dead-letter behavior;
- lifecycle-aware Sales-to-Activities processing through the production `CapabilityGateway`;
- rebuildable Deal timeline and Task status projections with tenant checkpoints and replay;
- real `crm-application-runtime` composition boundary and thin deployable `services/crm-api` process host;
- governed HTTP mutation/query endpoints and versioned gRPC application gateway;
- health, readiness, background workers and graceful shutdown;
- process-level acceptance covering real `crm-api`, PostgreSQL, HTTP, gRPC, link delivery and projections.

Current phase: **Phase 7 — In progress**.

The golden module foundation (#56 / merged PR #64) is complete as `15bf3ddeac0375325a3c59518e3ac55a3903c20d`.

The generalized projection runtime (#65 / merged PR #67) is complete on `main` as `195448ab3cd70fe051967faf4f8ed87372fb3551`.

The current Phase 7 packet is **#66 / draft PR #68 — tenant- and permission-aware search with deterministic reindexing**. The packet now includes the search runtime, PostgreSQL generation/index adapter, governed public query capability, live permission re-checking, application-runtime composition and canonical migration gates. It remains **In progress** until one exact review head passes all required checks and the remaining gate findings are resolved.

## Phase 6 completion

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
| 6I | Optional Sales–Activities link module and production event delivery | Complete — merged in PR #63 |
| 6J | Rebuildable Deal timeline and Task status projections | Complete — merged in PR #63 |
| 6K | Production `crm-api` application composition root | Complete — merged in PR #63 |
| 6L | Complete Phase 6 process-level production E2E and closure | Complete — merged in PR #63 |

Final review head `25793548e46bdbd57312a513b4e9ffbceb33a2c1` passed Contract CI, Governance CI, Rust CI, Database CI, Event Runtime CI, Application Runtime CI and generic Rust Generated Sync simultaneously before merge.

## Phase 7 foundations

### Golden module foundation — Complete

Issue #56 / merged PR #64 established the repository-supported module creation path:

- separate governed scaffolding for authoritative owner modules and optional link modules;
- explicit owner-object and link-dependency decisions before generation;
- overwrite-safe generation, workspace duplicate protection, dependency-range validation and `--dry-run`;
- architecture-safe module crate/manifests plus explicit contract, adapter and acceptance-test TODO boundaries;
- a compiling ignored acceptance-test scaffold gate that must be replaced before module readiness can rise above Foundation;
- stable cross-platform commands for architecture checks, manifest validation, formatting, lockfile synchronization, focused tests, full tests and the common Rust quality gate;
- Governance CI that validates generated manifests, compiles a freshly generated module with `cargo check --all-targets`, and verifies generated dependencies against `architecture-policy.json`.

A generated module is **Foundation only** and does not count as a production vertical slice.

### Generalized projection runtime — Complete

Issue #65 / merged PR #67 generalized the Phase 6 projection proof without moving business-event decoding into infrastructure:

- `crm-projection-runtime` owns typed projection registration, checkpoint-based history paging, deterministic handler execution, poison/failure handling and rebuild orchestration;
- `crm-core-events` exposes the platform `ProjectionStore` port and failure contract;
- `crm-core-data` adapts the existing PostgreSQL projection tables/runtime to that port without a new migration;
- a deterministic handler failure marks the projection checkpoint failed without advancing the last successful cursor and blocks further replay until reset or repair;
- the existing Deal timeline and Task status handlers remain concrete composition-layer handlers but execute through the generic runner;
- the existing `Phase6ProjectionWorker` remains only as a compatibility facade around the generic runner;
- dedicated `Projection Runtime CI` proves failed-checkpoint persistence/reset and existing Deal/Task rebuild behavior against real PostgreSQL.

The generic runtime has no Sales, Activities or PostgreSQL implementation dependency. PR #67 is merged and #65 is complete.

### Permission-aware search and deterministic reindexing — In progress

Issue #66 / draft PR #68 is the active executable packet:

- the search index is rebuildable and candidate-only, never authoritative for permissions or business state;
- every candidate is checked against live resource and field visibility before resource identity, fields or match metadata may be disclosed;
- logical search generations reuse the generalized projection runtime and keep the previous active generation queryable while a replacement is built;
- rebuilding the currently active generation in place is rejected before projection reset;
- generation coordinates are immutable after the building lifecycle state;
- PostgreSQL remains a replaceable search adapter with FORCE-RLS generation metadata and deterministic ordering;
- `search.global.query` is a versioned read-only capability routed through the governed production `QueryGateway`;
- the application runtime composes search indexing/catch-up and the production query router;
- acceptance covers immediate permission revocation, hidden-field non-disclosure, generation switching, deterministic cursor progression and cross-tenant isolation;
- Search Runtime CI, canonical Database CI and Application Runtime CI include the Phase 7B schema/runtime paths.

Current gate work is compile/Clippy/test stabilization and exact-head evidence. The packet is not complete and must not be merged until the required checks are simultaneously green on one review head.

## Product readiness summary

The architecture and backend platform now have a complete first production-composed modular proof. The breadth of end-user CRM functionality is still intentionally much smaller than the target universal expert CRM.

### Implemented business owner modules

- `crm.sales` — production Deal vertical slice; broader Sales expert functionality remains planned.
- `crm.activities` — production Task vertical slice; broader activity/calendar/productivity functionality remains planned.

### Implemented link module

- `crm.sales-activities-link` — independently governed optional link module with pure core, published contract adapter, durable event delivery, lifecycle gating and production end-to-end acceptance.

### Not yet complete

- Phase 7B search packet gate closure and merge — #66 / PR #68;
- Admin Studio metadata builders and publication workflows;
- web/mobile product shell and product-quality frontend;
- canonical customer master, identity resolution and consent;
- catalog, pricing, CPQ and quote-to-revenue lifecycle;
- communications, marketing, support/service and other expert domains;
- AI-native layer;
- signed marketplace/WASM sandbox;
- enterprise operational proof, restore/failover/security/SLO drills.

## Immediate delivery sequence

1. Finish exact-head gate stabilization for tenant- and permission-aware search, then merge #66 / PR #68 only after all required checks are green together.
2. Build the typed web product shell, generated client boundary, authentication/session integration, permission-aware routing and design-system baseline.
3. Build Admin Studio metadata publication foundations with validation, auditability and rollback.
4. Begin the domain-wave program tracked by #57; keep customer master/identity/consent (#28) and catalog/CPQ/commercial lifecycle (#29) as explicit owner-domain programs rather than absorbing them into Sales.
5. Continue frontend and expert backend modules as end-to-end vertical slices.

## Development mode

- one branch per coherent delivery packet, not per mechanical edit;
- incremental commits are allowed during implementation;
- full CI runs at architecture, behavior and final-delivery checkpoints;
- final PR history is reduced to semantic commits where repository tooling permits;
- architecture, contract, tenant, authorization, audit and rollback gates remain strict.

See `DEVELOPMENT_WORKFLOW.md` and `MODULE_DEVELOPMENT.md`.

## Documentation hygiene rule

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.

# Ultimate CRM — Project Status

Status date: 2026-07-12

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

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

The tenant- and permission-aware search packet **#66 / merged PR #68** is complete. It delivers the search runtime, PostgreSQL generation/index adapter, governed public query capability, backend-consistent field-local match evidence, live permission re-checking, application-runtime composition and canonical migration gates. Final review head `90d8ad4afc15ba31bc27297e4a9c7081e64ac4e7` passed all required checks and was squash-merged into `main` as `49272918cb4b767eedc2ca34574abba40718eae1`.

The exact-SHA two-agent development system **#70 / merged PR #72** is complete on `main` as `ae3bcd7a0ac23f1db0e969488a3085c3d33e0b42`. The active product-plane packet is **#71 / draft PR #73** (in progress / architecture review candidate preparation).

The qualification is complete. Codex is qualified as:
* Codex — LEVEL_4_CAPABLE, EFFECTIVE LEVEL_3_CO_IMPLEMENTER
* Level 4 promotion pending successful end-to-end closure of #71.

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

### Permission-aware search and deterministic reindexing — Complete

Issue #66 / merged PR #68 completed the production search foundation:

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

Final review head `90d8ad4afc15ba31bc27297e4a9c7081e64ac4e7` passed Contract CI, Governance CI, Rust CI, Database CI, Projection Runtime CI, Event Runtime CI, Search Runtime CI, Application Runtime CI and Rust Generated Sync simultaneously before squash merge as `49272918cb4b767eedc2ca34574abba40718eae1`.

## Development system v2

Issue #70 / merged PR #72 formalized the repository's exact-SHA multi-agent model:

- the **Architect / Implementer** owns packet scope, architecture, contracts, primary implementation, tests, fixes and checkpoint publication;
- the **Local Integrator / Verifier** checks an exact immutable SHA in a complete local toolchain and returns reproducible structured evidence;
- overlapping code has one primary writer at a time;
- every verification handoff names branch, exact SHA, mode, scope, environment and required commands;
- every report names the exact SHA actually tested and explicitly lists anything unverified;
- a new commit invalidates green evidence for checks not rerun on the new SHA;
- local checkpoints mirror architecture, behavior and delivery stages;
- GitHub CI remains the final exact-head merge authority.

Issue #74 adds capability-based qualification for the ChatGPT Codex local agent:

- a qualified local agent should maintain a persistent checkout and report its real absolute path, origin, branch, HEAD and worktree state;
- responsibility is graded from Level 1 exact-SHA verifier through Level 2 local integrator, Level 3 co-implementer and Level 4 delivery-packet owner;
- higher capability should receive higher responsibility rather than being artificially limited to passive verification;
- parallel implementation is allowed only on explicit non-overlapping workstreams or through an explicit writer handoff;
- the active packet's published authority remains unchanged until qualification evidence is reviewed.

The standard coordination signals are `SECOND_AGENT_NOT_NEEDED`, `CONNECT_SECOND_AGENT`, `SECOND_AGENT_REPORT_NEEDED` and `READY_FOR_EXACT_HEAD_CI`. The committed issue/branch/PR/SHA state remains authoritative over chat-only coordination.

## Product readiness summary

The architecture and backend platform now have a complete first production-composed modular proof. The breadth of end-user CRM functionality is still intentionally much smaller than the target universal expert CRM.

### Implemented business owner modules

- `crm.sales` — production Deal vertical slice; broader Sales expert functionality remains planned.
- `crm.activities` — production Task vertical slice; broader activity/calendar/productivity functionality remains planned.

### Implemented link module

- `crm.sales-activities-link` — independently governed optional link module with pure core, published contract adapter, durable event delivery, lifecycle gating and production end-to-end acceptance.

### Not yet complete

- Admin Studio metadata builders and publication workflows;
- product-quality web/mobile shell and broad CRM frontend experience — foundation in progress in #71 / PR #73;
- canonical customer master, identity resolution and consent;
- catalog, pricing, CPQ and quote-to-revenue lifecycle;
- communications, marketing, support/service and other expert domains;
- AI-native layer;
- signed marketplace/WASM sandbox;
- enterprise operational proof, restore/failover/security/SLO drills.

## Immediate delivery sequence

1. Qualify the ChatGPT Codex local agent through #74, then assign the maximum safe responsibility level for #71 and later packets.
2. Complete #71 / PR #73: typed web product shell, generated client boundary, authentication/session integration, permission-aware routing and design-system baseline.
3. Build Admin Studio metadata publication foundations with validation, auditability and rollback.
4. Begin the domain-wave program tracked by #57; keep customer master/identity/consent (#28) and catalog/CPQ/commercial lifecycle (#29) as explicit owner-domain programs rather than absorbing them into Sales.
5. Continue frontend and expert backend modules as end-to-end vertical slices.

## Development mode

- one branch per coherent delivery packet, not per mechanical edit;
- incremental commits are allowed during implementation;
- one primary writer at a time for overlapping multi-agent scope;
- exact-SHA local handoffs may be used at architecture, behavior and delivery checkpoints;
- verifier mode defaults to `VERIFY_ONLY` only until a packet-specific handoff or qualification grants broader authority;
- qualified agents may own bounded integration fixes, non-overlapping workstreams or full delivery packets according to `CODEX_AGENT_QUALIFICATION.md`;
- full GitHub CI remains mandatory on the exact final review head;
- final PR history is reduced to semantic commits where repository tooling permits;
- architecture, contract, tenant, authorization, audit and rollback gates remain strict.

See `DEVELOPMENT_WORKFLOW.md`, `MULTI_AGENT_DEVELOPMENT.md`, `CODEX_AGENT_QUALIFICATION.md` and `MODULE_DEVELOPMENT.md`.

## Documentation hygiene rule

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.

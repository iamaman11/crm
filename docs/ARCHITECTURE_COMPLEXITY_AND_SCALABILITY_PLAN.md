# Ultimate CRM — Architecture Complexity, Scalability and Developer Experience 10/10 Plan

Status: **Normative cross-cutting execution plan**  
Tracking issue: #194  
Original audit baseline: **2026-07-27**  
Current execution checkpoint: **2026-08-02**

This plan governs repository structure, Rust workspace packaging, module composition, dependency and exception governance, contracts, persistence ownership, CI/test selection, developer tooling, documentation navigation, local development, frontend architecture and production operations.

Governing precedence:

1. `SYSTEM_INVARIANTS.md`;
2. published contracts and accepted ADRs;
3. `APPLICATION_ARCHITECTURE.md` and accepted readiness evidence;
4. this execution plan;
5. descriptive module, packet and orientation documentation.

This document is the **single current execution plan for architecture complexity and developer experience**. `PROJECT_STATUS.md` records the concise checkpoint; `repository-packet.json` declares the one active bounded packet; accepted packet documents and pull requests remain immutable historical evidence.

## 1. Executive decision

The foundational architecture is sound and must not be replaced.

The long-term model remains:

- modular monolith with independently governed owner, link and read-composition modules;
- one authoritative owner for every mutable aggregate;
- pure domain code behind stable contracts and governed ports;
- no direct cross-module storage access;
- exact versioned mutation, query, event and worker coordinates;
- durable tenant activation and live authorization;
- transactional state, idempotency, outbox and audit evidence;
- FORCE RLS and cross-tenant negative proof;
- rebuildable non-authoritative projections, search and caches;
- unchanged exact-head acceptance discipline.

The remaining risk is accidental complexity: unnecessary physical packages, central owner-specific composition, dependency/feature divergence, copied acceptance wiring, expanding change fan-out, incomplete local lifecycle automation, contract retirement gaps, frontend/operations parity gaps and documentation drift.

The required direction is:

> Preserve strict ownership, security and governed runtime boundaries while making the normal cost of adding or changing one capability close to constant with respect to total product size.

No big-bang rewrite, premature microservice split, broad package collapse or weakening of acceptance rules is authorized.

## 2. Baseline and current checkpoint

### 2.1 Original expert baseline

The 2026-07-27 audit recorded planning signals, not completion claims:

| Dimension | Baseline | Target |
|---|---:|---:|
| Business modularity | 9.4/10 | 10/10 |
| Layering | 9.1/10 | 10/10 |
| Architecture purity | 8.7/10 | 10/10 |
| Change isolation and safety | 9.5/10 | 10/10 |
| Extensibility cost | 7.4/10 | 10/10 |
| Developer comprehension | 7.5/10 | 10/10 |
| Build and CI scalability | 6.9/10 | 10/10 |
| Local development reproducibility | 6.5/10 | 10/10 |
| Overall architecture maturity | 8.3/10 | 10/10 |

No score is raised by declaration. The same dimensions must be mechanically remeasured at Steps 22 and 25.

### 2.2 Workspace history and current fact

The historical Step 13 measurement established 113 workspace packages, 841 internal dependency edges, maximum dependency depth 18 and a conservative public Rust surface of 5,379 items before the accepted consolidation.

Repository Step 14 is accepted through PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d44691371c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head.

Current blocking baseline after Step 14:

| Metric | Historical Step 13 | Current |
|---|---:|---:|
| Workspace packages | 113 | **112** |
| Internal dependency edges | 841 | **835** |
| Maximum dependency depth | 18 | **18** |
| Conservative public Rust items | 5,379 | **5,377** |
| Dependency declarations | 270 | **270** |
| Workspace dependency declarations | 4 | **4** |
| Heavy-feature declarations | 65 | **65** |
| Suppression occurrences | 91 after direct-lint retirement | **91** |

An ordinary capability added to an existing owner creates **zero new crates** by enforced default. That is not an absolute ban: a new authoritative owner normally creates three to five technical packages, and a real provider, secrets, KMS/HSM, trust, process, extraction or compiler-enforced visibility boundary may justify a dedicated crate after architecture preflight.

### 2.3 Current program position

| Stage | Current state | Accepted evidence | Remaining exit work |
|---|---|---|---|
| A — documentation and policy baseline | **Complete** | one source hierarchy, stable index, generated navigation and permanent consistency guards | preserve freshness and avoid duplicate live roadmaps |
| B — dependency, crate and exception governance | **Complete** | PRs #253, #255 and #257: reproducible measurement, exact Rust 1.97.1, zero-warning Rust/Clippy, lockfile preservation, blocking suppression/direct-lint/process-host/change-cost/dependency governance | preserve non-growth and remeasure at Steps 22 and 25 |
| C — golden owner package and persistence model | **In progress** | Customer Privacy golden package, final subject policy, restriction/legal-hold placement, retention precedence, durable owner execution/outcomes, governed access/export and owner actions | Party tombstone/convergence at Step 15, worker lifecycle at Step 19, Phase 8A closure at Step 21 |
| D — contribution aggregation | **Complete** | all active first-party owner contributions aggregated through `crm-first-party-modules` by PRs #246, #248 and #249 | preserve bounded owner-owned contribution boundaries |
| E — affected-scope CI | **Complete** | PR #239: deterministic Rust closure and declarative contract/API/migration/PostgreSQL/process/product/frontend/operations ownership with unknown-path fail closed | preserve policy/workflow compatibility |
| F — generic conformance and contract lifecycle | **In progress** | reusable mutation/query conformance accepted through PR #235 | worker conformance Step 16, contract lifecycle Step 17, real worker adoption Step 19 |
| G — transitional consolidation | **Complete** | PR #259 removes one redundant Customer Accounts package behavior-neutrally and lowers measured budgets | preserve the reduction and prove later changes remain bounded |
| H — reproducible environment and navigation | **In progress** | `affected`, `check-affected`, `explain`, fail-closed `packet-check`, generated active packet and repository map | deterministic `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo`, `smoke` at Step 18 |
| I — frontend and operations parity | **Incomplete** | existing product/process checks remain mandatory | frontend/accessibility/browser and restore/SLO/performance/security/supply-chain evidence at Steps 20–21 |

Stages are completion ledgers, not parallel implementation queues. Repository steps are the only executable order.

### 2.4 Single repository execution order

At most one implementation packet may be active. The next permitted packet is the first unfinished item. No item may be described as “next” when an earlier unfinished item exists.

1. supported Rust toolchain, workspace `rust-version` and measured lint baseline — **complete through PR #218**;
2. Customer Privacy approval runtime — **complete through PR #220**;
3. first bounded contribution-aggregation packet — **complete through PR #222**;
4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **complete through PR #226**;
5. `repo.py explain`, `repo.py packet-check`, generated active packet and repository map — **complete through PR #228**;
6. Customer Privacy legal-hold and mandatory-retention precedence — **complete through PR #230**;
7. reusable generic mutation and query conformance suites — **complete through PR #235**;
8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — **complete through PR #237**;
9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — **complete through PR #239**;
10. governed Customer Privacy access/export assembly — **complete through PR #241**;
11. owner-specific deletion, anonymization and supported crypto-shred execution — **complete through PR #244**;
12. complete first-party contribution aggregation for all currently active owners — **complete through PR #249**;
13. complete ADR-031 governance closure: measurement, blocking suppression/direct-lint governance and remaining process-host/change-cost/dependency exit evidence — **complete through PR #257**;
14. first measured behavior-neutral transitional domain-cluster consolidation — **complete through PR #259**;
15. Party tombstone, no-orphan proof and projection/search/cache convergence — **next, not started**;
16. reusable generic worker conformance adopted by representative real workers;
17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement;
18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;
19. real Customer Privacy worker lifecycle and complete process/end-to-end acceptance;
20. Phase 8A frontend, accessibility, browser and operations evidence;
21. Phase 8A closure;
22. Phase 8A architecture remeasurement and remaining-gate review — **checkpoint, not final 10/10**;
23. first Phase 8B expert-domain wave proving bounded extension cost;
24. second contrasting expert-domain wave proving bounded extension cost as module count grows;
25. final architecture 10/10 closure review only when every Section 12 criterion is mechanically proven.

Feature behavior and crate consolidation must be separate PRs. Documentation evidence synchronization must be separate from implementation when the packet rules require an accepted implementation merge first.

## 3. Non-negotiable architecture invariants

Every later packet must preserve:

- authoritative aggregate ownership and no direct cross-owner storage bypass;
- tenant activation, live authorization, FORCE RLS and cross-tenant concealment;
- transaction, idempotency, audit and outbox atomicity;
- exact versioned public coordinates and route classifications;
- immutable lineage and strict rehydration for durable evidence;
- owner-owned persistence and migration rollback/reapply proof;
- rebuildability of projections, search and caches;
- fail-closed affected scope and unchanged meaningful exact-head acceptance;
- no temporary workflow, hidden suppression, untracked exception or undocumented budget increase.

## 4. Target package and composition model

### 4.1 Ordinary capability cost

A normal capability added to an existing owner should:

- create zero new crates;
- touch the owner domain/application/persistence/production closure only;
- avoid generic-runtime and process-host changes;
- reuse owner-local mutation/query/worker contribution factories;
- select only the affected package and permanent workflow closure;
- add no unregistered lint, feature or architecture exception.

### 4.2 New owner cost

A new authoritative owner normally uses three to five technical packages:

- domain/contracts;
- application/ports;
- persistence adapter;
- production contribution;
- optional process/provider boundary only when independently justified.

Package names do not prove architecture. Ownership, dependency direction, persistence, security and process boundaries do.

### 4.3 Transitional consolidation rules

A package may be consolidated only when measurement proves that it has no independent ownership, persistence, security, publication, process or compiler-enforced visibility boundary.

Stop immediately when a candidate owns:

- a distinct authoritative aggregate or migration set;
- transaction-scoped security/final-policy enforcement;
- process isolation, provider trust, secrets or KMS/HSM concerns;
- independently versioned public contracts;
- a separately governed affected-scope or lifecycle boundary.

PR #259 is the first accepted proof: `crm-customer-accounts-capability-composition` had one production consumer and no independent boundary, so its contribution moved into `crm-customer-accounts-query-adapter` without behavior change.

## 5. Dependency, public-surface and exception governance

Blocking policies must permit reductions and reject unmeasured growth.

Required controls:

- root `[workspace.dependencies]` for accepted shared versions where centralization is proven safe;
- accepted version/feature divergence recorded explicitly;
- no direct package lint tables outside an exact time-bounded exception;
- no new source-level `allow` or `expect` equivalent to a retired suppression;
- role-aware budgets for central runtimes, SDK/contracts, infrastructure ports and process hosts;
- direct dependency and transitive reverse-impact non-growth;
- conservative public Rust item non-growth unless a versioned public API need is proven;
- exact exception owner, scope, reason, compensating checks, expiry and removal condition;
- expired exceptions equal zero.

Current exact reduction budgets are stored in `step13-complexity-policy.json`; Rust adoption cohorts are stored in `rust-governance-policy.json`.

## 6. Affected-scope and CI scalability

Affected Scope CI is a correctness boundary, not an optimization hint.

Every changed path must resolve to:

- an owner or repository responsibility;
- affected Rust package/reverse closure where applicable;
- required permanent workflows;
- a fail-closed reason when impact is unknown.

Contracts, Protobuf/API, migrations, PostgreSQL, process/runtime, product, frontend and operations changes have explicit planes. Workflow filters and policy selections must remain mechanically compatible.

Representative leaf changes should avoid unrelated full-workspace fan-out. Any broadening requires a machine-readable reason and must be remeasured at Steps 22–24.

## 7. Generic conformance and contract lifecycle

Mutation and query conformance are accepted. Remaining work must prove:

- reusable worker conformance with activation, authorization, tenant, replay, lease/crash and no-side-effect guarantees;
- published-version compatibility checks;
- deprecation telemetry and consumer inventory;
- governed consumer migration;
- explicit retirement gates that prevent removal while live consumers remain.

Owner-specific semantics stay outside generic conformance algorithms.

## 8. Reproducible local development and navigation

Accepted navigation:

- `python scripts/repo.py affected`;
- `python scripts/repo.py check-affected`;
- `python scripts/repo.py explain <module-or-coordinate>`;
- `python scripts/repo.py packet-check --base origin/main`;
- generated `docs/ACTIVE_PACKET.md`;
- generated `docs/generated/REPOSITORY_MAP.md`.

Step 18 must add deterministic clean-machine commands:

- `python scripts/repo.py doctor`;
- `python scripts/repo.py bootstrap`;
- `python scripts/repo.py dev-up`;
- `python scripts/repo.py dev-reset`;
- `python scripts/repo.py seed-demo`;
- `python scripts/repo.py smoke`.

They must be pinned, repeatable, safe, production-aligned and proven on a clean environment.

## 9. Frontend and operations parity

Before Phase 8A closure, critical journeys require:

- domain-oriented frontend ownership and bounded dependency direction;
- component, browser and accessibility acceptance;
- permission-denial and cross-tenant negative proof;
- deterministic demo/seed state;
- executable backup/restore and disaster-recovery proof;
- SLO, observability, performance, security and supply-chain gates.

Backend completeness alone cannot close a product module or Phase 8A.

## 10. Accepted Step 14 closure evidence

PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d44691371c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged exact head proves:

- one redundant transitional package removed;
- exact Customer Accounts mutation/query inventory and contribution ordering preserved;
- activation gates, Party-reference validation and live query visibility preserved;
- contracts, Protobuf, routes, schemas, migrations, persistence and workers unchanged;
- tenant isolation, FORCE RLS, live authorization, idempotency, audit and transaction semantics unchanged;
- workspace packages reduced 113 → 112;
- internal dependency edges reduced 841 → 835;
- conservative public Rust items reduced 5,379 → 5,377;
- maximum dependency depth remains 18;
- dependency declarations remain 270;
- suppression occurrences remain 91;
- Approval, Discovery and Planning permanent workflow package baselines synchronized to 112 without removing behavioral, rollback or reapply checks.

This completes Repository Step 14 and Stage G only. It does not complete Customer Privacy, Phase 8A or architecture 10/10.

## 11. Next bounded packet — Repository Step 15

Repository Step 15 is **next and not started**.

Required scope:

- authoritative Party tombstone semantics;
- no-orphan proof for owned and referencing records;
- deterministic projection, search and cache convergence;
- tenant/RLS/authorization/audit/idempotency preservation;
- rollback, replay and crash-window evidence;
- no unrelated worker, contract-lifecycle, local-environment, frontend or operations work.

The packet must start from current `main`, declare exact allowed/forbidden paths and pass every applicable permanent workflow on one unchanged meaningful user-authored head.

## 12. Final architecture 10/10 closure criteria

Issue #194 may close and architecture 10/10 may be declared only when all are mechanically proven:

1. one authoritative current-state hierarchy with no contradictory live roadmap;
2. ordinary existing-owner capabilities add zero crates and avoid generic-runtime edits;
3. new owner waves remain within the measured three-to-five-package target unless an independent boundary is proven;
4. central dependency, reverse-impact, public-surface and process-host budgets do not regress;
5. expired exceptions, hidden suppressions and unregistered lint bypasses are zero;
6. affected-scope policy and permanent workflow filters remain exact and fail closed;
7. reusable mutation, query and worker conformance are adopted by contrasting real owners;
8. contract compatibility, deprecation, consumer migration and retirement are permanently enforced;
9. local lifecycle commands are deterministic on a clean machine;
10. frontend, accessibility, browser and production operations proof exists for critical journeys;
11. Phase 8A closes without weakening owner, tenant, RLS, authorization, audit, rollback or route invariants;
12. Step 22 remeasurement shows no hidden regression;
13. two contrasting later expert-domain waves at Steps 23 and 24 keep extension cost bounded as module count grows;
14. a separate Step 25 review reproduces the metrics and confirms every prior criterion.

Until then:

- architecture 10/10 remains unclaimed;
- Phase 8A and Customer Privacy remain incomplete;
- current product-complete expert modules remain **0**.

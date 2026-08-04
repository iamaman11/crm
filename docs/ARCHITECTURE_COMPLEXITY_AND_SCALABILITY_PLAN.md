# Ultimate CRM — Architecture Complexity, Scalability and Developer Experience 10/10 Plan

Status: **Normative cross-cutting execution plan**  
Tracking issue: #194  
Original audit baseline: **2026-07-27**  
Current execution checkpoint: **2026-08-03**

This plan governs repository structure, Rust workspace packaging, module composition, dependency and exception governance, contracts, persistence ownership, CI/test selection, developer tooling, documentation navigation, local development, frontend architecture and production operations.

Governing precedence:

1. `SYSTEM_INVARIANTS.md`;
2. published contracts and accepted ADRs, including ADR-031 and ADR-032;
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

Repository Step 14 is accepted through PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d4469137c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head.

Repository Step 15 is accepted through PRs #263–#267. The final process-host closure is PR #267 / accepted source `f1b72dbee09f152005cb3584b9bcc1573bf2c4fe` / squash merge `4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf` / 19 of 19 applicable permanent workflows.

Repository Step 16 is accepted through PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20 and PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17. The complete exact evidence ledger is recorded in Section 11 and `PROJECT_STATUS.md`.

Repository Step 17 is accepted through PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23, PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8 and PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22.

The accepted Step 17 result keeps `activities.task.create@1.1.0` as the sole live create coordinate. `activities.task.create@1.0.0` is absent from the current provider manifest and production capability catalog, rejected before payload decoding, retained only as immutable historical registry/lifecycle evidence and classified `never_externally_released`. The ordinary 30-day/production-zero-usage path remains binding for released contracts; no production history was fabricated. Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. The next permitted bounded implementation packet is Repository Step 19: the real Customer Privacy worker lifecycle and complete process/end-to-end acceptance.


Current blocking baseline after the accepted Step 18 doctor/bootstrap and dev-up/dev-reset slices:

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
| B — dependency, crate and exception governance | **Complete** | PRs #253, #255 and #257: reproducible measurement, exact Rust 1.97.1, zero-warning Rust/Clippy, lockfile preservation, blocking suppression/direct-lint/process-host/change-cost/dependency governance | preserve non-growth; complete ADR-032 runtime fan-in and gate-value decisions at Step 22 |
| C — golden owner package and persistence model | **In progress** | Customer Privacy golden package, final subject policy, restriction/legal-hold placement, retention precedence, durable owner execution/outcomes, governed access/export, owner actions and accepted Party tombstone/no-orphan convergence through Step 15 | worker lifecycle at Step 19 and Phase 8A closure at Step 21 |
| D — contribution aggregation | **Complete** | all active first-party owner contributions aggregated through `crm-first-party-modules` by PRs #246, #248 and #249 | preserve bounded owner-owned contribution boundaries |
| E — affected-scope CI | **Complete** | PR #239: deterministic Rust closure and declarative contract/API/migration/PostgreSQL/process/product/frontend/operations ownership with unknown-path fail closed | preserve policy/workflow compatibility and review gate value/cost at Step 22 |
| F — generic conformance and contract lifecycle | **In progress** | reusable mutation/query conformance through PR #235, reusable worker conformance through PRs #269–#270 and complete contract lifecycle enforcement through PRs #275, #278 and #279 | real Customer Privacy worker adoption Step 19 |
| G — transitional consolidation | **Complete** | PR #259 removes one redundant Customer Accounts package behavior-neutrally and lowers measured budgets | preserve the reduction and prove later changes remain bounded |
| H — reproducible environment and navigation | **Complete through PR #285** | `affected`, `check-affected`, `explain`, fail-closed `packet-check`, generated active packet/repository map, plus accepted deterministic `doctor`, locked `bootstrap` and checkout-owned PostgreSQL `dev-up` / `dev-reset` through PRs #281 and #283 | complete deterministic doctor/bootstrap/dev-up/dev-reset/seed-demo/smoke lifecycle accepted through PR #285 |
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
15. Party tombstone, no-orphan proof and projection/search/cache convergence — **complete through PR #267**;
16. reusable generic worker conformance adopted by representative real workers — **complete through PR #270**;
17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement — **complete through PR #279**;
18. deterministic local lifecycle commands — **complete through PR #285**;
19. real Customer Privacy worker lifecycle and complete process/end-to-end acceptance;
20. Phase 8A frontend, accessibility, browser and operations evidence;
21. Phase 8A closure;
22. Phase 8A architecture remeasurement, `crm-application-runtime` fan-in decision and permanent-gate value/cost review — **checkpoint, not final 10/10**;
23. first Phase 8B expert-domain wave proving bounded extension cost and validating the Step 22 runtime/gate conclusions;
24. second contrasting expert-domain wave proving bounded extension cost as module count grows and validating the Step 22 runtime/gate conclusions;
25. final architecture 10/10 closure review only when every Section 13 criterion is mechanically proven.

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

### 4.4 `crm-application-runtime` closure obligation

The current broad `crm-application-runtime` direct dependency surface is a controlled architecture debt, not a completed simplification claim.

ADR-032 governs its Step 22 resolution. Every internal direct dependency must finish Step 22 as exactly one of `removed`, `platform-generic`, `owner-specific-unavoidable` or `test-only`. Mere non-growth is insufficient.

Every safely removable owner-specific dependency must be removed or moved behind an existing owner-owned production boundary. Every retained owner-specific dependency must prove a concrete unavoidable process, trust, security, provider, persistence, projection or ownership boundary and must not participate in ordinary owner changes.

An ordinary capability in an existing owner, and the Step 23–24 expert-domain waves, must not require edits to `crm-application-runtime/Cargo.toml` or owner-specific process-composition source. Any contrary evidence reopens Step 22 and blocks Step 25.

## 5. Dependency, public-surface and exception governance

Blocking policies must permit reductions and reject unmeasured growth.

Required controls:

- root `[workspace.dependencies]` for accepted shared versions where centralization is proven safe;
- accepted version/feature divergence recorded explicitly;
- no direct package lint tables outside an exact time-bounded exception;
- no new source-level `allow` or `expect` equivalent to a retired suppression;
- complete inventory of source-level `allow`, `expect`, ignored tests and equivalent bypass forms;
- role-aware budgets for central runtimes, SDK/contracts, infrastructure ports and process hosts;
- direct dependency and transitive reverse-impact non-growth;
- conservative public Rust item non-growth unless a versioned public API need is proven;
- exact exception owner, scope, reason, compensating checks, expiry and removal condition;
- expired exceptions equal zero.

Current exact reduction budgets are stored in `step13-complexity-policy.json`; Rust adoption cohorts are stored in `rust-governance-policy.json`; the registered source-level suppression multiset is stored in `step13-suppression-baseline.json`.

These policies are floors for safe change, not proof that every retained dependency or suppression is optimal.

## 6. Affected-scope and CI scalability

Affected Scope CI is a correctness boundary, not an optimization hint.

Every changed path must resolve to:

- an owner or repository responsibility;
- affected Rust package/reverse closure where applicable;
- required permanent workflows;
- a fail-closed reason when impact is unknown.

Contracts, Protobuf/API, migrations, PostgreSQL, process/runtime, product, frontend and operations changes have explicit planes. Workflow filters and policy selections must remain mechanically compatible.

Representative leaf changes should avoid unrelated full-workspace fan-out. Any broadening requires a machine-readable reason and must be remeasured at Steps 22–24.

### 6.1 Permanent-gate value and cost governance

A permanent gate is justified only by a concrete prevented failure mode, distinct value and acceptable execution/maintenance cost.

Every new permanent gate proposal must declare before acceptance:

- concrete prevented failure mode;
- why existing gates do not already prevent it;
- expected affected scope, duration, runner cost and expensive environment setup;
- named owner;
- false-positive controls;
- emitted success/failure evidence;
- review and retirement condition.

A gate must not become permanent merely because it checks another governance mechanism.

Before Step 22 remeasurement is accepted, every existing permanent workflow, job and repository gate must appear in a complete value/cost ledger recording:

- failure mode;
- observed defects or specific preventive rationale;
- authoritative inputs and scope;
- overlap and duplication;
- execution cost and operational maintenance;
- retain, simplify, merge or remove decision;
- owner and retirement/re-review condition;
- compensating checks for any simplification, merge or removal.

Duplicate or low-value gates must be simplified, merged or removed unless an independent failure mode and cost justification is proven. A gate without a concrete failure mode cannot remain permanent.

## 7. Generic conformance and contract lifecycle

Mutation, query and worker conformance are accepted. Remaining work must prove:

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

Repository Step 18 has accepted the complete deterministic clean-machine command surface through PRs #281, #283 and #285:

- `python scripts/repo.py doctor`;
- `python scripts/repo.py bootstrap`;
- `python scripts/repo.py dev-up`;
- `python scripts/repo.py dev-reset`;
- `python scripts/repo.py seed-demo`;
- `python scripts/repo.py smoke`.

The commands are repository-pinned, repeatable, fail closed, production-aligned and permanently proven on clean environments. `seed-demo` uses the governed Party gateway and `smoke` proves readiness plus permission, authentication and tenant negative paths through a real `crm-api` process.

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

PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d4469137c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged exact head proves:

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

## 11. Accepted Step 15 closure evidence

Repository Step 15 is complete through five bounded accepted slices:

1. PR #263 / source `6c2a54f6780988a12fec3cd77ca2cd39ad349140` / merge `bd205e0af77b676654dff8ddf26d3b5b195880b2` / 32 of 32 — stable Party privacy tombstones converge into Search generation `g3`, and inactive lifecycle documents are excluded before text matching and disclosure.
2. PR #264 / source `e6c9d2901109c8d5b9e0f3cf783214407e26451a` / merge `e9fe1f352386d80a29d122db5d1ed6c47266bfaf` / 6 of 6 — Customer 360 replaces the stable `party:<id>` contribution with a non-personal tombstone, records erased/privacy-minimized lifecycle and removes root membership.
3. PR #265 / source `ef572bdf31c584c397c215cd1b62ee47cad54e64` / merge `2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a` / 19 of 19 — canonical owner execution emits the immutable event; Customer 360 rebuild and Search replay remove stale personal data while preserving authoritative Party, outbox and audit evidence.
4. PR #266 / source `ded5d80ae11bbf044b5bfe5b572e8dab521f884a` / merge `1f889a810c82da3d0fee12427eacccbe43613bac` / 19 of 19 — the production Customer 360 identity advances to `customer.customer-360.v2`; normal `run_batch` replays immutable history while legacy `v1` remains historical and non-authoritative.
5. PR #267 / source `f1b72dbee09f152005cb3584b9bcc1573bf2c4fe` / merge `4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf` / 19 of 19 — a real `crm-api` restart automatically repairs missing Customer 360 v2 derived state through the production background-worker cycle on clean and rollback/reapplied PostgreSQL schemas.

The combined evidence proves authoritative stable non-reusable Party identity tombstones, strict event/version lineage, non-disclosure in Search and Customer 360, deterministic replay/rebuild, automatic fresh-generation rollout and real process-host no-orphan convergence. No workspace package, internal dependency edge, public route, public contract or migration was added by Step 15.

At the Step 15 boundary, Customer Privacy, Phase 8A, worker conformance, contract lifecycle, frontend/operations evidence and architecture 10/10 remained incomplete.

### 11.1 Accepted Step 16 closure evidence

Repository Step 16 is complete through two bounded accepted slices:

1. PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20 — the business-neutral standard-library conformance helper proves denial without side effects, retryable preservation and exact recovery; Customer Enrichment and CRM API import representatives prove activation, live authorization, tenant isolation, completed replay and crash/restart recovery.
2. PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17 — two live production `crm-api` executors are observed in one PostgreSQL blocking chain while competing for the same durable import work; existing transaction/idempotency ownership serializes the second executor and release converges to exactly one Party record, idempotency record, event, audit record and completed checkpoint.

The combined evidence proves reusable worker conformance and contrasting real-worker adoption without owner-specific generic logic, a generic lease API or production algorithm/contract/schema/migration/package/dependency/workflow changes. Customer Privacy continues to publish zero workers, so its real worker lifecycle remains Step 19.

This completes Repository Step 16 only. Customer Privacy, Phase 8A, contract lifecycle, local lifecycle, frontend/operations evidence and architecture 10/10 remain incomplete.

### 11.2 Accepted Step 17 closure evidence

Repository Step 17 is complete through three bounded accepted slices:

1. PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23 — published wire-compatible `activities.task.create@1.1.0`, migrated the sole repository-owned consumer and made live authorization overlap exact-version safe.
2. PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8 — made production zero-usage retirement evidence SHA-256-bound, complete, append-only and fail closed.
3. PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22 — proved the coordinate was never externally released through live empty GitHub Releases, tags and deployments, non-publishable packages and no external consumer record, then retired `activities.task.create@1.0.0` while preserving its historical tombstone and keeping `telemetry.zero_since` null.

The combined evidence keeps `activities.task.create@1.1.0` as the sole live create coordinate, preserves the ordinary production zero-usage path for released contracts and fabricates no production history. Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. The next permitted bounded implementation packet is Repository Step 19: the real Customer Privacy worker lifecycle and complete process/end-to-end acceptance.

## 12. Repository Step 22 mandatory decision and review

Step 22 is not a passive dashboard refresh. It must execute ADR-032 before its architecture remeasurement can be accepted.

### 12.1 Runtime fan-in decision

Step 22 must:

1. inventory every internal direct dependency of `crm-application-runtime`;
2. separate production and test-only surfaces;
3. classify every dependency as `removed`, `platform-generic`, `owner-specific-unavoidable` or `test-only`;
4. remove every safely removable owner-specific dependency;
5. prove every retained owner-specific dependency protects a concrete unavoidable boundary;
6. prove ordinary owner changes do not modify the runtime manifest or owner-specific runtime source;
7. leave zero unresolved dependency classifications.

The current maximum direct-dependency budget is not sufficient completion evidence by itself.

### 12.2 Permanent-gate review

Step 22 must review every permanent workflow, job and gate for:

- real prevented failure mode;
- defects actually detected or exact preventive rationale;
- duplicate/overlapping results;
- duration, runner and environment cost;
- false-positive and maintenance history where measurable;
- retain, simplify, merge or remove disposition;
- named owner and retirement condition.

All immediately safe simplifications, merges and removals must be completed before Step 22 closes. A deferred action requires a named owner, exact rationale and deadline before Step 25; known safe simplification cannot be deferred merely to preserve the current gate count.

## 13. Final architecture 10/10 closure criteria

Issue #194 may close and architecture 10/10 may be declared only when all are mechanically proven:

1. one authoritative current-state hierarchy with no contradictory live roadmap;
2. ordinary existing-owner capabilities add zero crates and avoid generic-runtime edits;
3. new owner waves remain within the measured three-to-five-package target unless an independent boundary is proven;
4. every `crm-application-runtime` internal direct dependency has a final Step 22 classification, every safely removable owner-specific dependency is removed, and every retained owner-specific dependency has unavoidable-boundary evidence;
5. central dependency, reverse-impact, public-surface and process-host budgets do not regress;
6. expired exceptions, hidden suppressions and unregistered lint bypasses are zero;
7. every permanent gate has a complete value/cost/overlap/owner/retirement entry and duplicate or low-value gates have been simplified, merged or removed unless independent value is proven;
8. affected-scope policy and permanent workflow filters remain exact and fail closed;
9. reusable mutation, query and worker conformance are adopted by contrasting real owners;
10. contract compatibility, deprecation, consumer migration and retirement are permanently enforced;
11. local lifecycle commands are deterministic on a clean machine;
12. frontend, accessibility, browser and production operations proof exists for critical journeys;
13. Phase 8A closes without weakening owner, tenant, RLS, authorization, audit, rollback or route invariants;
14. Step 22 remeasurement shows no hidden regression and zero unresolved runtime-fan-in or gate-value decisions;
15. two contrasting later expert-domain waves at Steps 23 and 24 keep extension cost bounded, avoid `crm-application-runtime` owner-specific edits and validate the Step 22 gate decisions as module count grows;
16. a separate Step 25 review reproduces the metrics and confirms every prior criterion.

Until then:

- architecture 10/10 remains unclaimed;
- Phase 8A and Customer Privacy remain incomplete;
- current product-complete expert modules remain **0**.

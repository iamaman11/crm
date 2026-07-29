# Ultimate CRM — Architecture Complexity, Scalability and Developer Experience 10/10 Plan

Status: **Normative cross-cutting execution plan**  
Tracking issue: #194  
Original audit baseline: **2026-07-27**  
Current execution checkpoint: **2026-07-29**

Applies to repository structure, Rust workspace packaging, module composition, dependency governance, contracts, persistence ownership, CI/test selection, developer tooling, documentation navigation, local development, frontend architecture and production operations.

Governing precedence:

1. `SYSTEM_INVARIANTS.md`;
2. published contracts and accepted ADRs;
3. `APPLICATION_ARCHITECTURE.md` and the accepted readiness baseline;
4. this execution plan;
5. descriptive module, packet and orientation documentation.

This document is the **single current execution plan for architecture complexity and developer experience**. `PROJECT_STATUS.md` and issue #194 carry the current checkpoint; accepted packet documents remain historical evidence and are not rewritten into live trackers.

## 1. Executive decision

The foundational architecture is sound and must not be replaced.

The correct long-term model is already established:

- modular monolith with independently governed owner and link modules;
- one authoritative owner for every mutable aggregate;
- pure domain code behind stable contracts and governed ports;
- no direct cross-module storage access;
- exact versioned mutation, query, event and worker coordinates;
- durable tenant activation and live authorization;
- transactional state, idempotency, outbox and audit evidence;
- FORCE RLS and cross-tenant negative proof;
- rebuildable non-authoritative projections, search and caches;
- unchanged exact-head acceptance discipline.

The remaining risk is accidental complexity:

- too many physical crates for one business capability;
- manual domain-specific composition in the central application runtime;
- repeated dependency declarations and feature divergence;
- copied acceptance wiring and expanding workflow fan-out;
- too many files and packages touched by a normal vertical change;
- incomplete local-environment lifecycle automation;
- remaining affected-scope coverage outside the accepted real-diff Rust/tooling closure;
- uncontrolled public Rust surface, contract retirement and migration ownership;
- descriptive documents that can drift from authoritative current state.

The required direction is:

> Preserve strict ownership, security and governed runtime boundaries while making the normal cost of adding or changing one capability close to constant with respect to total product size.

No big-bang rewrite, premature microservice split or weakening of acceptance rules is authorized.

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

No later score is authoritative until the same dimensions are mechanically remeasured.

### 2.2 Workspace history and current fact

At plan creation the root declared 109 members. Stage B made one already-effective implicit package explicit, producing the accepted 110-package baseline. The behavior-neutral Customer Privacy golden-package pilot then established three target technical packages, so the current accepted workspace contains **113 effective packages**.

That increase is explicit and bounded. An ordinary capability added to an existing owner now creates **zero new crates** by enforced default; existing transitional capability-specific packages remain later consolidation candidates.

### 2.3 Current program position

| Stage | Current state | Accepted progress | Remaining exit work |
|---|---|---|---|
| A — documentation and policy baseline | **Complete** | one plan, stable `docs/README.md`, source hierarchy and permanent consistency guard | preserve freshness and avoid duplicate live roadmaps |
| B — dependency, crate and exception governance | **In progress** | reproducible baseline, crate justification, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1` toolchain/workspace `rust-version`, measured zero-warning Rust/Clippy baseline and three exact expiring legacy lint exceptions | additional homogeneous dependency cohorts, removal of the three direct-lint exceptions, public-surface/fan-out calibration |
| C — golden owner package and persistence model | **In progress** | Customer Privacy domain/application/postgres/production pilot, transaction-scoped final policy port, authoritative deny-only restriction decision, public placement and first protected-owner integration are accepted; ordinary Customer Privacy capabilities add zero crates | generalize scaffolding, migration ownership and visibility policy; adopt the model for later owners without forced rewrites |
| D — contribution aggregation | **In progress** | owner-owned contribution pattern proven; first bounded Customer Accounts registration-inventory aggregation accepted through the first-party bundle | migrate remaining owners, expand the first-party bundle and remove additional concrete domain imports from generic runtime |
| E — affected-scope CI | **In progress** | changed paths, Rust reverse closure, structural preflight, explainable broadening and real-diff packet-check enforcement are accepted | complete contract, migration, process, product, frontend and operations scope selection with safe fallback |
| F — generic conformance and contract lifecycle | **Foundation only** | native conformance, manifest/route parity and exact-head gates exist | reusable mutation/query/worker suites plus compatibility, deprecation and retirement enforcement |
| G — transitional consolidation | **Not started** | candidates and stop rules are defined | complete at least one behavior-neutral domain-cluster consolidation with measured improvement |
| H — reproducible environment and navigation | **In progress** | stable docs index, `affected`, `check-affected`, deterministic `explain`, fail-closed `packet-check`, generated active packet and repository map are accepted through PR #228 | `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` |
| I — frontend and operations parity | **Not started as a complete stage** | existing product/process checks remain preserved | domain-oriented frontend proof, accessibility/browser evidence, restore/SLO/performance/security/supply-chain gates |

Repository step 1 is accepted through PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 permanent workflows. The exact supported compiler and workspace `rust-version` are `1.97.1`; Rust and Clippy warning/error budgets are zero; the 113-package workspace and `Cargo.lock` are unchanged; three pre-existing direct `too_many_arguments` lint tables are exact, expiring, no-growth exceptions.

Repository step 2 is accepted through PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows. Approval remains inside the existing Customer Privacy packages and adds no restrictions, hold/retention adjudication, owner execution, destructive actions, workers, dependency upgrades or crate consolidation.

Repository step 3 is accepted through PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows. The packet routes selected Customer Accounts data-only mutation/query inventory factories through `crm-first-party-modules`, preserves exact inventory and contribution order plus activation behavior, and changes no route, coordinate, persistence, dependency, `Cargo.lock`, package count or product semantics.

The architecture preflight for repository step 4 proved that restriction placement could not satisfy the accepted final-state enforcement rule without a stable transaction-scoped policy port and deterministic composition of owner-specific plus Customer Privacy final guards. PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts that smallest inserted prerequisite. It changes no route, public inventory, restriction runtime, owner integration, persistence, dependency, `Cargo.lock`, package count or product behavior and provides no allow-all implementation.

Repository step 4 is accepted through PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows. It promotes deny-only restriction placement, implements the authoritative FORCE-RLS final decision, shares the canonical Party lock with protected Contact Point creation and proves public placement, immediate denial without side effects, tenant isolation, rollback/reapply and repeated real-process acceptance.

Repository step 5 is accepted through PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows. It adds deterministic explanation, fail-closed packet validation, real-diff Affected Scope enforcement and generated navigation without changing product behavior, contracts, persistence, dependencies, `Cargo.lock` or package count.

The next permitted implementation packet is repository step 6: Customer Privacy legal-hold and mandatory-retention precedence.

### 2.4 Single repository execution order

Repository development is strictly sequential. Product and architecture work are categories of packets, not independent queues.

Execution rules:

1. At most one implementation packet may be active in the repository.
2. A second implementation branch or pull request must not begin until the current packet has unchanged exact-head acceptance and is merged or explicitly closed.
3. The next permitted packet is the first unfinished item in the numbered master sequence below.
4. A documentation/evidence synchronization PR belongs to closure of the packet that produced the accepted fact and must finish before the next implementation packet begins.
5. Every product packet starts with architecture preflight against ownership, packaging, dependency, composition, affected-scope and test budgets.
6. If that preflight proves the product packet cannot satisfy an existing hard rule, insert only the smallest required architecture prerequisite immediately before the blocked product packet, accept and merge it, then return to that same product packet. This is the only permitted insertion rule; unrelated work must not skip ahead.
7. Feature behavior, architecture governance, contribution refactoring, crate consolidation and evidence synchronization remain separate pull requests even when adjacent in the sequence.
8. Stage labels A–I describe completion accounting. They do not authorize work outside this master order.

The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, and repository step 5 is complete through PR #228. None changes the master numbering.

Current master sequence:

1. supported Rust toolchain, workspace `rust-version` and measured lint baseline — **Complete through PR #218**;
2. Customer Privacy approval runtime only — **Complete through PR #220**;
3. first bounded contribution-aggregation packet: expand owner-owned first-party registration and reduce selected concrete generic-runtime imports without behavior changes — **Complete through PR #222**;
4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **Complete through PR #226**;
5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Complete through PR #228**;
6. Customer Privacy legal-hold and mandatory-retention precedence — **Next**;
7. reusable generic mutation and query conformance suites adopted by representative owners;
8. replay-safe resumable Customer Privacy owner execution and crash-window recovery;
9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks;
10. governed Customer Privacy access/export assembly;
11. owner-specific deletion, anonymization and supported crypto-shred execution;
12. first measured behavior-neutral transitional domain-cluster consolidation;
13. Party tombstone, no-orphan proof and projection/search/cache convergence;
14. reusable generic worker conformance suite;
15. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;
16. Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
17. Phase 8A frontend, accessibility, browser, restore, SLO, performance, security and supply-chain evidence;
18. Phase 8A closure;
19. architecture remeasurement, remaining-gate review and publication of the next numbered sequence;
20. first Phase 8B packet only after step 19 is accepted.

No item may be described as “next” when an earlier unfinished item exists. Any change to this order requires the change-control evidence in section 13 and synchronized updates to the normative roadmap, Phase 8 plan, project status and active issues before implementation starts.

## 3. 10/10 target state

Architecture and developer experience reach 10/10 only when the conditions below are executable, measured and difficult to bypass.

### 3.1 Business ownership

- Every mutable domain has one authoritative owner.
- Business modules never import another business module's internals, repositories or tables.
- Cross-domain mutation uses exact capabilities; asynchronous coordination uses versioned events and optional link modules.
- Search, analytics, projections and caches remain rebuildable and non-authoritative.
- Disable, upgrade, rollback and uninstall behavior is explicit and tested.

### 3.2 Physical packaging

- A **normal capability added to an existing owner creates zero new crates**.
- A normal owner targets **three to five technical packages**: pure domain, application, PostgreSQL/infrastructure, production contribution and one optional real provider/trust/process/extraction boundary.
- Crates exist for dependency, trust, reuse, lifecycle, process or extraction boundaries—not for every handler.
- Transitional capability-specific packages are consolidated gradually through behavior-neutral packets with measured benefit.

### 3.3 Layering

Required dependency direction:

```text
domain <- application <- adapters <- production composition <- delivery
```

Target owner layout:

```text
modules/crm-<domain>/
crates/crm-<domain>-application/
crates/crm-<domain>-postgres/
crates/crm-<domain>-production/
```

Infrastructure cannot leak into the pure owner core. A small owner may combine application and production only when that rule remains mechanically enforceable.

### 3.4 Extensibility and change locality

A normal capability follows:

```text
internal command/query/worker
→ existing owner application package
→ existing owner adapter when needed
→ existing owner-owned production contribution
→ generic conformance
→ affected owner acceptance
```

It must not require:

```text
new crate
→ new generic runtime dependency
→ new business switch
→ copied platform test suite
→ unrelated migration
→ unrelated full-workspace iteration
```

A representative leaf change touches one owner closure, zero generic-runtime files, zero unrelated owners, zero unrelated migrations and zero new workflows unless a real platform boundary changes.

### 3.5 Module-owned production contribution

Every owner production package exposes one stable contribution entry point. The first-party bundle aggregates owner-owned entry points only and contains no route catalog, capability switch or owner-specific dispatch.

The generic application runtime consumes contribution sets and must not gain direct domain dependencies for ordinary registration.

### 3.6 Dependency and exception governance

- Common third-party dependencies are declared under root `[workspace.dependencies]` only after consumer and feature semantics are calibrated.
- Internal packages inherit approved versions and feature sets.
- Existing direct/non-inheriting debt cannot grow without a complete expiring exception.
- Multiple versions or feature families require a named blocker, owner and removal condition.
- Heavy features, reverse fan-out, lockfile drift and public-surface growth are reported mechanically.
- Every new workspace member requires review-visible architecture justification.
- Temporary exceptions are machine-readable, owned, scoped, compensating and expiring.

### 3.7 Rust public API surface

- Implementation visibility defaults to private or `pub(crate)`.
- Cross-package APIs are limited to published contracts, stable ports and contribution interfaces.
- Concrete adapters, repositories and infrastructure clients are not re-exported for convenience.
- Shared DTOs do not replace owner-domain value objects.
- Every new public Rust symbol requires a real consumer and compatibility rationale.

### 3.8 Contract lifecycle

Published versions remain immutable. A semantic change creates a new version.

```text
publish new version
→ compatibility and impact report
→ overlapping support window
→ usage/deprecation telemetry
→ consumer migration
→ explicit retirement gate
→ removal only after zero supported consumers and rollback proof
```

Deprecation retains owner, replacement, deadline, consumer inventory and removal condition. Silent reinterpretation or deletion is forbidden.

### 3.9 Persistence and migration ownership

- Every owner declares PostgreSQL namespaces and migration ownership.
- A migration may not alter another owner's authoritative tables.
- Ownership transfer requires ADR, dual-version compatibility, data migration, cutover and rollback.
- RLS, tenant context, indexes, retention, legal-hold and recovery consequences are schema-review requirements.
- Forward, rollback/schema-removal, reapply and repeated acceptance are required where applicable.
- Persisted envelopes evolve independently from public wire contracts.

### 3.10 Build and test scalability

During development:

- structural preflight always runs;
- changed packages and reverse dependencies are calculated;
- affected contracts, migrations, routes, workers, frontend and operations scopes select required checks;
- skipped checks have machine-readable reasons;
- unknown impact defaults to broader validation.

Full repository proof remains mandatory for shared/core/runtime changes, architecture policy changes, phase closure, releases, nightly validation and any change whose closure cannot be proven. **Affected-scope CI** never weakens the unchanged exact-head merge rule.

### 3.11 Reproducible local development

The supported command surface must eventually include:

```bash
python scripts/repo.py doctor
python scripts/repo.py bootstrap
python scripts/repo.py dev-up
python scripts/repo.py dev-reset
python scripts/repo.py seed-demo
python scripts/repo.py smoke
```

It must validate pinned Rust, Python, Node, pnpm, Buf and PostgreSQL versions; provide deterministic database/demo setup; support safe reset; match production ingress/composition; and emit clear diagnostics without requiring developers to memorize raw CI commands.

### 3.12 Developer comprehension and navigation

A developer or coding agent must be able to answer:

- who owns the behavior;
- where the contract lives;
- which command/query/worker implements it;
- which persistence adapter and migrations are authoritative;
- how it enters production composition;
- which tests and workflows are required;
- what is explicitly out of scope.

Required tooling:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
python scripts/repo.py packet-check --base origin/main
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
```

Required navigation:

- stable human index `docs/README.md`;
- generated `docs/ACTIVE_PACKET.md`;
- generated `docs/generated/REPOSITORY_MAP.md`.

Generated navigation is reproducible orientation, never an independent source of truth.

### 3.13 Frontend and operations

- frontend features are organized by stable domain surfaces;
- generated/governed clients are the only backend boundary;
- component, accessibility and browser tests cover critical journeys;
- session, routing, navigation and search concerns are separated;
- backup/restore, SLO, performance, security and supply-chain evidence are executable;
- restore tests preserve restrictions, legal holds, tombstones and audit integrity.

## 4. Hard rules for all new work

1. A normal capability creates zero new crates.
2. A new owner module is justified by a new authoritative mutable domain, not a screen, table, report, team or workflow step.
3. A new crate protects a real dependency, trust, reuse, process, lifecycle or extraction boundary.
4. Generic router and worker code does not branch on business IDs.
5. Central manual composition LOC does not grow for ordinary owner registration.
6. **Feature behavior and crate consolidation must be separate PRs.**
7. Shared abstractions are extracted only after at least two contrasting real implementations prove common behavior.
8. No cross-owner SQL, internal imports or repository access.
9. No replacement of live authorization with cached approval.
10. No reduction of PostgreSQL, process, rollback, RLS or exact-head evidence for speed.
11. Every structural PR reports before/after complexity and change-locality impact.
12. Documentation status and navigation freshness are mechanically checked.
13. Temporary exceptions are machine-readable, owned and expiring.
14. A planned command or generated artifact is never represented as implemented before permanent tests prove it.
15. Repository execution follows section 2.4; only one implementation packet is active and earlier unfinished items block later items.

## 5. New-crate decision

A new crate is accepted only when it protects at least one real boundary:

- forbidden infrastructure dependency;
- provider SDK, arbitrary HTTP, secrets, broker or object storage;
- independent trust/security boundary;
- separately operating worker/process;
- multiple independent consumers;
- credible extraction seam documented by ADR;
- visibility that package boundaries must enforce.

A crate is rejected when it contains only one handler, planner, thin re-export, copied validation or capability-specific composition function.

Required review note:

```text
New crate justification:
- protected boundary:
- isolated dependencies:
- expected consumers:
- why an internal module is insufficient:
- lifecycle or extraction seam:
- expected build/test fan-out:
- removal or consolidation condition:
```

## 6. Contribution aggregation program

For every first-party owner:

1. define one owner-owned contribution entry point;
2. compose validators, planners, handlers, queries and workers inside the owner production package;
3. expose exact route/worker metadata;
4. merge into the mechanically checked first-party bundle;
5. remove corresponding concrete imports from generic runtime;
6. retain startup rejection for duplicate coordinates, owner mismatch, route-kind mismatch and incomplete handlers.

Completion evidence:

- adding a capability changes no generic runtime source;
- adding a worker changes no generic worker algorithm;
- adding a module changes its package/manifest and generated/mechanically checked bundle only;
- manual module-ID lists outside authoritative inventories are absent;
- central runtime direct domain dependencies and LOC decrease or remain stable.

## 7. Test architecture program

### 7.1 Structural preflight

Always verify architecture boundaries, manifests/IR, Protobuf bindings, route classifications, generated-source freshness, formatting, dependency policy, crate justification, exception validity and documentation/navigation consistency.

### 7.2 Generic conformance

**Generic conformance** owns repeated platform guarantees:

- mutation: activation, malformed input, tenant mismatch, denied live authorization, idempotency/replay, safe errors and atomic evidence;
- query: activation, authorization denial, not-found concealment, tenant mismatch, malformed cursor, no query-side writes and pagination evidence;
- worker: activation, deterministic phase, bounded work, retry classification, idempotent claim/crash recovery and no fixed central wiring.

Owner suites remain responsible for unique domain semantics.

### 7.3 CI levels

- Level 0: fast structural preflight.
- Level 1: affected Rust/package closure.
- Level 2: affected owner-domain acceptance.
- Level 3: real process acceptance for route, worker, persistence, authorization or protocol changes.
- Level 4: full repository matrix.

## 8. Developer tooling and documentation model

`docs/README.md` is the stable task-oriented index. Root README and `AGENTS.md` are orientation only and must not maintain independent live phase or next-packet claims.

`repo.py explain` traces manifest → contract → application → adapter/migration → contribution → ingress/worker → tests/workflows.

`repo.py packet-check` reports active packet, baseline SHA, allowed/forbidden paths, affected closure, contracts/routes/workers/migrations, required checks, evidence state and blockers.

`docs/ACTIVE_PACKET.md` and `docs/generated/REPOSITORY_MAP.md` are generated with deterministic ordering and source digests. CI rejects stale output; generated sections are not hand-edited.

Distinct source purposes:

- `SYSTEM_INVARIANTS.md` — hard rules;
- contracts and ADRs — immutable machine/public decisions;
- `APPLICATION_ARCHITECTURE.md` — stable layering/composition;
- this document — cross-cutting 10/10 execution program and repository master order;
- `IMPLEMENTATION_ROADMAP.md` — product dependency order and reference to the repository master order;
- `PHASE8_DELIVERY_PLAN.md` — active Phase 8 dependency detail within the repository master order;
- `PROJECT_STATUS.md` — concise merged state and next permitted repository packet;
- `MODULE_CATALOG.md` — owner and completeness accounting;
- issue #194 — active architecture packet and order checkpoint;
- accepted packet documents — historical boundaries;
- root README, `AGENTS.md`, `docs/README.md` and generated navigation — orientation only.

Mechanically reject stale phases, owner counts, next packets, completion claims, planned-command claims and execution-order ambiguity.

## 9. Transitional crate consolidation

**Transitional crate consolidation** is gradual and evidence-based.

Prioritize one-consumer capability-specific adapters, thin composition crates, command/query crates with no unique boundary and domain clusters with many physical locations per capability.

Do not automatically consolidate pure owner modules, core contracts/SDKs/runtimes, provider/secret boundaries, independent processes, packages with multiple stable consumers or deliberate extraction seams.

Every consolidation PR is behavior-neutral, preserves public coordinates and route/activation parity, proves applicable PostgreSQL/process behavior and reports package count, fan-out, public surface, build/test effect and files-per-capability before/after. Stop or revert when measured complexity worsens.

## 10. Current Customer Privacy architecture rule

Customer Privacy is the golden owner pilot.

Accepted through the current checkpoint:

- stable domain/application/postgres/production package boundary;
- production scope discovery and immutable snapshots;
- trusted-internal deterministic planning;
- permission-aware plan and future-safe empty outcome reads;
- public approval runtime;
- exact Rust `1.97.1` toolchain/workspace `rust-version` and measured zero-warning governance;
- 6 mutations / 4 queries / 0 workers;
- 113 workspace packages and zero new dependency families for the later capability packets;
- behavior-neutral repository step 3 contribution aggregation with unchanged Customer Privacy inventory and semantics;
- transaction-scoped final customer-subject policy port and deterministic aggregate guard composition accepted through PR #224;
- authoritative deny-only restriction placement and final owner guard accepted through PR #226, with no allow-all fallback.

The next permitted repository packet is **repository step 6: Customer Privacy legal-hold and mandatory-retention precedence**. It must preserve shared subject locking and immediate deny-only behavior while adding authoritative legal-hold-over-retention-over-approved-action adjudication; it must not include owner execution, access/export assembly, destructive action, workers, dependency upgrades or crate consolidation.

Issue #126 supplies product semantics. Issue #194 supplies architecture acceptance and the repository order. Their scopes remain separate, and section 2.4 determines exactly which packet may start.

## 11. Metrics and budgets

Publish per structural PR:

```text
Workspace members
Business modules
Packages by category
Maximum dependency depth and reverse fan-out
Duplicate dependency versions/features
Public Rust symbols
Manual composition LOC and registration points
Packages and files affected by representative leaf change
Generic-runtime and unrelated-owner files touched
Focused/domain/full test duration
Clean and incremental build duration
Local bootstrap and smoke duration
Generic versus owner-specific test ratio
Navigation freshness and unresolved explain gaps
```

Initial budgets:

- normal capability: zero new crates;
- normal owner: three to five technical packages;
- generic-runtime files touched by ordinary capability: zero;
- unrelated owner/migration/workflow files touched: zero;
- duplicate direct dependency versions: zero unless allowlisted;
- generic composition LOC: no growth for ordinary registration;
- manual module-ID lists: zero outside generated authoritative paths;
- more than 15% build/test/bootstrap regression requires explanation;
- more than 25% regression blocks unless explicitly accepted;
- leaf-domain affected closure does not grow without dependency rationale;
- expired architecture exceptions: zero.

Budgets move from measurement to warning to blocking only after representative baselines are collected.

## 12. 10/10 completion criteria

Issue #194 closes only when:

1. ordinary capabilities create zero new crates by enforced default;
2. new owner domains use the golden package and migration-ownership model;
3. generic application runtime no longer grows for ordinary registration;
4. all active first-party owners expose module-owned contribution entry points;
5. workspace dependency, feature, public-surface and exception policy is enforced;
6. affected-scope CI has explainable broadening and safe fallback;
7. generic mutation/query/worker conformance suites are adopted;
8. contract compatibility, deprecation and retirement lifecycle is enforced;
9. at least one transitional domain cluster is consolidated with measured improvement;
10. `docs/README.md`, `docs/ACTIVE_PACKET.md`, `docs/generated/REPOSITORY_MAP.md`, `explain` and `packet-check` are available and freshness-checked;
11. `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` provide reproducible local development;
12. stale phase/status/next-packet documentation is mechanically rejected;
13. frontend critical journeys have component/browser/accessibility evidence;
14. restore, SLO, performance, security and supply-chain gates are executable;
15. exact-head, tenant, RLS, authorization, audit, idempotency, rollback and route parity guarantees remain unchanged;
16. at least two later expert-domain waves demonstrate bounded extension cost as module count grows.

## 13. Change control

This plan, including the numbered repository order, may change only with explicit architecture rationale, ownership/dependency impact, before/after complexity evidence, acceptance/rollback strategy and synchronized issue/status updates.

An optimization is accepted only when it makes the correct architecture easier to extend, easier to understand, easier to verify and harder to bypass.

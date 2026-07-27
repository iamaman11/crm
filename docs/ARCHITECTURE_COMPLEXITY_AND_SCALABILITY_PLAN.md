# Ultimate CRM — Architecture Complexity, Scalability and Developer Experience 10/10 Plan

Status: **Normative cross-cutting execution plan**  
Tracking issue: #194  
Audit baseline: **2026-07-27**  
Applies to: repository structure, Rust workspace packaging, module composition, dependency governance, contracts, persistence ownership, CI/test selection, developer tooling, documentation navigation, local development, frontend architecture and production operations.

Governing precedence:

1. `SYSTEM_INVARIANTS.md`;
2. published contracts and accepted ADRs;
3. `APPLICATION_ARCHITECTURE.md` and the accepted readiness baseline;
4. this execution plan;
5. descriptive module, packet and orientation documentation.

This document is the **single current execution plan for architecture complexity and developer experience**. Historical PR evidence remains in accepted packet documents and must not be duplicated here.

## 1. Executive decision

The foundational architecture is sound and must not be replaced.

The platform already has the correct long-term model:

- modular monolith with independently governed owner and link modules;
- one authoritative owner for every mutable aggregate;
- pure domain modules behind stable contracts and governed ports;
- no direct cross-module storage access;
- exact versioned mutation, query, event and worker coordinates;
- durable tenant activation;
- live authorization immediately before side effects;
- transactionally consistent state, idempotency, outbox and audit evidence;
- FORCE RLS and cross-tenant negative proof;
- rebuildable non-authoritative projections, search and caches;
- exact-head acceptance discipline.

The remaining risk is accidental complexity:

- too many physical crates for one business capability;
- manual domain-specific composition in the central application runtime;
- repeated dependency declarations and feature divergence;
- copied acceptance wiring and expanding workflow fan-out;
- too many files and packages touched by a normal vertical change;
- incomplete local-environment automation;
- incomplete repository navigation and packet explanation tooling;
- documentation entry points that can drift;
- uncontrolled public Rust surface, contract retirement and migration ownership.

The required direction is:

> Preserve strict ownership, security and governed runtime boundaries while making the normal cost of adding or changing one capability close to constant with respect to total product size.

No big-bang rewrite, premature microservice split or weakening of acceptance rules is authorized.

## 2. Current expert assessment

Audit score at the 2026-07-27 baseline:

| Dimension | Current | Target |
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

These scores are planning signals, not product-completion claims.

The root Cargo workspace currently contains **109 members**. Strict compile-time boundaries remain valuable, but workspace growth must no longer track individual commands, queries or delivery packets.

## 3. 10/10 target state

Architecture and developer experience are 10/10 only when the conditions below are executable, measured and difficult to bypass.

### 3.1 Business ownership

- Every mutable domain has one authoritative owner.
- Business modules never import another business module's internals, repositories or tables.
- Cross-domain mutation uses exact capabilities; asynchronous coordination uses versioned events and optional link modules.
- Search, analytics, projections and caches remain rebuildable and non-authoritative.
- Disable, upgrade, rollback and uninstall behavior is explicit and tested.

### 3.2 Physical packaging

- A **normal capability added to an existing owner creates zero new crates**.
- A normal owner domain targets **three to five technical packages**:
  - pure domain;
  - application;
  - PostgreSQL/infrastructure;
  - production contribution;
  - one optional provider, trust, process or extraction boundary.
- Crates exist for real dependency, trust, reuse, lifecycle, process or extraction boundaries—not for every handler.
- Transitional capability-specific crates are consolidated gradually through behavior-neutral packets with measured benefit.

### 3.3 Layering

Required dependency direction:

```text
domain <- application <- adapters <- production composition <- delivery
```

Target owner layout:

```text
modules/crm-<domain>/
    src/domain/
    src/policy/
    src/value/

crates/crm-<domain>-application/
    src/commands/
    src/queries/
    src/workers/
    src/validation/
    src/ports/

crates/crm-<domain>-postgres/
    src/repositories/
    src/read_models/
    src/locks/

crates/crm-<domain>-production/
    src/contribution.rs
    src/routes.rs
    src/workers.rs
    src/composition.rs
```

A small owner may combine application and production only when infrastructure cannot leak into the pure core.

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

Generic router and worker algorithms remain unchanged when one owner adds a route or worker.

A representative leaf change should touch one owner closure. It must touch zero generic-runtime files, zero unrelated owner files, zero unrelated migrations and zero new workflows unless a real platform boundary changes.

### 3.5 Module-owned production contribution

Every owner production package exposes one stable entry point:

```rust
pub fn build_contribution(
    context: &ProductionContext,
) -> Result<ModuleContributionSet, SdkError>;
```

The first-party bundle aggregates owner-owned entry points only. It contains no route catalog, capability switch or owner-specific dispatch.

The generic application runtime consumes contribution sets and does not gain direct domain dependencies for ordinary registration.

### 3.6 Dependency governance

- Common third-party dependencies are declared under root `[workspace.dependencies]`.
- Internal packages inherit approved versions and feature sets.
- Duplicate direct dependency families are zero unless explicitly allowlisted.
- Multiple major/minor versions require a named blocker, owner and removal condition.
- Heavy features, reverse-dependency fan-out and lockfile drift are reported mechanically.
- Every new workspace member requires a review-visible architecture justification.

### 3.7 Rust public API surface

- Implementation visibility defaults to private or `pub(crate)`.
- Cross-package APIs are limited to published contracts, stable ports and production contribution interfaces.
- Concrete adapters, repositories and infrastructure clients are not re-exported for convenience.
- Shared DTOs do not replace owner-domain value objects.
- Every new public Rust symbol requires a real consumer and compatibility rationale.
- Public-surface growth is measured per structural PR.

### 3.8 Contract lifecycle

Published versions remain immutable. A semantic change creates a new version.

The full lifecycle is:

```text
publish new version
→ compatibility and impact report
→ parallel support window
→ usage/deprecation telemetry
→ consumer migration
→ explicit retirement gate
→ removal only after zero supported consumers and rollback proof
```

A deprecated coordinate must retain owner, replacement, deadline, consumer inventory and removal condition. Silent reinterpretation or deletion is forbidden.

### 3.9 Persistence and migration ownership

- Every owner declares its PostgreSQL namespaces and migration ownership.
- A migration may not alter another owner's authoritative tables.
- Ownership transfer requires an ADR, dual-version compatibility, data migration, rollback and exact consumer cutover.
- RLS, tenant context, indexes, retention, legal-hold and recovery consequences are part of schema review.
- Forward, rollback/schema-removal, reapply and repeated acceptance are required where applicable.
- Persisted envelopes evolve independently from public wire contracts.

### 3.10 Build and test scalability

During development:

- structural preflight always runs;
- changed packages and reverse dependencies are calculated;
- affected contracts, migrations, routes, workers and frontend packages select required checks;
- skipped checks have machine-readable reasons;
- unknown impact defaults to broader validation.

Full repository proof remains mandatory for shared/core/runtime changes, architecture policy changes, phase closure, releases, nightly validation and any change whose closure cannot be proven.

**Affected-scope CI** never weakens the unchanged exact-head merge rule.

### 3.11 Reproducible local development

The supported local environment must be discoverable and repeatable through one command surface:

```bash
python scripts/repo.py doctor
python scripts/repo.py bootstrap
python scripts/repo.py dev-up
python scripts/repo.py dev-reset
python scripts/repo.py seed-demo
python scripts/repo.py smoke
```

Requirements:

- pinned and validated Rust, Python, Node, pnpm, Buf and PostgreSQL versions;
- one documented configuration path with safe defaults;
- deterministic database, migration and demo-tenant setup;
- safe complete reset without hidden manual cleanup;
- local smoke path matching production ingress/composition;
- clear diagnostics for missing tools, ports and environment variables;
- no requirement to memorize raw CI commands;
- local commands must call the same scripts/libraries used by CI where practical.

### 3.12 Developer comprehension and navigation

A developer or coding agent must answer from one command:

- who owns this behavior;
- where its contract lives;
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

Generated navigation is never an independent source of truth.

### 3.13 Frontend and operations

The same standard applies beyond backend architecture:

- frontend features are organized by stable domain surfaces;
- generated/governed clients are the only backend boundary;
- component, accessibility and browser tests cover critical journeys;
- session, routing, navigation and search concerns are separated;
- backup/restore, SLO, performance, security and supply-chain evidence are executable;
- restore tests prove active restrictions, legal holds, tombstones and audit integrity survive recovery.

## 4. Hard rules for all new work

1. A normal capability creates zero new crates.
2. A new owner module is justified by a new authoritative mutable domain, not a screen, table, report, team or workflow step.
3. A new crate protects a real dependency, trust, reuse, process, lifecycle or extraction boundary.
4. Generic router and worker code does not branch on business IDs.
5. Central manual composition LOC does not grow for an ordinary owner capability.
6. **Feature behavior and crate consolidation must be separate PRs.**
7. Shared abstractions are extracted only after at least two contrasting real implementations prove common behavior.
8. No cross-owner SQL, internal imports or repository access.
9. No replacement of live authorization with cached approval.
10. No reduction of PostgreSQL, process, rollback, RLS or exact-head evidence for speed.
11. Every structural PR reports before/after complexity and change-locality impact.
12. Documentation status and navigation freshness are mechanically checked.
13. Temporary exceptions are machine-readable, owned and expiring.
14. A planned command or generated artifact is never represented as implemented before permanent tests prove it.

## 5. Crate creation decision

A new crate is accepted only when at least one condition is true:

- it prevents a forbidden dependency from entering pure domain/application code;
- it isolates SQLx, arbitrary HTTP, provider SDKs, secrets, brokers or object storage;
- it is reused by at least two independent consumers;
- it represents a separate trust or security boundary;
- it is a separately operating worker/process boundary;
- it is a credible future extraction seam documented by ADR;
- package visibility cannot enforce the same rule adequately.

A new crate is rejected when it contains only one handler, one planner, one capability-specific composition function, a thin re-export, copied validation or types belonging to an existing contract/application package.

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

Current gap: `crm-first-party-modules` proves the aggregation model for a limited owner set, while `crm-application-runtime` still imports many concrete domain adapters and composition crates.

For every first-party owner:

1. define one owner-owned `build_contribution`;
2. compose validators, planners, handlers, queries and workers inside the owner production package;
3. expose exact route/worker metadata;
4. merge into the mechanically checked first-party bundle;
5. remove corresponding concrete imports from generic runtime;
6. retain startup rejection for duplicates, owner mismatch, route-kind mismatch and incomplete handlers.

Completion evidence:

- adding a capability changes no generic runtime source;
- adding a worker changes no generic worker algorithm;
- adding a module changes its package/manifest and generated/mechanically checked bundle only;
- manual module-ID lists outside authoritative inventories are absent;
- central runtime direct domain dependencies and LOC decrease or remain stable.

## 7. Workspace dependency and exception governance

Introduce root `[workspace.dependencies]` for common families such as `serde`, `serde_json`, `sha2`, `prost`, `tokio`, `sqlx`, `tonic`, `http` and test libraries.

Use `cargo metadata` and `cargo tree --duplicates` to report:

- duplicate dependency families and major/minor versions;
- packages not inheriting available workspace dependencies;
- feature-set divergence and unexpectedly heavy features;
- reverse-dependency fan-out changes;
- public API and workspace-member growth.

Lockfile-only drift must be reproducible through `python scripts/repo.py lock`.

Temporary exceptions live in one machine-readable registry and include:

```text
id
owner
rule being bypassed
reason and risk
scope
created date
expiry date
removal condition
compensating checks
tracking issue
```

Expired, ownerless or undocumented exceptions block conformance.

## 8. Test architecture program

### 8.1 Structural preflight

Always verify architecture boundaries, manifests/IR, Protobuf bindings, route classifications, generated-source freshness, formatting, dependency policy, crate justification, exception validity and documentation/navigation consistency.

### 8.2 Generic conformance

**Generic conformance** suites own standard platform behavior.

Mutation conformance covers activation, malformed input, tenant mismatch, denied live authorization, idempotency/replay, safe errors and atomic audit/outbox/idempotency evidence.

Query conformance covers activation, authorization denial, not-found concealment, tenant mismatch, malformed cursor, no query-side writes and pagination evidence.

Worker conformance covers activation, deterministic phase, bounded work, retry classification, idempotent claim/crash recovery and no fixed central wiring.

Owner suites remain responsible for unique domain semantics.

### 8.3 CI levels

- Level 0: fast structural preflight.
- Level 1: affected Rust/package closure.
- Level 2: affected owner-domain acceptance.
- Level 3: real process acceptance for route, worker, persistence, authorization or protocol changes.
- Level 4: full repository matrix.

## 9. Developer tooling and navigation program

### 9.1 Stable documentation index

`docs/README.md` is the task-oriented human navigation entry point. It links to authoritative sources and does not copy live evidence.

README and `AGENTS.md` are orientation only. They point to the index, status and active issue rather than maintaining independent roadmaps.

### 9.2 `repo.py explain`

The command traces:

```text
module manifest
→ published contract
→ command/query/worker
→ validator/planner/handler
→ persistence/external adapter and migrations
→ production contribution
→ ingress or worker inventory
→ focused tests
→ required workflows
```

### 9.3 `repo.py packet-check`

The command reports active packet and baseline SHA, allowed/forbidden paths, affected closure, contracts/routes/workers/migrations, missing documentation or generated updates, required checks, evidence state and blockers.

### 9.4 Active packet

`docs/ACTIVE_PACKET.md` is generated from authoritative status, issue and contract inputs. It includes:

- packet identity and state;
- accepted baseline SHA;
- owner and exact scope;
- explicit exclusions;
- allowed and forbidden architecture areas;
- required acceptance levels;
- links to authoritative sources.

### 9.5 Repository map

`docs/generated/REPOSITORY_MAP.md` is generated from manifests, Cargo metadata, contracts, route/worker inventories, migration ownership and focused test metadata.

Each module entry includes identity, owner, authoritative objects, pure core, application, infrastructure, production contribution, contracts, routes/workers, migrations, tests and applicable policies.

### 9.6 Freshness

Generated navigation has deterministic ordering and a source digest. CI rejects stale output. Human editing of generated sections is forbidden.

## 10. Documentation source-of-truth model

Use documents for distinct purposes:

- `SYSTEM_INVARIANTS.md` — hard architecture rules;
- published contracts and ADRs — immutable machine/public decisions;
- `APPLICATION_ARCHITECTURE.md` — stable layer/composition model;
- this document — 10/10 cross-cutting execution program;
- `IMPLEMENTATION_ROADMAP.md` — product dependency order;
- `PHASE8_DELIVERY_PLAN.md` — active Phase 8 sequence;
- `PROJECT_STATUS.md` — concise merged state and next step;
- `MODULE_CATALOG.md` — owner and product-completeness accounting;
- active issues/PRs — executable work state;
- accepted packet documents — historical boundaries;
- `docs/README.md` and generated navigation — orientation only.

Mechanically reject stale phases, stale owner counts, conflicting next packets, unsupported completion claims, changing status in historical documents and duplicate normative lists.

## 11. Transitional crate consolidation

**Transitional crate consolidation** is gradual and evidence-based.

Prioritize one-consumer capability-specific adapters, thin composition crates, command/query crates with no unique boundary, private packages with no lifecycle and domain clusters with many physical locations per capability.

Do not automatically consolidate pure owner modules, core contracts/SDKs/runtimes, provider/secret boundaries, independent processes, packages with multiple stable consumers or deliberate extraction seams.

Every consolidation PR is behavior-neutral, preserves public coordinates and route/activation parity, proves applicable PostgreSQL/process behavior and reports package count, fan-out, public surface, build/test effect and files-per-capability before/after.

Stop or revert when measured complexity worsens.

## 12. Immediate Customer Privacy rule

The next Phase 8A packet remains scope discovery and immutable snapshot.

Do not implement it by adding one crate per command, query, worker or composition fragment.

Required sequence:

1. freeze the discovery/snapshot contract and acceptance boundary;
2. identify target Customer Privacy package ownership;
3. perform necessary consolidation only in a separate behavior-neutral PR;
4. implement inside target application/postgres/production packages;
5. add generic conformance only for proven shared behavior;
6. run focused, PostgreSQL, rollback/reapply and real-process acceptance;
7. synchronize roadmap/status/catalog/issue evidence after merge.

Target packaging:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

## 13. Delivery sequence

The program runs alongside product delivery and is tracked by issue #194.

### Stage A — documentation and policy baseline

Establish one plan, synchronize status/roadmap, add stale-document checks, create the stable documentation index and record reproducible metrics.

Exit: one unambiguous source hierarchy and baseline.

### Stage B — dependency, crate and exception governance

Add workspace dependency inheritance; report duplicates, features, fan-out and public surface; require crate justification; introduce expiring exception registry; promote calibrated rules from measurement to warning to blocking.

Exit: changes cannot silently add packaging or dependency debt.

### Stage C — golden owner package and persistence model

Approve domain/application/postgres/production structure; extend scaffolding; enforce migration ownership and Rust visibility; prove on one owner; require zero new crates for normal capabilities.

Exit: predictable code and schema ownership.

### Stage D — contribution aggregation

Finalize contribution interface, migrate a small owner, migrate Customer Privacy, migrate remaining owners and remove concrete domain imports from generic runtime.

Exit: generic composition remains stable.

### Stage E — affected-scope CI

Map changed paths, reverse dependencies, contracts, migrations, routes, workers and frontend scopes; explain broadening; retain full nightly/closure/release matrices.

Exit: iteration cost follows affected closure.

### Stage F — generic conformance and contract lifecycle

Centralize mutation/query/worker guarantees; add compatibility/deprecation/retirement checks; migrate owners without losing semantic tests; measure copied-test reduction.

Exit: shared proof is reusable and published versions have controlled lifecycle.

### Stage G — transitional consolidation

Rank candidates, consolidate one domain cluster at a time, preserve behavior and stop when metrics do not improve.

Exit: fewer physical locations without weaker boundaries.

### Stage H — reproducible developer environment and navigation

Implement `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo`, `smoke`, `explain` and `packet-check`; generate active packet and repository map; freshness-check navigation; include changed-scope reasoning in CI.

Exit: a new developer can prepare the environment and locate the correct change path without repository archaeology.

### Stage I — Frontend and operations parity

Add component/browser/accessibility proof, organize frontend by domain surfaces and add restore, SLO, performance, security and supply-chain gates.

Exit: architecture quality is proven across product and operations.

## 14. Metrics and budgets

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

## 15. 10/10 completion criteria

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
16. at least two later expert domain waves demonstrate bounded extension cost as module count grows.

## 16. Change control

This plan may change only with explicit architecture rationale, ownership/dependency impact, before/after complexity evidence, acceptance/rollback strategy and synchronized issue/status updates.

An optimization is accepted only when it makes the correct architecture easier to extend, easier to understand, easier to verify and harder to bypass.

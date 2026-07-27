# Ultimate CRM — Architecture Complexity, Scalability and Developer Experience 10/10 Plan

Status: **Normative cross-cutting execution plan**  
Tracking issue: #194  
Audit baseline: **2026-07-27**  
Applies to: repository structure, Rust workspace packaging, module composition, dependency governance, CI/test selection, developer tooling, documentation consistency, frontend architecture and production operations.

Governing precedence:

1. `SYSTEM_INVARIANTS.md`;
2. accepted ADRs and published contracts;
3. `APPLICATION_ARCHITECTURE.md`;
4. this execution plan;
5. descriptive module and packet documentation.

This document is the single current execution plan for architecture complexity and developer experience. Historical PR evidence remains in the relevant packet documents and must not be duplicated here.

## 1. Executive decision

The foundational architecture is sound and must not be replaced.

The platform already has the correct long-term model:

- a modular monolith with independently governed owner and link modules;
- one authoritative owner for every mutable aggregate;
- pure domain modules behind stable contracts and SDK ports;
- no direct cross-module storage access;
- exact versioned mutation, query, event and worker coordinates;
- durable tenant activation;
- live authorization immediately before side effects;
- transactionally consistent state, idempotency, outbox and audit evidence;
- FORCE RLS and cross-tenant negative proof;
- rebuildable non-authoritative projections, search and caches;
- exact-head acceptance discipline.

The remaining architecture risk is accidental complexity:

- too many physical crates for one business capability;
- manual domain-specific composition in the central application runtime;
- repeated dependency declarations and feature divergence;
- copied acceptance wiring and expanding workflow fan-out;
- too many files and packages touched by a normal vertical change;
- incomplete repository navigation and packet explanation tooling;
- documentation entry points that can drift from the active roadmap.

The required direction is:

> Preserve strict business ownership and governed runtime boundaries while making the normal cost of adding or changing one capability close to constant with respect to total product size.

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
| Overall architecture maturity | 8.3/10 | 10/10 |

These scores are planning signals, not product-completion claims. Product-complete expert modules remain counted separately in `MODULE_CATALOG.md` and `PROJECT_STATUS.md`.

The root Cargo workspace currently contains 109 members. Strict compile-time boundaries are valuable, but workspace growth must no longer track individual commands, queries or delivery packets.

## 3. 10/10 target state

Architecture and developer experience are considered 10/10 only when all conditions below are executable and measured.

### 3.1 Business ownership

- Every mutable domain has one authoritative owner.
- Business modules never import another business module's internals, repositories or tables.
- Cross-domain mutation uses exact capabilities; asynchronous coordination uses versioned events and optional link modules.
- Search, analytics, projections and caches remain rebuildable and non-authoritative.
- Disable, upgrade, rollback and uninstall behavior is explicit and tested.

### 3.2 Physical packaging

- A normal capability added to an existing owner creates **zero new crates**.
- A normal owner domain targets **three to five technical packages**:
  - pure domain;
  - application;
  - PostgreSQL/infrastructure;
  - production contribution;
  - optional extra package only for a real provider, trust, process or extraction boundary.
- Crates exist for dependency, trust, reuse, process or extraction boundaries—not for every handler.
- Transitional capability-specific crates are consolidated gradually through behavior-neutral packets with measured benefit.

### 3.3 Layering

The required dependency direction remains:

```text
domain <- application <- adapters <- production composition
```

The normal owner layout is:

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

### 3.4 Extensibility

A normal new capability follows:

```text
internal command/query/worker
-> existing owner application package
-> existing owner production contribution
-> generic conformance
-> affected owner acceptance
```

It must not require:

```text
new crate
-> new central runtime dependency
-> new business switch
-> copied platform test suite
-> unrelated full-workspace iteration
```

Generic router and worker algorithms must remain unchanged when one owner adds a route or worker.

### 3.5 Composition

Every owner production package exposes one stable contribution entry point:

```rust
pub fn build_contribution(
    context: &ProductionContext,
) -> Result<ModuleContributionSet, SdkError>;
```

The first-party bundle only aggregates owner-owned entry points. It contains no route catalog, capability switch or owner-specific dispatch.

The generic application runtime consumes contribution sets and must not grow direct dependencies for ordinary capability registration.

### 3.6 Dependency governance

- Common third-party dependencies are declared under root `[workspace.dependencies]`.
- Internal packages inherit workspace versions and approved feature sets.
- Duplicate direct dependency families are zero unless explicitly allowlisted.
- Multiple major/minor versions require a named blocker and removal condition.
- New heavy features, reverse-dependency fan-out and lockfile drift are reported mechanically.
- A new workspace member requires a short architecture justification.

### 3.7 Build and test scalability

During development:

- structural preflight always runs;
- changed packages and reverse dependencies are calculated;
- affected contracts, migrations, routes, workers and frontend packages select required checks;
- skipped checks have a machine-readable reason;
- unknown impact defaults to broader validation.

Full repository proof remains mandatory for:

- shared/core/runtime changes;
- architecture policy changes;
- phase closure;
- release;
- nightly validation;
- any change whose affected closure cannot be proven.

Affected-scope optimization never weakens the unchanged exact-head merge rule.

### 3.8 Developer comprehension

A developer or coding agent must be able to answer from one command:

- who owns this behavior;
- where its contract lives;
- which command/query/worker implements it;
- which persistence adapter is authoritative;
- how it enters production composition;
- what tests and workflows are required;
- what is explicitly out of scope.

Required tooling:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
python scripts/repo.py packet-check --base origin/main
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
```

Required generated navigation:

- `docs/ACTIVE_PACKET.md`;
- `docs/generated/REPOSITORY_MAP.md`.

Generated navigation is never an independent source of truth.

### 3.9 Frontend and operations

The same standard applies beyond backend architecture:

- frontend features are organized by stable domain surfaces;
- generated/governed clients are the only backend boundary;
- component, accessibility and browser tests cover critical workflows;
- session, routing, navigation and search concerns are separated;
- backup/restore, SLO, performance, security and supply-chain evidence are executable;
- restore tests prove active restrictions, legal holds, tombstones and audit integrity survive recovery.

## 4. Hard rules for all new work

1. A normal capability creates zero new crates.
2. A new owner module is justified by a new authoritative mutable domain, not by a screen, table, report, team or workflow step.
3. A new crate must protect a real dependency, trust, reuse, process, lifecycle or extraction boundary.
4. Generic router and worker code must not branch on business IDs.
5. Central manual composition LOC must not grow for an ordinary owner capability.
6. Feature behavior and crate consolidation must be separate PRs.
7. Shared abstractions are extracted only after at least two contrasting real implementations prove common behavior.
8. No cross-owner SQL, internal imports or repository access.
9. No replacement of live authorization with cached approval.
10. No reduction of PostgreSQL, process, rollback, RLS or exact-head evidence for speed.
11. Every structural PR reports before/after complexity impact.
12. Documentation status changes are mechanically checked against the active roadmap.

## 5. Crate creation decision

A new crate is accepted only when at least one condition is true:

- it prevents a forbidden dependency from entering pure domain/application code;
- it isolates SQLx, arbitrary HTTP, provider SDKs, secrets, brokers or object storage;
- it is reused by at least two independent consumers;
- it represents a separate trust or security boundary;
- it is a separately operating worker/process boundary;
- it is a credible future extraction seam documented by ADR;
- package visibility cannot enforce the same rule adequately.

A new crate is rejected when it contains only:

- one command handler;
- one query planner;
- one capability-specific composition function;
- a thin re-export;
- copied validation for one owner;
- types that belong to an existing contract or application package.

Required review note:

```text
New crate justification:
- protected boundary:
- isolated dependencies:
- expected consumers:
- why an internal Rust module is insufficient:
- lifecycle or extraction seam:
- expected build/test fan-out:
```

## 6. Module-owned production contribution program

### 6.1 Current gap

`crm-first-party-modules` proves the aggregation model for a limited owner set, but `crm-application-runtime` still imports many concrete domain adapters and composition crates.

This is the highest-priority extensibility hotspot.

### 6.2 Target

For every first-party owner:

1. define one owner-owned `build_contribution`;
2. compose validators, planners, handlers, queries and workers inside the owner production package;
3. expose exact route/worker metadata;
4. merge the result into the mechanically checked first-party bundle;
5. remove corresponding concrete imports from the generic application runtime;
6. retain startup rejection for duplicates, owner mismatch, route-kind mismatch and incomplete handlers.

### 6.3 Completion evidence

- adding a capability changes no generic runtime source;
- adding a worker changes no generic worker algorithm;
- adding a module changes its manifest/package and generated/mechanically checked bundle only;
- manual module-ID lists outside authoritative inventories are absent;
- central application-runtime direct domain dependencies and LOC decrease or remain stable.

## 7. Workspace dependency program

### 7.1 Target root policy

Introduce root `[workspace.dependencies]` for common families, including as applicable:

- `serde`;
- `serde_json`;
- `sha2`;
- `prost`;
- `tokio`;
- `sqlx`;
- `tonic`;
- `http`;
- common test libraries.

Internal packages use `.workspace = true` where compatible.

### 7.2 Mechanical checks

Use `cargo metadata` and `cargo tree --duplicates` to report:

- duplicate direct dependency families;
- duplicate major/minor versions;
- packages not inheriting an available workspace dependency;
- feature-set divergence;
- unexpectedly enabled heavy features;
- reverse-dependency fan-out changes.

Lockfile-only drift must be reproducible through `python scripts/repo.py lock`.

## 8. Test architecture program

### 8.1 Structural preflight

Always verify:

- architecture dependency/source boundaries;
- manifest and normalized-IR parity;
- Protobuf binding freshness;
- production route and non-runtime classification parity;
- generated source freshness;
- formatting;
- dependency policy;
- new-crate justification;
- documentation consistency.

### 8.2 Generic conformance

Reusable suites own standard platform behavior.

Mutation conformance:

- module inactive/not installed;
- malformed coordinate or payload;
- tenant mismatch;
- denied live authorization;
- idempotency/replay;
- safe typed errors;
- audit/outbox/idempotency expectations.

Query conformance:

- module inactive/not installed;
- live authorization denial;
- not-found concealment;
- tenant mismatch;
- malformed cursor;
- no query-side writes;
- pagination evidence.

Worker conformance:

- durable activation gating;
- deterministic phase;
- bounded work;
- retry classification;
- idempotent claim and crash recovery;
- no fixed central wiring.

Owner suites remain responsible for unique domain semantics.

### 8.3 CI levels

- **Level 0:** fast structural preflight.
- **Level 1:** affected Rust/package closure.
- **Level 2:** affected owner domain acceptance.
- **Level 3:** real process acceptance for route, worker, persistence, authorization or protocol changes.
- **Level 4:** full repository matrix.

## 9. Developer tooling program

### 9.1 `repo.py explain`

The command traces:

```text
module manifest
-> published contract
-> command/query/worker
-> validator/planner/handler
-> persistence/external adapter
-> production contribution
-> ingress or worker inventory
-> focused tests
-> required workflows
```

### 9.2 `repo.py packet-check`

The command reports:

- active packet and accepted baseline SHA;
- changed architecture areas;
- allowed and forbidden paths;
- affected package closure;
- contracts/routes/workers/migrations in scope;
- missing documentation or inventory updates;
- required checks;
- exact-head evidence state;
- blockers to gate review.

### 9.3 Repository map

Each module entry includes:

- identity and owner;
- authoritative objects;
- pure core;
- application;
- infrastructure;
- production contribution;
- contracts, capabilities, queries, events and workers;
- migrations;
- focused commands;
- applicable policies.

## 10. Documentation source-of-truth model

Use documents for distinct purposes:

- `SYSTEM_INVARIANTS.md` — hard architecture rules;
- `APPLICATION_ARCHITECTURE.md` — stable layer and composition model;
- this document — current 10/10 cross-cutting execution program;
- `IMPLEMENTATION_ROADMAP.md` — dependency order of product phases and cross-cutting programs;
- `PHASE8_DELIVERY_PLAN.md` — active Phase 8 sequence and packet constraints;
- `PROJECT_STATUS.md` — concise current merged state and next step;
- `MODULE_CATALOG.md` — business owner and product-completeness accounting;
- active GitHub issues — executable work state;
- accepted packet documents — historical acceptance boundaries.

README is orientation only. It must not contain a second independently maintained roadmap.

Mechanically reject:

- stale phase claims;
- stale owner counts;
- conflicting next-packet statements;
- product-completion claims unsupported by the catalog;
- execution status embedded in historical packet documents;
- duplicate normative lists maintained independently.

## 11. Transitional crate consolidation

Consolidation is gradual and evidence-based.

Prioritize:

- one-consumer capability-specific adapters;
- thin composition crates;
- command/query crates with no unique dependency boundary;
- private packages with no independent lifecycle;
- domain clusters with many physical locations per capability.

Do not automatically consolidate:

- pure owner modules;
- core contracts, SDKs and runtimes;
- provider transports and secret boundaries;
- independent worker/process boundaries;
- packages with multiple stable consumers;
- deliberate extraction seams.

Every consolidation PR must:

- be behavior-neutral;
- preserve public contracts and exact coordinates;
- preserve route/manifest/activation parity;
- prove focused, PostgreSQL and real-process behavior as applicable;
- report package count, dependency fan-out, build/test effect and files-per-capability before and after;
- be reverted or stopped when measurable complexity worsens.

## 12. Immediate Customer Privacy rule

The next Phase 8A packet remains scope discovery and immutable snapshot.

Do not implement it by adding one crate per command, query, worker or composition fragment.

Required sequence:

1. freeze the discovery/snapshot contract and acceptance boundary;
2. identify the target Customer Privacy package ownership;
3. perform any necessary behavior-neutral consolidation separately;
4. implement discovery/snapshot inside the target application/postgres/production packages;
5. add generic conformance only for proven shared platform behavior;
6. run focused, PostgreSQL, rollback/reapply and real-process acceptance;
7. synchronize roadmap/status/catalog/issue evidence after merge.

Target Customer Privacy packaging:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

Provider or process packages may be added only when a real boundary exists.

## 13. Delivery sequence

The cross-cutting program is tracked by issue #194 and proceeds without pausing product delivery.

### Stage A — documentation and policy baseline

- establish this single current plan;
- synchronize README, roadmap, Phase 8 plan and status;
- add mechanical stale-document checks;
- record baseline workspace/package and composition metrics.

Exit: one unambiguous source hierarchy and reproducible baseline.

### Stage B — dependency and crate governance

- add workspace dependency inheritance;
- report duplicates/features/fan-out;
- require new-crate justification;
- begin with warnings, then promote calibrated hard failures.

Exit: normal changes cannot silently add packaging or dependency debt.

### Stage C — golden owner package model

- approve domain/application/postgres/production structure;
- extend scaffolding;
- prove it on one owner;
- require zero new crates for normal capabilities.

Exit: predictable paths and constant-cost capability additions.

### Stage D — contribution aggregation

- finalize stable contribution interface;
- migrate one small owner;
- migrate Customer Privacy;
- migrate remaining owners incrementally;
- remove concrete domain imports from generic runtime.

Exit: generic composition size is stable.

### Stage E — affected-scope CI

- map changed paths and reverse dependencies;
- map contracts, migrations, routes, workers and frontend scopes;
- make check selection explainable;
- retain full nightly/closure/release matrices.

Exit: local and PR iteration cost follows the affected closure.

### Stage F — generic conformance

- centralize standard mutation/query/worker guarantees;
- migrate owners without losing domain-specific tests;
- measure copied-test reduction.

Exit: standard platform proof is reusable and owner semantics remain explicit.

### Stage G — transitional consolidation

- rank candidates by one-consumer status, fan-out and files-per-capability;
- consolidate one domain cluster at a time;
- preserve behavior and exact coordinates;
- stop when metrics do not improve.

Exit: fewer physical locations without weaker boundaries.

### Stage H — developer navigation

- generate active packet and repository map;
- implement `explain` and `packet-check`;
- include changed-scope reasoning in CI.

Exit: a new developer can locate the correct change path without reconstructing the repository manually.

### Stage I — frontend and operations parity

- add real component/browser/accessibility proof;
- organize frontend by domain surfaces;
- add restore, SLO, performance, security and supply-chain gates.

Exit: architecture quality is proven across the full product and operational lifecycle.

## 14. Metrics and budgets

Publish per structural PR:

```text
Workspace members:
Business modules:
Packages by category:
Maximum dependency depth:
Maximum reverse fan-out:
Duplicate dependency versions:
Manual composition LOC:
Manual registration points:
Packages affected by representative leaf change:
Files changed per normal capability:
Focused/domain/full test duration:
Clean and incremental build duration:
Generic versus owner-specific test ratio:
```

Initial budgets:

- normal capability: zero new crates;
- normal owner: three to five technical packages;
- duplicate direct dependency versions: zero unless allowlisted;
- generic composition LOC: no growth for ordinary capability registration;
- manual module-ID lists: zero outside authoritative generated paths;
- more than 15% build/test regression requires explanation;
- more than 25% regression blocks unless explicitly accepted for necessary functionality;
- leaf-domain affected closure must not grow without dependency rationale.

Budgets move from measurement to warning to blocking only after a representative baseline is collected.

## 15. 10/10 completion criteria

Issue #194 closes only when:

1. ordinary capabilities create zero new crates by enforced default;
2. new owner domains use the golden package model;
3. generic application runtime no longer grows for ordinary module registration;
4. all active first-party owners expose module-owned contribution entry points;
5. workspace dependency and feature policy is mechanically enforced;
6. affected-scope CI is implemented with explainable broadening and safe fallback;
7. generic mutation/query/worker conformance suites are adopted;
8. at least one transitional domain cluster is consolidated with measured improvement;
9. `ACTIVE_PACKET`, repository map, `explain` and `packet-check` are available;
10. stale phase/status/next-packet documentation is mechanically rejected;
11. frontend critical journeys have component/browser/accessibility evidence;
12. restore, SLO, performance, security and supply-chain gates are executable;
13. exact-head, tenant, RLS, authorization, audit, idempotency, rollback and route parity guarantees remain unchanged;
14. at least two later expert domain waves demonstrate that extension cost remains bounded as module count grows.

## 16. Change control

This plan may be changed only with:

- explicit architecture rationale;
- effect on ownership and dependency enforcement;
- before/after complexity evidence;
- acceptance and rollback strategy;
- synchronized roadmap/status/issue changes.

An optimization is accepted only when it makes the correct architecture easier to extend, easier to understand, easier to verify and harder to bypass.

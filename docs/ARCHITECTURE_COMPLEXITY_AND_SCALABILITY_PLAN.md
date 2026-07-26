# Ultimate CRM — Architecture Complexity and Scalability Plan

Status: **Normative architecture evolution plan**  
Audit baseline: **2026-07-25**  
Execution status: **Scalability runway accepted through PR #173; five privacy owners accepted through PR #183; behavior-neutral shared support accepted through PR #176 and mechanically proven with Customer Accounts and Contact Points as its third and fourth consumers.**  
Applies to: repository structure, Rust workspace packaging, production composition, dependency governance, test selection, delivery tooling and agent-oriented development ergonomics.  
Governing documents: `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `ARCHITECTURE_READINESS.md`, `DELIVERY_GOVERNANCE.md`, `IMPLEMENTATION_ROADMAP.md`, `MODULE_DEVELOPMENT.md`.

## 1. Executive decision

The current architecture is fundamentally sound and must not be replaced. The repository already enforces the most important properties of a universal modular CRM:

- one authoritative owner for mutable business state;
- pure business modules behind governed SDK and contract boundaries;
- no direct cross-module storage access;
- exact versioned capability and event coordinates;
- generic routing and module-owned production contributions;
- durable tenant activation;
- transactionally consistent persistence, outbox, audit and idempotency;
- tenant isolation and FORCE RLS proof;
- exact-head acceptance discipline.

The identified risk is not incorrect domain architecture. The risk is that physical technical decomposition, manual composition and repeated acceptance wiring may grow faster than useful product functionality.

At the audit baseline the workspace contains approximately:

- 84 technical crates;
- 13 business modules;
- one service composition root.

This provides precise compile-time boundaries, but it also increases navigation cost, clean-build cost, dependency duplication risk, CI fan-out and the number of files touched by one vertical change.

The required response is therefore:

> Preserve strict business ownership and governed runtime boundaries, while reducing unnecessary physical compilation boundaries, central manual wiring and repeated test implementation.

This plan does **not** authorize a broad rewrite or indiscriminate crate consolidation. Every structural change must preserve behavior, contracts, route parity, tenant isolation and exact-head evidence.

## 2. Confidence statement and conditions

The changes in this plan are expected to improve both development efficiency and architecture quality when the following conditions are enforced:

1. Business modules remain authoritative ownership boundaries regardless of crate packaging.
2. Pure domain code remains infrastructure-free.
3. Cross-owner interaction remains contract-driven and governed.
4. Generic runtime algorithms never gain owner-specific branching.
5. Consolidation packets are behavior-neutral and separately reviewed from feature packets.
6. Common abstractions are extracted only after at least two contrasting real implementations prove the repeated protocol.
7. CI optimization changes which checks run during iteration, but never weakens final exact-head acceptance.
8. Complexity changes are measured before they become mandatory policy.

Under these conditions the plan improves:

- modularity, by making business boundaries more visible than implementation fragments;
- layering, by standardizing domain/application/infrastructure/production packages;
- extensibility, by replacing central registration with module-owned contribution aggregation;
- correctness, by centralizing repeated protocol validation and generic conformance tests;
- testability, by separating universal runtime invariants from owner-specific semantics;
- build performance, by reducing duplicate dependencies and unnecessary package fan-out;
- maintainability, by reducing the number of physical locations required for one capability;
- agent usability, by generating an explicit repository map, affected scope and active packet context;
- operational confidence, by preserving full nightly, phase-closure and release verification.

## 3. Problem model

### 3.1 Healthy complexity

The following complexity is intentional and must remain:

- distinct owner modules for distinct mutable domains;
- versioned public contracts;
- explicit authorization and data-class boundaries;
- separate adapters for genuinely different infrastructure or trust boundaries;
- link modules for optional cross-domain coordination;
- exact route and worker inventories;
- migration, rollback, RLS and process acceptance.

### 3.2 Accidental complexity

The following complexity should be reduced:

- one crate per capability or bounded delivery slice;
- one composition crate per small handler when no independent trust or reuse boundary exists;
- repeated dependency declarations and multiple versions of the same third-party library;
- manually maintained module-ID lists in more than one source;
- central composition files that import and wire every concrete adapter;
- copied request, lineage, registry, pagination and evidence validation across owner adapters;
- full-workspace test execution for every leaf-domain iteration;
- documentation entry paths that require an agent to reconstruct the active packet from many files;
- public infrastructure primitives whose allowed consumers are documented but not mechanically enforced.

### 3.3 Growth modes

Project complexity remains approximately linear when:

- a new capability is an internal use case inside one owner package;
- the owner contributes its routes and workers through a stable production interface;
- generic conformance tests automatically cover standard behavior;
- only the changed package and reverse-dependency closure are rebuilt during iteration;
- full validation runs at exact-head gate, nightly, phase closure and release.

Project complexity becomes quadratic or worse when:

- modules directly depend on many other modules;
- every capability adds another crate and central registration branch;
- a new domain requires edits across many unrelated packages;
- each owner reimplements the full cross-owner protocol;
- every change invalidates and reruns all unrelated database/process/browser suites during normal development.

## 4. Architecture objectives

### 4.1 Primary objectives

1. Keep business ownership boundaries strict and mechanically enforced.
2. Make the normal cost of adding a capability close to constant with respect to total module count.
3. Keep generic runtime and composition algorithms stable as product domains grow.
4. Ensure a leaf-domain change recompiles and retests only its affected closure during iteration.
5. Preserve complete exact-head proof before merge.
6. Make the repository self-explanatory to a human or development agent.
7. Prevent dependency and test cost from growing without visible metrics and explicit justification.

### 4.2 Non-goals

This plan does not:

- convert the modular monolith into microservices;
- merge authoritative business domains;
- remove exact versioned contracts;
- permit direct database access between modules;
- replace module manifests with Rust conventions;
- remove PostgreSQL, process or browser acceptance;
- weaken tenant activation, authorization, audit, idempotency or RLS rules;
- introduce dynamic plugin loading for trusted first-party modules;
- optimize crate count as an isolated vanity metric;
- pause active product delivery for a repository-wide rewrite.

## 5. Core decision: business module is not equal to crate

The repository must explicitly distinguish three concepts.

### 5.1 Business module

A business module is a long-lived ownership boundary. It owns authoritative state, invariants, public contracts and lifecycle behavior.

Examples include Parties, Consents, Customer Privacy, Sales, Activities, Catalog, Pricing, Orders, Contracts, Subscriptions, Billing and Service.

A business module remains independent even if several of its implementation layers are physically packaged together.

### 5.2 Crate

A crate is a compile-time and dependency boundary. It is justified only when it provides at least one concrete benefit:

- blocks an otherwise dangerous dependency;
- isolates infrastructure or a heavy third-party SDK;
- is independently reused by multiple consumers;
- represents a separate trust boundary;
- represents a plausible process extraction seam;
- has an independent lifecycle or feature set;
- materially improves incremental compilation.

A crate is not justified merely because:

- a new capability was added;
- a delivery packet is reviewed independently;
- a source file became large;
- an adapter has a distinct name;
- a use case has its own public contract;
- a team wants a separate directory.

### 5.3 Capability

A capability is a versioned public behavior. By default it is implemented as an internal command/query/worker module inside an existing domain application or production package.

The default rule is:

> Adding a capability to an existing owner module creates zero new crates.

Exceptions require an explicit crate justification reviewed with the packet.

## 6. Crate creation policy

Every newly added workspace member must include a machine-readable or review-visible justification:

```text
New crate justification:
- protected boundary:
- isolated dependencies:
- expected consumers:
- reason an internal Rust module is insufficient:
- lifecycle or extraction seam:
- expected effect on build/test fan-out:
```

### 6.1 Hard approval conditions

A new crate must satisfy at least one of:

1. It is a pure owner-domain boundary required to prevent infrastructure dependencies.
2. It isolates SQLx, HTTP, secrets, broker, object storage, provider SDK or another heavy infrastructure dependency.
3. It is a stable platform boundary consumed by at least two independent packages.
4. It is a production process or worker boundary with independent runtime characteristics.
5. It is a separately testable trust boundary that cannot be equivalently enforced by module visibility and architecture policy.
6. It is a deliberate future extraction seam documented in an ADR.

### 6.2 Default rejection conditions

A new crate should be rejected when it contains only:

- one handler and its registration;
- one query planner;
- one capability-specific composition function;
- a thin re-export layer;
- a duplicate protocol implementation for one owner;
- types that belong in an existing stable contract or module package.

### 6.3 Policy enforcement

Introduce a repository check that detects new workspace members and requires an entry in an allowlisted architecture decision file or package metadata.

The first implementation should report warnings. After one full phase of baseline collection, unjustified new crates become a blocking error.

## 7. Target domain package model

A complex owner domain should normally use three or four packages.

```text
modules/crm-<domain>/
    Cargo.toml
    module.yaml
    src/
        domain/
        policy/
        value/
        lib.rs

crates/crm-<domain>-application/
    src/
        commands/
        queries/
        workers/
        validation/
        ports/
        errors.rs
        lib.rs

crates/crm-<domain>-postgres/
    src/
        repositories/
        read_models/
        locks/
        migrations_support/
        lib.rs

crates/crm-<domain>-production/
    src/
        contribution.rs
        routes.rs
        workers.rs
        composition.rs
        lib.rs
```

A small owner domain may combine application and production when doing so does not introduce forbidden infrastructure into the pure core.

A provider-heavy or independently deployable domain may use additional packages for:

- provider HTTP transport;
- secrets;
- object storage;
- dedicated worker process;
- external protocol SDK.

These are exceptions based on real dependency/trust boundaries, not capability count.

### 7.1 Customer Privacy target example

The long-term Customer Privacy structure should converge toward:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

Commands such as create, submit, subject verify, cancel, approve and restriction placement belong under application commands rather than separate crates by default.

Queries such as get, list, plan get and outcomes list belong under application queries.

Owner orchestration workers belong under the production package unless a real independent process boundary exists.

### 7.2 Migration rule

Existing capability-specific crates are transitional, not automatically defective. They must be consolidated only when:

- behavior is already stable;
- contracts and route coordinates are frozen;
- the consolidation can be proven behavior-neutral;
- the resulting package has a clearer ownership and dependency boundary;
- build and test metrics improve or remain neutral;
- no independent trust boundary is lost.

Feature delivery and crate consolidation must not be mixed in one PR.

## 8. Production contribution aggregation

### 8.1 Current risk

The application runtime currently contains increasing domain-specific imports and manual adapter wiring. Although this is not a forbidden central business router, it creates a high-change composition hotspot and duplicate sources of truth.

### 8.2 Target contribution interface

Every production owner package should expose one stable entry point:

```rust
pub fn build_contribution(
    context: &ProductionContext,
) -> Result<ModuleContributionSet, SdkError>;
```

`ModuleContributionSet` may contain:

- exact mutation routes;
- exact query routes;
- pre-authorization semantic validators;
- visibility contributions;
- activation-gated workers;
- deterministic worker phases;
- startup validation metadata.

The owner production package composes its concrete planners, validators, handlers and adapters internally.

### 8.3 First-party module bundle

Create a narrow `crm-first-party-modules` package that aggregates module-owned entry points.

Its generated or mechanically verified source should resemble:

```rust
pub fn build_all(
    context: &ProductionContext,
) -> Result<Vec<ModuleContributionSet>, SdkError> {
    Ok(vec![
        crm_sales_production::build_contribution(context)?,
        crm_parties_production::build_contribution(context)?,
        crm_consents_production::build_contribution(context)?,
        crm_customer_privacy_production::build_contribution(context)?,
    ])
}
```

The application runtime then operates only on generic contributions:

```rust
let contributions = crm_first_party_modules::build_all(&context)?;
let application = ApplicationComposition::from_contributions(contributions)?;
```

### 8.4 Source-of-truth rule

Module identities and production coordinates must continue to originate from manifests, compiled definitions and machine-readable inventories.

The bundle may be generated from manifests or checked against them. It must never become another independently edited module catalog.

### 8.5 Completion criterion

After this migration:

- adding a capability does not modify generic application runtime;
- adding a worker does not modify generic worker algorithms;
- adding a business module changes its own manifest/package and the generated first-party bundle only;
- manual module-ID lists outside the authoritative generated path are removed;
- startup still rejects duplicates, owner mismatch, route-kind mismatch and incomplete handlers.

## 9. Dependency version and feature governance

### 9.1 Workspace dependencies

Common third-party dependencies must be declared under root `[workspace.dependencies]` and inherited by internal packages.

Example:

```toml
[workspace.dependencies]
prost = "0.14"
serde = "1"
serde_json = "1"
sha2 = "0.10"
tokio = "1"
sqlx = { version = "0.9", default-features = false }
```

Internal packages use:

```toml
prost.workspace = true
serde.workspace = true
sqlx.workspace = true
```

### 9.2 Version rules

- A new direct dependency version must not be introduced when the workspace already owns the dependency family.
- Multiple major/minor versions require an explicit temporary exception.
- Feature sets should be centralized where possible to prevent each crate enabling incompatible combinations.
- Exceptions must name the blocker, affected packages and removal condition.

### 9.3 Mechanical checks

Add checks based on `cargo metadata` and `cargo tree --duplicates` that report:

- duplicate direct dependency families;
- duplicate major/minor versions;
- packages not inheriting a workspace dependency;
- unexpectedly enabled heavy features;
- reverse-dependency fan-out changes.

### 9.4 Immediate application to the active Privacy adapter

Before promotion of the Parties Privacy scope adapter:

- align `prost` with the workspace/main version;
- align `sqlx` with the workspace/main version;
- avoid introducing duplicate dependency families;
- record any unavoidable exception explicitly.

## 10. Privacy owner contribution protocol

### 10.1 Risk

The Privacy program requires contributions from multiple authoritative owner modules. Copying the entire request validation, topology lineage, registry verification, pagination, evidence hashing and response logic into every owner adapter would create large correctness and maintenance risk.

Prematurely designing a universal framework from one Parties adapter would create a different risk: an abstraction shaped around only one owner.

### 10.2 Required two-implementation rule

1. Complete the Parties implementation without runtime promotion shortcuts.
2. Implement a second contrasting owner, preferably Consent or another domain with materially different authoritative records.
3. Compare the implementations.
4. Extract only behavior proven common by both.
5. Build remaining owner contributions on the validated protocol support.

No repository-wide privacy framework should be accepted based on a single owner implementation.

### 10.3 Candidate shared protocol package

A shared package may contain:

```text
crm-privacy-contribution-protocol/
    request_validation.rs
    case_subject_lineage.rs
    owner_registry_validation.rs
    pagination.rs
    evidence_hash.rs
    response_builder.rs
    contract_errors.rs
    conformance.rs
```

It may own:

- Protobuf envelope and coordinate validation;
- tenant, case and subject consistency validation;
- canonical Party and topology-generation checks;
- registry version/digest checks;
- deterministic pagination and cursor evidence;
- request/output hashing;
- common error classification;
- reference-only response constraints;
- generic protocol conformance tests.

It must not own:

- another owner’s authoritative SQL;
- owner record semantics;
- owner-specific retention policy;
- owner-specific data classification decisions;
- cross-owner mutation;
- central dispatch by owner ID.

### 10.4 Owner-specific interface

The shared protocol should call an owner-defined reader such as:

```rust
pub trait PrivacyScopeOwner {
    fn owner_module_id(&self) -> &ModuleId;

    async fn load_resources(
        &self,
        transaction: &mut BoundReadTransaction<'_>,
        request: &ValidatedScopeRequest,
    ) -> Result<OwnerResourcePage, ScopeError>;
}
```

The final interface must be selected only after the second implementation.

### 10.5 Size and duplication target

After extraction, an owner implementation should primarily contain:

- owner-specific authoritative query;
- strict row decoding/rehydration;
- owner resource typing and classification;
- owner-specific evidence mapping.

A typical owner contribution should be materially smaller than the initial full protocol implementation. Repeated validation code should approach zero outside the shared package.

## 11. Infrastructure boundary enforcement

### 11.1 Raw transaction risk

A public helper returning a raw SQLx transaction creates an attractive bypass around governed persistence abstractions. A documentation comment restricting usage is not sufficient.

### 11.2 Required control

Introduce mechanical enforcement that limits raw or bound transaction APIs to an exact allowlist of infrastructure/composition packages.

Possible mechanisms:

- architecture-policy source/import rules;
- a dedicated private infrastructure package with restricted public exports;
- sealed wrapper types that expose only approved read operations;
- package metadata checked by `check_architecture.py`;
- exact consumer allowlist generated from approved package identities.

### 11.3 Preferred long-term API

Where practical, prefer a governed wrapper:

```rust
pub struct BoundReadTransaction<'a> {
    inner: sqlx::Transaction<'a, Postgres>,
}
```

The wrapper should:

- set read-only mode;
- bind tenant context;
- prevent commit-side mutation helpers;
- expose only the minimum query execution surface;
- preserve no-write acceptance proof;
- remain available only to approved adapter packages.

Raw SQLx access may remain internally available where necessary, but its consumer set must be mechanically controlled.

## 12. Test architecture

The repository must preserve exact-head final proof while making normal iteration affected-scope aware.

### 12.1 Level 0 — structural preflight

Run for every relevant change:

- architecture dependency/source rules;
- module manifest validation;
- contract binding freshness;
- production route classification parity;
- generated source freshness;
- formatting;
- dependency version policy;
- new-crate justification;
- documentation consistency rules.

Target: fast enough for frequent local execution.

### 12.2 Level 1 — affected package closure

Introduce:

```bash
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
```

The implementation uses `git diff` and `cargo metadata` to determine:

- directly changed packages;
- reverse dependency closure;
- affected contracts;
- affected migrations;
- affected production routes/workers;
- affected frontend packages;
- required specialized workflows.

The command must print its reasoning so a developer or agent can understand why each check is required.

### 12.3 Level 2 — domain acceptance

For every affected owner domain run:

- pure domain unit tests;
- application command/query tests;
- adapter contract tests;
- PostgreSQL acceptance;
- RLS/cross-tenant negatives;
- optimistic concurrency;
- idempotency and replay where applicable;
- migration clean/apply/rollback/reapply where persistence changes.

Unrelated owner-domain database suites are not required during every local iteration.

### 12.4 Level 3 — process acceptance

Required when a packet changes:

- public routes;
- workers;
- production composition;
- public contracts;
- authorization or visibility;
- persistence behavior;
- ingress/egress protocols.

Process acceptance must exercise the real composition path, not an isolated handler.

### 12.5 Level 4 — full repository matrix

Run:

- nightly;
- when core SDK/runtime/shared contracts change;
- at phase closure;
- before release;
- when architecture policy or generic composition changes;
- when affected-scope analysis cannot prove isolation.

The full matrix includes all Rust, database, process, frontend, migration, route parity and operational gates.

### 12.6 Exact-head discipline

Affected-scope execution optimizes iteration only. A packet may reach Gate review only when all applicable checks pass on one unchanged candidate SHA under existing delivery governance.

No source or documentation commit may reuse earlier exact-head evidence.

## 13. Generic conformance suites

### 13.1 Purpose

Standard runtime guarantees should be tested once as reusable conformance behavior rather than hand-copied into every capability suite.

### 13.2 Mutation conformance

Every registered governed mutation should automatically support tests for relevant standard behavior:

- module disabled or not installed;
- missing or denied authorization;
- wrong owner/identifier/version/kind;
- malformed contract payload;
- tenant mismatch;
- duplicate/replay behavior where declared;
- safe public error mapping;
- audit/idempotency expectations where declared.

### 13.3 Query conformance

Every registered governed query should automatically support tests for relevant standard behavior:

- module disabled or not installed;
- live authorization denied;
- resource concealment;
- tenant mismatch;
- no query-side writes;
- malformed cursor or request;
- stable pagination evidence where declared.

### 13.4 Worker conformance

Every registered worker should automatically support tests for:

- tenant activation gating;
- deterministic phase assignment;
- bounded work;
- retry classification;
- idempotent claim/recovery behavior where declared;
- no fixed central worker wiring.

### 13.5 Owner-specific tests

Owner suites remain responsible for unique domain behavior, for example:

- Party merge topology;
- Consent withdrawal semantics;
- Privacy legal-hold precedence;
- Catalog effective dating;
- Quote revision rules;
- Subscription proration;
- Billing reconciliation.

The generic suite must not replace domain-specific invariant tests.

## 14. Agent and developer ergonomics

### 14.1 Active packet context

Generate `docs/ACTIVE_PACKET.md` from current roadmap/status/issue/manifests.

It should contain:

- current packet and state;
- authoritative owner and coordinator;
- accepted baseline SHA;
- allowed and forbidden paths;
- contracts and route coordinates in scope;
- runtime/persistence impact;
- required tests and workflows;
- explicit out-of-scope behavior;
- next completion condition.

The file is a generated entry point, not a replacement for normative documents.

### 14.2 Repository map

Generate `docs/generated/REPOSITORY_MAP.md` with one section per module:

- module identity and owner;
- authoritative objects;
- pure core path;
- application path;
- infrastructure path;
- production contribution path;
- capabilities, queries, events and workers;
- migrations;
- focused test commands;
- applicable architecture policies.

### 14.3 Explain command

Add:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
```

The output should trace:

```text
manifest ownership
-> contract binding
-> planner/validator/handler
-> persistence adapter
-> production contribution
-> ingress or worker inventory
-> focused tests
-> applicable workflows
```

### 14.4 Packet check command

Add:

```bash
python scripts/repo.py packet-check --base origin/main
```

It should report:

- detected architectural areas changed;
- affected package closure;
- missing documentation/inventory updates;
- required checks;
- current gate evidence;
- blockers preventing Gate review.

### 14.5 Standard internal layout

Production/application packages should use predictable directories:

```text
src/
    commands/
    queries/
    workers/
    validation/
    persistence/
    contracts/
    contribution.rs
    errors.rs
    lib.rs
```

Consistency is more valuable than a custom layout for every delivery packet.

## 15. Complexity observability

Introduce:

```text
docs/ARCHITECTURE_COMPLEXITY_POLICY.md
architecture-complexity-policy.json
scripts/analyze_workspace.py
```

The analyzer should publish machine-readable and human-readable metrics.

### 15.1 Required metrics

- number of business modules;
- number of technical crates;
- crates by category: core, application, infrastructure, production, service, tooling;
- direct internal dependency count per crate;
- reverse dependency fan-out per crate;
- maximum dependency depth;
- dependency cycles;
- duplicate third-party dependency families and versions;
- common dependency feature divergence;
- clean build duration;
- incremental leaf-domain build duration;
- focused/domain/full test duration;
- central composition manual LOC;
- manual module registration points;
- crates added per phase;
- files and packages changed per capability;
- repeated-code indicators across owner adapters;
- generic versus owner-specific test ratio.

### 15.2 Change report

Every structural PR should receive a report such as:

```text
Technical crates: 84 -> 85
Business modules: 13 -> 13
Maximum reverse fan-out: 31 -> 31
Duplicate dependency families: 2 -> 0
Manual composition LOC: 402 -> 402
Packages affected by leaf privacy change: 14 -> 9
```

### 15.3 Policy rollout

1. Measurement-only period for one delivery phase.
2. Warnings for unexplained regressions.
3. Blocking hard invariants.
4. Blocking relative budgets after the repository has a stable baseline.

### 15.4 Hard invariants

The following may become immediately blocking:

- business modules importing another business module’s internals;
- new central capability-ID or owner-ID switches;
- duplicate authoritative module-ID lists;
- new workspace member without crate justification;
- new direct dependency version when a workspace version exists;
- generic router/worker algorithm edits required only to register one feature;
- unapproved consumer of raw infrastructure transaction APIs.

### 15.5 Relative performance budgets

Absolute time budgets depend on CI hardware and must be calibrated. Use rolling relative budgets:

- more than 15% build/test regression requires explanation;
- more than 25% regression blocks unless approved as necessary functionality;
- a leaf-domain change should not expand affected package closure without explicit dependency reasoning;
- manual composition LOC should not grow as new owner capabilities are added;
- a normal capability should add zero crates;
- a normal new owner domain should target three to five technical packages, with documented exceptions.

## 16. Phased implementation plan

### Phase A — stabilize the active Privacy adapter

Goal: finish the current bounded slice without carrying known structural debt into nine owner implementations.

Actions:

1. Align direct dependency versions with workspace/main.
2. Split the large adapter into contract, request, PostgreSQL, response, evidence and error modules.
3. Keep one package unless an additional real boundary is proven.
4. Mechanically restrict the bound/raw read transaction API.
5. Add malformed, cross-tenant, stale-lineage, no-write and strict response evidence.
6. Keep the adapter non-runtime until its issue-defined production decision is reached.
7. Obtain unchanged exact-head checks.
8. Synchronize merged owner scope contract/status documentation.

Exit criteria:

- no new duplicate dependency family;
- clear internal layering;
- raw transaction consumers controlled;
- focused and PostgreSQL tests complete;
- exact-head evidence available;
- no premature common framework.

### Phase B — establish complexity measurement and policy

Goal: make complexity visible before changing repository-wide packaging.

Actions:

1. Add workspace analyzer.
2. Categorize existing crates.
3. Capture clean and incremental build baselines.
4. Capture CI/test duration baselines.
5. Add new-crate justification warnings.
6. Add duplicate dependency and fan-out reports.
7. Publish reports as CI artifacts and PR summaries.

Exit criteria:

- stable baseline collected across representative PRs;
- no policy blocks based on uncalibrated numbers;
- known high-fan-out packages identified;
- consolidation candidates ranked by evidence.

### Phase C — define the golden domain production package

Goal: prevent future capability-specific crate proliferation.

Actions:

1. Approve a standard domain/application/postgres/production package model.
2. Extend scaffold tooling to generate the model.
3. Add crate decision metadata.
4. Use the standard for the next new owner domain.
5. Document exceptions for provider-heavy and process-separated domains.

Exit criteria:

- a new capability can be added without a new crate;
- a new owner domain receives a predictable package layout;
- architecture policy still prevents infrastructure in pure modules;
- scaffold acceptance compiles and validates the generated packages.

### Phase D — module-owned contribution aggregation

Goal: stop central composition from growing with every domain.

Actions:

1. Define stable `build_contribution` interface.
2. Introduce first-party module bundle.
3. Migrate one small owner package.
4. Verify route/manifests/activation parity.
5. Migrate Customer Privacy.
6. Migrate remaining owners incrementally.
7. Remove duplicate module-ID lists.
8. Keep application runtime generic.

Exit criteria:

- new capability registration does not modify generic runtime;
- new module registration is manifest-driven/generated and exact;
- startup mismatch checks remain intact;
- composition root size becomes stable.

### Phase E — affected-scope CI

Goal: keep iteration cost proportional to the changed dependency closure.

Actions:

1. Implement affected package calculation.
2. Map contracts, migrations, routes and frontend packages to checks.
3. Add explainable `check-affected` output.
4. Split GitHub workflows into preflight, affected Rust, affected database, affected process and affected web.
5. Retain full nightly, phase-closure and release matrices.
6. Retain exact-head final gate rules.

Exit criteria:

- leaf changes no longer run unrelated domain suites during iteration;
- core/shared changes still expand to full matrix;
- false-negative risk is covered by nightly/full gates;
- every skipped suite has a machine-explainable reason.

### Phase F — second Privacy owner and protocol extraction

Goal: prevent repeated privacy protocol implementation without premature abstraction.

Actions:

1. Implement a contrasting second owner adapter.
2. Compare repeated logic with Parties.
3. Extract shared protocol support.
4. Add generic protocol conformance tests.
5. Migrate both owner adapters to the shared support.
6. Use the support for remaining owners.

Exit criteria:

- shared package contains protocol behavior only;
- owner SQL and semantics remain owner-specific;
- repeated validation is materially reduced;
- both implementations retain strict focused/PostgreSQL proof;
- no owner-ID central dispatch is introduced.

### Phase G — gradual consolidation of transitional crates

Goal: reduce accidental compilation boundaries without destabilizing product delivery.

Actions:

1. Rank one-consumer capability-specific crates.
2. Select one domain cluster at a time.
3. Create behavior-neutral consolidation PRs.
4. Preserve public contracts and route coordinates.
5. Run compare, route parity and process tests.
6. Measure build, fan-out and navigation effects.
7. Stop or revert consolidation that loses a real boundary or worsens metrics.

Initial candidates may include:

- thin capability composition crates;
- one-handler private adapter crates;
- privacy command/query composition pairs;
- private crates with one consumer and no unique infrastructure dependency.

Do not automatically consolidate:

- pure owner modules;
- core contracts/SDK/runtime;
- provider transport and secret boundaries;
- independent workers/processes;
- packages with multiple stable consumers;
- deliberate extraction seams.

Exit criteria:

- fewer physical locations per capability;
- equal or better incremental build behavior;
- no loss of mechanical architecture enforcement;
- no contract or runtime behavior change.

### Phase H — agent ergonomics and documentation synchronization

Goal: make correct development the easiest path for a human or agent.

Actions:

1. Generate active packet context.
2. Generate repository map.
3. Implement `repo.py explain`.
4. Implement `repo.py packet-check`.
5. Add changed-scope/check reasoning to CI output.
6. Ensure roadmap/status/catalog synchronization is part of packet completion.

Exit criteria:

- an agent can identify ownership, paths, contracts and required tests from one command;
- stale status sources are mechanically detectable where possible;
- generated navigation never becomes an independent source of truth.

### Phase I — frontend and operational parity

Goal: apply the same modularity and evidence model to the product shell and operations.

Actions:

1. Remove no-test pass-through as the normal frontend standard.
2. Add component tests and Playwright smoke/e2e coverage.
3. Split session, routing, navigation and search concerns.
4. Organize frontend features by stable domain surfaces.
5. Add accessibility and contract-compatibility checks.
6. Add backup/restore, SLO, performance, security and supply-chain operational evidence.

Exit criteria:

- frontend changes have affected-scope tests;
- critical user journeys have real browser proof;
- operational readiness is measured rather than planned only.

## 17. Recommended delivery order

The recommended order is:

```text
active Privacy adapter hygiene and proof
-> complexity baseline and dependency governance
-> golden domain production package
-> first-party contribution aggregation
-> affected-scope CI
-> second Privacy owner implementation
-> shared Privacy protocol extraction
-> remaining owner contributions
-> gradual transitional-crate consolidation
-> frontend and operational parity
```

This order avoids blocking active Privacy work while preventing its first implementation from becoming a copied template for all remaining owners.

## 18. Risks and mitigations

### 18.1 Risk: consolidation weakens boundaries

Mitigation:

- preserve pure domain modules;
- keep infrastructure behind package/module visibility;
- extend architecture source/import checks;
- require behavior-neutral consolidation packets;
- reject consolidation without measurable benefit.

### 18.2 Risk: first-party bundle becomes a central router

Mitigation:

- bundle only calls owner-owned contribution entry points;
- no business branching by capability or owner ID;
- generic runtime consumes typed contribution sets;
- startup parity checks remain authoritative;
- bundle source is generated or mechanically checked.

### 18.3 Risk: affected-scope CI skips a required test

Mitigation:

- shared/core changes expand to full matrix;
- full nightly and phase-closure suites remain mandatory;
- check selection prints reasoning;
- unknown impact defaults to broader checks;
- exact-head final acceptance remains unchanged.

### 18.4 Risk: shared Privacy protocol hides owner semantics

Mitigation:

- extract only after two contrasting implementations;
- keep authoritative SQL and classification owner-specific;
- prohibit owner-ID dispatch;
- maintain owner-specific acceptance suites;
- keep contracts and registry exact.

### 18.5 Risk: complexity policy becomes bureaucracy

Mitigation:

- automate reports;
- use warnings before blockers;
- require short objective justifications;
- measure outcomes rather than optimize raw crate count;
- allow documented exceptions for real boundaries.

### 18.6 Risk: structural work delays product delivery

Mitigation:

- integrate improvements at natural packet boundaries;
- use one pilot domain before repository-wide rollout;
- separate feature and consolidation PRs;
- prioritize changes that prevent repeated future work;
- do not perform a big-bang workspace rewrite.

## 19. Architecture quality scorecard

The target architecture is considered sustainable at expert level when all of the following are true.

| Dimension | Required state |
|---|---|
| Business modularity | Every mutable domain has one authoritative owner; no cross-owner internals/storage access |
| Physical packaging | Capabilities normally add zero crates; owner domains use a predictable small package set |
| Layering | Pure core, application, infrastructure and production responsibilities remain explicit |
| Extensibility | New capabilities/modules do not modify generic router or worker algorithms |
| Composition | Module-owned contribution entry points; generated/mechanically verified aggregation |
| Dependencies | Workspace versions centralized; duplicates and heavy features controlled |
| Build scalability | Leaf changes rebuild an explainable reverse-dependency closure |
| Test scalability | Generic conformance plus owner-specific tests; full matrix retained at required gates |
| Data safety | Transaction, RLS, authorization, audit, idempotency and rollback proof unchanged |
| Protocol reuse | Shared behavior extracted after two real implementations, not before |
| Agent usability | Active packet, repository map, explain and packet-check tooling available |
| Governance | Complexity metrics visible; regressions require explanation; status remains synchronized |
| Frontend quality | Real component/browser coverage and modular feature boundaries |
| Operations | Restore, resilience, SLO, performance, security and supply-chain evidence |

## 20. Quantitative target behavior

The purpose is not a fixed final crate count. The desired growth behavior is:

```text
new capability
-> internal command/query/worker module
-> owner-owned production contribution
-> generic conformance tests
-> affected domain acceptance
```

Instead of:

```text
new capability
-> new crate
-> new dependency graph node
-> new central wiring
-> copied end-to-end suite
-> broader full-workspace iteration
```

Expected targets after rollout:

- normal capability: zero new crates;
- normal owner domain: three to five technical packages;
- generic composition LOC: stable as capabilities increase;
- duplicate direct dependency versions: zero unless allowlisted;
- repeated owner protocol validation: centralized after proven reuse;
- leaf-domain incremental checks: affected closure only;
- full matrix: nightly, architecture/core changes, phase closure and release;
- business module count: grows only when authoritative ownership truly grows;
- technical crate count: grows slower than delivered capability count and may decline through evidence-based consolidation.

## 21. Definition of done for this plan

This plan is complete only when:

1. It is linked from the normative delivery/roadmap hierarchy.
2. Complexity baseline tooling exists and produces reproducible output.
3. New-crate justification and workspace dependency policies are enforced.
4. A golden domain production package is accepted.
5. Generic application runtime no longer grows for ordinary module contributions.
6. Affected-scope CI is implemented without weakening exact-head acceptance.
7. Two Privacy owner adapters validate the shared protocol extraction.
8. At least one transitional domain cluster is consolidated with measured improvement.
9. Agent navigation commands and generated repository map are available.
10. Frontend and operational quality gaps have explicit executable gates.

## 22. Change control

This document is subordinate to `SYSTEM_INVARIANTS.md` and `ARCHITECTURE_READINESS.md`.

Any implementation that conflicts with authoritative ownership, governed contracts, exact-coordinate routing, durable activation, transactionality, tenant isolation or exact-head evidence is invalid even when it appears to reduce crate count or build time.

Changes to the hard rules in this plan require:

- explicit architecture rationale;
- impact on business ownership and dependency enforcement;
- before/after complexity metrics;
- affected acceptance strategy;
- migration/rollback plan;
- synchronized roadmap/status documentation.

Optimization is accepted only when it makes the correct architecture easier to extend, easier to verify and harder to bypass.

# Workspace Dependency, Packaging and CI Complexity Baseline

Snapshot source head: `6d1d6119aa3b759031206c02cd3befe8a65f80d0`

Tracking issue: #194  
Product boundary preserved: #126

> Stage B measurement baseline. Dependency/version/feature/fan-out/public-surface observations are reports and warnings, not calibrated blocking budgets. Invalid or expired architecture exceptions, missing exception ownership/documentation, broken core workspace invariants, unjustified newly added workspace members and violations of explicit calibrated workspace dependency policies are blocking.

## Accepted Repository Step 18 lifecycle complexity non-effect

PR #281 / accepted source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7 applicable permanent workflows added deterministic repository-pinned `doctor` and locked isolated `bootstrap`. PR #283 / accepted source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7 applicable permanent workflows added checkout-owned PostgreSQL `dev-up` / `dev-reset`, immutable image and schema-digest checks, fail-closed ownership/reset semantics and permanent real-Docker create/reuse/reset acceptance. PR #285 / accepted source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows added governed versioned demo seeding, real-process permission/authentication/tenant smoke acceptance and permanent clean reset/seed/replay/smoke Governance coverage. The accepted Step 18 slices changed no workspace package, dependency declaration, feature set, internal edge, public Rust surface, Cargo manifest or `Cargo.lock`; this file remains a historical Stage B measurement baseline rather than a replacement for current generated complexity artifacts.

The snapshot is reproducible through:

```bash
python scripts/analyze_workspace.py \
  --json-output artifacts/workspace-complexity-baseline.json \
  --markdown-output artifacts/workspace-complexity-baseline.md

python scripts/analyze_workspace_governance.py \
  --base-ref origin/main \
  --check \
  --json-output artifacts/workspace-governance-baseline.json \
  --markdown-output artifacts/workspace-governance-baseline.md

python scripts/check_workspace_dependency_policy.py \
  --check \
  --json-output artifacts/workspace-dependency-policy.json \
  --markdown-output artifacts/workspace-dependency-policy.md
```

`Complexity Baseline CI` runs the same analyzers and calibrated policy checker on the exact pull-request head and publishes all machine-readable reports as workflow artifacts.

## 1. Workspace configuration

| Property | Current fact |
|---|---|
| Cargo resolver | `2` |
| Effective workspace packages | 110 |
| Root-declared member paths | 110 |
| Technical crates | 96 |
| Business modules | 13 |
| Deployable services | 1 |
| Shared lockfile | root `Cargo.lock`, format version 4 |
| Nested lockfiles | 0 |
| `[workspace.package].edition` | `2024` |
| `[workspace.package].license` | `Proprietary` |
| `[workspace.package].rust-version` | not defined; warning |
| `[workspace.lints]` | not defined; warning |
| Package metadata inheritance | no package currently inherits edition, license, rust-version or lints |

The explicit member-list correction accepted in the first Stage B packet was behavior-neutral. Cargo metadata already included `crates/crm-identity-resolution-merge-query-adapter` through an internal path dependency; its existing transitional boundary and removal condition remain recorded in `architecture-governance.json`.

## 2. Dependency and packaging measurements

| Metric | Value |
|---|---:|
| External direct dependency declarations | 268 |
| Internal workspace dependency edges | 773 |
| Maximum dependency depth | 15 |
| Maximum direct internal dependents | 102 |
| Maximum transitive reverse impact | 103 |
| One-consumer packages | 26 |
| Conservative public Rust item count | 4,283 |
| Root workspace dependency families | 4 |
| Direct dependency requirement divergences | 1 |
| Direct feature divergences | 4 |
| Workspace dependency families with non-inheriting consumers | 4 |
| Heavy-feature declarations reported | 64 |
| Resolved duplicate external dependency families | 11 |

The public-surface value is a deterministic source-text count of public Rust items. It is useful for before/after structural comparisons but is not a semantic rustdoc compatibility model.

## 3. Accepted `[workspace.dependencies]` waves

The accepted root entries preserve the requirements that already existed before centralization:

| Dependency | Normative existing requirement | Inherited consumers after wave 3 | Remaining direct consumers not yet inheriting |
|---|---|---|---:|
| `serde` | `1`, features `derive` | foundation wave, all 13 business modules, Customer Enrichment privacy-scope adapter | 15 |
| `serde_json` | `1` | foundation wave, all 13 business modules, Customer Enrichment privacy-scope adapter | 23 |
| `sha2` | `0.10` | foundation wave, all 13 business modules, all nine privacy-scope adapters | 16 |
| `prost` | `0.14` | `crm-core-data` dev-dependencies and all nine privacy-scope adapters | 53 |

Wave 1 established the root entries and migrated `crm-module-manifest`, `crm-module-sdk` and `crm-core-data` where applicable.

Wave 2 covered the complete pure owner/link module cohort. Exact evidence: 13 matched manifests, 39 governed declarations and zero policy violations.

Wave 3 covers the nine already accepted owner privacy-scope adapter implementations. Exact evidence: nine matched manifests, 20 governed declarations and zero policy violations. All nine inherit `prost` and `sha2`; the Customer Enrichment adapter also inherits its existing `serde` and `serde_json` requirements.

The waves intentionally exclude `sqlx`, `tokio`, `tonic`, HTTP dependencies and complex test libraries. Their feature sets are not uniform and require separate consumer analysis. The wave 3 source diff does not change `Cargo.lock`; exact Rust CI regenerates and compares the committed lockfile before acceptance.

## 4. Most repeated direct dependencies

| Dependency | Consuming packages |
|---|---:|
| `prost` | 63 |
| `sha2` | 40 |
| `serde_json` | 39 |
| `tokio` | 35 |
| `serde` | 32 |
| `sqlx` | 27 |
| `axum` | 3 |
| `http` | 3 |
| `semver` | 3 |
| `tonic` | 3 |
| `prost-build` | 2 |
| `prost-types` | 2 |
| `protoc-bin-vendored` | 2 |
| `reqwest` | 2 |

Repetition is a prioritization signal, not proof that a dependency should immediately move to the root. Version, default-feature and consumer-specific feature requirements must be inspected first.

## 5. Direct requirement and feature divergence

### Direct requirement divergence

| Dependency | Requirements |
|---|---|
| `sqlx` | `0.9`, `0.9.0` |

These requirements currently resolve compatibly, but the textual divergence remains visible. SQLx is not changed by the accepted inheritance waves because its feature and PostgreSQL acceptance boundaries require a dedicated packet.

### Feature divergence

| Dependency | Current variants |
|---|---:|
| `reqwest` | 2 |
| `sqlx` | 2 |
| `tokio` | 8 |
| `tonic` | 2 |

Notable facts:

- `sqlx` consumers differ on the `json` feature;
- `tokio` consumers use eight combinations across runtime, macros, net, signal, sync, time and process needs;
- `tonic` differs in default-feature policy;
- `reqwest` differs between JSON-only and Rustls/stream consumers.

These are warning-only until each consumer class and feature-unification effect is measured. No automatic root centralization is authorized from this table alone.

## 6. Resolved duplicate dependency families

`Cargo.lock` contains the following external families at more than one resolved version:

| Dependency | Versions |
|---|---|
| `block-buffer` | 0.10.4, 0.12.1 |
| `cpufeatures` | 0.2.17, 0.3.0 |
| `crypto-common` | 0.1.7, 0.2.2 |
| `digest` | 0.10.7, 0.11.3 |
| `foldhash` | 0.1.5, 0.2.0 |
| `getrandom` | 0.2.17, 0.4.3 |
| `hashbrown` | 0.15.5, 0.16.1, 0.17.1 |
| `hmac` | 0.12.1, 0.13.0 |
| `sha2` | 0.10.9, 0.11.0 |
| `syn` | 2.0.119, 3.0.3 |
| `windows-sys` | 0.52.0, 0.61.2 |

These are resolved transitive families, not automatically actionable direct-version debt. A later alignment packet must trace owners, upstream constraints, security implications and lockfile effects before changing any family.

## 7. Reverse fan-out and dependency depth

Highest reverse-impact packages:

| Package | Direct dependents | Transitive reverse impact |
|---|---:|---:|
| `crm-module-sdk` | 102 | 103 |
| `crm-core-contracts` | 15 | 89 |
| `crm-capability-runtime` | 71 | 80 |
| `crm-query-runtime` | 40 | 78 |
| `crm-metadata-runtime` | 4 | 77 |
| `crm-proto-contracts` | 68 | 77 |
| `crm-core-events` | 11 | 76 |
| `crm-metadata-schema` | 2 | 76 |
| `crm-metadata-api-adapter` | 3 | 75 |
| `crm-projection-runtime` | 5 | 75 |
| `crm-core-data` | 67 | 73 |

Highest measured dependency depth begins with `crm-api` at 15, `crm-application-runtime` at 14 and several owner privacy-scope/application composition packages at 13. These values identify critical review surfaces; they do not authorize merging core packages or weakening their boundaries.

## 8. Representative affected closures

| Representative package | Package count including the changed package |
|---|---:|
| `crm-customer-enrichment-privacy-scope-adapter` | 1 |
| `crm-data-quality-privacy-scope-adapter` | 1 |
| `crm-customer-privacy-cancel-capability-adapter` | 4 |
| `crm-module-manifest` | 2 |
| `crm-api` | 1 |

The report is based on declared reverse workspace dependencies. Contract, migration, process, frontend and operational coupling remains selected by affected-scope and specialized workflows.

## 9. Public Rust surface

The baseline reports 4,283 conservative public Rust items. The largest measured surfaces include:

| Package | Public items |
|---|---:|
| `crm-customer-data-operations` | 393 |
| `crm-customer-enrichment` | 304 |
| `crm-data-quality` | 277 |
| `crm-customer-privacy` | 193 |
| `crm-identity-resolution` | 150 |
| `crm-customer-data-operations-capability-adapter` | 140 |
| `crm-core-data` | 126 |
| `crm-application-runtime` | 96 |
| `crm-module-sdk` | 91 |

Stage B uses this only as a structural before/after signal. New public-symbol compatibility policy remains a later calibrated gate.

## 10. Crate, exception and dependency inheritance governance

`architecture-governance.json` remains the single machine-readable exception and new-crate registry. Temporary exceptions require owner, bypassed rule, reason/risk, exact scope, created/expiry dates, removal condition, compensating checks and tracking issue. Newly added effective workspace packages require a complete review-visible boundary and lifecycle justification. Expired, ownerless, malformed or undocumented records fail governance. Current registry exceptions: 0.

`workspace-dependency-policy.json` contains two calibrated blocking inheritance rules:

| Policy | Scope | Governed dependencies when present | Exact evidence |
|---|---|---|---|
| `owner-module-common-serialization-and-hashing` | `modules/*/Cargo.toml` | `serde`, `serde_json`, `sha2` | 13 manifests, 39 declarations, 0 violations |
| `privacy-scope-adapter-contract-and-hashing` | `crates/*-privacy-scope-adapter/Cargo.toml` | `prost`, `serde`, `serde_json`, `sha2` | 9 manifests, 20 declarations, 0 violations |

For a governed dependency the required form is `workspace = true`. Local version, default-feature, feature, path, git, package or registry overrides are rejected. A scoped package is not forced to declare an unused dependency.

The checker is part of `scripts/check_architecture.py`, Governance CI and Complexity Baseline CI. Broader dependency observations remain warnings until separately calibrated.

## 11. CI scale baseline

| Metric | Value |
|---|---:|
| Permanent workflows | 32 |
| Workflow jobs | 33 |
| Workflow path-filter entries | 1,367 |
| PostgreSQL-service workflows | 23 |
| Pull-request workflows | 31 |
| Workflows with concurrency control | 31 |
| Main-only push workflows | 29 |
| Application composition non-comment LOC | 1,239 |
| Application runtime non-comment LOC | 8,326 |

The dependency waves do not change CI selection semantics. Broad exact-head proof remains mandatory while later Stage E work improves proportionality.

## 12. Customer Privacy boundary

Stage B does not block or implement Customer Privacy discovery/snapshot behavior. Issue #126 remains a separate product lane.

Customer Privacy discovery and immutable snapshot must target:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

The privacy-scope adapter inheritance wave changes only dependency declaration ownership for already accepted contract-only owner contributions. It adds no discovery, planner, owner action, route, worker, migration or capability-specific crate. Any necessary package consolidation remains a separate behavior-neutral PR with before/after package, fan-out, build/test and files-per-capability evidence.

## 13. Next Stage B calibration

The next dependency-governance packet should:

1. migrate another identical technical-crate dependency-role cohort only when its complete feature semantics are verified;
2. classify `sqlx`, `tokio`, `tonic` and HTTP feature variants by real consumer role before centralization;
3. decide and pin a repository `rust-version` based on supported toolchain policy;
4. introduce a workspace lint policy only after current warning debt is measured;
5. collect repeated public-surface and fan-out reports across representative leaf, owner, shared-core and workflow-only PRs;
6. promote only demonstrated low-noise rules from warning to blocking.

## Repository Step 19 accepted closure

Repository Step 19 is complete only through the combined accepted evidence below, each on one unchanged exact source head with no unresolved comments, reviews or review threads:

- PR #287 / source `23b2f4ea660bcd46884fe054cd0c37e89b1495c4` / squash merge `c0fec3ae08c836ab483737442ed4377c99c85e9a` / **11 of 11** applicable permanent workflows — added the bounded Customer Privacy owner-worker boundary without public ingress or new schema/dependency surface;
- PR #288 / source `b99a4fc4ff7ef5cfd47813b487900b4c2a9f3b77` / squash merge `bc653de5f1a853791d3ab4a03f59f3daad54bf54` / **24 of 24** — added PostgreSQL ready-work discovery for planned Customer Privacy owner actions;
- PR #289 / source `3e21e79e1600727ebcda222af389d568d857cff8` / squash merge `d1c4dd278853a1e6a426fab284c70b3529d42833` / **24 of 24** — registered `crm.customer-privacy` / `owner-execution` at phase `260` in the production `ApplicationRuntime`, with activation gating and replay-safe canonical execution;
- PR #290 / source `9bbb339f39133955a7f42ea67f3334e597066e2e` / squash merge `49c5e35814adceb2be9d4cc2302bf10032b807a0` / **19 of 19** — proved the assembled real `crm-api` lifecycle on clean and rollback/reapplied PostgreSQL schemas: ready-work discovery, a real Parties privacy action, one durable attempt, successful outcome, completed checkpoint, audit evidence, owner event/outbox and final case transition, plus restart no-duplicate proof and uninstall no-discovery/no-effect proof.

The accepted first-party background-worker inventory is now **one Customer Privacy owner worker**: module `crm.customer-privacy`, worker `owner-execution`, phase `260`. This is a production background worker, not a new public capability route; the latest public Customer Privacy inventory remains **seven mutations and four permission-aware public queries**.

Repository Steps 1–21 are complete. The bounded Repository Step 20A product-plane slice is accepted. Repository Step 21 is complete through PR #296 / accepted source `fd84cd25dfa25a75eac0fdc4a719cc76c84cfc95` / squash merge `c21894f47f24e81da1cc150f9ea457fcfdc2bd63` / 35 of 35 applicable permanent workflows on one unchanged exact head. Phase 8A.11 and Phase 8A are complete. Repository Step 22 Phase 8A architecture remeasurement, `crm-application-runtime` runtime-fan-in decision and permanent-gate value/cost review is the sole next permitted implementation packet. Phase 8A.11 / issue #126 is complete; Customer Privacy is not product-complete; current product-complete expert modules remain zero; architecture 10/10 and the Universal CRM product are not declared complete.

The Step 19 packets add no crate, dependency, route, public API, module manifest, migration or schema. The conservative public Rust surface remains **5,377**, suppression occurrences remain **91**, and `crm-application-runtime` non-comment/source LOC remains within the frozen **7,269** ceiling.

## Accepted Repository Step 20A evidence

PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows on one unchanged exact head accepts the bounded Customer Privacy product-plane slice.

The accepted evidence proves:

- exact typed `customer_privacy.case.list@1.0.0` and `customer_privacy.case.get@1.0.0` governed clients with envelope, contract, descriptor-hash, data-class, payload-size and retention checks before rendering;
- an authenticated capability-gated `/customer/privacy` route while backend authentication, tenant isolation, authorization and visibility remain authoritative;
- a bounded accessible case list/detail experience with explicit loading, empty, error and retry states, live announcements, deterministic focus behavior and permission/not-found concealment;
- a governed Party and verified PrivacyCase fixture created through assembled production composition and mutations, with no direct Customer Privacy record writes and no mock backend;
- real PostgreSQL, assembled `crm-api`, Vite and Chromium acceptance for keyboard-only list/detail review, session expiry and cross-tenant concealment;
- no backend route, capability, contract, manifest, schema, migration, dependency, lockfile or Rust production-source change.

Step 20A is accepted. Repository Step 20 is complete; Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is accepted through PR #294; Repository Step 21 Phase 8A closure is the only next permitted packet. Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete. The accepted one-worker Customer Privacy inventory, seven public mutations, four permission-aware public queries, 5,377 public Rust items, 91 suppressions and the `crm-application-runtime` 7,269 LOC ceiling remain unchanged.

## Repository Step 20 accepted closure

Repository Step 20 is complete through PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows and PR #294 / source `f9c5faa667f4d5483335ec2cb5bac31596d818c8` / squash merge `ef3457c11646b1069e5e65683d3618b3d470136e` / 8 of 8 applicable permanent workflows, each accepted on one unchanged exact head with zero unresolved comments, reviews or review threads.

Step 20A proves the typed governed Customer Privacy browser product plane against real PostgreSQL, assembled `crm-api`, Vite and Chromium. Step 20B proves independent PostgreSQL logical backup and restore, restored-process startup and readiness, active `customer_privacy.case.list` and `customer_privacy.case.get` metrics, cross-tenant and expired-session concealment, startup `0.101` seconds, nearest-rank readiness p95 `2.977` milliseconds, backup SHA-256 `700b8ae13a71af30010b11877f70b6a4b3efe1b0ec3beddaf0f3e3bc19533d3c`, backup size `1,118,941` bytes and Chromium 3 of 3.

Repository Steps 1–21 are complete. Repository Step 22 Phase 8A architecture remeasurement, `crm-application-runtime` runtime-fan-in decision and permanent-gate value/cost review is the sole next permitted implementation packet. Phase 8A.11, Phase 8A, Customer Privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete.

## Accepted Repository Step 21 and Phase 8A closure

PR #296 / accepted source `fd84cd25dfa25a75eac0fdc4a719cc76c84cfc95` / squash merge `c21894f47f24e81da1cc150f9ea457fcfdc2bd63` / 35 of 35 applicable permanent workflows on one unchanged exact head completes Repository Step 21, Phase 8A.11 / issue #126 and Phase 8A.

The accepted final Customer Privacy production inventory is exactly **nine public mutations**, **seven permission-aware public queries** and **one first-party owner worker** (`crm.customer-privacy` / `owner-execution`, phase `260`). The accepted lifecycle includes processing-restriction and legal-hold release/read coordinates, optimistic versioning, exact idempotent replay, immutable event/audit/outbox/business-transaction evidence, FORCE-RLS visibility and uniform concealment, clean PostgreSQL rollback/reapply, real `crm-api` process proof and bounded operations search-projection convergence before backup.

Customer Privacy is the first **Product complete** expert module. Current product-complete expert modules: **1**. The broader Universal CRM product remains incomplete, issue #194 remains open and architecture 10/10 is **not declared**. Repository Step 22 Phase 8A architecture remeasurement, `crm-application-runtime` runtime-fan-in decision and permanent-gate value/cost review is the sole next permitted implementation packet.


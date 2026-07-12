# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**  
Parent epic: [#2](https://github.com/iamaman11/crm/issues/2)  
Governing rules: [`SYSTEM_INVARIANTS.md`](SYSTEM_INVARIANTS.md) and accepted ADRs.  
Structural guide: [`APPLICATION_ARCHITECTURE.md`](APPLICATION_ARCHITECTURE.md).  
Current concise state: [`PROJECT_STATUS.md`](PROJECT_STATUS.md).  
Business-module accounting: [`MODULE_CATALOG.md`](MODULE_CATALOG.md).

## 1. Purpose

This roadmap turns the architecture specification into a controlled delivery sequence. It is not a feature wishlist. Every phase establishes guarantees required by later phases, has explicit acceptance gates, and must preserve all system invariants.

The target is a universal modular expert CRM platform where first-party and marketplace modules can be developed, tested, released, installed, activated, upgraded, suspended and removed independently without direct infrastructure access or cross-module state mutation.

Universal means that Sales is not allowed to become the owner of customer identity, catalog, pricing, order, contract, subscription, service, communication or billing state. Those domains require explicit owner modules and versioned integration boundaries.

## 2. Delivery rules

1. Work is delivered through coherent reviewable delivery packets linked to roadmap issues. A packet may use one long-lived implementation branch with incremental commits, but must end at a natural architecture boundary with green acceptance gates.
2. Contract CI, Governance CI and Rust CI must remain green before merge; Database CI is mandatory whenever runtime, SQL, migrations or PostgreSQL behavior changes. Specialized runtime/process gates are mandatory where defined by the phase.
3. Published contracts, policies, metadata and module versions are immutable.
4. A phase is complete only when its acceptance gates are automated or supported by a documented operational drill.
5. New state-changing behavior must enter through a versioned capability and produce typed audit evidence.
6. Business modules may depend only on stable platform contracts and governed SDK ports.
7. Search, analytics, caches and projections remain rebuildable and non-authoritative.
8. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements, not later enhancements.
9. Technical debt discovered by a gate is fixed or explicitly recorded before dependent work begins.
10. The roadmap and GitHub issues are updated in the same delivery packet when scope or sequencing changes.
11. Exact money, time, identity, lifecycle and authorization semantics must be represented by typed contracts rather than convention or free-form strings.
12. No milestone may claim the complete CRM product is finished while required owner domains or production gates remain open.
13. `README.md` is stable orientation only. Current phase state is synchronized through this roadmap, `PROJECT_STATUS.md`, `MODULE_CATALOG.md` and the active GitHub phase issue.
14. A backend phase is not application-complete while its production components exist only as libraries or tests and the deployable composition root remains a skeleton.
15. Frontend is a separate product-plane workstream after the first backend vertical proof, then evolves in parallel with later expert modules rather than waiting for all backend domains to finish.

## 3. Work states

- **Planned** — scoped but prerequisites are incomplete.
- **Ready** — prerequisites and contracts are stable enough to start.
- **In progress** — an implementation branch or PR exists.
- **Gate review** — implementation is complete and acceptance evidence is green or under final merge verification.
- **Complete** — merged and all required gates have passed.
- **Blocked** — a named dependency, decision or defect prevents progress.

## 4. Phase map

| Phase | Issue | Primary result | State | Depends on |
|---|---:|---|---|---|
| 0.1 | [#3](https://github.com/iamaman11/crm/issues/3) | Repository hardening and executable roadmap | **Complete** | Governance v1 |
| 1 | [#4](https://github.com/iamaman11/crm/issues/4) | Typed Module Manifest IR and deterministic identity | **Complete** | #3 |
| 2 | [#5](https://github.com/iamaman11/crm/issues/5) | Governed Module SDK and test harness | **Complete** | #4 |
| 3 | [#6](https://github.com/iamaman11/crm/issues/6) | Module lifecycle and registry runtime | **Complete** | #4, #5 |
| 4 | [#7](https://github.com/iamaman11/crm/issues/7) | PostgreSQL tenant, record, outbox and audit foundation | **Complete** | #6 |
| 5 | [#8](https://github.com/iamaman11/crm/issues/8) | Capability execution gateway | **Complete** | #5, #7 |
| 6 | [#9](https://github.com/iamaman11/crm/issues/9) | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, generalized projections, Admin Studio and product shell | **In progress** | #9 |
| 8 | [#11](https://github.com/iamaman11/crm/issues/11) | Expert modules and product-quality UX | **Planned** | #5, #9, #10 |
| 8A | [#28](https://github.com/iamaman11/crm/issues/28) | Canonical customer master, identity resolution and consent | **Planned** | #9, #10 |
| 8B | [#29](https://github.com/iamaman11/crm/issues/29) | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | #9, #10, #28 |
| 9 | [#12](https://github.com/iamaman11/crm/issues/12) | AI-native governed actor/tool layer | **Planned** | #8, #10 |
| 10 | [#13](https://github.com/iamaman11/crm/issues/13) | Signed marketplace and WASM sandbox | **Planned** | #6, #8, #10 |
| 11 | [#14](https://github.com/iamaman11/crm/issues/14) | Enterprise security and production proof | **Planned / continuous hardening** | all critical runtime phases |

## 5. Phase 0.1 — Repository hardening — Complete

Delivered version-controlled roadmap and issue hierarchy, stable CODEOWNERS, executable architecture-policy boundaries, generated-artifact/validation documentation and required Contract/Governance/Rust/Database checks.

Gate: the repository must not claim stale checksums, warnings or committed artifacts that do not exist. Every implementation phase has an issue, prerequisites and measurable acceptance criteria.

## 6. Phase 1 — Typed Module Manifest IR — Complete

Runtime boundary:

```text
module.yaml
→ strict YAML 1.2 JSON-compatible subset
→ JSON Schema validation
→ semantic validation
→ normalized JSON IR
→ typed Rust ModuleManifest
→ crm.cjson/v1 canonical bytes
→ SHA-256 identity
→ immutable publication
```

Delivered strict typed structures, unknown-field rejection, SemVer/dependency validation, ownership indexes, dependency-cycle detection and deterministic Python-to-Rust canonical identity parity.

## 7. Phase 2 — Governed Module SDK — Complete

Delivered governed `CapabilityClient`, `RecordClient`, `RelationshipClient`, `EventPublisher`, `ModuleStateStore`, `WorkflowClient`, `FileClient`, `Clock`, `RandomSource` and observability ports with tenant/actor/execution-context binding and deterministic test doubles.

Business modules are compile-time excluded from raw database, broker, object-storage, arbitrary HTTP, secret-store and LLM-provider clients.

## 8. Phase 3 — Module lifecycle and registry — Complete

Delivered validate, publish, install, activate, suspend, upgrade, rollback and uninstall transitions; deterministic dependency resolution; immutable module versions; tenant-scoped installations; link-module support; uninstall impact/blockers; and audited idempotent lifecycle state transitions.

## 9. Phase 4 — PostgreSQL foundation — Complete

Authoritative guarantees include tenant-scoped records and relationships, FORCE-RLS boundaries where required, optimistic versions, atomic business state/idempotency/outbox/audit evidence, append-only tenant audit ledger, controlled typed payload metadata and clean-install/legacy-upgrade/rollback/reapply tests.

Cross-tenant negative tests and transaction fault injection remain required regression gates after every later runtime or migration change.

## 10. Phase 5 — Capability execution gateway — Complete

Execution chain:

```text
request
→ authentication
→ tenant and actor resolution
→ ExecutionContext
→ exact capability resolution
→ typed and semantic validation
→ rate and approval policy
→ live authorization
→ synchronous deterministic planner
→ one atomic PostgreSQL execution
→ outbox and audit
→ typed safe response
```

Delivered guarantees:

- public HTTP/gRPC mutations cannot bypass the gateway;
- live authorization is the last awaited policy operation before transactional execution;
- replay does not duplicate side effects;
- idempotency-key reuse with different semantic input is a typed conflict;
- missing required evidence rolls back state and preserves audit integrity;
- external behavior does not depend on parsing error text.

## 11. Phase 6 — First modular production proof — Complete

Issue: [#9](https://github.com/iamaman11/crm/issues/9)  
Delivery packet: merged PR #63 / issue #55.  
Merge commit: `82910fa17f21074b1e091615a4251092cfa8ab2f`.

### 6A–6H — owner modules, contracts, mutations and queries

Established:

- typed independent Sales `Deal` and Activities `Task` owner-domain aggregates;
- publication-compatible versioned Protobuf contracts;
- transactional audit materialization and transaction-aware aggregate execution;
- validated persisted codecs and generated contract runtime;
- production Sales/Activities mutation planners and PostgreSQL execution;
- authenticated mutation acceptance with replay/conflict/revocation/rollback evidence;
- permission-bound Deal/Task get/list queries;
- HMAC-bound opaque keyset cursors;
- resource/field visibility, non-disclosing denial and authenticated HTTP/gRPC query ingress.

### 6I — optional Sales–Activities link and governed event delivery

Delivered:

- restart-safe event lineage and immutable outbox-to-`EventDelivery` reconstruction;
- durable consumer delivery ledger with lease, retry, recovery and dead-letter behavior;
- lifecycle-aware link processing that checks tenant installation state before target invocation;
- pure link-module core free of PostgreSQL, generated Protobuf and target owner internals;
- target execution through `GatewayCapabilityClient` and the production `CapabilityGateway`;
- duplicate delivery, retry recovery, delivery-ledger rebuild, suspension/removal and cross-tenant acceptance.

### 6J — rebuildable Deal timeline and Task status projections

Delivered:

- FORCE-RLS projection checkpoints, applied-event deduplication and rebuildable JSON documents;
- keyset replay over immutable event history with tenant-scoped checkpoints;
- Deal timeline and Task status projection handlers based on published event contracts;
- atomic `applied event + projection documents + checkpoint` application;
- resume, duplicate-idle, cross-tenant non-disclosure and reset/rebuild equivalence;
- no authoritative owner-state writes or mutation evidence from projection processing.

### 6K — production application composition root

Delivered:

- `crm-application-runtime` as the single production composition boundary;
- thin `services/crm-api` process host depending only on the application runtime;
- validated environment configuration and PostgreSQL/gateway/auth/visibility/worker composition;
- governed HTTP mutation/query endpoints;
- versioned `crm.gateway.v1.ApplicationGatewayService` gRPC mutation/query transport;
- health, readiness, background link/projection workers and graceful shutdown;
- architecture enforcement preventing direct process-host imports of PostgreSQL adapters or owner-module internals.

### 6L — complete process-level production acceptance

Application Runtime CI proves:

1. PostgreSQL startup and migrations `0001`–`0008`;
2. production fixture seeding;
3. real `crm-api` process startup and readiness;
4. unauthenticated HTTP denial;
5. authenticated Sales mutations;
6. background Sales-to-Activities link delivery;
7. Deal timeline and Task status projection materialization;
8. governed gRPC query execution;
9. clean SIGINT graceful shutdown.

Migration `0008` is also part of canonical clean-install, rollback, reapply and legacy-upgrade Database CI paths.

Final review head `25793548e46bdbd57312a513b4e9ffbceb33a2c1` passed Contract CI, Governance CI, Rust CI, Database CI, Event Runtime CI, Application Runtime CI and generic Rust Generated Sync simultaneously before merge.

## 12. Phase 7 — Search, generalized projections, Admin Studio and product-shell foundation — In progress

Phase 7 is the active roadmap phase.

### Golden module tooling — Complete

Issue #56 / merged PR #64 established repository-supported module creation and validation so later platform/domain work inherits the proven architecture by construction:

- separate owner-module and optional link-module scaffolding patterns;
- explicit owner object declarations before generation;
- explicit source/target dependencies and no authoritative record ownership for link modules;
- overwrite-safe generation, dependency-range validation and dry-run preview;
- architecture-safe Rust crate/manifests plus explicit contract, adapter and acceptance-test TODO boundaries;
- compiling generated acceptance-test placeholder that remains ignored until replaced with real production evidence;
- permanent cross-platform repository commands for architecture, manifest validation, formatting, lockfile synchronization, focused tests, full tests and the common Rust quality gate;
- Governance CI that validates generated manifests, compiles a fresh generated module with `cargo check --all-targets` and verifies generated dependencies against `architecture-policy.json`.

Generated scaffolds are **Foundation only** and do not count as production vertical slices. PR #64 was merged as `15bf3ddeac0375325a3c59518e3ac55a3903c20d` and #56 is complete.

### Generalized projection runtime — #65 / merged PR #67 — Complete

The second Phase 7 packet generalizes the Phase 6 projection proof without moving owner-domain decoding or business semantics into infrastructure:

- `crm-projection-runtime` owns typed projection identity, registration, checkpoint-based history paging, deterministic handler execution, poison/failure handling and rebuild orchestration;
- `crm-core-events` exposes the platform `ProjectionStore` port and typed `ProjectionFailure` contract;
- `crm-core-data` adapts the existing PostgreSQL projection tables/runtime to that port without introducing a new migration;
- deterministic handler failure marks the projection checkpoint failed without advancing the last successful cursor or applied-event count and blocks further replay until reset or repair;
- existing Deal timeline and Task status handlers remain concrete composition-layer handlers but execute through the generic registry/runner;
- `Phase6ProjectionWorker` remains only as a compatibility facade so application/process composition does not regress while orchestration moves to the platform runtime;
- dedicated `Projection Runtime CI` proves failed-checkpoint persistence/reset and the existing Deal/Task rebuild behavior against real PostgreSQL.

The generic runtime has no Sales, Activities or PostgreSQL implementation dependency. PR #67 was merged as `195448ab3cd70fe051967faf4f8ed87372fb3551`; #65 is complete.

### Permission-aware search and deterministic reindexing — Complete

[#66](https://github.com/iamaman11/crm/issues/66) / merged PR #68 is **Complete**: tenant- and permission-aware search with deterministic reindexing on the shared projection runtime.

The packet delivers candidate-only search, live resource/field visibility re-checks, backend-consistent field-local match evidence, immutable logical index generations, PostgreSQL FTS as a replaceable adapter, governed `search.global.query` ingress, application-runtime composition, immediate permission-revocation acceptance and canonical migration/runtime gates.

Final review head `90d8ad4afc15ba31bc27297e4a9c7081e64ac4e7` passed Contract, Governance, Rust, Database, Projection Runtime, Event Runtime, Search Runtime, Application Runtime and Rust Generated Sync simultaneously. PR #68 was squash-merged into `main` as `49272918cb4b767eedc2ca34574abba40718eae1`; #66 is complete.

### Typed web product shell and governed client boundary — Complete

[#71](https://github.com/iamaman11/crm/issues/71) / merged PR #73 is **Complete**.

The packet established:

- a reproducible Node 24 / pnpm 11 / strict TypeScript workspace;
- `apps/web`, `packages/client` and `packages/ui` product-plane boundaries;
- mechanically generated Protobuf-ES browser contracts;
- browser access through the existing governed `ApplicationGatewayService` over gRPC-Web;
- typed `GovernedClient.searchGlobal` with exact request/output contract identity validation;
- centralized typed session state and stable product-owned error mapping;
- permission-aware routing as UX only, with backend authorization remaining authoritative;
- no public arbitrary raw gateway/query escape hatch;
- design-system/application-shell, accessibility, responsive, loading/error and localization/time-zone foundations;
- hermetic Playwright E2E against ephemeral PostgreSQL for the real governed search path and negative authentication/authorization behavior.

Final review head `b62dd50225fde6e58aac9a6b4cec307bd2245616` passed all applicable checks before PR #73 was merged and #71 closed.

### Immutable metadata publication lifecycle — Complete

[#77](https://github.com/iamaman11/crm/issues/77) / merged PR #78 is **Complete**.

The packet established:

- `crm-metadata-runtime` as a pure platform-domain crate with no PostgreSQL, transport, browser or business owner-module dependency;
- typed metadata kinds for object, field, relationship, layout, view, pipeline, permission and workflow definitions;
- validated namespaced metadata identifiers;
- complete metadata-bundle snapshots with explicit intra-bundle dependencies;
- deterministic SHA-256 revision identity under `crm.metadata.bundle.sha256/v1`;
- immutable/idempotent content-addressed publication;
- deterministic structural impact analysis for additions, modifications and removals;
- explicit breaking-change confirmation before activation;
- optimistic activation generations and rollback across immutable revisions.

Final review head `9595ce934f0ceaf23025676474f340e62bdd960d` passed Governance, Rust, Rust Generated Sync, Database, Event, Projection, Search and Application Runtime CI before PR #78 was squash-merged as `de1ea407790d8c6c74f363b21622d332df85f727`.

### Tenant-scoped metadata publication authority — Complete

[#79](https://github.com/iamaman11/crm/issues/79) / merged PR #80 is **Complete**.

The hardening packet makes tenant publication authority explicit at the public composition boundary:

- the deterministic single-scope catalog engine remains private;
- application-facing callers use `TenantMetadataCatalog`;
- publication, revision lookup, impact analysis, activation and rollback require an explicit tenant identity;
- revision hashes are identities, not authorization secrets;
- Tenant B cannot read, impact-analyze or activate a revision published only by Tenant A;
- identical content may retain identical deterministic identity after independent publication into each tenant authority;
- activation generations and rollback histories remain tenant-isolated.

Final review head `675d389695e4881e62732bcec17b4eadcaf62917` passed architecture, lockfile, `rustfmt`, Clippy, full workspace tests and Rust Generated Sync before PR #80 was squash-merged as `fcf2d8d7ab0d1c94999b8a6feea7b3be9f97db7f`.

### Current executable Phase 7 packet — typed Admin Studio metadata schemas

[#81](https://github.com/iamaman11/crm/issues/81) / draft PR #82 is **In progress**.

The packet replaces opaque authoring payloads with strict typed v1 definitions before persistence, public APIs or UI composition:

- `crm-metadata-schema` as a pure typed schema/validation crate;
- object, field, relationship, layout, saved-view, pipeline, permission-template and workflow definitions;
- bounded text/decimal/enum and collection semantics;
- strict duplicate and intra-definition reference validation;
- deterministic dependency extraction into runtime `MetadataKey` references;
- deterministic canonical UTF-8 JSON under `crm.metadata.definition/v1`;
- set-like members canonicalized independently of insertion order while meaningful authoring order remains identity-significant;
- workflows restricted to exact SemVer governed capability references with no script, raw SQL or arbitrary HTTP execution primitive.

The packet must finish with exact-head evidence for all applicable gates and synchronized roadmap/status state before merge.

### Remaining Phase 7 platform deliverables

- durable tenant-scoped metadata publication/activation persistence with typed audit evidence;
- governed metadata publication/query contracts and application composition;
- first Admin Studio workflows through the product plane;
- typed UI-extension runtime with host-shell failure isolation;
- further golden-module/tooling evolution as new module classes prove additional stable patterns.

### Phase 7 gate

Deleting search or projections cannot destroy authoritative data. Permission changes cannot leak stale results. Published metadata is immutable, tenant-authorized and activated through a validated, impact-analyzed, audited and reversible pipeline. UI-extension failure cannot break the host shell or record page. Frontend code cannot bypass governed mutation/query paths.

Phase 7 remains open until the Admin Studio publication pipeline and UI-extension foundations satisfy these acceptance gates.

## 13. Phase 8 — Expert modules and product experience — Planned

After Phase 7, backend and frontend evolve as end-to-end vertical slices rather than as long disconnected projects.

Required owner-domain programs include:

- canonical customer master, identity resolution and consent — #28;
- Sales and Activities expert expansion;
- communications and omnichannel interaction history;
- support and service management;
- marketing segmentation, journeys and attribution;
- product catalog, pricing, CPQ, quote, order, contract and subscription lifecycle — #29;
- billing and governed ERP/payment/tax integrations;
- projects/cases/configurable work management;
- documents and e-signature;
- analytics, forecasting and performance management.

Product experience includes global search, command palette, keyboard navigation, fast tables, saved views, bulk actions, timelines, explainable permissions, transparent automation runs, onboarding, imports, responsive/mobile behavior, accessibility and localization.

Gate: each module owns typed domain invariants, contracts, manifest, CI target and release notes. Critical rules cannot be bypassed by arbitrary metadata, scripts or AI. Customer identity and commercial commitment ownership remain explicit and non-overlapping.

## 14. Phase 9 — AI-native layer — Planned

AI is an Actor, not an infrastructure shortcut.

Deliver model routing by tenant/data class/purpose/residency/cost, permission-scoped tools generated from the Capability Registry, permission-filtered retrieval, approval flows, reversible actions, budgets, prompt-injection/leakage/correctness evaluations and complete actor/tool/model/cost audit evidence.

Gate: AI has no alternate mutation path. Restricted data is default-deny for external providers. Every tool call repeats live authorization before side effects.

## 15. Phase 10 — Marketplace — Planned

Deliver signed packages, publisher identity, WASM sandbox, SBOM/provenance verification, vulnerability policy, capability/data/network/secret grants, quotas, kill switch and safe upgrade/rollback/uninstall.

Gate: untrusted or policy-violating modules cannot install. Marketplace code cannot access resources outside explicit host grants.

## 16. Phase 11 — Enterprise and production proof — Planned / continuous

Deliver SSO/OIDC/SAML, SCIM, tenant key hierarchy, field encryption, legal hold, WORM audit export, privacy deletion, crypto-shredding, backup/PITR, tenant restore, tenant mobility, data residency, SBOM/dependency/secret scans, penetration/load/chaos tests, SLOs and runbooks.

Gate: enterprise claims require automated and operational evidence, not configuration placeholders.

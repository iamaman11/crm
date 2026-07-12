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
16. Exact-SHA evidence is invalidated by every source-changing or documentation-changing commit until all applicable checks are rerun on the new head.

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

## 5. Phases 0.1–5 — platform control plane — Complete

### Phase 0.1 — repository hardening

Delivered version-controlled roadmap and issue hierarchy, stable CODEOWNERS, executable architecture-policy boundaries, generated-artifact/validation documentation and required Contract/Governance/Rust/Database checks.

### Phase 1 — typed Module Manifest IR

Delivered the strict pipeline:

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

Unknown fields, invalid SemVer/dependencies and dependency cycles are rejected; Python/Rust canonical identity parity is tested.

### Phase 2 — governed Module SDK

Delivered governed `CapabilityClient`, `RecordClient`, `RelationshipClient`, `EventPublisher`, `ModuleStateStore`, `WorkflowClient`, `FileClient`, `Clock`, `RandomSource` and observability ports with tenant/actor/execution-context binding and deterministic test doubles.

Business modules are compile-time excluded from raw database, broker, object-storage, arbitrary HTTP, secret-store and LLM-provider clients.

### Phase 3 — module lifecycle and registry

Delivered validate, publish, install, activate, suspend, upgrade, rollback and uninstall transitions; deterministic dependency resolution; immutable module versions; tenant-scoped installations; link-module support; uninstall impact/blockers; and audited idempotent lifecycle transitions.

### Phase 4 — PostgreSQL foundation

Delivered tenant-scoped records and relationships, FORCE-RLS boundaries, optimistic versions, atomic business state/idempotency/outbox/audit evidence, append-only tenant audit ledger, controlled typed payload metadata and migration clean-install/upgrade/rollback/reapply coverage.

Cross-tenant negative tests and transaction fault injection remain mandatory regression gates.

### Phase 5 — capability execution gateway

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

Public mutations cannot bypass the gateway. Live authorization is repeated immediately before transactional execution. Replay cannot duplicate side effects, semantic idempotency conflicts are typed, and missing evidence rolls back state.

## 6. Phase 6 — first modular production proof — Complete

Issue: [#9](https://github.com/iamaman11/crm/issues/9)  
Delivery packet: merged PR #63 / issue #55.  
Merge commit: `82910fa17f21074b1e091615a4251092cfa8ab2f`.

Phase 6 established:

- independent typed Sales `Deal` and Activities `Task` owner aggregates;
- publication-compatible versioned Protobuf contracts;
- production PostgreSQL mutations and permission-bound queries;
- authenticated HTTP/gRPC query and mutation ingress;
- durable event lineage, retries, recovery and dead-letter behavior;
- optional `crm.sales-activities-link` execution through the production `CapabilityGateway`;
- rebuildable Deal timeline and Task status projections;
- `crm-application-runtime` as the production composition boundary;
- deployable `services/crm-api` with readiness, workers and graceful shutdown;
- real process-level acceptance against PostgreSQL.

Application Runtime CI proves process startup, authenticated mutation, background link delivery, projection materialization, governed gRPC query execution and clean SIGINT shutdown.

Final review head `25793548e46bdbd57312a513b4e9ffbceb33a2c1` passed Contract, Governance, Rust, Database, Event Runtime, Application Runtime and Rust Generated Sync simultaneously before merge.

## 7. Phase 7 — search, generalized projections, Admin Studio and product shell — In progress

Phase 7 is the active roadmap phase.

### 7A — golden module tooling — Complete

[#56](https://github.com/iamaman11/crm/issues/56) / merged PR #64 established repository-supported owner/link module scaffolding, overwrite-safe generation, dependency validation, architecture-safe crate/manifests, acceptance placeholders and permanent cross-platform quality commands.

Generated scaffolds are **Foundation only** and do not count as production vertical slices.

### 7B — generalized projection runtime — Complete

[#65](https://github.com/iamaman11/crm/issues/65) / merged PR #67 introduced `crm-projection-runtime` for typed projection registration, checkpoint-based replay, deterministic handler execution, poison/failure state and rebuild orchestration while keeping owner-domain decoding outside infrastructure.

PostgreSQL remains an adapter. Failure does not advance the last successful cursor, and rebuildable projections never become authoritative state.

### 7C — permission-aware search — Complete

[#66](https://github.com/iamaman11/crm/issues/66) / merged PR #68 delivered candidate-only search, live resource/field visibility re-checks, deterministic logical index generations, PostgreSQL FTS as a replaceable adapter, governed `search.global.query` ingress and immediate permission-revocation acceptance.

Final review head `90d8ad4afc15ba31bc27297e4a9c7081e64ac4e7` passed all applicable Contract, Governance, Rust, Database, Projection, Event, Search, Application Runtime and Rust Generated Sync gates.

### 7D — typed web product shell — Complete

[#71](https://github.com/iamaman11/crm/issues/71) / merged PR #73 established:

- reproducible Node 24 / pnpm 11 / strict TypeScript workspace;
- `apps/web`, `packages/client` and `packages/ui` boundaries;
- mechanically generated Protobuf-ES browser contracts;
- browser access only through the governed `ApplicationGatewayService` over gRPC-Web;
- typed `GovernedClient.searchGlobal` with exact contract identity checks;
- centralized typed session state and safe product-owned errors;
- permission-aware routing as UX only, with backend authorization authoritative;
- responsive/accessibility/localization foundations;
- hermetic Playwright E2E against ephemeral PostgreSQL.

Final review head `b62dd50225fde6e58aac9a6b4cec307bd2245616` passed all applicable checks before merge.

### 7E-1 — immutable metadata publication lifecycle — Complete

[#77](https://github.com/iamaman11/crm/issues/77) / merged PR #78 established `crm-metadata-runtime`:

- validated metadata coordinates and complete bundle snapshots;
- deterministic SHA-256 identity under `crm.metadata.bundle.sha256/v1`;
- immutable/idempotent content-addressed publication;
- structural impact analysis;
- explicit breaking-change confirmation;
- optimistic activation generations;
- rollback across immutable revisions.

Final review head `9595ce934f0ceaf23025676474f340e62bdd960d` passed all applicable gates before PR #78 was squash-merged as `de1ea407790d8c6c74f363b21622d332df85f727`.

### 7E-2 — tenant-scoped metadata publication authority — Complete

[#79](https://github.com/iamaman11/crm/issues/79) / merged PR #80 made publication authority tenant-scoped by construction:

- the deterministic single-scope engine is private;
- application callers use `TenantMetadataCatalog`;
- publish/read/impact/activate/rollback require tenant identity;
- a revision hash is an identity, never an authorization secret;
- identical content may retain identical identity only after independent publication into each tenant authority;
- activation generations and rollback histories remain tenant-isolated.

Final review head `675d389695e4881e62732bcec17b4eadcaf62917` passed architecture, lockfile, `rustfmt`, Clippy, full workspace tests and Rust Generated Sync before merge.

### 7E-3 — typed Admin Studio metadata schemas — Complete

[#81](https://github.com/iamaman11/crm/issues/81) / merged PR #82 introduced `crm-metadata-schema`:

- strict typed object, field, relationship, layout, saved-view, pipeline, permission-template and workflow definitions;
- bounded text/decimal/enum and collection semantics;
- duplicate and intra-definition reference validation;
- deterministic dependency extraction into runtime `MetadataKey` references;
- deterministic canonical UTF-8 JSON under `crm.metadata.definition/v1`;
- set-like canonicalization independent of insertion order while preserving meaningful ordered identity;
- exact SemVer governed-capability workflow actions with no script, raw SQL or arbitrary HTTP primitive.

Final review head `889a5161233283a1b1460a221df2b406522b588b` passed Governance, Rust, Rust Generated Sync, Database, Event, Projection, Search and Application Runtime CI before PR #82 was squash-merged as `885f479bcfa85ccd52817900359ea397e7a20544`.

### Current executable packet — 7F durable metadata persistence

[#83](https://github.com/iamaman11/crm/issues/83) / draft PR #84 is **In progress**.

The packet persists the immutable metadata lifecycle without moving metadata semantics into SQL:

- migration `0010_metadata_publication_runtime`;
- immutable tenant-scoped revision headers, canonical documents and explicit dependency edges;
- deterministic PostgreSQL reconstruction with revision identity verification;
- tenant-scoped optimistic activation heads;
- per-tenant transaction advisory locking plus expected-generation conflicts for concurrent activation;
- a durable push/pop rollback stack that cannot toggle a rolled-back revision forward;
- structural impact and breaking-change analysis delegated back to `crm-metadata-runtime`;
- append-only publish/activate/rollback transition evidence bound to actor, request, capability and business-transaction context;
- FORCE RLS and transaction-local write-context enforcement on all metadata tables;
- immutable UPDATE/DELETE rejection for published revision state and transition evidence;
- real PostgreSQL acceptance for identity round-trip, idempotence, cross-tenant non-disclosure, concurrent activation, breaking confirmation, rollback, RLS and immutability;
- dedicated migration clean-install, rollback and reapply verification.

The persistence packet intentionally does not fabricate unrelated outbox/idempotency rows merely to enter the existing global business-transaction audit chain. The follow-on governed metadata capability/API packet must produce canonical `crm.audit_records` evidence through the normal public capability transaction contract.

### Remaining Phase 7 platform deliverables

1. Governed metadata publish/activate/rollback/query contracts and application composition.
2. Canonical global audit evidence for public metadata mutations through the existing capability transaction contract.
3. First Admin Studio workflows through the product plane.
4. Typed UI-extension runtime with host-shell and record-page failure isolation.
5. Further golden-module/tooling evolution only when new stable module classes are proven by real vertical slices.

### Phase 7 gate

Deleting search or projections cannot destroy authoritative data. Permission changes cannot leak stale results. Published metadata is immutable, tenant-authorized, durably reconstructable, strictly validated, impact-analyzed, audited and reversible. UI-extension failure cannot break the host shell or record page. Frontend code cannot bypass governed mutation/query paths.

Phase 7 remains open until the governed Admin Studio publication pipeline and UI-extension foundations satisfy these acceptance gates.

## 8. Phase 8 — expert modules and product experience — Planned

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

## 9. Phase 9 — AI-native layer — Planned

AI is an Actor, not an infrastructure shortcut.

Deliver model routing by tenant/data class/purpose/residency/cost, permission-scoped tools generated from the Capability Registry, permission-filtered retrieval, approval flows, reversible actions, budgets, prompt-injection/leakage/correctness evaluations and complete actor/tool/model/cost audit evidence.

Gate: AI has no alternate mutation path. Restricted data is default-deny for external providers. Every tool call repeats live authorization before side effects.

## 10. Phase 10 — marketplace — Planned

Deliver signed packages, publisher identity, WASM sandbox, SBOM/provenance verification, vulnerability policy, capability/data/network/secret grants, quotas, kill switch and safe upgrade/rollback/uninstall.

Gate: untrusted or policy-violating modules cannot install. Marketplace code cannot access resources outside explicit host grants.

## 11. Phase 11 — enterprise and production proof — Planned / continuous

Deliver SSO/OIDC/SAML, SCIM, tenant key hierarchy, field encryption, legal hold, WORM audit export, privacy deletion, crypto-shredding, backup/PITR, tenant restore, tenant mobility, data residency, SBOM/dependency/secret scans, penetration/load/chaos tests, SLOs and runbooks.

Gate: enterprise claims require automated and operational evidence, not configuration placeholders.

## 12. Immediate delivery sequence

1. Complete #83 / PR #84 with all applicable exact-head gates green.
2. Add governed metadata publish/activate/rollback/query capabilities and canonical public audit evidence.
3. Compose those contracts into `crm-application-runtime` and the deployable process host.
4. Build the first Admin Studio workflows in the product plane against governed APIs only.
5. Complete the typed UI-extension runtime and host failure isolation required to close Phase 7.
6. Begin the Phase 8 domain-wave program with customer master/identity/consent (#28) and commercial lifecycle (#29) as explicit owner domains.

## 13. Documentation hygiene

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.

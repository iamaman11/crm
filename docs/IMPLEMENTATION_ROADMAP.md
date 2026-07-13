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
17. A typed UI extension is not an authorization or infrastructure boundary. It receives only bounded host-owned context; backend authority remains on governed mutation/query paths. Untrusted third-party execution requires the later signed marketplace sandbox rather than same-realm JavaScript claims.

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
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, generalized projections, Admin Studio, product shell and UI-extension isolation | **In progress** | #9 |
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

## 7. Phase 7 — search, generalized projections, Admin Studio and product shell — In progress

Phase 7 is the active roadmap phase. All packets through 7H are complete. 7I is the final planned foundation packet required to close the phase.

### 7A — golden module tooling — Complete

[#56](https://github.com/iamaman11/crm/issues/56) / merged PR #64 established repository-supported owner/link module scaffolding, overwrite-safe generation, dependency validation, architecture-safe crate/manifests, acceptance placeholders and permanent cross-platform quality commands.

### 7B — generalized projection runtime — Complete

[#65](https://github.com/iamaman11/crm/issues/65) / merged PR #67 introduced `crm-projection-runtime` for typed projection registration, checkpoint-based replay, deterministic handler execution, poison/failure state and rebuild orchestration while keeping owner-domain decoding outside infrastructure.

### 7C — permission-aware search — Complete

[#66](https://github.com/iamaman11/crm/issues/66) / merged PR #68 delivered candidate-only search, live resource/field visibility re-checks, deterministic logical index generations, PostgreSQL FTS as a replaceable adapter, governed `search.global.query` ingress and immediate permission-revocation acceptance.

### 7D — typed web product shell — Complete

[#71](https://github.com/iamaman11/crm/issues/71) / merged PR #73 established the strict TypeScript product workspace, generated browser contracts, governed typed client boundary, centralized session state, permission-aware UX routing, design-system/application-shell primitives and hermetic Playwright E2E.

### 7E — immutable tenant-authorized typed metadata — Complete

[#77/#78](https://github.com/iamaman11/crm/issues/77), [#79/#80](https://github.com/iamaman11/crm/issues/79) and [#81/#82](https://github.com/iamaman11/crm/issues/81) established:

- immutable complete metadata-bundle snapshots;
- deterministic SHA-256 revision identity;
- tenant-scoped publication authority;
- impact analysis, explicit breaking confirmation, optimistic activation and rollback;
- strict typed object, field, relationship, layout, saved-view, pipeline, permission-template and workflow schemas;
- canonical JSON and deterministic dependency extraction;
- exact governed-capability workflow actions with no arbitrary script, SQL or HTTP primitive.

### 7F — durable tenant-scoped metadata persistence — Complete

[#83](https://github.com/iamaman11/crm/issues/83) / merged PR #84 established durable PostgreSQL metadata revisions, canonical documents, dependency edges, optimistic activation heads, per-tenant transaction locking, pop-only rollback history, append-only transition evidence, FORCE RLS and real PostgreSQL/migration acceptance.

PR #84 merged as `adbb639da69f5d87873b3c603a1388021c8359da`.

### 7G — governed metadata API and application composition — Complete

[#85](https://github.com/iamaman11/crm/issues/85) / merged PR #86 established:

- exact versioned Protobuf mutations/queries for publish, impact, activate, revision read, activation read and rollback;
- typed schema-to-bundle conversion;
- `CapabilityGateway` metadata mutations and `QueryGateway` metadata reads;
- PostgreSQL-backed production adapters and application composition;
- canonical global audit plus normal idempotency/business-transaction evidence;
- typed browser metadata operations over one shared governed gRPC-Web transport;
- no generic raw metadata gateway or frontend arbitrary coordinate escape hatch.

Final review head `7989ea1256f01bfd4e8ee2d33f5ad8370d6cc645` passed all 11 applicable workflows simultaneously. PR #86 merged as `970548d14faf26f4b8f6cb47f7d9f168e61d9c28`.

### 7H — first governed Admin Studio workflow — Complete

[#87](https://github.com/iamaman11/crm/issues/87) / merged PR #88 delivered:

- permission-aware typed Admin Studio route;
- object-definition authoring with no raw JSON mode;
- immutable publish → impact → explicit breaking confirmation → optimistic activate → activation read → rollback;
- user-intent-scoped mutation idempotency;
- safe product-owned error states;
- real browser E2E against fresh PostgreSQL and `crm-api` proving a breaking second revision and rollback.

Final review head `f78f1c75bf97733ff88eafcd2d2ed2ab6c7615d9` passed Product Plane CI including real browser/process acceptance and Rust CI. PR #88 merged as `0f01f22e6c77cd4f138a6b678d75d259f3ac71ff`.

### Current executable packet — 7I typed UI-extension runtime and host failure isolation

[#89](https://github.com/iamaman11/crm/issues/89) / draft PR #90 is **In progress** and is the active delivery packet.

The packet closes the remaining Phase 7 product-plane foundation:

- exact typed record-page extension surfaces;
- deterministic owner-bound extension coordinates and ordering;
- immutable validated registration with duplicate/invalid rejection;
- host-owned typed context only, without session/client/gateway injection;
- independent lazy loading and `Suspense` per extension instance;
- independent render/load error boundaries and bounded retry/reset;
- safe failure events containing only extension identifiers, surface, phase and attempt;
- reporter failure isolation so observational hooks cannot take down the host;
- real record-page proof with core host content and healthy sibling extensions surviving deliberate render and lazy-load failures;
- real browser acceptance for host-shell/record-page survival.

This same-realm runtime is for trusted product code. It does not claim arbitrary third-party JavaScript sandboxing. Signed packages and untrusted execution remain Phase 10 responsibilities.

### Phase 7 gate

Phase 7 may close only when all of the following are true on merged code:

- deleting search or projections cannot destroy authoritative data;
- permission changes cannot leak stale search results;
- published metadata is immutable, tenant-authorized, durably reconstructable, strictly validated, impact-analyzed, globally audited and reversible;
- Admin Studio changes use typed governed operations with strict validation, explicit breaking confirmation and rollback;
- one UI-extension render or lazy-load failure cannot break the host shell, record page, core host content or healthy sibling extensions;
- frontend code exposes no generic mutation/query bypass;
- final exact-SHA applicable CI is green.

After #89 / PR #90 merges with those gates green, Phase 7 should be marked **Complete** and Phase 8A / #28 becomes the first active expert owner-domain packet.

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

1. Complete #89 / PR #90, freeze one exact head with all applicable Product Plane, Rust and architecture/governance gates green, and merge promptly.
2. Mark Phase 7 / #10 Complete only after merged-code acceptance confirms UI-extension failure isolation.
3. Start Phase 8A / #28: canonical customer master, identity resolution and consent as a dedicated owner domain.
4. Follow with Phase 8B / #29: product catalog, pricing, CPQ and quote-to-revenue lifecycle without absorbing commercial ownership into Sales.
5. Continue frontend and expert backend modules as end-to-end vertical slices while enterprise/security hardening remains continuous.

## 13. Documentation hygiene

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.

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

1. Work is delivered through coherent reviewable delivery packets linked to roadmap issues. A packet must end at a natural architecture boundary with green acceptance gates.
2. Contract CI, Governance CI and Rust CI must remain green before merge; Database CI is mandatory whenever runtime, SQL, migrations or PostgreSQL behavior changes. Specialized runtime/process/product gates are mandatory where defined by the phase.
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
15. Frontend evolves in parallel with expert owner domains as end-to-end vertical slices rather than waiting for all backend domains to finish.
16. Exact-SHA evidence is invalidated by every source-changing or documentation-changing commit until all applicable checks are rerun on the new head.
17. A typed UI extension is not an authorization or infrastructure boundary. It receives only bounded host-owned context; backend authority remains on governed mutation/query paths. Untrusted third-party execution requires the later signed marketplace sandbox rather than same-realm JavaScript claims.
18. Canonical customer identity, consent and commercial commitment ownership must remain explicit. Downstream domains reference stable resources and may not create competing local masters.

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
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, generalized projections, Admin Studio, product shell and UI-extension isolation | **Complete** | #9 |
| 8 | [#11](https://github.com/iamaman11/crm/issues/11) | Expert modules and product-quality UX | **In progress** | #5, #9, #10 |
| 8A | [#28](https://github.com/iamaman11/crm/issues/28) | Canonical customer master, identity resolution and consent | **In progress** | #9, #10 |
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

Delivered governed capability, record, relationship, event, state, workflow, file, time, randomness and observability ports with tenant/actor/execution-context binding and deterministic test doubles. Business modules are compile-time excluded from raw database, broker, object-storage, arbitrary HTTP, secret-store and LLM-provider clients.

### Phase 3 — module lifecycle and registry

Delivered validate, publish, install, activate, suspend, upgrade, rollback and uninstall transitions; deterministic dependency resolution; immutable module versions; tenant-scoped installations; link-module support; uninstall impact/blockers; and audited idempotent lifecycle transitions.

### Phase 4 — PostgreSQL foundation

Delivered tenant-scoped records and relationships, FORCE-RLS boundaries, optimistic versions, atomic business state/idempotency/outbox/audit evidence, append-only tenant audit ledger, controlled typed payload metadata and migration clean-install/upgrade/rollback/reapply coverage.

### Phase 5 — capability execution gateway

Public mutations follow the governed chain:

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

Replay cannot duplicate side effects, semantic idempotency conflicts are typed, and missing evidence rolls back state.

## 6. Phase 6 — first modular production proof — Complete

Issue: [#9](https://github.com/iamaman11/crm/issues/9)  
Delivery packet: merged PR #63 / issue #55.  
Merge commit: `82910fa17f21074b1e091615a4251092cfa8ab2f`.

Phase 6 established independent typed Sales `Deal` and Activities `Task` owner aggregates, publication-compatible Protobuf contracts, production PostgreSQL mutations and permission-bound queries, authenticated HTTP/gRPC ingress, durable event delivery, the optional `crm.sales-activities-link` module, rebuildable projections, a real application composition root and deployable `crm-api` process with process-level PostgreSQL acceptance.

## 7. Phase 7 — search, metadata, Admin Studio and product plane — Complete

Issue: [#10](https://github.com/iamaman11/crm/issues/10).

### 7A — golden module tooling — Complete

[#56](https://github.com/iamaman11/crm/issues/56) / merged PR #64 established repository-supported owner/link module scaffolding, overwrite-safe generation, dependency validation, architecture-safe manifests/crates, acceptance placeholders and permanent cross-platform quality commands.

### 7B — generalized projection runtime — Complete

[#65](https://github.com/iamaman11/crm/issues/65) / merged PR #67 introduced deterministic typed projection registration/execution, checkpoint replay, poison/failure state and rebuild orchestration without moving owner-domain decoding into infrastructure.

### 7C — permission-aware search — Complete

[#66](https://github.com/iamaman11/crm/issues/66) / merged PR #68 delivered candidate-only search, live resource/field visibility re-checks, deterministic logical index generations, PostgreSQL FTS as a replaceable adapter and governed `search.global.query` ingress.

### 7D — typed web product shell — Complete

[#71](https://github.com/iamaman11/crm/issues/71) / merged PR #73 established the strict TypeScript product workspace, generated browser contracts, governed typed client boundary, centralized session state, permission-aware UX routing, design-system/application-shell primitives and hermetic Playwright E2E.

### 7E — immutable tenant-authorized typed metadata — Complete

[#77/#78](https://github.com/iamaman11/crm/issues/77), [#79/#80](https://github.com/iamaman11/crm/issues/79) and [#81/#82](https://github.com/iamaman11/crm/issues/81) established immutable complete metadata-bundle snapshots, deterministic revision identity, tenant-scoped publication authority, impact analysis, explicit breaking confirmation, optimistic activation/rollback, strict typed metadata schemas, deterministic dependency extraction and exact governed-capability workflow actions.

### 7F — durable tenant-scoped metadata persistence — Complete

[#83](https://github.com/iamaman11/crm/issues/83) / merged PR #84 established durable PostgreSQL metadata revisions, canonical documents, dependency edges, optimistic activation heads, per-tenant transaction locking, pop-only rollback history, append-only transition evidence, FORCE RLS and real PostgreSQL/migration acceptance. Merge commit: `adbb639da69f5d87873b3c603a1388021c8359da`.

### 7G — governed metadata API and application composition — Complete

[#85](https://github.com/iamaman11/crm/issues/85) / merged PR #86 established exact versioned Protobuf metadata mutations/queries, typed schema-to-bundle conversion, `CapabilityGateway` mutations, `QueryGateway` reads, PostgreSQL-backed production adapters, canonical global audit evidence and typed browser metadata operations over one shared governed gRPC-Web transport. Final review head `7989ea1256f01bfd4e8ee2d33f5ad8370d6cc645`; merge commit `970548d14faf26f4b8f6cb47f7d9f168e61d9c28`.

### 7H — first governed Admin Studio workflow — Complete

[#87](https://github.com/iamaman11/crm/issues/87) / merged PR #88 delivered permission-aware typed Admin Studio authoring with no raw JSON mode and the full immutable publish → impact → explicit breaking confirmation → optimistic activate → activation read → rollback lifecycle. Real browser E2E proves a breaking second revision and rollback. Final review head `f78f1c75bf97733ff88eafcd2d2ed2ab6c7615d9`; merge commit `0f01f22e6c77cd4f138a6b678d75d259f3ac71ff`.

### 7I — typed UI-extension runtime and host failure isolation — Complete

[#89](https://github.com/iamaman11/crm/issues/89) / merged PR #90 closed the final Phase 7 product-plane foundation:

- exact typed record-page extension surfaces;
- immutable validated registration with owner-bound deterministic coordinates and locale-independent ordering;
- deterministic rejection of invalid and duplicate registrations;
- readonly host-owned context without session/client/raw gateway/infrastructure injection;
- independent lazy loading and `Suspense` per extension instance;
- independent load/render error boundaries and bounded retry/reset;
- safe failure evidence with no raw error or record-payload leakage;
- failure-observer isolation;
- development-only lazy-loaded record host proof instead of a fake production data surface;
- real browser acceptance proving the shell, core record content and healthy sibling extensions survive deliberate render and lazy-load failures and targeted retry.

The duplicate-coordinate unit gate exposed and prevented a real implementation defect before merge. Final review head `874dde11f5d558bd5e53f2def3e8903ff12f361a` passed Governance CI, Rust CI and Product Plane CI including generated sync, strict typecheck, lint, unit tests, production build and fresh PostgreSQL/process/browser E2E. PR #90 merged as `0fb389c72b148311f590c3fdbae2a4f89fffd915`.

### Phase 7 closure gate — Satisfied

Merged Phase 7 now proves:

- deleting search indexes or projections cannot destroy authoritative data;
- permission changes cannot rely on stale search visibility;
- published metadata is immutable, tenant-authorized, durably reconstructable, strictly validated, impact-analyzed, globally audited and reversible;
- Admin Studio uses typed governed operations with explicit breaking confirmation and rollback;
- one trusted-code UI-extension render or lazy-load failure cannot break the host shell, record page, core host content or healthy sibling extensions;
- frontend code exposes no generic mutation/query bypass;
- final Phase 7I exact-head Governance, Rust and Product Plane acceptance was green before merge.

The trusted same-realm UI-extension runtime is not an untrusted-code sandbox. Signed packages and untrusted execution remain Phase 10 responsibilities.

## 8. Phase 8 — expert owner domains and product experience — In progress

After Phase 7, backend and frontend evolve as end-to-end vertical slices rather than as long disconnected projects.

### 8A — canonical customer master, identity resolution and consent — In progress

Active issue: [#28](https://github.com/iamaman11/crm/issues/28).

This program establishes the canonical customer identity foundation required by Sales, Service, Marketing, Billing, projects and AI.

Required owner domains:

- Party — person and organization identities;
- Account — customer/commercial relationship referencing one or more parties;
- Contact Point — email, phone, postal, social/messaging handle and channel preference;
- Party Relationship — employment, household, parent/subsidiary, partner and configurable typed roles with validity intervals;
- Consent and Communication Preference — purpose, channel, legal basis, jurisdiction, source, proof, effective/expiry/withdrawal time;
- Identity Resolution — source identifiers, match evidence, survivorship decisions and immutable merge/unmerge history.

Architectural constraints:

- customer master is authoritative for identity;
- downstream domains store stable references and explicitly justified snapshots only;
- no downstream table mutation of customer-master state;
- merge, unmerge and consent withdrawal are capability-governed, idempotent, approval-aware where required and audited;
- source evidence and field-level provenance are never silently discarded;
- search/projections remain rebuildable and permission-aware;
- PII access, masking, export and deletion repeat live authorization;
- AI may suggest matches but cannot merge identities or change consent through an alternate path.

Acceptance must cover cross-tenant isolation, deterministic import/replay, merge/unmerge lineage and rollback, immediate consent-withdrawal enforcement, explainable duplicate candidates, privacy export/deletion/legal-hold interactions, contract compatibility, migrations and performance.

Current delivery position: Party create/update/get/list/search is production-proven. The first authoritative Account create/update/get/list slice with typed Party associations and platform-level reference integrity is merged and production-proven (#101 / merged PR #102; final verified head `0d6d79dce31aaea4d2a0998fadb1ac842fdcfde4`; merge commit `7ee48530d880ef8aeb6abf2140b524ac724d4fc9`). Contact Point lifecycle is now active in #103 on `develop/phase8a3b-contact-point-lifecycle`; the branch currently contains the first typed lifecycle/verification domain foundation but not yet the public contracts, adapters, runtime composition or production process proof required for merge. Party Relationship and Customer 360 follow. Consent, identity resolution, merge/unmerge, provenance and privacy lifecycle remain later explicit 8A packets.

### 8B — product catalog, CPQ and quote-to-revenue lifecycle — Planned

[#29](https://github.com/iamaman11/crm/issues/29) follows the customer-master foundation. Catalog, pricing, quote, order, contract and subscription ownership must remain explicit and must not be absorbed into Sales.

### Additional Phase 8 owner-domain programs

- Sales and Activities expert expansion;
- communications and omnichannel interaction history;
- support and service management;
- marketing segmentation, journeys and attribution;
- billing and governed ERP/payment/tax integrations;
- projects/cases/configurable work management;
- documents and e-signature;
- analytics, forecasting and performance management;
- product-quality global search, command palette, tables, saved views, bulk actions, timelines, onboarding, imports, responsive/mobile behavior, accessibility and localization.

Gate: each module owns typed domain invariants, contracts, manifest, CI target and release notes. Critical rules cannot be bypassed by arbitrary metadata, scripts or AI.

## 9. Phase 9 — AI-native layer — Planned

AI is an Actor, not an infrastructure shortcut.

Deliver model routing by tenant/data class/purpose/residency/cost, permission-scoped tools generated from the Capability Registry, permission-filtered retrieval, approval flows, reversible actions, budgets, prompt-injection/leakage/correctness evaluations and complete actor/tool/model/cost audit evidence.

Gate: AI has no alternate mutation path. Restricted data is default-deny for external providers. Every tool call repeats live authorization before side effects.

## 10. Phase 10 — signed marketplace and sandbox — Planned

Deliver signed packages, publisher identity, WASM sandbox, SBOM/provenance verification, vulnerability policy, capability/data/network/secret grants, quotas, kill switch and safe upgrade/rollback/uninstall.

Gate: untrusted or policy-violating modules cannot install. Marketplace code cannot access resources outside explicit host grants.

## 11. Phase 11 — enterprise and production proof — Planned / continuous

Deliver SSO/OIDC/SAML, SCIM, tenant key hierarchy, field encryption, legal hold, WORM audit export, privacy deletion, crypto-shredding, backup/PITR, tenant restore, tenant mobility, data residency, SBOM/dependency/secret scans, penetration/load/chaos tests, SLOs and runbooks.

Gate: enterprise claims require automated and operational evidence, not configuration placeholders.

## 12. Immediate delivery sequence

1. Continue Phase 8A.3b / #103 from `develop/phase8a3b-contact-point-lifecycle`: complete the Contact Point domain, versioned contracts, deterministic persistence, governed mutation/query adapters, application composition, Party-reference integrity and real PostgreSQL/`crm-api` process acceptance.
2. Deliver Party Relationship lifecycle/hierarchy and permission-aware Customer 360 composition.
3. Continue Phase 8A with Consent, identity resolution, merge/unmerge, provenance, import/export, data quality and privacy lifecycle proof.
4. Follow with Phase 8B / #29 commercial lifecycle without moving catalog/pricing/order/contract ownership into Sales.
5. Continue frontend and expert backend modules as end-to-end vertical slices while enterprise/security/operational hardening remains continuous.

## 13. Documentation hygiene

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.

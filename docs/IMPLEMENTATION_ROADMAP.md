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
| 6 | [#9](https://github.com/iamaman11/crm/issues/9) | Sales + Activities + link/projection/application vertical proof | **Gate review** | #8 |
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, generalized projections, Admin Studio and product shell | **Planned** | #9 |
| 8 | [#11](https://github.com/iamaman11/crm/issues/11) | Expert modules and product-quality UX | **Planned** | #5, #9, #10 |
| 8A | [#28](https://github.com/iamaman11/crm/issues/28) | Canonical customer master, identity resolution and consent | **Planned** | #9, #10 |
| 8B | [#29](https://github.com/iamaman11/crm/issues/29) | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | #9, #10, #28 |
| 9 | [#12](https://github.com/iamaman11/crm/issues/12) | AI-native governed actor/tool layer | **Planned** | #8, #10 |
| 10 | [#13](https://github.com/iamaman11/crm/issues/13) | Signed marketplace and WASM sandbox | **Planned** | #6, #8, #10 |
| 11 | [#14](https://github.com/iamaman11/crm/issues/14) | Enterprise security and production proof | **Planned / continuous hardening** | all critical runtime phases |

## 5. Phase 0.1 — Repository hardening — Complete

Delivered:

- version-controlled roadmap and issue hierarchy;
- stable CODEOWNERS and executable architecture-policy boundaries;
- correct generated-artifact and validation documentation;
- required Contract, Governance, Rust and Database checks.

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

Delivered guarantees include strict typed structures, unknown-field rejection, SemVer/dependency validation, ownership indexes, dependency-cycle detection and deterministic Python-to-Rust canonical identity parity.

## 7. Phase 2 — Governed Module SDK — Complete

Delivered governed `CapabilityClient`, `RecordClient`, `RelationshipClient`, `EventPublisher`, `ModuleStateStore`, `WorkflowClient`, `FileClient`, `Clock`, `RandomSource` and observability ports with tenant/actor/execution-context binding and deterministic test doubles.

Business modules are compile-time excluded from raw database, broker, object-storage, arbitrary HTTP, secret-store and LLM-provider clients.

## 8. Phase 3 — Module lifecycle and registry — Complete

Delivered validate, publish, install, activate, suspend, upgrade, rollback and uninstall transitions; deterministic dependency resolution; immutable module versions; tenant-scoped installations; link-module support; uninstall impact/blockers; and audited idempotent lifecycle state transitions.

## 9. Phase 4 — PostgreSQL foundation — Complete

Authoritative guarantees:

- tenant-scoped records and relationships;
- FORCE-RLS boundaries where required;
- optimistic versions;
- atomic business state, idempotency, outbox and audit evidence;
- append-only tenant audit ledger;
- controlled typed payload metadata;
- clean install, legacy upgrade, rollback and reapply tests.

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

## 11. Phase 6 — First modular proof — Gate review

Issue: [#9](https://github.com/iamaman11/crm/issues/9)  
Delivery packet: PR #63 / issue #55.

### Completed slices 6A–6H

6A–6H established:

- typed independent Sales `Deal` and Activities `Task` owner-domain aggregates;
- publication-compatible versioned Protobuf contracts;
- transactional audit materialization and transaction-aware aggregate execution;
- validated persisted codecs and generated contract runtime;
- production Sales/Activities mutation planners and PostgreSQL execution;
- authenticated production mutation acceptance with replay/conflict/revocation/rollback evidence;
- permission-bound Deal/Task get/list queries;
- HMAC-bound opaque keyset cursors;
- resource/field visibility, non-disclosing denial and authenticated HTTP/gRPC query ingress.

Authoritative merged work includes PRs #26, #27, #30, #31, #33, #34, #37, #40 and #42–#46.

### Gate-review slice 6I — optional Sales–Activities link and governed event delivery

Implementation packet: PR #63. Tracking issue: #47.

- Added restart-safe event lineage and reconstruction of immutable `EventDelivery` from authoritative outbox history.
- Added a durable consumer delivery ledger with lease, retry, recovery and dead-letter behavior.
- Added lifecycle-aware link processing that checks tenant installation state before contract decoding or target invocation.
- Preserved the pure link-module boundary: generated Protobuf, PostgreSQL and target owner internals remain outside `modules/crm-sales-activities-link`.
- Routed target execution through `GatewayCapabilityClient` and the production `CapabilityGateway`, preserving tenant, actor, idempotency and correlation/causation/trace lineage.
- Proved duplicate delivery, retry recovery, delivery-ledger rebuild, suspension/removal and cross-tenant denial without duplicate Activities business effects.

### Gate-review slice 6J — rebuildable Deal timeline and Task status projections

Implementation packet: PR #63. Tracking issue: #48.

- Added FORCE-RLS rebuildable projection checkpoints, applied-event deduplication and JSON projection documents.
- Added keyset replay over immutable event history with tenant-scoped checkpoints.
- Added Deal timeline and Task status projection handlers that depend on published event contracts rather than owner-module internals.
- Applied `applied event + projection documents + checkpoint` atomically.
- Proved resume, duplicate-idle behavior, cross-tenant non-disclosure and reset/rebuild equivalence without creating authoritative mutation evidence.

### Gate-review slice 6K — production application composition root

Implementation packet: PR #63. Tracking issue: #49.

- Added `crm-application-runtime` as the single production composition boundary.
- Made `services/crm-api` a thin process host that depends only on the application runtime.
- Added validated environment configuration, PostgreSQL construction, mutation/query gateways, authentication, live authorization, query visibility and worker composition.
- Added governed HTTP mutation/query endpoints and versioned `crm.gateway.v1.ApplicationGatewayService` gRPC mutation/query transport.
- Added health, readiness, background link/projection workers and graceful shutdown.
- Enforced the thin-host architecture boundary so process/transport code cannot directly import owner-domain internals or PostgreSQL adapters.

### Gate-review slice 6L — complete process-level production acceptance

Implementation packet: PR #63. Tracking issue: #50.

- Added dedicated Application Runtime CI that applies migrations `0001` through `0008`, seeds production fixtures and starts the real `crm-api` binary against PostgreSQL.
- Proved unauthenticated HTTP denial, authenticated Sales mutations, background Sales-to-Activities link delivery, Deal timeline and Task status projection materialization, governed gRPC query execution and clean SIGINT shutdown.
- Added migration `0008` to canonical clean install, rollback, reapply and legacy-upgrade Database CI paths.
- Preserved Database and Event Runtime regression acceptance alongside the process-level gate.

### Final Phase 6 merge gate

Implementation checkpoint `acba0b0d97998e7a0a347749032e1f7002fa6b34` passed **Contract CI, Governance CI, Rust CI, Database CI, Event Runtime CI, Application Runtime CI and generic Rust Generated Sync simultaneously**.

Sales and Activities remain independently governed owner modules. The optional link is lifecycle-gated and cannot bypass the target capability gateway. Duplicate source-event delivery produces no duplicate Task or projection business effect. Every mutation follows the authenticated gateway and commits state only with required idempotency, outbox and audit evidence. Query paths are permission-bound, projections are rebuildable, and `crm-api` is a real governed production process rather than a skeleton.

Phase 6 remains **Gate review** until PR #63 is merged. After merge, Phase 6 becomes **Complete** and Phase 7 becomes the next active roadmap phase.

## 12. Phase 7 — Search, generalized projections, Admin Studio and product-shell foundation — Planned

Phase 7 begins after the Phase 6 production composition proof is merged.

Platform deliverables:

- generalize the proven projection/checkpoint/retry/rebuild runtime for broader read models;
- tenant- and permission-aware search with deterministic reindexing;
- object, field, relationship, layout, view, pipeline, permission and workflow builders;
- impact reports, immutable metadata versions and rollback behavior;
- typed UI-extension runtime with safe fallback;
- golden module generator and permanent repository commands so new modules inherit architecture/gates by construction.

Product-plane deliverables:

- web product shell and typed generated client boundary;
- navigation, authentication/session integration and permission-aware routing;
- design-system primitives, accessibility baseline and responsive layout system;
- localization/time-zone strategy and consistent error/loading conventions;
- first Admin Studio workflows against governed metadata publication APIs.

Gate: deleting search or projections cannot destroy authoritative data; permission changes cannot leak stale results; Admin changes are validated, audited and reversible; UI-extension failure cannot break the host shell; frontend code cannot bypass governed mutation/query paths.

## 13. Phase 8 — Expert modules and product experience — Planned

After Phase 7, backend and frontend evolve as end-to-end vertical slices rather than as long disconnected projects.

Required owner-domain programs include:

- canonical customer master, identity resolution and consent — [#28](https://github.com/iamaman11/crm/issues/28);
- Sales and Activities expert expansion;
- communications and omnichannel interaction history;
- support and service management;
- marketing segmentation, journeys and attribution;
- Product Catalog, Price Books, CPQ, Quote, Order, Contract and Subscription lifecycle — [#29](https://github.com/iamaman11/crm/issues/29);
- governed billing/ERP/payment/tax/fulfillment integration boundaries;
- projects, cases and configurable work management;
- documents and e-signature;
- analytics, forecasting and performance management.

Product experience includes global search, command palette, keyboard navigation, fast tables, saved views, bulk actions, timelines, explainable permissions, transparent automation runs, onboarding, imports, responsive/mobile behavior, accessibility and localization.

Gate: each module owns typed domain invariants, contracts, manifest, CI target and release evidence. Critical rules cannot be bypassed by arbitrary metadata, scripts or AI. Customer identity and commercial commitment ownership remain explicit and non-overlapping.

## 14. Phase 9 — AI-native layer — Planned

AI is an Actor, not an infrastructure shortcut.

Deliverables:

- AI Gateway and model routing by tenant, data class, purpose, residency and cost;
- permission-scoped tools generated from Capability Registry;
- permission-filtered retrieval;
- approval flows, reversible actions and budgets;
- prompt-injection, leakage and correctness evaluations;
- complete actor, tool, model and cost audit evidence.

Gate: AI has no alternate mutation path. Restricted data is default-deny for external providers. Every tool call repeats live authorization before side effects.

## 15. Phase 10 — Marketplace — Planned

Deliver signed packages, publisher identity, WASM sandbox, SBOM/provenance verification, vulnerability policy, capability/data/network/secret grants, quotas, kill switch and safe upgrade/rollback/uninstall.

Gate: untrusted or policy-violating modules cannot install. Marketplace code cannot access resources outside explicit host grants.

## 16. Phase 11 — Enterprise and production proof — Planned / continuous

Deliver SSO/OIDC/SAML, SCIM, tenant key hierarchy, field encryption, legal hold, WORM audit export, privacy deletion, crypto-shredding, backup/PITR, tenant restore, tenant mobility, data residency, SBOM/dependency/secret scans, penetration/load/chaos tests, SLOs and runbooks.

Gate: production deployment requires documented restore, failover, security, privacy, observability and SLO evidence. Enterprise controls must preserve the same governed capability, tenant and audit invariants rather than introducing bypass paths.

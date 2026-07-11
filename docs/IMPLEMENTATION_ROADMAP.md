# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**  
Parent epic: [#2](https://github.com/iamaman11/crm/issues/2)  
Governing rules: [`SYSTEM_INVARIANTS.md`](SYSTEM_INVARIANTS.md) and accepted ADRs.  
Structural guide: [`APPLICATION_ARCHITECTURE.md`](APPLICATION_ARCHITECTURE.md).  
Current concise state: [`PROJECT_STATUS.md`](PROJECT_STATUS.md).  
Business-module accounting: [`MODULE_CATALOG.md`](MODULE_CATALOG.md).

## 1. Purpose

This roadmap turns the architecture specification into a controlled delivery sequence. It is not a feature wishlist. Every phase establishes platform guarantees required by later phases, has explicit acceptance gates, and must preserve all system invariants.

The target is a universal modular expert CRM platform where first-party and marketplace modules can be developed, tested, released, installed, activated, upgraded, suspended and removed independently without direct infrastructure access or cross-module state mutation.

Universal means that Sales is not allowed to become the owner of customer identity, catalog, pricing, order, contract, subscription, service, communication or billing state. Those domains require explicit owner modules and versioned integration boundaries.

## 2. Delivery rules

1. Work is delivered through small reviewable pull requests linked to a roadmap issue.
2. Contract CI, Governance CI and Rust CI must remain green before merge; Database CI is mandatory whenever runtime, SQL, migrations or PostgreSQL behavior changes.
3. Published contracts, policies, metadata and module versions are immutable.
4. A phase is complete only when its acceptance gates are automated or supported by a documented operational drill.
5. New state-changing behavior must enter through a versioned capability and produce typed audit evidence.
6. Business modules may depend only on stable platform contracts and governed SDK ports.
7. Search, analytics, caches and projections remain rebuildable and non-authoritative.
8. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements, not later enhancements.
9. Technical debt discovered by a gate is fixed or explicitly recorded before dependent work begins.
10. The roadmap and GitHub issues are updated in the same PR when scope or sequencing changes.
11. Exact money, time, identity, lifecycle and authorization semantics must be represented by typed contracts rather than convention or free-form strings.
12. No milestone may claim the complete CRM product is finished while required owner domains or production gates remain open.
13. `README.md` is stable orientation only. Current phase state is synchronized through this roadmap, `PROJECT_STATUS.md`, `MODULE_CATALOG.md` and the active GitHub phase issue.
14. A backend phase is not considered application-complete while its production components exist only as libraries or tests and the deployable composition root remains a skeleton.
15. Frontend is a separate product-plane workstream after the first backend vertical proof, then evolves in parallel with later expert modules rather than waiting for all backend domains to finish.

## 3. Work states

- **Planned** — scoped but prerequisites are incomplete.
- **Ready** — prerequisites and contracts are stable enough to start.
- **In progress** — an implementation branch or PR exists.
- **Gate review** — implementation is complete and acceptance evidence is being verified.
- **Complete** — merged and all gates have passed.
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
| 6 | [#9](https://github.com/iamaman11/crm/issues/9) | Sales + Activities + link/projection/application vertical proof | **In progress** | #8 |
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, projections, Admin Studio and product-shell foundation | **Planned** | #9 |
| 8 | [#11](https://github.com/iamaman11/crm/issues/11) | Expert modules and product-quality UX | **Planned** | #5, #9, #10 |
| 8A | [#28](https://github.com/iamaman11/crm/issues/28) | Canonical customer master, identity resolution and consent | **Planned** | #9, #10 |
| 8B | [#29](https://github.com/iamaman11/crm/issues/29) | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | #9, #10, #28 |
| 9 | [#12](https://github.com/iamaman11/crm/issues/12) | AI-native governed actor/tool layer | **Planned** | #8, #10 |
| 10 | [#13](https://github.com/iamaman11/crm/issues/13) | Signed marketplace and WASM sandbox | **Planned** | #6, #8, #10 |
| 11 | [#14](https://github.com/iamaman11/crm/issues/14) | Enterprise security and production proof | **Planned / continuous hardening** | all critical runtime phases |

## 5. Phase 0.1 — Repository hardening — Complete

### Delivered

- Version-controlled roadmap and issue hierarchy.
- Stable CODEOWNERS and architecture-policy boundaries.
- Correct generated-artifact and validation documentation.
- Required Contract, Governance, Rust and Database checks.

### Gate

The repository must not claim stale checksums, warnings or committed artifacts that do not exist. Every implementation phase has an issue, prerequisites and measurable acceptance criteria.

## 6. Phase 1 — Typed Module Manifest IR — Complete

### Runtime boundary

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

### Delivered guarantees

- Strict typed structures and unknown-field rejection.
- SemVer and dependency-range validation.
- Module, object, capability and event ownership indexes.
- Required-dependency cycle detection.
- Deterministic canonical serialization and digest.
- Python-to-Rust IR and digest parity tests.

## 7. Phase 2 — Governed Module SDK — Complete

### Delivered guarantees

- Governed `CapabilityClient`, `RecordClient`, `RelationshipClient`, `EventPublisher`, `ModuleStateStore`, `WorkflowClient`, `FileClient`, `Clock`, `RandomSource` and observability ports.
- Tenant, actor and execution-context binding on side-effecting calls.
- In-memory deterministic test doubles.
- Compile-time exclusion of raw database, broker, object-storage, arbitrary HTTP, secret-store and LLM-provider clients from module APIs.

## 8. Phase 3 — Module lifecycle and registry — Complete

### Delivered guarantees

- Versioned validate, publish, install, activate, suspend, upgrade, rollback and uninstall transitions.
- Deterministic dependency resolution.
- Immutable module versions and tenant-scoped installations.
- Link-module and uninstall-blocker support.
- Audited idempotent lifecycle state machine.

## 9. Phase 4 — PostgreSQL foundation — Complete

### Authoritative capabilities

- Tenant-scoped records and relationships.
- Optimistic versions.
- Atomic business state, idempotency, outbox and audit evidence.
- Append-only tenant audit ledger.
- Controlled typed payload metadata.
- Clean install, legacy upgrade, rollback and reapply tests.

### Preserved gate

Cross-tenant negative tests and transaction fault injection must remain green after every later runtime or migration change.

## 10. Phase 5 — Capability execution gateway — Complete

### Execution chain

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

### Delivered guarantees

- Public HTTP and gRPC mutation paths cannot bypass the gateway.
- Live authorization is the last awaited operation before the transactional executor.
- Replays do not duplicate side effects.
- Idempotency-key reuse with different semantic input returns a typed conflict.
- Missing required evidence rolls back state and preserves the audit head.
- External behavior never depends on parsing error text.

## 11. Phase 6 — First modular proof — In progress

Issue: [#9](https://github.com/iamaman11/crm/issues/9)

### Completed slice 6A — typed owner-domain contracts

Merged in PR #26.

- Exact shared money, currency, basis-point, calendar-date, patch and pagination value objects.
- Encapsulated Sales `Deal` aggregate with ownership, pipeline/stage identity, lifecycle, close outcome, amount, probability, expected close date and optimistic versioning.
- Encapsulated Activities `Task` aggregate with owner, priority, status, related resources, due/reminder/completion semantics and optimistic versioning.
- Deterministic lifecycle and transition tests.
- Sales and Activities compile independently and do not import each other.

### Completed slice 6B — publication-compatible Protobuf contracts

Merged in PR #27.

- Versioned Sales and Activities commands, responses and event payloads.
- Compatibility-preserving evolution of the original Sales v1 schema.
- Exact money, time, ownership and resource-reference wire types.
- Buf build, canonical formatting, STANDARD lint and FILE-level breaking checks.
- Machine-checked manifest capability/event bindings against an immutable descriptor set.

### Completed slice 6C — transactional audit materialization

Merged in PR #30.

- Planners emit deterministic audit intent rather than guessing sequence numbers or previous hashes.
- PostgreSQL acquires tenant-scoped audit serialization inside the business transaction.
- The runtime assigns contiguous sequence numbers, materializes previous hashes and computes canonical chained record hashes atomically.
- Concurrent commits preserve one linear tenant audit history.

### Completed slice 6D — transaction-aware aggregate execution

Merged in PR #31.

- The authoritative owner-module record is locked before read-modify-write planning.
- Full typed capability responses are replayed from idempotency evidence.
- Stale optimistic versions fail without partial record, outbox or audit effects.
- PostgreSQL remains the first awaited operation after live authorization.

### Completed slice 6E — persisted codecs and generated contract runtime

Merged in PR #33 and PR #34.

- Validated independent Deal and Task persisted-state codecs with bounded payloads and exact money preservation.
- Safe persisted-corruption errors without leaking internal bytes.
- Canonical generated Rust Protobuf types and one descriptor universe for published wire contracts.
- Public wire schema identity remains independent from owner-module persisted-state identity.

### Completed slice 6F — audited no-op foundation and Sales/Activities capability adapters

Merged in PR #37.

- Recovered the incomplete PR #35 merge into ordinary reviewable source code and removed temporary payload/bootstrap artifacts.
- Added shared deterministic `crm-capability-plan-support`, audited semantic no-op transactions and migration `0005_audited_noop_transactions`.
- Added production Sales deal create/update/stage-advance planners and Activities task create/update/complete/reminder planners with exact money, tenant/resource validation and generated Protobuf mapping.
- Verified mutation, replay, stale conflict, semantic no-op, rollback, migration and evidence behavior.

### Completed slice 6G — authenticated production PostgreSQL mutation acceptance

Authoritative PR: #40. Issue: #39.

- Added `crm-sales-activities-capability-composition` as the non-transport boundary for exactly seven published Sales and Activities mutation capabilities.
- Added a deterministic exact-version catalog and synchronous planner router that rejects unsupported, version-mismatched, owner-mismatched and request-mismatched coordinates before PostgreSQL.
- Preserved transport boundaries: `crm-capability-ingress` and `services/crm-api` gained no production dependency on business owner modules or persistence implementations.
- Exercised real Sales create/update/stage-advance and Activities create/update/reminder/complete commands through authenticated HTTP/gRPC ingress, semantic validation, live authorization and `PostgresTransactionalAggregateExecutor`.
- Proved invalid bearer and tenant denial, cross-tenant resource rejection, exact replay, idempotency conflict, stale-version conflict, live permission revocation and audited semantic no-op behavior.
- Measured exact committed evidence and proved that replay, denied and conflicting requests add no duplicate or partial evidence.
- Added production evidence-omission rollback acceptance for missing outbox, audit and idempotency evidence, with typed safe failure and zero committed transactional side effects.

### Completed slice 6H — permission-bound production queries

Authoritative PRs: #42, #43, #44, #45 and #46. Issue: #41.

- Added HMAC-SHA256 opaque cursor tokens bound to tenant, actor, exact query/version, resource type, normalized filters, sort and effective page size.
- Added FORCE-RLS tenant-scoped PostgreSQL get/list readers with deterministic keyset ordering and no mutation evidence.
- Added a dedicated read-only `QueryGateway` path with exact-version capability binding and live authorization immediately before authoritative read execution.
- Added four production query coordinates: `sales.deal.get`, `sales.deal.list`, `activities.task.get` and `activities.task.list`.
- Added independent resource/field visibility grants, non-disclosing resource denial and field masking before Protobuf serialization.
- Added authenticated HTTP and gRPC query ingress with query-only execution context; idempotency keys and business-transaction IDs are structurally absent from the query contract.
- Proved cross-tenant denial, cursor tamper/binding rejection, live authorization revocation between pages, HTTP/gRPC authentication and tenant failures, and zero record/outbox/audit/idempotency/business-transaction evidence delta across reads.
- Preserved a separate seven-capability mutation catalog so read coordinates cannot enter the transactional mutation gateway.

### Next implementation sequence

1. **6I — optional Sales–Activities link module:** consume source Sales events through governed delivery contracts; invoke Activities only through `CapabilityClient`; own deterministic delivery deduplication/configuration state; prove duplicate delivery, tenant binding and independent disable/uninstall.
2. **6J — rebuildable projections:** deliver deal timeline and task-status projections with tenant checkpoints, retries, replay, poison handling and deletion/rebuild equivalence.
3. **6K — production application composition root:** turn `services/crm-api` from a skeleton into the real deployable process that validates configuration, constructs infrastructure/runtime adapters, composes capability and query catalogs, starts HTTP/gRPC, exposes health/readiness and shuts down gracefully without importing owner-domain internals into transport code.
4. **6L — complete Phase 6 production E2E:** prove the complete composed path for cross-tenant denial, stale conflict, duplicate delivery, disabled/uninstalled link behavior, transaction rollback/fault injection, projection rebuild and process-level application startup/readiness.

### Completion gate

Sales and Activities remain independently installable and functional when the link module is disabled. Duplicate source-event delivery produces no duplicate task or projection effect. Every mutation follows the authenticated gateway and commits state only with idempotency, outbox and audit evidence. Query paths are permission-bound, projections are rebuildable, `crm-api` is a real governed production composition root rather than a skeleton, and all required checks are green on one clean merge head.

## 12. Phase 7 — Search, projections, Admin Studio and product-shell foundation — Planned

Phase 7 begins only after Phase 6 proves the complete backend application composition boundary.

### Platform deliverables

- Generalized idempotent projection workers, checkpoints, retries and rebuild.
- Tenant- and permission-aware search with reindexing.
- Object, field, relationship, layout, view, pipeline, permission and workflow builders.
- Impact reports, immutable versions and rollback behavior.
- Typed UI extension runtime with safe fallback.

### Product-plane foundation

- Introduce the web product shell and typed generated client boundary.
- Establish navigation, authentication/session integration and permission-aware routing.
- Establish design-system primitives, accessibility baseline, localization/time-zone strategy and error/loading conventions.
- Build the first Admin Studio workflows against governed metadata publication APIs.
- Keep all business invariants authoritative in owner modules; the frontend orchestrates user interaction but does not become a second domain runtime.

### Gate

Deleting search or projections cannot destroy authoritative data. Permission changes cannot leak stale results. Admin changes are validated, audited and reversible. UI extension failure cannot break the host shell. The frontend cannot bypass governed mutation/query paths.

## 13. Phase 8 — Expert modules and product experience — Planned

After Phase 7, backend and frontend evolve as end-to-end vertical slices rather than as two long disconnected projects.

### Required owner domains

- Canonical customer master and consent — [#28](https://github.com/iamaman11/crm/issues/28).
- Sales and Activities expert expansion.
- Communications and omnichannel interaction history.
- Support and service management.
- Marketing segmentation, journeys and attribution.
- Product catalog, price books, CPQ, quote, order, contract and subscription lifecycle — [#29](https://github.com/iamaman11/crm/issues/29).
- Billing and governed ERP/payment/tax integrations.
- Projects, cases and configurable work management.
- Documents and e-signature.
- Analytics, forecasting and performance management.

### Product experience

Global search, command palette, keyboard navigation, fast tables, saved views, bulk actions, timelines, explainable permissions, transparent automation runs, onboarding, imports, responsive/mobile behavior, accessibility and localization.

### Gate

Each module owns typed domain invariants, contracts, manifest, CI target and release notes. Critical rules cannot be bypassed by arbitrary metadata, scripts or AI. Customer identity and commercial commitment ownership remain explicit and non-overlapping.

## 14. Phase 9 — AI-native layer — Planned

AI is an Actor, not an infrastructure shortcut.

### Deliverables

- AI Gateway and model routing by tenant, data class, purpose, residency and cost.
- Permission-scoped tools generated from Capability Registry.
- Permission-filtered retrieval.
- Approval flows, reversible actions and budgets.
- Prompt-injection, leakage and correctness evaluations.
- Complete actor, tool, model and cost audit evidence.

### Gate

AI has no alternate mutation path. Restricted data is default-deny for external providers. Every tool call repeats live authorization before side effects.

## 15. Phase 10 — Marketplace — Planned

### Deliverables

Signed packages, publisher identity, WASM sandbox, SBOM and provenance verification, vulnerability policy, capability/data/network/secret grants, quotas, kill switch and safe upgrade, rollback and uninstall.

### Gate

Untrusted or policy-violating modules cannot install. Marketplace code cannot access resources outside explicit host grants.

## 16. Phase 11 — Enterprise and production proof — Planned / continuous

### Deliverables

SSO/OIDC/SAML, SCIM, tenant key hierarchy, field encryption, legal hold, WORM audit export, privacy deletion, crypto-shredding, backup/PITR, tenant restore, tenant mobility, data residency, SBOM/dependency/secret scans, penetration/load/chaos tests, SLOs and runbooks.

### Gate

The platform is production-ready only after documented restore, failover, incident-response, key-rotation, privacy-request, marketplace-kill-switch and tenant-mobility drills pass under measured SLOs.
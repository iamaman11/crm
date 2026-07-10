# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**  
Parent epic: [#2](https://github.com/iamaman11/crm/issues/2)  
Governing rules: [`SYSTEM_INVARIANTS.md`](SYSTEM_INVARIANTS.md) and accepted ADRs.

## 1. Purpose

This roadmap turns the architecture specification into a controlled delivery sequence. It is not a feature wishlist. Every phase establishes platform guarantees required by later phases, has explicit acceptance gates, and must preserve all system invariants.

The target is a universal modular expert CRM platform where first-party and marketplace modules can be developed, tested, released, installed, activated, upgraded, suspended and removed independently without direct infrastructure access or cross-module state mutation.

Universal means that Sales is not allowed to become the owner of customer identity, catalog, pricing, order, contract, subscription, service, communication or billing state. Those domains require explicit owner modules and versioned integration boundaries.

## 2. Delivery rules

1. Work is delivered through small reviewable pull requests linked to a roadmap issue.
2. Contract CI, Governance CI and Rust CI must remain green before merge; Database CI is mandatory whenever runtime, SQL, migrations or PostgreSQL behavior changes.
3. Published contracts, policies, metadata and module versions are immutable.
4. A phase is complete only when its acceptance gates are automated or supported by a documented operational drill.
5. New behavior must enter through a versioned capability and produce typed audit evidence.
6. Business modules may depend only on stable platform contracts and governed SDK ports.
7. Search, analytics, caches and projections remain rebuildable and non-authoritative.
8. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements, not later enhancements.
9. Technical debt discovered by a gate is fixed or explicitly recorded before dependent work begins.
10. The roadmap and GitHub issues are updated in the same PR when scope or sequencing changes.
11. Exact money, time, identity, lifecycle and authorization semantics must be represented by typed contracts rather than convention or free-form strings.
12. No milestone may claim the complete CRM product is finished while required owner domains or production gates remain open.

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
| 6 | [#9](https://github.com/iamaman11/crm/issues/9) | Sales + Activities + link-module vertical slice | **In progress** | #8 |
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, projections and Admin Studio foundation | **Planned** | #9 |
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

### Current slice 6C — transactional audit materialization and PostgreSQL capability planners

Branch: `feature/transactional-audit-materialization-v1`.

Before production domain planners are added, the batch runtime must stop requiring planners to guess tenant audit sequence and previous hash outside the transaction. The runtime will:

1. accept deterministic audit intent from the planner;
2. acquire tenant-scoped transactional serialization;
3. read the current audit head inside the same PostgreSQL transaction;
4. assign contiguous sequence numbers and previous hashes;
5. compute chained record hashes from canonical envelopes;
6. insert audit evidence atomically with records, events and idempotency state;
7. preserve the invariant that PostgreSQL remains the first awaited operation after live authorization.

After that foundation is green, add Sales and Activities mutation planners and public gateway PostgreSQL acceptance.

### Remaining slices

- **6C:** domain-to-persistence conversion, mutation planners and public create/update/advance/complete/reminder paths.
- **6D:** permission-bound get/list query paths with stable cursor pagination.
- **6E:** `crm-sales-activities-link` event consumer using `CapabilityClient` only and deterministic event-delivery idempotency.
- **6F:** rebuildable deal-timeline and task-status projections with tenant checkpoints and replay.
- **6G:** complete cross-tenant, disable-link, replay, optimistic-conflict, rollback and projection-rebuild acceptance.

### Completion gate

Sales and Activities remain independently installable and functional when the link module is disabled. Duplicate source-event delivery produces no duplicate task or projection effect. Every mutation follows the authenticated gateway and commits state only with idempotency, outbox and audit evidence.

## 12. Phase 7 — Search, projections and Admin Studio — Planned

### Deliverables

- Idempotent projection workers, checkpoints, retries and rebuild.
- Tenant- and permission-aware search with reindexing.
- Object, field, relationship, layout, view, pipeline, permission and workflow builders.
- Impact reports, immutable versions and rollback UI.
- Typed UI extension runtime with safe fallback.

### Gate

Deleting search or projections cannot destroy authoritative data. Permission changes cannot leak stale results. Admin changes are validated, audited and reversible.

## 13. Phase 8 — Expert modules and product experience — Planned

Parallel workstreams start only after the Module SDK, first vertical slice and search/Admin foundations prove the boundaries.

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

### Final gate

A production-readiness review must prove every system invariant using automated evidence or a repeatable operational drill. Restore exercises must meet declared RPO/RTO and preserve audit/event consistency.

## 17. Pull-request sequence from the current point

1. Transactional audit materialization inside the PostgreSQL batch runtime — Phase 6C foundation.
2. Sales and Activities domain-to-persistence conversion and mutation planners.
3. Public gateway PostgreSQL acceptance for Sales and Activities mutations.
4. Permission-bound get/list query paths and stable cursor pagination.
5. Sales/Activities link-module consumer and deterministic event deduplication.
6. Rebuildable deal-timeline and task-status projections.
7. Complete Phase 6 end-to-end acceptance and close #9.
8. Search and Admin Studio foundation — #10.
9. Canonical customer master — #28 — and parallel expert modules/UX — #11.
10. Product catalog, CPQ and quote-to-revenue — #29.
11. AI-native layer — #12.
12. Marketplace — #13.
13. Enterprise production proof — #14.

Each PR must state invariant impact, migration and rollback behavior, test evidence, compatibility impact and the next unblocked issue.

## 18. Current status

- Governance Foundation and Phases 0.1–5: **Complete**.
- Phase 6 / issue #9: **In progress**.
- Phase 6A typed Sales and Activities domain contracts: **Complete**, PR #26.
- Phase 6B versioned Sales and Activities Protobuf contracts: **Complete**, PR #27.
- Phase 6C transactional audit materialization and PostgreSQL planners: **In progress** on `feature/transactional-audit-materialization-v1`.
- Issues #28 and #29: **Planned mandatory universal-CRM owner domains**.
- Phases 7–11: **Planned**, subject to their prerequisite gates.
- The complete CRM product is **not yet finished**.

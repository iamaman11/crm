# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**  
Parent epic: [#2](https://github.com/iamaman11/crm/issues/2)  
Governing rules: [`SYSTEM_INVARIANTS.md`](SYSTEM_INVARIANTS.md) and accepted ADRs.

## 1. Purpose

This roadmap turns the architecture specification into a controlled delivery sequence. It is not a feature wishlist. Every phase establishes platform guarantees required by later phases, has explicit acceptance gates, and must preserve all system invariants.

The target is a modular expert CRM platform where first-party and marketplace modules can be developed, tested, released, installed, activated, upgraded, suspended and removed independently without direct infrastructure access or cross-module state mutation.

## 2. Delivery rules

1. Work is delivered through small reviewable pull requests linked to a roadmap issue.
2. Contract CI, Governance CI and Rust CI must remain green before merge.
3. Published contracts, policies, metadata and module versions are immutable.
4. A phase is complete only when its acceptance gates are automated or supported by a documented operational drill.
5. New behavior must enter through a versioned capability and produce typed audit evidence.
6. Business modules may depend only on stable platform contracts and governed SDK ports.
7. Search, analytics, caches and projections remain rebuildable.
8. Security, privacy, tenant isolation and rollback are implementation requirements, not later enhancements.
9. Technical debt discovered by a gate is fixed or explicitly recorded before dependent work begins.
10. The roadmap and GitHub issues are updated in the same PR when scope or sequencing changes.

## 3. Work states

- **Planned** — scoped but prerequisites are incomplete.
- **Ready** — prerequisites and contracts are stable enough to start.
- **In progress** — an implementation branch/PR exists.
- **Gate review** — implementation is complete and acceptance evidence is being verified.
- **Complete** — merged and all gates have passed.
- **Blocked** — a named dependency, decision or defect prevents progress.

## 4. Phase map

| Phase | Issue | Primary result | Depends on | Parallelism |
|---|---:|---|---|---|
| 0.1 | [#3](https://github.com/iamaman11/crm/issues/3) | Repository hardening and executable roadmap | Governance v1 | Sequential foundation |
| 1 | [#4](https://github.com/iamaman11/crm/issues/4) | Typed Module Manifest IR and deterministic identity | #3 | Sequential foundation |
| 2 | [#5](https://github.com/iamaman11/crm/issues/5) | Governed Module SDK and test harness | #4 | Sequential foundation |
| 3 | [#6](https://github.com/iamaman11/crm/issues/6) | Module lifecycle and registry runtime | #4, #5 | Registry/contracts can split after interfaces stabilize |
| 4 | [#7](https://github.com/iamaman11/crm/issues/7) | PostgreSQL tenant, record, outbox and audit foundation | #6 | Data, migrations and test infrastructure can split |
| 5 | [#8](https://github.com/iamaman11/crm/issues/8) | Capability execution gateway | #5, #7 | Auth, policy and transaction substreams |
| 6 | [#9](https://github.com/iamaman11/crm/issues/9) | Sales + Activities + link-module vertical slice | #8 | Three independent module teams |
| 7 | [#10](https://github.com/iamaman11/crm/issues/10) | Search, projections and Admin Studio foundation | #9 | Search and Admin Studio parallel |
| 8 | [#11](https://github.com/iamaman11/crm/issues/11) | Expert modules and product-quality UX | #5, #9, #10 | Broad parallel development |
| 9 | [#12](https://github.com/iamaman11/crm/issues/12) | AI-native governed actor/tool layer | #8, #10 | AI gateway/evals/RAG parallel |
| 10 | [#13](https://github.com/iamaman11/crm/issues/13) | Signed marketplace and WASM sandbox | #6, #8, #10 | Packaging/sandbox/review parallel |
| 11 | [#14](https://github.com/iamaman11/crm/issues/14) | Enterprise security and production proof | all critical runtime phases | Continuous hardening, final gate sequential |

## 5. Phase 0.1 — Repository hardening

### Deliverables

- This roadmap and issue hierarchy.
- Correct validation documentation and generated-artifact policy.
- Stable ownership boundaries for platform, contracts and modules.
- Pinned or bounded development dependencies.
- Documented required checks and merge policy.

### Gate

The repository must not claim stale checksums, warnings or committed artifacts that do not exist. Every future phase must have an issue, prerequisites and measurable acceptance criteria.

## 6. Phase 1 — Typed Module Manifest IR

### Runtime boundary

`module.yaml` is human-authored input only:

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

### Deliverables

- `crates/crm-module-manifest` with strict typed structures.
- Unknown-field rejection.
- Semantic validation matching the authoring validator.
- SemVer and dependency-range validation.
- Module, object, capability and event ownership indexes.
- Required-dependency cycle detection.
- Deterministic canonical serialization and digest.
- Python-to-Rust parity pipeline using emitted normalized IR and digest files.
- Positive, negative and deterministic test fixtures.

### Gate

Equivalent manifests must produce identical bytes and digest. Invalid versions, ownership conflicts, duplicate providers and dependency cycles must fail deterministically. The runtime crate must have no infrastructure dependency.

## 7. Phase 2 — Governed Module SDK

### Deliverables

- `CapabilityClient`, `RecordClient`, `RelationshipClient`, `EventPublisher`, `ModuleStateStore`, `WorkflowClient`, `FileClient`, `Clock`, `RandomSource` and `ObservabilityContext`.
- Tenant/actor/execution-context binding on every port.
- In-memory test doubles and deterministic time/randomness.
- Compile-time enforcement that public SDK APIs expose no raw DB, NATS, object storage, arbitrary HTTP, secret-store or LLM provider clients.

### Gate

A module can be independently unit-tested with no external infrastructure. All side-effecting calls require governed context.

## 8. Phase 3 — Module lifecycle and registry

### Deliverables

- Versioned lifecycle contracts: validate, publish, install, activate, suspend, upgrade, rollback, uninstall and impact report.
- Deterministic dependency resolver supporting required, optional and conflicting modules.
- Immutable module versions and tenant-scoped installations.
- Link-module support and uninstall blockers.
- Audited state machine with idempotent transitions.

### Gate

Invalid lifecycle transitions are impossible through the public API. Failed upgrades can roll back without losing retained business records.

## 9. Phase 4 — PostgreSQL foundation

### Authoritative tables

Tenants, actors, teams, module versions/installations, capability registry, metadata packages, object/field definitions, records, relationships, idempotency records, outbox events, audit records, workflow state and module state.

### Required properties

- RLS and tenant-bound transaction context.
- Optimistic aggregate versions.
- Atomic business state + idempotency + outbox + audit reference.
- Append-only audit.
- Controlled JSONB with owner/schema/version/data-class metadata.
- Clean-install and upgrade migration tests.

### Gate

Cross-tenant negative tests pass for every data boundary. Transaction fault injection proves no mutation can commit without event and audit evidence.

## 10. Phase 5 — Capability execution gateway

### Execution chain

```text
request
→ authentication
→ tenant and actor resolution
→ ExecutionContext
→ capability resolution
→ typed validation
→ live authorization
→ approval binding
→ idempotency
→ transaction
→ outbox and audit
→ typed response
```

### Gate

No state-changing API bypasses the gateway. Retries do not duplicate side effects. External behavior never depends on parsing error text.

## 11. Phase 6 — First modular proof

### Owner modules

- `crm-sales`: deal create/update/stage progression.
- `crm-activities`: task create/complete/reminder.
- `crm-sales-activities-link`: consumes `sales.deal.created` and invokes `activities.task.create`.

### Gate

Sales and Activities compile and test independently, share no source/table access, and remain functional when the link module is disabled. Duplicate delivery creates no duplicate task.

## 12. Phase 7 — Search, projections and Admin Studio

### Deliverables

- Idempotent projection workers, checkpoints, retries and rebuild.
- Tenant- and permission-aware search with reindexing.
- Object, field, relationship, layout, view, pipeline, permission and workflow builders.
- Impact reports, immutable versions and rollback UI.
- Typed UI extension runtime with safe fallback.

### Gate

Deleting search/projections cannot destroy authoritative data. Permission changes cannot leak stale results. Admin changes are validated, audited and reversible.

## 13. Phase 8 — Expert modules and product experience

Parallel workstreams may start only after Module SDK and the first vertical slice prove the boundaries.

### Expert modules

Sales, Activities, Communications, Support, Billing, Marketing, Projects/Case Management, Documents/e-sign and Analytics.

### Product experience

Global search, command palette, keyboard navigation, fast tables, saved views, bulk actions, timelines, explainable permissions, transparent automation runs, onboarding, imports, responsive/mobile behavior, accessibility and localization.

### Gate

Each module owns typed domain invariants, contracts, manifest, CI target and release notes. Critical rules cannot be bypassed by arbitrary metadata or scripting.

## 14. Phase 9 — AI-native layer

AI is an Actor, not an infrastructure shortcut.

### Deliverables

- AI Gateway and model routing by tenant, data class, purpose, residency and cost.
- Permission-scoped tools generated from Capability Registry.
- Permission-filtered retrieval.
- Approval flows, reversible actions and budgets.
- Prompt-injection, leakage and correctness evaluations.
- Complete actor/tool/model/cost audit evidence.

### Gate

AI has no alternate mutation path. Restricted data is default-deny for external providers. Every tool call repeats live authorization before side effects.

## 15. Phase 10 — Marketplace

### Deliverables

Signed packages, publisher identity, WASM sandbox, SBOM/provenance verification, vulnerability policy, capability/data/network/secret grants, quotas, kill switch and safe upgrade/rollback/uninstall.

### Gate

Untrusted or policy-violating modules cannot install. Marketplace code cannot access resources outside explicit host grants.

## 16. Phase 11 — Enterprise and production proof

### Deliverables

SSO/OIDC/SAML, SCIM, tenant key hierarchy, field encryption, legal hold, WORM audit export, privacy deletion, crypto-shredding, backup/PITR, tenant restore, tenant mobility, data residency, SBOM/dependency/secret scans, penetration/load/chaos tests, SLOs and runbooks.

### Final gate

A production-readiness review must prove every system invariant using automated evidence or a repeatable operational drill. Restore exercises must meet declared RPO/RTO and preserve audit/event consistency.

## 17. Pull-request sequence

1. Roadmap, cleanup and Typed Module Manifest IR — issues #3 and #4.
2. Module SDK — #5.
3. Lifecycle contracts and registry — #6.
4. PostgreSQL foundation — #7.
5. Capability execution pipeline — #8.
6. Sales/Activities/link vertical slice — #9.
7. Search and Admin Studio foundation — #10.
8. Parallel expert modules and UX — #11.
9. AI-native layer — #12.
10. Marketplace — #13.
11. Enterprise proof — #14.

Each PR must state the invariant impact, migration/rollback behavior, test evidence, compatibility impact and the next unblocked issue.

## 18. Current status

- Governance Foundation v1: **Complete**.
- Phase 0.1: **In progress** on issue #3.
- Phase 1: **In progress** on issue #4.
- Phases 2–11: **Planned**, subject to prerequisite gates.

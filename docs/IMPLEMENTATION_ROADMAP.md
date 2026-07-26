# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose

This roadmap defines dependency order for a universal modular expert CRM platform. It is not a feature wishlist or a historical status log.

A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by unchanged exact-head evidence. Every mutable aggregate retains one authoritative owner; search, analytics, projections and caches remain rebuildable and non-authoritative.

## 2. Delivery rules

1. Deliver coherent reviewable packets linked to roadmap issues.
2. Preserve one authoritative owner for every mutable aggregate.
3. Enter state-changing behavior through exact versioned capabilities with typed audit evidence.
4. Never access another module's storage directly.
5. Treat security, privacy, tenant isolation, compatibility and rollback as implementation requirements.
6. Require real composition, persistence and process evidence before runtime claims.
7. Invalidate exact-SHA evidence after every source or documentation change until applicable checks rerun.
8. Synchronize roadmap, phase plan, status, catalog, issues and PR descriptions under `DELIVERY_GOVERNANCE.md`.
9. Do not mark the universal CRM product complete while required capability families remain incomplete.

## 3. Work states

- Planned
- Ready
- In progress
- Gate review
- Complete
- Blocked
- Superseded

Only merged `main` work may be represented as **Complete**.

## 4. Phase map

| Phase | Issue | Primary result | State | Depends on |
|---|---:|---|---|---|
| 0.1 | #3 | Repository hardening and executable roadmap | **Complete** | Governance v1 |
| 1 | #4 | Typed Module Manifest IR and deterministic identity | **Complete** | #3 |
| 2 | #5 | Governed Module SDK and test harness | **Complete** | #4 |
| 3 | #6 | Module lifecycle and registry runtime | **Complete** | #4, #5 |
| 4 | #7 | PostgreSQL tenant, record, artifact, outbox and audit foundation | **Complete** | #6 |
| 5 | #8 | Capability execution gateway | **Complete** | #5, #7 |
| 6 | #9 | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | #10 | Search, generalized projections, Admin Studio, product shell and UI-extension isolation | **Complete** | #9 |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** | #5, #9, #10 |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** | #9, #10 |
| 8B | #29 | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | completed 8A baseline |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** | mature domain capabilities |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** | #6, #8, #10 |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous hardening** | all critical phases |

## 5. Completed foundation

### Phases 0.1–5 — Complete

Repository governance, immutable module identity, governed Module SDK, module lifecycle, PostgreSQL tenant/RLS/record/artifact/idempotency/outbox/audit foundations and exact-version capability execution are merged.

### Phase 6 — Complete

Independent Sales and Activities owners, optional governed link, projections and deployable application composition are merged.

### Phase 7 — Complete

Generalized projections, permission-aware search, typed product shell, metadata/Admin Studio and trusted UI-extension isolation are merged.

### Native application-composition integrity — Complete

Issue #134 / PR #135 established module-owned exact-coordinate routing, tenant activation, pre-authorization cross-owner semantics, deterministic worker contributions and production-route parity.

## 6. Phase 8A — canonical customer master and governed customer-data lifecycle

State: **In progress**  
Parent issue: #28

Completed packets:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import sources, parsing/validation, resumable Party import and recovery;
- **8A.8** — governed Party export, immutable selection/manifests, deterministic artifacts and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

### 6.1 Phase 8A.11 Customer Privacy — current merged boundary

Issue #126 is **In progress**.

Merged runtime inventory:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

Accepted owner-scope implementations:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186, accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c`, merge `509eb304a76055c9f49b0beed3b007963a91cb22`, 25/25 permanent workflows.

All nine owner coordinates remain contract-only/non-runtime. Accepted owner implementations add no public ingress, application registration or Customer Privacy worker.

### 6.2 Active sequence

1. **Customer Data Operations owner slice — Ready:** implement `customer_data.privacy.scope.contribute@1.0.0` under `CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md`.
2. **Data Quality owner slice:** freeze and implement its owner-specific subject-evidence boundary.
3. **Customer Enrichment owner slice:** freeze and implement provider/provenance/review/application evidence scope.
4. **Complete nine-owner set:** require all owner gates before production discovery.
5. **Scope discovery and immutable snapshot:** prove owner availability/staleness, deterministic completeness and immutable scope evidence.
6. **Deterministic planning and reads:** prove owner/data-class actions and permission-aware plan/outcome disclosure.
7. **Approval and immediate restrictions:** preserve deny-only enforcement and final subject locking.
8. **Legal hold and retention:** prove legal-hold > mandatory-retention > approved-action precedence.
9. **Resumable execution and recovery:** prove exact owner capabilities, idempotency, checkpoints and crash-window reconciliation.
10. **Access/export and deletion/anonymization:** use governed artifacts and owner-specific actions without cross-owner storage bypass.
11. **Tombstone and convergence:** prove Party tombstone/no-orphan behavior and projection/search/cache convergence.
12. **Worker and end-to-end acceptance:** prove disable/uninstall fail-closed behavior and complete lifecycle processing.
13. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged.
14. **Phase 8B / #29:** start only from the completed Phase 8A baseline.

### 6.3 Next bounded owner slice — Customer Data Operations

The packet covers subject-level:

- `customer_data.import_row`;
- `customer_data.export_selection_item`;
- `customer_data.export_execution_stage`;
- `customer_data.export_execution_outcome`.

It must:

- strictly rehydrate exact owner persistence envelopes;
- resolve historical Party references to the accepted canonical subject under bounded topology evidence;
- join export stage/outcome evidence only through authoritative selection identity;
- exclude multi-subject jobs, boundaries, progress and complete artifacts from automatic subject scope;
- paginate deterministically across all four resource families;
- emit reference-only evidence;
- prove clean/rollback/schema-removal/reapply PostgreSQL acceptance and zero writes;
- remain non-runtime with no shared-support behavior expansion.

## 7. Phase 8B — Product Catalog, Pricing, CPQ and Quote-to-Revenue

State: **Planned; blocked on completed Phase 8A baseline**.

Required owner domains include:

- Product Catalog;
- Price Books and Pricing;
- CPQ and immutable quote revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage;
- governed billing/ERP/payment/tax/fulfillment boundaries.

These domains must not be absorbed into Sales.

## 8. Later expert domains

Planned work includes broader Sales/Activities, omnichannel, Marketing, Service/Knowledge/Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

## 9. Completion rule

Current product-complete expert modules: **0**.

A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists. Completion requires the defined domain breadth, governed APIs, persistence, authorization, audit, product workflow and production/operational evidence.

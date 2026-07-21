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

This roadmap defines the dependency order for a universal modular expert CRM platform. It is not a feature wishlist or a second status page.

A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by the required exact-head automated or operational evidence. Universal does not mean one giant Sales module: customer identity, communications, service, catalog, pricing, commercial commitments, subscriptions, billing, consent and other independent domains retain explicit versioned ownership.

## 2. Delivery rules

1. Work is delivered as coherent reviewable packets linked to roadmap issues.
2. Every mutable aggregate has exactly one authoritative owner module.
3. State-changing behavior enters through an exact versioned capability and produces typed audit evidence.
4. Search, analytics, caches and projections remain rebuildable and non-authoritative.
5. Published contracts, policies, metadata and module versions are immutable.
6. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements.
7. Business modules use governed SDK/platform boundaries and never access another module’s storage directly.
8. Exact money, time, identity, lifecycle and authorization semantics use typed contracts rather than conventions.
9. A backend packet is not production-ready while real composition, persistence or process acceptance is missing.
10. Every source or documentation change invalidates earlier exact-SHA gate evidence until applicable checks rerun.
11. Roadmap, status, phase plan, module catalog, issues and PR descriptions are synchronized under `DELIVERY_GOVERNANCE.md`.
12. No milestone may claim the universal CRM product is complete while required capability families remain unimplemented or unclassified.

## 3. Work states

- Planned
- Ready
- In progress
- Gate review
- Complete
- Blocked
- Superseded

Only merged work may be represented as **Complete** in `main` documentation.

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

## 5. Completed platform foundation

### Phases 0.1–5 — Complete

Delivered repository governance, immutable module identity, governed Module SDK, module lifecycle, PostgreSQL tenant/RLS/record/artifact/idempotency/outbox/audit foundations and exact-version authenticated capability execution.

### Phase 6 — Complete

Delivered independent Sales `Deal` and Activities `Task` owners, versioned contracts, PostgreSQL-backed mutation/query paths, authenticated HTTP/gRPC ingress, governed event delivery, the optional Sales–Activities link, rebuildable projections and a deployable `crm-api` process.

### Phase 7 — Complete

Delivered golden module tooling, generalized projection runtime, permission-aware global search, typed web shell, immutable tenant-authorized metadata, Admin Studio publication/rollback and trusted-code UI-extension failure isolation.

### Native application-composition integrity — Complete

Issue #134 / PR #135 / merge `023fa5ef1d510d5bcc32222c739e6d58e5696fb8` established module-owned exact-coordinate routing, durable tenant activation, pre-authorization cross-owner semantics, deterministic worker contributions, exact production-route parity, immutable compatibility gates and production contribution scaffolding.

## 6. Phase 8A — canonical customer master and governed customer-data lifecycle

State: **In progress**  
Parent issue: #28

Completed packets:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7** — governed immutable import sources, server-side parsing/validation, resumable Party import and crash/retry recovery (#120 / PR #121).
- **8A.8** — governed Party export jobs, immutable selection/manifests, deterministic artifacts, reconciliation and both crash-window recoveries (#123 / PR #130).
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132; merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`).
- **8A.10** — Governed Customer Enrichment and Provenance (#125 / PR #137; accepted source `f92d101206886e3ceaf94d0e56e52580cec21093`; merge `150e44b95d9dbdc08c1792563de03ec73f34aed1`).

Active sequence:

1. **8A.11 / #126 — In progress:** ownership/enforcement architecture freeze in draft PR #140, followed by Customer Privacy runtime implementation.
2. **Phase 8A closure:** only after 8A.11 is merged and the full customer-master baseline is reconciled.
3. **8B / #29:** starts only from the completed Phase 8A baseline.

### Phase 8A.10 accepted boundary

The frozen inventory is exactly **6 public mutations + 6 permission-aware queries + 5 activation-gated worker-only coordinates**. All 17 manifest-bound coordinates are public runtime or worker runtime; no completed Customer Enrichment coordinate remains non-runtime.

The merged packet includes:

- immutable provider-profile, mapping, request, response receipt/conflict, suggestion, review, usage and application evidence;
- exact registry HTTP transport outside the pure module core;
- tenant-bound secret resolution, endpoint allowlisting, bounded network behavior, quota and circuit controls;
- independent live authorization for dispatch, response, materialization and owner application;
- deterministic replay, exact/semantic duplicate reconciliation and fail-closed response conflicts;
- governed review and exact Party owner-capability application;
- permission-aware reads, declarative redaction, activation shutdown and cross-tenant concealment;
- transaction-scoped immutable provider-profile and exact Party-version guards;
- FORCE RLS, migration rollback/reapply and fresh-PostgreSQL process evidence;
- permanent real-`crm-api` public-ingress acceptance and dedicated provider/materialization/review/application worker-process acceptance;
- exact background phase order 240 → 245 → 250 and disable/uninstall shutdown.

### Phase 8A.11 active architecture freeze

Issue #126 / draft PR #140 freezes `crm.customer-privacy` as the privacy case and orchestration owner before contract expansion.

The coordinator owns privacy cases, verified subject binding, scope snapshots, current restrictions, customer-data legal holds, retention decisions, deterministic action plans, per-owner attempts/outcomes, orchestration checkpoints, governed export references and convergence evidence. Existing modules retain all authoritative Party, Account, Contact Point, Relationship, Consent, Identity Resolution, import/export, Data Quality and Enrichment values.

The initial inventory is exactly:

- **9 public mutations**;
- **7 permission-aware public queries**;
- **9 trusted worker/internal coordinates** in deterministic phases 260 → 270 → 280 → 290;
- **1 reasoned non-runtime crypto-shredding coordinate**.

The freeze requires:

- subject and owner-resource discovery without unauthorized bulk disclosure;
- access/export integrated with existing governed Customer Data Operations artifacts;
- immediate processing and communication restrictions enforced by a shared tenant + canonical Party lock;
- deletion/anonymization planning by authoritative owner and data class;
- legal-hold and retention precedence with immutable blocking reasons;
- resumable per-owner orchestration with deterministic attempts/outcomes;
- search, projection and cache convergence without treating derived state as authority;
- non-reusable erased Party tombstones and no orphaned authoritative references;
- immutable audit/Consent/identity/provenance evidence preservation where deletion is prohibited;
- fresh-PostgreSQL, real-process, cross-tenant, migration and exact-head acceptance.

The architecture freeze does not claim Protobuf, manifest, migration, route or runtime implementation.

Phase 8A is complete only when privacy access/export/restriction/deletion/legal-hold interactions are merged and reconciled with Consent, Identity Resolution, Import/Export, Data Quality and Customer Enrichment.

## 7. Phase 8B — product catalog, pricing, CPQ and quote-to-revenue

State: **Planned**  
Issue: #29

Required owner domains include Product Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements and governed billing/ERP/payment/tax/fulfillment integration boundaries. Catalog, pricing and commercial commitment ownership must not be absorbed into Sales.

## 8. Additional expert-product waves

After stable prerequisite domains, Phase 8 continues with Sales/Activities expert expansion, communications and omnichannel, Service/Support/Knowledge/Field Service, Marketing, Customer Success/PRM, projects/configurable work, documents/e-signature, analytics/performance management, workflow/approvals/collaboration and complete responsive accessible product UX.

Each authoritative domain receives an explicit owner and cannot be hidden inside generic metadata or a giant Sales module.

## 9. Later platform phases

### Phase 9 — AI-native CRM

AI is an authenticated audited Actor using permission-scoped governed tools. Required outcomes include tenant/data-class/purpose/residency/cost-aware routing, permission-filtered retrieval, live authorization, approvals, budgets/failure controls and security/correctness evaluations.

### Phase 10 — signed marketplace and sandbox

Required outcomes include signed packages, publisher identity, dependency/compatibility resolution, SBOM/provenance policy, explicit grants, sandboxed untrusted execution, quotas, timeouts, kill switch and safe lifecycle operations.

### Phase 11 — enterprise security and production proof

Required outcomes include OIDC/SAML, SCIM, enterprise authorization, key hierarchy/encryption, WORM audit export, privacy/legal-hold integration, backup/PITR/restore, residency, supply-chain/security testing, load/chaos proof, SLOs, alerting, incident response and runbooks.

## 10. Immediate authoritative delivery sequence

1. Validate and merge the Phase 8A.11 ownership/enforcement architecture freeze in draft PR #140 on one unchanged exact SHA.
2. Scaffold `crm.customer-privacy` and publish immutable contracts only from the accepted freeze.
3. Implement FORCE RLS persistence, shared-lock live enforcement, owner contributions, governed privacy export, retention/legal-hold planning, replay-safe execution and convergence evidence.
4. Close Phase 8A only after the full merged customer-master acceptance baseline is proven.
5. Begin Phase 8B / #29 from the completed customer-master baseline.

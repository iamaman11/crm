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

This roadmap defines the dependency order for a universal modular expert CRM platform. It is not a feature wishlist and it is not a second status page.

Every phase establishes guarantees required by later phases. A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by required automated or operational evidence.

Universal means that Sales is not allowed to become the owner of customer identity, communications, service, catalog, pricing, quotes, orders, contracts, subscriptions, billing, consent or other independent business domains. Authoritative ownership remains explicit and versioned.

## 2. Delivery rules

1. Work is delivered as coherent reviewable packets linked to roadmap issues.
2. Every mutable aggregate has exactly one authoritative owner module.
3. New state-changing behavior enters through an exact versioned capability and produces typed audit evidence.
4. Search, analytics, caches and projections remain rebuildable and non-authoritative.
5. Published contracts, policies, metadata and module versions are immutable.
6. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements.
7. Business modules use governed SDK/platform boundaries and do not access another module’s storage directly.
8. Exact money, time, identity, lifecycle and authorization semantics use typed contracts rather than conventions.
9. A backend packet is not production-complete while real application composition, persistence or process acceptance is missing.
10. Frontend and backend evolve as end-to-end vertical slices where the packet has a user-facing surface.
11. Every source-changing or documentation-changing commit invalidates previous exact-SHA gate evidence until applicable checks rerun on the new head.
12. Roadmap, status, phase plan, module catalog, issues and PR descriptions are synchronized under `DELIVERY_GOVERNANCE.md`.
13. No milestone may claim the universal CRM product is complete while required capability families remain unimplemented or unclassified.

## 3. Work states

The normative state vocabulary is defined in `DELIVERY_GOVERNANCE.md`:

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
| 4 | #7 | PostgreSQL tenant, record, file/artifact, outbox and audit foundation | **Complete** | #6 |
| 5 | #8 | Capability execution gateway | **Complete** | #5, #7 |
| 6 | #9 | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | #10 | Search, generalized projections, Admin Studio, product shell and UI-extension isolation | **Complete** | #9 |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** | #5, #9, #10 |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** | #9, #10 |
| 8B | #29 | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | completed 8A customer-master baseline |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** | #8, #10 and mature domain capabilities |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** | #6, #8, #10 |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous hardening** | all critical runtime phases |

## 5. Completed platform foundation

### Phases 0.1–5 — Complete

Delivered repository governance, immutable module identity, governed Module SDK, module lifecycle, PostgreSQL tenant/RLS/record/file/idempotency/outbox/audit foundations and exact-version authenticated capability execution.

### Phase 6 — Complete

Delivered independent Sales `Deal` and Activities `Task` owner aggregates, versioned contracts, PostgreSQL-backed mutation/query paths, authenticated HTTP/gRPC ingress, governed event delivery, the optional Sales–Activities link module, rebuildable projections and a real deployable `crm-api` process.

### Phase 7 — Complete

Delivered golden module tooling, generalized projection runtime, permission-aware global search, typed web product shell, immutable tenant-authorized metadata, Admin Studio publication/rollback and trusted-code UI-extension failure isolation.

### Native application-composition integrity — Complete

Issue #134 / PR #135 / merge `023fa5ef1d510d5bcc32222c739e6d58e5696fb8` completed module-owned exact-coordinate routing, durable tenant activation, pre-authorization cross-owner validation, deterministic worker contributions, exact production-route parity, immutable compatibility gates and production contribution scaffolding. This is the required baseline for all subsequent Phase 8 modules.

Untrusted third-party execution remains Phase 10.

## 6. Phase 8 — expert owner domains and product experience

Phase 8 is the broad product program. It is delivered as independent owner-domain and product-surface waves rather than one mega-branch.

Detailed sequencing lives in `PHASE8_DELIVERY_PLAN.md`.

### 8A — canonical customer master, identity, consent and customer-data lifecycle

State: **In progress**
Parent issue: #28

Completed packets:

- **8A.1** — canonical customer references and owner foundations (#92 / PR #93).
- **8A.2a** — Party create/get (#94 / PR #95).
- **8A.2b** — Party update/list (#96 / PR #97).
- **8A.2c** — Party search/customer discovery (#98 / PR #99).
- **8A.3a** — Account lifecycle and Party associations (#101 / PR #102).
- **8A.3b** — Contact Point lifecycle, verification and preference (#103 / PR #104).
- **8A.3c** — Party Relationship lifecycle and hierarchy foundations (#108 / PR #109).
- **8A.3d** — Customer 360 read composition (#110 / PR #111).
- **8A.4** — Consent and Communication Authorization (#112 / PR #113).
- **8A.5** — Identity Resolution duplicate-candidate cases and reviewer decisions (#114 / PR #115).
- **8A.6** — reversible merge/unmerge, immutable lineage, provenance and survivorship (#116 / PR #119; merge `d5cb4502ad0c49158e0789d8749dc09160da7895`).
- **8A.7** — governed immutable import sources, exact server-side parsing/validation, resumable Party import execution, retry recovery and crash/restart process acceptance (#120 / PR #121; merge `5f60f24d6d3a3bb46720658f4e98d4a7ebb15637`).
- **8A.8** — governed Party export jobs, immutable selection/manifests, deterministic artifacts, exact reconciliation, both execution crash-window recoveries and live-authorized audited artifact disclosure (#123 / PR #130; merge `0e7f9889362533446cc65d95dcf7969a60086a57`).
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132; merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`).

Active and planned sequence:

1. **8A.10 / #125 — Ready:** Governed Customer Enrichment and Provenance is the next customer-master production packet, based on completed architecture integrity issue #134 / PR #135.
2. **8A.11 / #126 — Planned:** Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold.

Phase 8A is complete only when the customer-master acceptance baseline covers:

- stable canonical identities and references;
- consent-aware live authorization;
- explainable duplicate candidates;
- reversible merge/unmerge and immutable provenance;
- deterministic import and export with resumability and reconciliation;
- data-quality and stewardship evidence;
- governed enrichment provenance;
- privacy access/export/restriction/deletion/legal-hold interaction proof;
- tenant isolation, migrations, compatibility, process acceptance and performance appropriate to the maturity claim.

#### Complete 8A.9 delivered boundary

The merged packet introduces a distinct `crm.data-quality` owner/coordinator for long-lived quality-governance state. It owns immutable/versioned quality rule sets and completeness profiles, exact source-version-bound outcomes/findings/observations/completeness results, stewardship lifecycle and assignment evidence, immutable remediation attempts and bounded evaluation coordination evidence.

It does not own or copy authoritative mutable Party, Account, Contact Point, Party Relationship, Consent or Identity Resolution values. Authoritative Party values are read only through governed owner/query composition, and display-name remediation changes Party state only through the exact `parties.party.update@1.0.0` capability with live authorization, optimistic concurrency, deterministic idempotency and audit evidence.

The merged 8A.9 packet proves:

- immutable published rule/evaluator semantics and deterministic replay;
- a bounded exact evaluator vocabulary with no arbitrary SQL, user code, filesystem or arbitrary network execution;
- exact owner/resource/resource-version evidence for every outcome, finding, observation and completeness result;
- deterministic logical finding identity and reevaluation without duplicate current findings;
- explicit historical lifecycle for open, acknowledged, waived, remediated and reopened evidence without deleting evaluation history;
- deterministic integer completeness scoring with exact component/outcome lineage and reconciliation;
- permission-aware finding, completeness and stewardship disclosure with signed context-bound pagination;
- stewardship assign, acknowledge and waive mutations with optimistic finding-version and exact-current-observation preconditions;
- stale source-version/remediation conflict proof;
- no direct cross-owner storage reads or writes;
- governed Party display-name remediation with immutable attempt evidence and exact target replay after the target-success/outcome-missing crash window;
- bounded scans, batches, payloads and per-tenant operational limits;
- process restart/retry without duplicate findings, assignments, remediation attempts or Party side effects;
- FORCE RLS, live authorization, field redaction and safe cross-tenant non-disclosure;
- migration clean apply and repository database gates;
- fresh-PostgreSQL real `crm-api` process acceptance;
- final unchanged source-authored candidate `c066c278edd75b5f78bbfcead792d34164c76ff5` with all 15 applicable workflows green before merge.

### 8B — product catalog, pricing, CPQ and quote-to-revenue

State: **Planned**
Issue: #29

Required owner domains include Product Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements and governed billing/ERP/payment/tax/fulfillment integration boundaries.

Catalog, pricing and commercial commitment ownership must not be absorbed into Sales.

### Additional Phase 8 waves

After or alongside stable prerequisite domains, Phase 8 continues with:

- Sales and Activities expert expansion;
- communications and omnichannel interaction history;
- Service/Support, Knowledge and optional Field Service;
- Marketing automation, segmentation, journeys and attribution;
- Customer Success and optional Partner Relationship Management;
- projects/configurable work, documents and e-signature;
- analytics, reporting and performance management;
- workflow, approvals, human tasks and collaboration;
- product completeness: responsive/mobile UX, accessibility, localization, onboarding, saved views, bulk actions, offline/retry states and critical browser E2E.

Each authoritative domain receives an explicit owner and cannot be hidden inside generic metadata or a giant Sales module.

## 7. Phase 9 — AI-native CRM

State: **Planned**
Issue: #12

AI is an authenticated audited Actor using permission-scoped governed tools. It has no alternate identity-merge, consent or mutation path.

Required outcomes include tenant/data-class/purpose/residency/cost-aware model routing, permission-filtered retrieval, live authorization, approval flows, budgets/failure controls, complete audit evidence and security/correctness evaluations.

## 8. Phase 10 — signed marketplace and sandbox

State: **Planned**
Issue: #13

Required outcomes include signed packages, publisher identity, dependency/compatibility resolution, SBOM/provenance policy, explicit grants, sandboxed untrusted execution, quotas, timeouts, kill switch and safe lifecycle operations.

## 9. Phase 11 — enterprise security and production proof

State: **Planned / continuous hardening**
Issue: #14

Required outcomes include OIDC/SAML, SCIM, enterprise authorization, key hierarchy/encryption, WORM audit export, privacy/legal-hold integration, backup/PITR/restore, residency, supply-chain/security testing, load/chaos proof, SLOs, alerting, incident response and runbooks.

Enterprise claims require automated and operational evidence, not configuration placeholders.

## 10. Immediate authoritative delivery sequence

1. Start #125 from accepted native-composition baseline `023fa5ef1d510d5bcc32222c739e6d58e5696fb8` as the single active customer-master production packet.
2. Freeze enrichment provider, secret-handle, mapping, provenance, review and exact owner-capability application boundaries before publishing public contracts.
3. Deliver #125 and then #126 in dependency order.
4. Close Phase 8A only after its full merged acceptance baseline is proven.
5. Begin Phase 8B / #29 from the completed customer-master baseline.
6. Continue other Phase 8 waves through explicit owner-domain packets while Phase 11 hardening remains continuous.
7. Begin Phase 9 and Phase 10 only through their governed boundaries; neither may bypass domain ownership or platform invariants.

## 11. Documentation hygiene

When implementation state changes, synchronize under `DELIVERY_GOVERNANCE.md`:

- `IMPLEMENTATION_ROADMAP.md` — phase and dependency sequence;
- `PROJECT_STATUS.md` — current concise state;
- `PHASE8_DELIVERY_PLAN.md` — detailed active Phase 8 packet state;
- `MODULE_CATALOG.md` — business-module readiness/count where justified;
- parent and packet issues;
- active PR body and exact validation state.

`README.md` remains stable orientation and must not become a second manually maintained roadmap.

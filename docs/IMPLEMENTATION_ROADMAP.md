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

Every phase establishes guarantees required by later phases. A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by the required automated or operational evidence.

Universal means that Sales is not allowed to become the owner of customer identity, communications, service, catalog, pricing, quotes, orders, contracts, subscriptions, billing, consent or other independent business domains. Authoritative ownership remains explicit and versioned.

## 2. Delivery rules

1. Work is delivered as coherent reviewable packets linked to roadmap issues.
2. Every mutable aggregate has exactly one authoritative owner module.
3. New state-changing behavior enters through an exact versioned capability and produces typed audit evidence.
4. Search, analytics, caches and projections remain rebuildable and non-authoritative.
5. Published contracts, policies, metadata and module versions are immutable.
6. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements, not later cosmetics.
7. Business modules use governed SDK/platform boundaries and do not access another module’s storage.
8. Exact money, time, identity, lifecycle and authorization semantics use typed contracts rather than conventions.
9. A backend packet is not production-complete while its real application composition, persistence or process acceptance is missing.
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
| 4 | #7 | PostgreSQL tenant, record, outbox and audit foundation | **Complete** | #6 |
| 5 | #8 | Capability execution gateway | **Complete** | #5, #7 |
| 6 | #9 | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | #10 | Search, generalized projections, Admin Studio, product shell and UI-extension isolation | **Complete** | #9 |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** | #5, #9, #10 |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** | #9, #10 |
| 8B | #29 | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | stable 8A customer references |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** | #8, #10 and mature domain capabilities |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** | #6, #8, #10 |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous hardening** | all critical runtime phases |

## 5. Completed platform foundation

### Phases 0.1–5 — Complete

Delivered:

- repository governance and executable architecture checks;
- strict typed Module Manifest IR and deterministic immutable identity;
- governed Module SDK and test harness;
- module publication/install/activate/suspend/upgrade/rollback/uninstall lifecycle;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundations;
- authenticated capability execution with exact-version routing, validation, live authorization, transactional execution and typed safe results.

### Phase 6 — Complete

Delivered the first modular production proof:

- independent Sales `Deal` and Activities `Task` owner aggregates;
- versioned contracts and PostgreSQL-backed mutation/query paths;
- authenticated HTTP/gRPC ingress;
- event delivery and the optional Sales–Activities link module;
- rebuildable projections;
- real application composition and deployable `crm-api` process acceptance.

### Phase 7 — Complete

Delivered:

- golden module tooling;
- generalized projection runtime;
- permission-aware global search;
- typed web product shell and governed browser-client boundary;
- immutable tenant-authorized typed metadata;
- durable metadata persistence and rollback;
- governed metadata API and application composition;
- Admin Studio publish/impact/activate/rollback workflow;
- typed trusted-code UI-extension runtime with failure isolation.

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
- **8A.6** — reversible merge/unmerge, immutable lineage, provenance and survivorship (#116 / merged PR #119; merge commit `d5cb4502ad0c49158e0789d8749dc09160da7895`).

Active and planned sequence:

1. **8A.7 / #120 — In progress:** Customer Import Jobs, Versioned Mappings and Resumable Execution. Draft PR #121 is the single active customer-master production packet.
2. **8A.8 / #123 — Planned:** Customer Export Jobs, Artifacts and Reconciliation Evidence.
3. **8A.9 / #124 — Planned:** Customer Data Quality Rules, Completeness and Stewardship.
4. **8A.10 / #125 — Planned:** Governed Customer Enrichment and Provenance.
5. **8A.11 / #126 — Planned:** Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold.

Phase 8A is complete only when the customer-master acceptance baseline covers:

- stable canonical identities and references;
- consent-aware live authorization;
- explainable duplicate candidates;
- reversible merge/unmerge and immutable provenance;
- deterministic import and export with resumability/reconciliation;
- data-quality and stewardship evidence;
- governed enrichment provenance;
- privacy access/export/restriction/deletion/legal-hold interaction proof;
- tenant isolation, migrations, compatibility, process acceptance and performance appropriate to the maturity claim.

### 8B — product catalog, pricing, CPQ and quote-to-revenue

State: **Planned**  
Issue: #29

Required owner domains include:

- Product Catalog;
- Price Books and Pricing;
- CPQ/configuration and pricing explanation;
- Quotes and immutable revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage references;
- governed billing/ERP/payment/tax/fulfillment integration boundaries.

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

AI is an authenticated audited Actor, not an infrastructure shortcut.

Required outcomes:

- tenant/data-class/purpose/residency/cost-aware model routing;
- permission-scoped tools generated from governed capability/query registries;
- permission-filtered retrieval;
- live authorization before retrieval and side effects;
- approval flows and reversible actions where supported;
- budgets, latency/provider-failure controls and complete audit evidence;
- prompt-injection, data-leakage, hallucination and tool-correctness evaluations.

AI has no alternate identity-merge, consent or mutation path.

## 8. Phase 10 — signed marketplace and sandbox

State: **Planned**  
Issue: #13

Required outcomes:

- signed packages and publisher identity;
- dependency/compatibility resolution;
- SBOM/provenance and vulnerability policy;
- explicit capability/data/network/secret grants;
- sandboxed untrusted execution, planned as WASM;
- quotas, timeouts and kill switch;
- safe install/upgrade/rollback/suspend/uninstall lifecycle.

## 9. Phase 11 — enterprise security and production proof

State: **Planned / continuous hardening**  
Issue: #14

Required outcomes include:

- OIDC/SAML and SCIM;
- enterprise authorization, delegation and separation of duties;
- tenant key hierarchy and field/data-class encryption where required;
- WORM audit export;
- privacy lifecycle and legal hold;
- backup/PITR/tenant restore and mobility;
- data residency;
- SBOM/dependency/secret scanning;
- penetration, load and chaos testing;
- SLOs, alerting, incident response and operational runbooks.

Enterprise claims require automated and operational evidence, not configuration placeholders.

## 10. Immediate authoritative delivery sequence

1. Adopt/merge documentation-governance synchronization issue #122.
2. Close superseded PR #118 so there is only one historical Phase 8A.6 implementation path.
3. Keep #120 / PR #121 as the single active customer-master production packet.
4. Complete 8A.7 contracts, adapters, PostgreSQL/runtime composition, process acceptance and exact-head gate.
5. Deliver #123, #124, #125 and #126 in dependency order.
6. Begin Phase 8B / #29 from the stable customer-master baseline.
7. Continue other Phase 8 waves through explicit owner-domain packets while Phase 11 hardening remains continuous.
8. Begin Phase 9 and Phase 10 only through their governed boundaries; neither may bypass domain ownership or platform invariants.

## 11. Documentation hygiene

When implementation state changes, synchronize under `DELIVERY_GOVERNANCE.md`:

- `IMPLEMENTATION_ROADMAP.md` — phase and dependency sequence;
- `PROJECT_STATUS.md` — current concise state;
- `PHASE8_DELIVERY_PLAN.md` — detailed active Phase 8 packet state;
- `MODULE_CATALOG.md` — business-module readiness/count where justified;
- parent and packet issues;
- active PR body and exact validation state.

`README.md` remains stable orientation and must not become a second manually maintained roadmap.
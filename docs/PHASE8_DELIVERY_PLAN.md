# Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Commercial follow-on: #29  
Functional scope guardrail: [`CRM_CAPABILITY_COVERAGE.md`](CRM_CAPABILITY_COVERAGE.md)

## Goal

Build the broad expert CRM domain layer on top of the completed governed platform foundations without collapsing ownership into Sales, without a giant long-lived Phase 8 branch and without weakening compatibility, tenant, authorization, audit or rollback guarantees.

The capability coverage baseline is normative for scope completeness: Phase 8 and later phases must collectively provide an explicit owner, governed integration boundary or intentional optional/vertical classification for the full CRM capability set.

## Delivery model

Phase 8 is one coherent architecture program delivered as multiple mergeable packets. Each packet must establish a stable boundary that downstream work can safely consume.

Every packet must state:

- authoritative owner domain and stable references;
- public capability/query/event contracts;
- persistence, tenant and authorization model;
- audit/idempotency/approval requirements;
- projection/search implications;
- frontend/product workflow and accessibility impact;
- import/migration/compatibility consequences;
- exact acceptance and operational gates.

A packet is merged only when its natural architecture boundary is complete and all applicable checks are green on one exact final SHA.

## Wave 8A — canonical customer master, identity and consent

### 8A.1 — identity/reference contracts and owner foundations — Complete

Delivered by #92 / merged PR #93:

- canonical typed resource identifiers/references for Party, Account and Contact Point;
- exact versioned contract foundations and owner-module boundaries;
- explicit prohibition on Sales/Service/Marketing/Billing defining competing customer identity owners;
- generated Rust/browser contract synchronization and compatibility gates.

### 8A.2 — Party lifecycle and discovery — Complete

Party remains the canonical person/organization identity owner. The complete first production lifecycle was deliberately split into three independently mergeable packets.

#### 8A.2a — authoritative Party create/get — Complete

Delivered by #94 / merged PR #95:

- Person and Organization Party aggregate;
- governed create through `CapabilityGateway`;
- permission-aware get through `QueryGateway`;
- tenant isolation, Personal data classification, idempotency, outbox and audit evidence;
- real PostgreSQL and process-level `crm-api` acceptance.

#### 8A.2b — optimistic Party update and permission-aware list — Complete

Delivered by #96 / merged PR #97:

- immutable Party identity/kind with optimistic display-name update;
- exact expected-version conflicts and deterministic version progression;
- governed `parties.party.update@1.0.0` with idempotency, event and audit evidence;
- deterministic tenant/actor/filter/sort/page-bound signed cursor listing;
- optional typed Party-kind filter;
- live per-resource and per-field visibility enforcement;
- process-level PostgreSQL acceptance for replay, conflicting replay, stale version, pagination and non-disclosure.

Final verified review head `4c4f19a19fab1764d6ba49c0210ca65a7f2456ad` passed all applicable Contract, Governance, Rust, Rust Generated Sync, Database, Event Runtime, Application Runtime and Product Plane workflows before merge.

#### 8A.2c — Party search and permission-aware customer discovery — Complete

Delivered by #98 / merged PR #99:

- neutral `crm-global-search-composition` cross-domain projection boundary;
- Party create/update indexing from immutable authoritative owner events;
- exact Party event contract, Personal data-class, aggregate identity and version validation;
- searchable Party `display_name` plus typed `kind` display metadata;
- live resource/field authorization before every search disclosure;
- conservative Personal classification for governed global search payloads;
- deterministic migration from search generation `g1` to `g2` so historical Party events could not be skipped by an old checkpoint;
- rebuild/validate/activate/retire through the existing `SearchReindexCoordinator`;
- process-level proof of update → worker convergence → governed search, superseded-name removal and cross-tenant non-disclosure.

Search remains candidate-only and rebuildable. It does not own Party identity.

### 8A.3 — Account, Contact Point, Party Relationship and Customer 360 — In progress

#### 8A.3a — authoritative Account lifecycle and Party associations — Complete

Delivered by #101 / merged PR #102 under #100:

- immutable typed Account identity owned solely by `crm.customer-accounts`;
- normalized bounded Account name and explicit Active/Inactive lifecycle;
- stable Party references and typed Primary/Member roles without copying Party identity attributes;
- exactly one primary Party association and deterministic duplicate-free association normalization;
- optimistic exact-version updates, monotonic governed mutation time and semantic no-op rejection;
- strict deterministic persisted Account state contract;
- additive v1 create/update/get/list and created/updated event contracts;
- immutable module manifest and exact contract bindings;
- governed transactional create/update through the production aggregate executor with idempotency, outbox and audit evidence;
- permission-aware get/list with signed tenant/actor/filter/sort/page-bound cursors and field redaction;
- cross-owner Party-reference integrity validation in the application/platform composition layer rather than inside the pure Account module;
- identical safe rejection for missing and cross-tenant Party references;
- real PostgreSQL + real `crm-api` acceptance covering prerequisite Party creation, reference integrity, create, replay, get, update, update replay, stale version, cursor pagination, status filtering, unauthenticated rejection, cross-tenant non-disclosure and durable evidence counts;
- independent fresh-database process acceptance for full application, Party and Account scenarios.

Final verified review head `0d6d79dce31aaea4d2a0998fadb1ac842fdcfde4` passed all 11 applicable workflows together. PR #102 merged to `main` as `7ee48530d880ef8aeb6abf2140b524ac724d4fc9`.

This packet intentionally does **not** absorb Contact Point, Party Relationship, hierarchy or Customer 360 projection ownership into Account.

#### 8A.3b — Contact Point lifecycle, verification and preference — Active / final verification

Tracked by #103 and draft PR #104 on `develop/phase8a3b-contact-point-lifecycle`.

The packet now implements:

- authoritative typed Contact Point identity with a stable Party reference and no copied Party identity attributes;
- Email, Phone, Postal, Web and Messaging endpoint kinds with deterministic canonical normalization and bounded validation;
- Active/Inactive lifecycle, preferred state, validity intervals and explicit verification evidence/time;
- verification preservation for lifecycle/validity/display-only changes and automatic reset only when the canonical endpoint value changes;
- exact optimistic version progression, monotonic mutation time, semantic no-op rejection and atomic version-exhaustion failure;
- strict deterministic versioned persisted Contact Point state with canonical rehydration checks;
- additive `crm.contact_points.v1` create/update/verify/get/list contracts and created/updated/verified events;
- immutable module manifest evolution, exact contract bindings and synchronized Rust/browser descriptor identities;
- governed transactional create/update/verify through the production aggregate executor with idempotency, outbox and audit evidence;
- permission-aware get/list with tenant/actor/filter/sort/page-bound signed cursors, typed filters, bounded visibility scans and live field redaction;
- application-level Party-reference integrity without SQL or cross-owner storage access in the Contact Point owner module;
- identical safe rejection for missing and cross-tenant Party references while real database/runtime failures remain distinguishable internally;
- `crm-application-runtime` composition and field-bounded visibility bootstrap;
- fresh-PostgreSQL real `crm-api` process acceptance covering Party prerequisites, lifecycle, verification, display-only preservation, verification reset on value change, replay, conflicting replay, stale version, filters, signed cursor pagination/tamper rejection, unauthenticated rejection, tenant non-disclosure and durable evidence counts.

The packet remains **Active** and PR #104 remains draft until all applicable workflows are green together on one exact final SHA. It is not marked complete or merged prematurely.

Consent and communication authorization, provider delivery state, Party Relationship and Customer 360 remain separate authoritative ownership and are not hidden inside Contact Point flags.

#### 8A.3c — Party Relationship lifecycle and hierarchy foundations

Deliver:

- typed Party-to-Party relationships such as employment, household, parent/subsidiary and configurable governed roles;
- validity intervals and temporal relationship state;
- hierarchy traversal projections without moving hierarchy ownership into Account or Sales;
- tenant, authorization, optimistic concurrency and non-disclosure proof.

#### 8A.3d — Customer 360 composition

Deliver a permission-aware rebuildable Customer 360 composition over Party, Account, Contact Point and Party Relationship owner contracts without creating a second identity master.

### 8A.4 — Consent and communication authorization

Deliver purpose/channel/jurisdiction/legal-basis/source/proof/effective/expiry/withdrawal semantics and an exact authorization decision boundary that downstream communication and marketing modules must use.

Withdrawal must take effect without waiting for a projection rebuild. Historical evidence remains immutable while current authorization decisions reflect live consent state.

### 8A.5 — identity resolution and duplicate candidates

Deliver deterministic candidate generation first, explainable evidence, review state and governed approval boundaries. Probabilistic/AI suggestions may enrich candidates later but cannot bypass governed merge approval.

### 8A.6 — merge, unmerge, provenance and survivorship

Deliver immutable lineage, source evidence preservation, reference redirection, field-level provenance and reversible merge history.

### 8A.7 — import/export, data quality and privacy lifecycle proof

Deliver versioned mapping, dry-run validation, resumable idempotent imports, reconciliation, governed enrichment provenance, data-quality/stewardship evidence, export and deletion/restriction/legal-hold interaction proof.

## Wave 8B — product catalog and quote-to-revenue

Begin against stable merged customer reference contracts.

Planned packets:

1. Product/Catalog ownership, variants/bundles/options and effective-dated versioning;
2. Price Book, currencies and governed pricing semantics;
3. CPQ/configuration, validation and quote revision lifecycle;
4. discount/exception approvals and commercial policy evidence;
5. Order and commercial commitment handoff;
6. Contract, amendment, renewal and termination lifecycle;
7. Subscription, entitlement and usage-reference lifecycle;
8. billing/ERP/payment/tax integration boundaries and reconciliation evidence.

Catalog, Pricing, Quote, Order, Contract, Subscription and Billing/ERP integration ownership must remain explicit and must not be absorbed into Sales.

## Wave 8C — Sales and productivity expert expansion

Deliver leads/prospects, qualification, richer opportunity roles, calendar synchronization, routing, territories, teams, quotas, forecasting, renewals/expansion, playbooks/sequences and product-quality list/Kanban/mobile/offline workflows.

## Wave 8D — Communications and omnichannel

Deliver email, telephony, SMS/messaging, chat and optional social adapters; unified conversations; agent inboxes and queues; consent-aware sending; delivery state; searchable interaction history; verified webhooks and provider reconciliation.

## Wave 8E — Service, support, knowledge and field service

Deliver cases/tickets, queues/routing, SLAs, entitlements, incidents, service automation, knowledge lifecycle, self-service boundaries and optional field-service work orders, dispatch and technician mobile/offline workflows.

## Wave 8F — Marketing and growth

Deliver campaigns, dynamic segmentation, suppression, forms/event ingestion, scoring, consent-aware journeys, experiments, attribution, account-based marketing and optional event/loyalty/referral modules.

## Wave 8G — Customer success, retention and partner/channel CRM

Deliver onboarding/success plans, health scores, adoption signals, risks/playbooks, renewal/expansion coordination and churn analytics; plus optional PRM for partner programs, deal registration, distribution, attribution and delegated portal access.

## Wave 8H — Projects, configurable work, documents and e-signature

Deliver projects/workstreams/milestones, configurable operational cases, secure document ownership/versioning, governed template generation, e-signature evidence and retention/legal-hold interaction.

## Wave 8I — Analytics, reporting and performance management

Deliver permission-aware semantic reporting, dashboards, funnels/cohorts, sales/service/marketing/customer-success analytics, territory/quota scorecards, scheduled delivery, metric lineage and warehouse/BI boundaries.

## Wave 8J — Workflow, collaboration and product completeness

Deliver governed workflow triggers/conditions/branches/waits/timers, approvals and human tasks, notifications and collaboration, global productivity surfaces, onboarding/import guidance, responsive/mobile behavior, accessibility, localization, offline/retry states and critical browser E2E.

## Cross-cutting Phase 8 obligations

Every relevant domain wave must include:

- API/webhook contracts, replay protection and reconciliation;
- bulk import/export and migration compatibility;
- data-quality and provenance expectations;
- connector credential/secret-handle boundaries;
- rate limits, quotas and tenant isolation;
- observability, failure recovery and operational runbooks appropriate to the maturity claim.

ERP, finance/accounting, payment, tax, telephony, messaging, identity-provider, ad-platform and external data-provider systems remain governed integrations unless a separate CRM owner domain is explicitly justified.

## Later platform programs

### Phase 9 — AI-native CRM

AI is an authenticated audited Actor using permission-scoped tools generated from governed capability/query contracts. It has no alternate mutation, identity-merge, consent or authorization path.

### Phase 10 — signed marketplace and sandbox

Deliver signed packages, publisher identity, explicit grants, SBOM/provenance checks, sandboxed untrusted execution, quotas, kill switches and safe lifecycle operations.

### Phase 11 — enterprise and production proof

Deliver SSO/OIDC/SAML, SCIM, tenant key hierarchy, field/data-class encryption where required, legal hold, WORM audit export, privacy lifecycle proof, backup/PITR/tenant restore, tenant mobility, data residency, security testing, SLOs and runbooks.

## Merge rule

A packet is merged when its natural architecture boundary is complete and all applicable exact-head gates are green. Later packets build from merged stable contracts rather than from a single accumulating Phase 8 mega-branch.

No phase or module is considered product-complete merely because schemas or crates exist. Readiness classification must follow `CRM_CAPABILITY_COVERAGE.md` and distinguish production-complete, platform-ready, planned, optional/vertical and external-integration capabilities.

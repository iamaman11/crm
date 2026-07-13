# Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
First owner-domain program: #28  
Commercial follow-on: #29  
Functional scope guardrail: [`CRM_CAPABILITY_COVERAGE.md`](CRM_CAPABILITY_COVERAGE.md)

## Goal

Build the broad expert CRM domain layer on top of the completed governed platform foundations without collapsing ownership into Sales, without a giant long-lived Phase 8 branch and without weakening compatibility, tenant, authorization, audit or rollback guarantees.

The capability coverage baseline is normative for scope completeness: Phase 8 and later phases must collectively provide an explicit owner, governed integration boundary or intentional optional/vertical classification for the full CRM capability set.

## Delivery model

Phase 8 is one coherent architecture program delivered as multiple mergeable packets. Each packet must establish a stable boundary that downstream work can safely consume.

Do not defer all Phase 8 merging until the end. A giant branch would make exact-SHA evidence, rollback, bisectability, code review and ownership discipline materially weaker.

Every packet must state:

- authoritative owner domain and stable references;
- public capability/query/event contracts;
- persistence, tenant and authorization model;
- audit/idempotency/approval requirements;
- projection/search implications;
- frontend/product workflow and accessibility impact;
- import/migration/compatibility consequences;
- exact acceptance and operational gates.

## Wave 8A — canonical customer master, identity and consent

### 8A.1 — identity/reference contracts and owner skeletons — Complete

Delivered by #92 / merged PR #93:

- canonical typed resource identifiers/references for Party, Account and Contact Point;
- exact versioned contract foundations and owner-module boundaries;
- explicit prohibition on Sales/Service/Marketing/Billing defining competing customer identity owners;
- generated Rust/browser contract synchronization and compatibility gates.

### 8A.2 — Party lifecycle and discovery — In progress

The Party slice is intentionally split into mergeable production packets rather than claiming completion from a create/get proof alone.

#### 8A.2a — authoritative Party create/get production slice — Complete

Delivered by #94 / merged PR #95:

- Person and Organization Party aggregate;
- governed create through `CapabilityGateway`;
- permission-aware get through `QueryGateway`;
- tenant isolation, Personal data classification, idempotency, outbox and audit evidence;
- production PostgreSQL and process-level acceptance.

#### 8A.2b — optimistic Party update and permission-aware list — Active

Tracked by #96 / draft PR #97.

Deliver:

- immutable Party identity/kind with optimistic display-name update;
- exact expected-version conflicts and deterministic version progression;
- governed `parties.party.update@1.0.0` with idempotency, event and audit evidence;
- deterministic tenant/actor/filter/sort/page-bound cursor listing;
- optional typed Party-kind filter;
- live per-resource and per-field visibility enforcement;
- process-level PostgreSQL acceptance for update/list/replay/conflict/non-disclosure.

#### 8A.2c — Party search and customer discovery projection

Deliver after 8A.2b is stable:

- Party indexing through the rebuildable search/projection architecture;
- permission-aware disclosure through live authorization;
- deterministic rebuild/switch behavior;
- propagation of Party update and privacy effects;
- no authoritative identity state stored in the search index.

8A.2 is complete only when 8A.2a–8A.2c are complete and the combined create/update/get/list/search behavior has exact-head production evidence.

### 8A.3 — Account, Contact Point and Party Relationship

Deliver customer/commercial relationship ownership, verified/preferred contact points, time-bounded typed relationships and hierarchy foundations.

Required sub-packets should separate at least:

1. Account lifecycle and Party membership/reference semantics;
2. Contact Point lifecycle, verification state and channel preference;
3. Party Relationship types, validity intervals and hierarchy traversal projections;
4. Customer 360 projection composition without creating a second identity owner.

### 8A.4 — Consent and communication authorization

Deliver purpose/channel/jurisdiction/legal-basis/source/proof/effective/expiry/withdrawal semantics and an exact authorization decision boundary that downstream communication and marketing modules must use.

Withdrawal must take effect without waiting for a projection rebuild. Historical evidence remains immutable while current authorization decisions reflect the live consent state.

### 8A.5 — identity resolution and duplicate candidates

Deliver deterministic candidate generation first, explainable evidence, review state and governed approval boundaries. Probabilistic/AI suggestions may enrich candidates later but cannot bypass governed merge approval.

### 8A.6 — merge, unmerge, provenance and survivorship

Deliver immutable lineage, source evidence preservation, reference redirection, field-level provenance and reversible merge history.

### 8A.7 — import/export, data quality and privacy lifecycle proof

Deliver versioned mapping, dry-run validation, resumable idempotent imports, reconciliation, governed enrichment provenance, data-quality/stewardship evidence, export and deletion/restriction/legal-hold interaction proof.

## Wave 8B — product catalog and quote-to-revenue

Begin only against stable merged customer reference contracts.

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

Deliver:

- leads/prospects and qualification;
- richer pipeline and opportunity relationship roles;
- activity/calendar synchronization and scheduling boundaries;
- routing, assignment, queues and workload distribution;
- territory, team/overlay and quota/target management;
- forecast categories, rollups and scenarios;
- renewals/expansion/cross-sell workflows;
- playbooks, sequences and guided next actions;
- product-quality lists, Kanban, timelines, saved views and bulk actions;
- responsive/mobile and offline-capable field workflows where the product tier requires them.

## Wave 8D — Communications and omnichannel

Deliver:

- email, telephony, SMS/messaging, chat and optional social adapters;
- unified conversation/thread and participant resolution;
- agent inboxes, queues and assignment;
- templates, attachments and provider delivery status;
- consent-aware sending, quiet hours and frequency policies;
- searchable interaction history with live authorization;
- verified webhooks, replay safety and provider reconciliation.

## Wave 8E — Service, support, knowledge and field service

Deliver:

- cases/tickets, queues, routing and configurable lifecycle;
- SLAs, business calendars, milestones and escalation;
- entitlements, warranties and service-contract references;
- major incident, parent/child and duplicate case relationships;
- macros, guided resolution and service automation;
- knowledge authoring/review/publication/localization/feedback;
- self-service portal/API boundaries and service feedback;
- optional field-service work orders, dispatch, appointments and technician mobile/offline flows with inventory/ERP integration boundaries.

## Wave 8F — Marketing and growth

Deliver:

- campaigns, campaign members and acquisition source history;
- dynamic segmentation, lists and suppression;
- forms/event ingestion boundaries;
- scoring and qualification models;
- consent-aware journeys, waits, branches, triggers and goals;
- experiments where applicable;
- attribution and touchpoint lineage;
- account-based marketing and buying-group support;
- optional event/webinar, loyalty and referral modules through governed boundaries.

## Wave 8G — Customer success, retention and partner/channel CRM

Deliver customer-success capabilities:

- onboarding and success plans;
- objectives/milestones and explainable health scores;
- adoption/usage signal integration;
- risks, alerts, playbooks and business reviews;
- renewal/expansion coordination and churn analytics.

Deliver optional PRM capabilities:

- partner organizations/contacts, programs, tiers and certifications;
- deal registration and conflict rules;
- lead/opportunity distribution;
- partner-sourced/influenced attribution;
- portal/delegated-access and partner performance boundaries.

## Wave 8H — Projects, configurable work, documents and e-signature

Deliver:

- projects, workstreams, milestones, dependencies and templates;
- configurable operational business cases distinct from support cases;
- portfolio/delivery projections;
- secure file/document ownership and versioning;
- document generation through governed templates;
- e-signature envelope/status integration and immutable signed-document evidence;
- retention/legal-hold/privacy interaction.

## Wave 8I — Analytics, reporting and performance management

Deliver:

- permission-aware semantic reporting and operational dashboards;
- funnels, cohorts, retention and lifecycle analytics;
- pipeline/forecast, service/SLA, marketing attribution and customer-success analytics;
- territory/quota/performance scorecards;
- scheduled report delivery with live authorization checks;
- metric lineage, freshness and reproducibility;
- warehouse/lakehouse/BI integration boundaries.

## Wave 8J — Workflow, collaboration and product completeness

Deliver expert product surfaces across the implemented domains:

- governed trigger/condition/branch/wait/timer/schedule workflow semantics;
- approvals, human tasks, retries and recovery/compensation strategies;
- notifications, mentions, collaboration threads and team work lists;
- global navigation, command palette, favorites and recent items;
- product-quality tables, filters, saved views, bulk actions and timelines;
- onboarding, import guidance and administrative diagnostics;
- responsive/mobile behavior, accessibility, localization and locale-aware formatting;
- offline/retry states where required;
- critical browser E2E against real governed application paths.

## Cross-cutting Phase 8 data and integration obligations

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

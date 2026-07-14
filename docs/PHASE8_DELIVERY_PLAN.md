# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Commercial follow-on: #29  
Delivery governance: `DELIVERY_GOVERNANCE.md`  
Functional scope guardrail: `CRM_CAPABILITY_COVERAGE.md`

## 1. Goal

Build the expert CRM domain layer on top of the completed governed platform foundations without collapsing ownership into Sales, without a giant long-lived Phase 8 branch and without weakening compatibility, tenant isolation, authorization, audit, rollback or exact-SHA evidence.

Phase 8 is delivered as a sequence of reviewable owner-domain and product packets. Every packet ends at a natural architecture boundary with explicit acceptance gates.

## 2. Packet contract

Every Phase 8 packet must define:

- authoritative owner domain and stable references;
- public capability/query/event contracts;
- persistence, tenant and authorization model;
- audit, idempotency and approval requirements;
- projection/search implications;
- frontend/product workflow where applicable;
- import/migration/compatibility consequences;
- exact process/browser/operational acceptance gates.

A packet may be marked **Complete** only after merge to `main`. Exact candidate evidence is invalidated by every later source or documentation change until applicable checks rerun.

## 3. Wave 8A — canonical customer master, identity, consent and customer-data lifecycle

### 8A.1 — identity/reference contracts and owner foundations — Complete

Delivered by #92 / merged PR #93.

Established canonical typed references and owner boundaries for Party, Account and Contact Point without allowing downstream domains to define competing customer identity masters.

### 8A.2 — Party lifecycle and discovery — Complete

#### 8A.2a — authoritative Party create/get — Complete

Delivered by #94 / merged PR #95.

#### 8A.2b — optimistic Party update and permission-aware list — Complete

Delivered by #96 / merged PR #97.

#### 8A.2c — Party search and customer discovery — Complete

Delivered by #98 / merged PR #99.

Party remains authoritative. Search remains rebuildable and non-authoritative.

### 8A.3 — Account, Contact Point, Party Relationship and Customer 360 — Complete

#### 8A.3a — Account lifecycle and Party associations — Complete

Delivered by #101 / merged PR #102.

#### 8A.3b — Contact Point lifecycle, verification and preference — Complete

Delivered by #103 / merged PR #104.

#### 8A.3c — Party Relationship lifecycle and hierarchy foundations — Complete

Delivered by #108 / merged PR #109.

#### 8A.3d — permission-aware rebuildable Customer 360 composition — Complete

Delivered by #110 / merged PR #111.

### 8A.4 — Consent and Communication Authorization — Complete

Delivered by #112 / merged PR #113.

Implemented immutable purpose/channel-scoped authorization assertions, governed withdrawal and an exact authoritative communication-authorization decision boundary. Withdrawal affects live authorization without waiting for projection rebuild.

### 8A.5 — Identity Resolution and Duplicate Candidates — Complete

Delivered by #114 / merged PR #115.

Implemented deterministic duplicate-candidate case identity, bounded evidence with source versions, explainable matcher/signal provenance and terminal reviewer decisions. Candidate evidence does not itself authorize a merge.

### 8A.6 — Merge, Unmerge, Provenance and Survivorship — Complete

Delivered by #116 / merged PR #119.  
Merge commit: `d5cb4502ad0c49158e0789d8749dc09160da7895`.

Implemented:

- approval-required reversible Party merge/unmerge;
- immutable merge-operation lineage;
- exact Party-version validation;
- field-level survivorship provenance;
- cycle-safe canonical redirection;
- permission-aware merge queries and canonical resolution;
- hard PostgreSQL topology invariants;
- fresh-PostgreSQL real `crm-api` process acceptance;
- all applicable exact-head workflows green before merge.

Party remains authoritative. Merge does not delete Party records or destructively rewrite historical references.

### 8A.7 — Customer Import Jobs, Versioned Mappings and Resumable Execution — In progress

Issue: #120  
Draft PR: #121

This is the **single active customer-master production packet**.

#### Ownership boundary

`crm.customer-data-operations` may coordinate only:

- import-job identity and lifecycle;
- immutable source-content identity metadata;
- immutable versioned mapping identity;
- deterministic row identity;
- durable row validation/execution outcomes;
- resumable checkpoints and counters;
- bounded safe diagnostics.

It does not own Party or other customer-master records. Successful target writes must invoke exact governed owner capabilities.

#### Already present on the active branch

- normative import architecture;
- module foundation and manifest;
- import-job and import-row domain models;
- deterministic source/mapping/row identities;
- partial-execution policy;
- resumable job/row lifecycle semantics;
- strict deterministic versioned private persistence.

#### Still required for production completion

- public versioned mutation/query/event contracts;
- governed capability/query adapters;
- exact application-runtime composition;
- PostgreSQL persistence, FORCE RLS and migrations;
- real dry-run path with zero target Party side effects;
- governed Party capability execution with deterministic target idempotency;
- durable interruption/restart resume;
- permission-aware job/row queries and signed cursors;
- fresh-PostgreSQL real `crm-api` process acceptance;
- one unchanged exact final SHA with all applicable gates green.

#### Additional design requirements before contract freeze

- external source identifiers must remain distinct from canonical CRM Party IDs;
- parsing must use an explicit immutable parser/import profile covering format, encoding, dialect/header semantics and parser/canonicalization version so source bytes cannot be reinterpreted silently under changed semantics.

### 8A.8 — Customer Export Jobs, Artifacts and Reconciliation Evidence — Planned

Issue: #123  
Depends on: #120

Deliver governed export-job lifecycle, immutable export specifications, live authorization/field filtering, secure artifact references, resumability and reconciliation evidence. Export artifacts remain derived data and never become authoritative customer state.

### 8A.9 — Customer Data Quality Rules, Completeness and Stewardship — Planned

Issue: #124  
Depends on: #120

Deliver versioned deterministic quality rules, explainable findings tied to exact resource/version evidence, completeness indicators and stewardship workflows. Remediation may mutate authoritative state only through exact owner capabilities.

### 8A.10 — Governed Customer Enrichment and Provenance — Planned

Issue: #125  
Depends on: #124

Deliver provider adapter boundaries, secret handles, versioned mappings, source/freshness/licensing provenance, review/approval policy where required and exact owner-capability application of accepted changes.

### 8A.11 — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — Planned

Issue: #126  
Depends on: #123, #124, #125

Deliver governed privacy request lifecycle, access/export, live restriction enforcement, owner-aware deletion/anonymization planning, retention/legal-hold conflict handling and downstream search/projection convergence with immutable evidence preservation where required.

### Phase 8A completion gate

Phase 8A is complete only when the merged system proves:

- canonical stable customer identities and references;
- Account, Contact Point and Party Relationship ownership;
- permission-aware Customer 360;
- authoritative consent and immediate withdrawal semantics;
- explainable duplicate candidates;
- reversible merge/unmerge and immutable provenance;
- deterministic import and export with resumability/reconciliation;
- explicit data-quality/stewardship evidence;
- governed enrichment provenance;
- privacy access/export/restriction/deletion/legal-hold interactions;
- cross-tenant isolation, compatibility, migration rollback/reapply and production acceptance evidence.

## 4. Wave 8B — Product Catalog and Quote-to-Revenue

State: **Planned**  
Issue: #29

Begin against stable customer-reference contracts.

Planned owner-domain packets include:

1. Product/Catalog ownership, variants, bundles, options and effective-dated versioning.
2. Price Books, currencies and governed pricing semantics.
3. CPQ/configuration, validation and pricing explanation.
4. Quote revisions, comparison, expiry and approvals.
5. Discount/exception approvals and commercial policy evidence.
6. Order and commercial commitment handoff.
7. Contract, amendment, renewal and termination lifecycle.
8. Subscription, entitlement and usage-reference lifecycle.
9. Billing/ERP/payment/tax/fulfillment integration boundaries and reconciliation.

Catalog, Pricing, Quote, Order, Contract, Subscription and Billing integration ownership must remain explicit and must not be absorbed into Sales.

## 5. Wave 8C — Sales and Productivity Expert Expansion

Deliver:

- leads/prospects and qualification;
- richer opportunity/deal pipelines and roles;
- appointments and recurring work;
- calendar synchronization boundaries;
- routing, queues and workload distribution;
- territories, teams and overlays;
- quotas and targets;
- forecasting and scenarios;
- renewals, expansion and cross-sell support;
- sequences, playbooks and guided next actions;
- product-quality list/Kanban/mobile/offline workflows.

## 6. Wave 8D — Communications and Omnichannel

Deliver email, telephony, SMS/messaging, chat and optional social adapters; unified conversations; agent inboxes/queues; consent-aware sending; delivery state; searchable interaction history; webhook verification and provider reconciliation.

## 7. Wave 8E — Service, Support, Knowledge and Field Service

Deliver cases/tickets, queues/routing, SLAs, entitlements, incidents, service automation, knowledge lifecycle, self-service boundaries and optional field-service work orders, dispatch and technician mobile/offline workflows.

## 8. Wave 8F — Marketing and Growth

Deliver campaigns, dynamic segmentation, suppression, forms/event ingestion, scoring, consent-aware journeys, experiments, attribution, account-based marketing and optional event/loyalty/referral modules.

## 9. Wave 8G — Customer Success and Partner/Channel CRM

Deliver onboarding/success plans, health scores, usage/adoption signals, risks/playbooks, renewal/expansion coordination and churn analytics; plus optional PRM for partner programs, deal registration, distribution, attribution and delegated portal access.

## 10. Wave 8H — Projects, Configurable Work, Documents and E-signature

Deliver projects/workstreams/milestones, configurable operational cases, secure document ownership/versioning, governed template generation, e-signature evidence and retention/legal-hold interaction.

## 11. Wave 8I — Analytics, Reporting and Performance Management

Deliver permission-aware semantic reporting, dashboards, funnels/cohorts, sales/service/marketing/customer-success analytics, territory/quota scorecards, scheduled delivery, metric lineage and governed warehouse/BI boundaries.

## 12. Wave 8J — Workflow, Collaboration and Product Completeness

Deliver governed workflow triggers/conditions/branches/waits/timers, approvals and human tasks, notifications and collaboration, global productivity surfaces, onboarding/import guidance, responsive/mobile behavior, accessibility, localization, offline/retry states and critical browser E2E.

## 13. Cross-cutting Phase 8 obligations

Every relevant domain wave must include:

- API/webhook contracts and replay protection;
- import/export and migration compatibility where applicable;
- data-quality and provenance expectations;
- connector credential/secret-handle boundaries;
- rate limits, quotas and tenant isolation;
- observability, failure recovery and operational runbooks appropriate to the maturity claim;
- synchronized roadmap/status/module/issue/PR state under `DELIVERY_GOVERNANCE.md`.

ERP, finance/accounting, payment, tax, telephony, messaging, identity-provider, ad-platform and external data-provider systems remain governed integrations unless a separate CRM owner domain is explicitly justified.

## 14. Later platform programs

### Phase 9 — AI-native CRM

AI is an authenticated audited Actor using permission-scoped governed tools. It has no alternate mutation, identity-merge, consent or authorization path.

### Phase 10 — signed marketplace and sandbox

Deliver signed packages, publisher identity, explicit grants, SBOM/provenance checks, sandboxed untrusted execution, quotas, kill switches and safe lifecycle operations.

### Phase 11 — enterprise and production proof

Deliver OIDC/SAML, SCIM, key hierarchy/encryption, legal hold, WORM audit export, backup/PITR/restore, tenant mobility/residency, security testing, SLOs and runbooks.

## 15. Immediate sequence

1. Merge/adopt #122 documentation-governance synchronization.
2. Close superseded PR #118.
3. Continue only the missing production layers of 8A.7 / #120 / PR #121.
4. Complete exact-head gate and merge 8A.7.
5. Deliver #123 -> #124 -> #125 -> #126.
6. Begin 8B / #29 from the stable customer-master baseline.

No packet is product-complete merely because schemas, manifests or crates exist.
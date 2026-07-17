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
- import/export and compatibility consequences;
- module-owned mutation/query/worker contributions and durable activation semantics;
- exact route-parity/classification impact with no central business switches;
- exact process/browser/operational acceptance gates.

A packet may be marked **Complete** only after merge to `main`. Exact candidate evidence is invalidated by every later source or documentation change until applicable checks rerun.

## 3. Wave 8A — canonical customer master, identity, consent and customer-data lifecycle

### 8A.1 — identity/reference contracts and owner foundations — Complete

Delivered by #92 / PR #93.

### 8A.2 — Party lifecycle and discovery — Complete

- **8A.2a:** authoritative Party create/get — #94 / PR #95.
- **8A.2b:** optimistic Party update and permission-aware list — #96 / PR #97.
- **8A.2c:** Party search and customer discovery — #98 / PR #99.

Party remains authoritative. Search remains rebuildable and non-authoritative.

### 8A.3 — Account, Contact Point, Party Relationship and Customer 360 — Complete

- **8A.3a:** Account lifecycle and Party associations — #101 / PR #102.
- **8A.3b:** Contact Point lifecycle, verification and preference — #103 / PR #104.
- **8A.3c:** Party Relationship lifecycle and hierarchy foundations — #108 / PR #109.
- **8A.3d:** permission-aware rebuildable Customer 360 composition — #110 / PR #111.

### 8A.4 — Consent and Communication Authorization — Complete

Delivered by #112 / PR #113.

Implemented immutable purpose/channel-scoped authorization assertions, governed withdrawal and an exact authoritative communication-authorization decision boundary. Withdrawal affects live authorization without waiting for projection rebuild.

### 8A.5 — Identity Resolution and Duplicate Candidates — Complete

Delivered by #114 / PR #115.

Implemented deterministic duplicate-candidate case identity, bounded evidence with source versions, explainable matcher/signal provenance and terminal reviewer decisions. Candidate evidence does not itself authorize a merge.

### 8A.6 — Merge, Unmerge, Provenance and Survivorship — Complete

Delivered by #116 / PR #119.
Merge commit: `d5cb4502ad0c49158e0789d8749dc09160da7895`.

Implemented approval-required reversible Party merge/unmerge, immutable merge-operation lineage, exact Party-version validation, field-level survivorship provenance, cycle-safe canonical redirection, permission-aware merge queries, hard PostgreSQL topology invariants and fresh-process acceptance.

Party remains authoritative. Merge does not delete Party records or destructively rewrite historical references.

### 8A.7 — Customer Import Jobs, Versioned Mappings and Resumable Execution — Complete

Delivered by #120 / PR #121.
Merge commit: `5f60f24d6d3a3bb46720658f4e98d4a7ebb15637`.

Implemented:

- `crm.customer-data-operations` ownership of import-job/source/mapping/row/checkpoint evidence;
- immutable source artifacts with sequential chunks, replay protection, exact byte length and SHA-256 finalization;
- immutable source-system identity and parser/import profiles;
- explicit separation of source external identifiers from canonical CRM Party IDs;
- server-side parsing and validation of finalized source bytes;
- true dry-run proof with zero Party-side effects;
- deterministic row identity and target Party idempotency;
- exact Party owner-capability execution with no direct Party storage writes;
- durable outcomes, checkpoints, completion and retry evidence;
- fresh-PostgreSQL crash/restart process acceptance;
- tenant non-disclosure, field visibility and signed-cursor tamper rejection.

### 8A.8 — Customer Export Jobs, Artifacts and Reconciliation Evidence — Complete

Delivered by #123 / PR #130.
Merge commit: `0e7f9889362533446cc65d95dcf7969a60086a57`.

Implemented:

- immutable versioned Party export specifications and bounded selection;
- deterministic manifest items bound to exact Party resource versions;
- governed Party selection/execution reads with live authorization and field visibility;
- deterministic spreadsheet-safe UTF-8 CSV canonicalization;
- approval-required production execution;
- replay-safe artifact chunks, outcomes, checkpoints and finalization;
- exact reconciliation and both required crash-window recoveries;
- live-authorized, retention-aware, integrity-verified and audited disclosure;
- fresh-PostgreSQL real `crm-api` process acceptance.

Exported bytes remain derived artifacts and never become authoritative customer-master state.

### 8A.9 — Customer Data Quality Rules, Completeness and Stewardship — Complete

Issue: #124
Delivered by: PR #132
Merge commit: `8a1664309be9dc0c5e3bf9014cf248b1c3680035`
Depends on: completed #123 / merged PR #130 (`0e7f9889362533446cc65d95dcf7969a60086a57`)

The Party-focused v1 implementation is merged and complete.

#### Delivered ownership boundary

`crm.data-quality` owns only:

- immutable/versioned Party quality rule-set definitions;
- immutable/versioned completeness-profile definitions;
- deterministic evaluation-job and exact staged Party evidence;
- immutable rule outcomes and exact completeness-result lineage;
- deterministic finding identities and immutable observations;
- stewardship assignment, acknowledgement, waiver and remediation-attempt evidence;
- bounded diagnostics, reconciliation and retry/process evidence.

It does not own or copy authoritative mutable Party, Account, Contact Point, Party Relationship, Consent or Identity Resolution state. Authoritative Party values are read only through governed owner/query composition. Party mutation occurs only through the exact `parties.party.update@1.0.0` owner capability.

#### Delivered safety and semantics

The v1 packet proves:

1. published rule/evaluator versions are immutable and cannot be silently reinterpreted;
2. evaluation uses exact bounded built-in evaluator identities;
3. arbitrary SQL, scripts, filesystem access, arbitrary network access and unbounded expressions are absent;
4. every outcome, finding, observation and completeness result binds exact authoritative Party resource-version evidence;
5. deterministic reevaluation does not create duplicate logical current findings;
6. historical evidence remains immutable across open, acknowledged, waived, remediated and reopened lifecycle transitions;
7. completeness uses deterministic integer scoring with exact component/outcome lineage;
8. stewardship mutations use optimistic finding versions and exact-current-observation preconditions;
9. stale Party/remediation evidence fails with a conflict;
10. remediation result evidence remains separate from historical evaluation truth;
11. signed list cursors are bound to tenant, actor, capability/version, filters, sort and page size;
12. live authorization, resource/field visibility, field redaction, FORCE RLS and cross-tenant non-disclosure are enforced;
13. process restart/retry cannot duplicate findings, observations, assignments, remediation attempts or Party side effects.

#### Delivered application surface

- evaluation-job get;
- finding get;
- finding list by Party;
- assigned-finding list;
- completeness-result get;
- immutable definition gets;
- assign, acknowledge and waive finding mutations;
- governed Party display-name remediation;
- deterministic target idempotency and immutable remediation-attempt output;
- target-success/outcome-missing crash recovery;
- pass-driven reevaluation to `REMEDIATED`.

#### Acceptance evidence

Final source-authored candidate `c066c278edd75b5f78bbfcead792d34164c76ff5` passed all 15 applicable workflows unchanged before merge, including:

- Contract and Governance checks;
- current Cargo lockfile, rustfmt, Clippy and all workspace tests;
- Database and Application Runtime checks;
- eight fresh-PostgreSQL Data Quality process scenarios;
- signed cursor tamper rejection;
- authorization denial, field redaction and cross-tenant concealment for evaluation get;
- stewardship concurrency and lifecycle behavior;
- governed remediation crash recovery with no duplicate Party update;
- strict persisted remediation evidence and FORCE RLS.

### 8A.10 — Governed Customer Enrichment and Provenance — Ready

Issue: #125
Depends on: completed #124 / merged PR #132 and completed architecture integrity #134 / merged PR #135 (`023fa5ef1d510d5bcc32222c739e6d58e5696fb8`)

This is the next customer-master production packet and must branch from the accepted native-composition baseline.

Deliver a distinct enrichment ownership boundary, module-owned production contributions, provider adapter boundaries, secret handles, versioned mappings, source/freshness/licensing provenance, review/approval policy where required and exact owner-capability application of accepted changes. Provider I/O remains an infrastructure adapter and must not enter the pure business core.

### 8A.11 — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — Planned

Issue: #126
Depends on: #123, completed #124 and #125

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

Begin only after the Phase 8A customer-master baseline is complete.

Planned owner-domain packets include Product/Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements and governed billing/ERP/payment/tax/fulfillment integration boundaries.

Catalog, Pricing, Quote, Order, Contract, Subscription and Billing integration ownership must remain explicit and must not be absorbed into Sales.

## 5. Wave 8C — Sales and Productivity Expert Expansion

Deliver leads/prospects and qualification, richer opportunity pipelines, appointments and recurring work, calendar synchronization boundaries, routing/queues, territories, quotas, forecasting, renewals/expansion, sequences/playbooks and product-quality list/Kanban/mobile/offline workflows.

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

AI remains an authenticated audited Actor using permission-scoped governed tools. It has no alternate mutation, consent, identity-resolution or data-export path.

### Phase 10 — signed marketplace and sandbox

Untrusted extensions remain signed, permissioned and sandboxed with explicit capability/data/network/secret grants and lifecycle controls.

### Phase 11 — enterprise security, resilience and production proof

Enterprise hardening remains continuous and culminates in production evidence for identity federation, provisioning, authorization, encryption, audit export, privacy/legal hold, backup/restore, tenant mobility/residency, security testing, load/chaos testing, SLOs and operational runbooks.

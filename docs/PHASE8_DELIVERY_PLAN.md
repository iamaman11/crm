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

- `crm.customer-data-operations` as the governed owner of import-job/source/mapping/row/checkpoint evidence;
- immutable source artifacts with sequential chunks, replay protection, exact byte length and SHA-256 finalization;
- immutable source-system identity and parser/import profile;
- explicit separation of source external identifiers from canonical CRM Party IDs;
- server-side parsing and validation of finalized source bytes;
- true dry-run proof with zero Party records, Party target idempotency, Party outbox or Party mutation-audit side effects;
- deterministic row identity and target Party idempotency;
- exact Party owner-capability execution through `GatewayCapabilityClient` with no direct Party storage writes;
- durable success, invalid-skip, retryable-failure, checkpoint and completion evidence;
- distinct restart-stable business transaction identities for target mutations and import-owned outcomes;
- fresh-PostgreSQL process proof for target-success/checkpoint crash recovery without duplicate Party creation;
- fresh-PostgreSQL process proof for durable retryable target failure without checkpoint advancement followed by successful restart/retry recovery;
- tenant non-disclosure, field visibility and signed-cursor tamper rejection;
- all applicable exact-head workflows green before merge.

### 8A.8 — Customer Export Jobs, Artifacts and Reconciliation Evidence — Complete

Delivered by #123 / PR #130.  
Merge commit: `0e7f9889362533446cc65d95dcf7969a60086a57`.

Implemented:

- immutable versioned Party export specifications with bounded maximum resource count;
- atomic first-start creation of one immutable creation-time selection boundary and durable progress;
- restart-safe opaque keyset continuation and deterministic Party creation ordering;
- deterministic manifest items bound to exact `PartyRef + resource_version` evidence and an exact boundary-bound manifest digest;
- governed Party selection/execution reads with separate top-level authorization and per-resource/field visibility;
- deterministic exclusion of not-visible, version-changed or unavailable selected resources;
- deterministic spreadsheet-safe UTF-8 CSV canonicalization;
- approval-required production Party export execution;
- deterministic staged artifact chunks and per-manifest-position outcome identities;
- checkpoint advancement only from a contiguous durable outcome prefix;
- exact reconciliation `selected = emitted + excluded_not_visible + excluded_version_changed + excluded_unavailable`;
- deterministic immutable artifact identity, chunk/hash/size evidence and replay-safe finalization;
- recovery from chunk-written/outcome-checkpoint-missing and artifact-finalized/completion-missing crash windows without duplicate logical artifacts;
- live-authorized, per-resource-visible, retention-aware, integrity-verified and audited artifact disclosure;
- rejection of unauthenticated, cross-tenant, cancelled/not-ready and expired artifact disclosure;
- fresh-PostgreSQL real `crm-api` process acceptance;
- all 14 applicable workflows green together on unchanged human-authored candidate `f219d9b418ed07a9328bb44d36cfb9f321ad9be3` before merge.

Exported bytes remain derived artifacts and never become authoritative customer-master state.

### 8A.9 — Customer Data Quality Rules, Completeness and Stewardship — Ready

Issue: #124  
Depends on: completed #123 / merged PR #130 (`0e7f9889362533446cc65d95dcf7969a60086a57`)

This is the **next customer-master production packet**. It becomes **In progress** only when its implementation branch/draft PR is created from the synchronized post-8A.8 `main` baseline.

#### Ownership boundary

Introduce a distinct `crm.data-quality` owner/coordinator for long-lived quality-governance state.

`crm.data-quality` may own only:

- immutable/versioned quality rule-set definitions;
- immutable/versioned completeness-profile definitions;
- deterministic evaluation-run identity plus bounded checkpoint/retry evidence where asynchronous evaluation is required;
- quality finding identity, exact evaluated owner/resource/resource-version evidence, rule version, severity and lifecycle;
- completeness result identity, exact component lineage and deterministic score evidence;
- stewardship case/queue assignment, triage status and remediation-attempt evidence;
- bounded safe diagnostics and reconciliation counters.

It must not own or copy authoritative mutable Party, Account, Contact Point, Party Relationship, Consent or Identity Resolution state. It does not own import/export artifacts, enrichment-provider payloads or a generic enterprise workflow engine.

Authoritative values are read only through governed owner/query composition ports with tenant/RLS and live authorization. No direct cross-module table reads or writes are permitted. Remediation may change owner state only by invoking an exact governed owner capability with normal authorization, optimistic concurrency, idempotency, approval and audit semantics.

#### Frozen safety strategy

Before public contract publication, the first v1 architecture must preserve these rules:

1. published rule/evaluator versions are immutable and cannot be silently reinterpreted;
2. rule execution uses a bounded declarative vocabulary or exact built-in evaluator identities;
3. arbitrary SQL, user code/scripts, filesystem access, arbitrary network access and unbounded expressions are forbidden;
4. every finding and completeness result binds exact authoritative owner/resource/resource-version evidence;
5. stale findings are not silently treated as current after the authoritative resource version changes;
6. deterministic reevaluation must not create duplicate logical current findings;
7. historical evaluation/finding evidence is retained when status becomes acknowledged, remediated, waived or stale/superseded;
8. completeness uses deterministic integer/fixed-point semantics with exact component lineage and reconciliation;
9. stewardship assignment/triage/remediation uses optimistic concurrency and bounded lifecycle transitions;
10. remediation results are separate evidence and cannot rewrite the original evaluation truth;
11. search, Customer 360 and projections may assist discovery but are not authoritative quality evidence without exact authoritative source-version lineage;
12. process restart/retry cannot duplicate findings, assignments or owner side effects.

#### Initial production vertical slice

The first v1 implementation proves the architecture against canonical Party quality before broadening to additional customer-master owners. The contract must remain additively extensible without introducing a generic untyped record dump or executable rule surface.

The implementation branch must freeze before public contract publication:

- exact Party evaluator/rule kinds included in v1;
- deterministic rule-set/version identity;
- deterministic finding identity and reevaluation semantics;
- completeness profile component and score canonicalization;
- authoritative Party read port and source-version binding;
- stewardship lifecycle and exact remediation capability boundary;
- event/outbox model and replay semantics;
- persistence, FORCE RLS, migration rollback/reapply and retention behavior;
- application-runtime worker scheduling, fairness, retry and readiness behavior;
- fresh-PostgreSQL real `crm-api` process acceptance.

#### Non-negotiable acceptance gates

- deterministic rule evaluation and exact replay;
- published rule versions cannot be reinterpreted silently;
- no arbitrary code/SQL/network/filesystem execution path;
- no direct cross-owner storage mutation or read bypass;
- every finding/completeness result binds exact authoritative source-version evidence;
- stale source-version/remediation conflict proof;
- deterministic reevaluation without duplicate logical current findings;
- historical finding/evaluation evidence retained across lifecycle changes;
- permission-aware field/resource disclosure and cross-tenant non-disclosure;
- exact completeness score/component reconciliation;
- stewardship assignment and lifecycle concurrency proof;
- remediation invokes only exact governed owner capabilities and preserves normal authorization/idempotency/audit behavior;
- process restart/retry without duplicate findings, assignments or owner side effects;
- bounded scans, batches, payloads and per-tenant operational limits;
- migration clean apply, reverse rollback and reapply;
- fresh-PostgreSQL real `crm-api` process acceptance;
- Contract, Governance, Rust, Database, Application Runtime and every other applicable workflow green on one unchanged final SHA.

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

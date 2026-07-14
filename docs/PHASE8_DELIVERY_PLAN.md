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

### 8A.8 — Customer Export Jobs, Artifacts and Reconciliation Evidence — In progress

Issue: #123  
Depends on: completed #120 / merged PR #121

This is the **single active customer-master production packet**.

#### Ownership boundary

`crm.customer-data-operations` may own only:

- export-job identity and lifecycle;
- immutable versioned export specification/profile identity;
- selected bounded customer-master resource scope;
- snapshot/watermark or equivalent immutable selection evidence;
- execution checkpoints and resumable evidence;
- derived export artifact references and lifecycle metadata;
- selected/emitted/excluded/redacted counts and reconciliation evidence;
- bounded safe diagnostics.

It must not own or copy authoritative mutable Party, Account, Contact Point, Party Relationship, Consent or Identity Resolution records. Exported bytes are derived artifacts, never a new source of truth.

#### Contract requirements before broad implementation

The v1 packet must freeze one exact deterministic strategy for:

1. export job identity and immutable export-profile/specification identity;
2. supported bounded resource scopes and output format/profile semantics;
3. stable snapshot/watermark or equivalent immutable source-selection evidence;
4. governed owner-domain reads without direct cross-module table access;
5. live resource authorization and field/data-class filtering before serialization;
6. staged artifact creation with no partial publication;
7. exactly-once logical artifact finalization with digest, byte-size, retention and expiry evidence;
8. deterministic retry/resume after interruption;
9. reconciliation of selected resources against emitted rows, exclusions and redactions;
10. tenant isolation and safe non-disclosure.

#### Required production layers

- additive versioned public export mutation/query/event contracts;
- domain model and lifecycle invariants for export jobs and reconciliation evidence;
- immutable export profile/specification identity;
- owner-domain query/composition adapters for the selected v1 resource scope;
- PostgreSQL persistence, FORCE RLS and migrations;
- staged derived-artifact writer/finalizer using governed file/artifact infrastructure;
- deterministic checkpoint/replay semantics;
- permission-aware job/artifact/reconciliation queries with signed cursors where listing is exposed;
- application-runtime execution worker and restart recovery;
- fresh-PostgreSQL real `crm-api` process acceptance;
- one unchanged exact final SHA with every applicable workflow green.

#### Non-negotiable acceptance gates

- export profile/specification validation with unknown-field rejection;
- same immutable job intent cannot be silently reinterpreted under changed profile semantics;
- live authorization and field/data-class visibility are repeated during execution;
- no privacy, consent, masking, restriction or legal-hold bypass through export;
- no direct cross-module authoritative storage reads;
- no partial artifact publication before finalization;
- deterministic retry/resume without duplicate logical artifacts;
- crash/restart acceptance across at least one artifact-finalization uncertainty window;
- exact artifact digest, byte size and lifecycle evidence;
- exact selected/emitted/excluded/redacted reconciliation counts;
- cross-tenant non-disclosure for jobs and artifact references;
- migration clean apply, rollback and reapply;
- Contract, Governance, Rust, Database, Application Runtime and every other applicable workflow green on one unchanged final SHA.

### 8A.9 — Customer Data Quality Rules, Completeness and Stewardship — Planned

Issue: #124  
Depends on: #123

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

AI is an authenticated audited Actor using permission-scoped governed tools. It has no alternate mutation, identity-merge, consent or authorization path.

### Phase 10 — signed marketplace and sandbox

Deliver signed packages, publisher identity, explicit grants, SBOM/provenance checks, sandboxed untrusted execution, quotas, kill switches and safe lifecycle operations.

### Phase 11 — enterprise and production proof

Deliver OIDC/SAML, SCIM, key hierarchy/encryption, legal hold, WORM audit export, backup/PITR/restore, tenant mobility/residency, security testing, SLOs and runbooks.

## 15. Immediate sequence

1. Keep #123 as the single active Phase 8A production packet.
2. Freeze the 8A.8 export ownership, snapshot, artifact and reconciliation contract.
3. Deliver 8A.8 through contracts, domain, persistence, runtime composition and fresh-process acceptance.
4. Deliver #124 -> #125 -> #126.
5. Close Phase 8A only after the full merged acceptance baseline is proven.
6. Begin 8B / #29 from the completed customer-master baseline.

No packet is product-complete merely because schemas, manifests or crates exist.

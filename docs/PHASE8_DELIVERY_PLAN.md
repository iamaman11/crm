# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Commercial follow-on: #29  
Delivery governance: `DELIVERY_GOVERNANCE.md`  
Functional scope guardrail: `CRM_CAPABILITY_COVERAGE.md`

## 1. Goal

Build the expert CRM domain layer on top of the completed governed platform without collapsing ownership into Sales, creating a giant long-lived Phase 8 branch or weakening compatibility, tenant isolation, authorization, audit, rollback and exact-SHA evidence.

Phase 8 is delivered as reviewable owner-domain and product packets. Every packet ends at a natural architecture boundary with explicit acceptance gates.

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
- exact route classification with no central business switches;
- real process/browser/operational acceptance gates.

A packet may be marked **Complete** only after merge to `main`. Every later source or documentation change invalidates earlier exact candidate evidence until applicable checks rerun.

## 3. Wave 8A — canonical customer master, identity, consent and customer-data lifecycle

### 8A.1–8A.6 — Complete

Delivered customer reference foundations, Party lifecycle/search, Account, Contact Point, Party Relationship, Customer 360, Consent/Communication Authorization, explainable duplicate candidates and approval-required reversible merge/unmerge with immutable provenance.

### 8A.7 — Customer Import — Complete

Delivered governed immutable sources, server-side parsing/validation, true dry-run, deterministic row/target identity, resumable Party owner-capability execution and fresh-PostgreSQL crash/retry acceptance.

### 8A.8 — Customer Export — Complete

Delivered immutable export specifications/manifests, governed Party reads, deterministic CSV artifacts, exact reconciliation, both crash-window recoveries and live-authorized audited artifact disclosure.

### 8A.9 — Customer Data Quality — Complete

Issue #124 / PR #132 / merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`.

Delivered immutable Party rule/completeness definitions, exact-version evaluation, findings/observations/completeness lineage, stewardship lifecycle, governed Party remediation, signed pagination, FORCE RLS and restart/crash recovery.

### 8A.10 — Governed Customer Enrichment and Provenance — Complete

Issue: #125  
PR: #137  
Accepted source checkpoint: `f92d101206886e3ceaf94d0e56e52580cec21093` — 17/17 permanent workflows successful unchanged  
Merge: `150e44b95d9dbdc08c1792563de03ec73f34aed1`

#### Frozen production inventory

- 6 public mutations;
- 6 permission-aware queries;
- 2 activation-gated worker coordinates;
- 3 provider/materialization coordinates classified worker-only with no public HTTP/gRPC ingress.

The machine-readable source of truth is `contracts/customer-enrichment-production-promotion.json`.

#### Ownership and architecture boundary

`crm.customer-enrichment` owns enrichment requests and immutable provider/mapping, response, conflict, suggestion, review, usage and application evidence. It does not own mutable Party, Account, Contact Point, Consent, Identity Resolution or Data Quality values.

Provider I/O, credentials and PostgreSQL transaction guards remain host-owned infrastructure outside the pure module core. Accepted changes re-enter the exact authoritative Party capability `parties.party.update@1.0.0`.

#### Delivered production behavior

- immutable content-derived provider-profile and mapping versions;
- deterministic request, response, conflict, suggestion, review and application identities;
- exact-coordinate registry HTTP transport with endpoint allowlisting, bounded bodies/deadlines, redirect rejection and sanitized failures;
- tenant-bound secret resolution without credential leakage;
- quota and circuit behavior;
- commit-before-provider-I/O and crash-safe recovery using the same provider idempotency lineage;
- independent live dispatch and response authorization;
- exact/semantic duplicate reconciliation and fail-closed canonical response conflicts;
- immutable operator resolution evidence for retain-first and terminal request rejection;
- deterministic materialization and owner-application recovery;
- permission-aware provider/mapping/request/suggestion reads with declarative redaction;
- live activation shutdown, disable/uninstall behavior and cross-tenant concealment;
- FORCE RLS and migration rollback/reapply proof;
- transaction-scoped immutable provider-profile and exact Party-version guards.

#### Real `crm-api` acceptance

The permanent fresh-database Application Runtime step starts the real `crm-api` binary and proves:

- unauthenticated HTTP returns bounded `401 {"error":"request_failed"}`;
- Party creation, provider-profile publication, mapping publication and legitimate-interest enrichment-request persistence succeed through real gRPC ingress;
- confidential profile definition is redacted by deployment field ceiling;
- cross-tenant lookup is concealed as `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND`;
- a tenant outside the token grant receives `TENANT_FORBIDDEN`;
- missing Consent evidence receives `CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED`;
- live suspension receives `MODULE_NOT_ACTIVE`;
- bootstrap-disabled live permission receives `CAPABILITY_PERMISSION_DENIED`;
- typed gRPC codes are non-retryable and safe, while HTTP hides governed details;
- credential/provider/internal markers never reach the public surface;
- request/event/audit/idempotency/business-transaction counters do not change after pre-persistence denials.

### 8A.11 — Customer Privacy Lifecycle — Ready

Issue: #126  
Depends on: merged 8A.10 at `150e44b95d9dbdc08c1792563de03ec73f34aed1`

Deliver governed privacy request lifecycle, access/export, live restriction enforcement, owner-aware deletion/anonymization planning, retention/legal-hold conflict handling and downstream search/projection convergence with immutable evidence preservation where required.

Before contract expansion, freeze:

- privacy request and case ownership;
- live restriction decision and enforcement boundaries;
- access/export relation to existing governed export artifacts;
- deletion/anonymization plans across authoritative owners;
- legal hold and retention conflict semantics;
- evidence that must survive deletion or anonymization;
- exact owner-capability and worker contributions;
- process, migration, rollback and cross-tenant acceptance matrix.

### Phase 8A completion gate

Phase 8A is complete only when merged `main` proves:

- canonical stable customer identities and references;
- Account, Contact Point and Party Relationship ownership;
- permission-aware Customer 360;
- authoritative Consent and immediate withdrawal semantics;
- explainable duplicate candidates and reversible merge/unmerge;
- deterministic import/export and data-quality/stewardship evidence;
- governed enrichment provenance;
- privacy access/export/restriction/deletion/legal-hold interactions;
- cross-tenant isolation, compatibility, migration rollback/reapply and production acceptance evidence.

## 4. Wave 8B — Product Catalog and Quote-to-Revenue

State: **Planned**  
Issue: #29

Begin only after the Phase 8A baseline is complete. Planned owners include Product/Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements and governed billing/ERP/payment/tax/fulfillment boundaries. None may be absorbed into Sales.

## 5. Additional Phase 8 waves

- Sales and productivity expert expansion;
- communications and omnichannel;
- Service/Support, Knowledge and optional Field Service;
- Marketing and growth;
- Customer Success and optional partner/channel CRM;
- projects, configurable work, documents and e-signature;
- analytics, reporting and performance management;
- workflow, collaboration and product completeness.

Every wave must include replay-safe contracts, authorization, tenant isolation, data/provenance expectations, credential boundaries, quotas, observability, recovery and synchronized governance state.

## 6. Later platform programs

### Phase 9 — AI-native CRM

AI remains an authenticated audited Actor using permission-scoped governed tools. It has no alternate mutation, Consent, identity-resolution or data-export path.

### Phase 10 — signed marketplace and sandbox

Untrusted extensions remain signed, permissioned and sandboxed with explicit capability/data/network/secret grants and lifecycle controls.

### Phase 11 — enterprise security, resilience and production proof

Enterprise hardening remains continuous and culminates in identity federation/provisioning, authorization, encryption, audit export, privacy/legal hold, backup/restore, residency, supply-chain/security testing, load/chaos proof, SLOs, alerting, incident response and runbooks.

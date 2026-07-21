# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Commercial follow-on: #29  
Delivery governance: `DELIVERY_GOVERNANCE.md`  
Functional scope guardrail: `CRM_CAPABILITY_COVERAGE.md`

## 1. Goal

Build the expert CRM domain layer on top of the completed governed platform without collapsing ownership into Sales, creating giant long-lived branches or weakening compatibility, tenant isolation, authorization, audit, rollback and exact-SHA evidence.

Every packet ends at a natural architecture boundary with explicit contracts, persistence, activation, authorization, recovery and real-process acceptance.

## 2. Packet contract

Every Phase 8 packet defines:

- authoritative owner domain and stable references;
- public capability/query/event contracts;
- persistence, tenant and authorization model;
- audit, idempotency and approval requirements;
- projection/search/cache implications;
- frontend/product workflow where applicable;
- import/export and compatibility consequences;
- module-owned routes, validators, visibility and workers;
- exact public/worker/non-runtime classifications;
- real process/browser/operational acceptance gates.

A packet may be marked **Complete** only after merge to `main`. Every later source or documentation change invalidates earlier exact candidate evidence until applicable checks rerun.

## 3. Wave 8A — canonical customer master, identity, consent and customer-data lifecycle

### 8A.1–8A.6 — Complete

Delivered customer references, Party lifecycle/search, Account, Contact Point, Party Relationship, Customer 360, Consent/Communication Authorization, explainable duplicate candidates and approval-required reversible merge/unmerge with immutable provenance.

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
Accepted source: `f92d101206886e3ceaf94d0e56e52580cec21093`  
Merge: `150e44b95d9dbdc08c1792563de03ec73f34aed1`

#### Frozen production inventory

- 6 public mutations;
- 6 permission-aware queries;
- 5 activation-gated worker-only coordinates;
- no completed Customer Enrichment non-runtime coordinates.

The machine-readable source of truth is `contracts/customer-enrichment-production-promotion.json`.

#### Ownership and architecture

`crm.customer-enrichment` owns enrichment requests and immutable provider/mapping, response, conflict, suggestion, review, usage and application evidence. It does not own mutable Party, Account, Contact Point, Consent, Identity Resolution or Data Quality values.

Provider HTTP, credentials, quotas/circuits and PostgreSQL transaction guards remain host-owned infrastructure outside the pure module core. Accepted values enter authoritative Party state only through `parties.party.update@1.0.0`.

#### Accepted production behavior

- immutable content-derived provider-profile and mapping versions;
- deterministic request, response, conflict, suggestion, review and application identities;
- exact registry HTTP transport with endpoint allowlisting, bounded bodies/deadlines, redirect rejection and sanitized failures;
- tenant-bound secret resolution without credential leakage;
- quota and circuit behavior;
- commit-before-provider-I/O and crash-safe recovery using the same provider idempotency lineage;
- independent live dispatch, response, materialization and application authorization;
- exact/semantic duplicate reconciliation and fail-closed canonical response conflicts;
- immutable retain-first and terminal-reject operator resolution evidence;
- deterministic materialization and owner-application recovery;
- permission-aware provider/mapping/request/suggestion reads with declarative redaction;
- live activation shutdown, disable/uninstall and cross-tenant concealment;
- transaction-scoped provider-profile and exact Party-version guards;
- FORCE RLS and migration rollback/reapply proof.

#### Production acceptance

A permanent fresh-database Application Runtime step starts the real `crm-api` binary and proves successful public persistence plus bounded authentication, tenant, visibility, Consent, activation and authorization denials through actual HTTP/gRPC ingress.

Dedicated fresh-PostgreSQL provider/materialization/review/application workflows exercise worker-only coordinates, exact transport and secret boundaries, replay/reconciliation, crash windows and owner-application recovery. Background registration tests prove exact phase order 240 → 245 → 250 and disable/uninstall shutdown.

All 17 permanent workflows passed on the unchanged accepted source SHA before merge.

### 8A.11 — Customer Privacy Lifecycle — In progress

Issue: #126  
Architecture-freeze PR: #140  
Depends on: merged and synchronized 8A.10

#### Objective

Deliver governed privacy request/case lifecycle, subject/resource discovery, access/export, live restrictions, owner-aware deletion/anonymization planning, retention/legal-hold conflict resolution and downstream convergence without losing immutable evidence required by law, audit or system integrity.

#### Frozen authoritative owner boundary

`crm.customer-privacy` owns privacy cases, verified subject binding, immutable scope snapshots, current restrictions, customer-data legal holds, retention decisions, deterministic plans, per-owner attempts/outcomes, orchestration checkpoints, governed export references and convergence evidence.

It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Import/Export, Data Quality or Enrichment storage. Those modules remain authoritative and participate only through exact module-owned privacy capabilities. Derived projections/search/caches remain non-authoritative.

#### Frozen initial inventory

The machine-readable authority is `contracts/customer-privacy-architecture-freeze.json`.

- 9 public mutations;
- 7 permission-aware public queries;
- 9 trusted worker/internal coordinates in phases 260 → 270 → 280 → 290;
- 1 non-runtime crypto-shredding coordinate pending subject-scoped key architecture.

#### Required behavior

- immutable privacy case/request identity and lifecycle;
- exact subject identity/canonical redirect handling;
- bounded owner-resource discovery with live visibility;
- access/export assembly using governed Customer Data Operations disclosure and artifact controls;
- immediate processing/communication restriction through a shared tenant + canonical Party lock;
- deterministic owner/data-class deletion or anonymization plans;
- explicit retention and legal-hold precedence/conflict evidence;
- resumable per-owner execution with deterministic idempotency and no duplicate effects;
- search/projection/cache tombstone or rebuild convergence;
- preservation of audit, merge lineage, Consent, provenance and legal evidence where deletion is prohibited;
- non-reusable erased Party tombstones and no orphan references;
- bounded safe errors, tenant isolation and fail-closed enforcement;
- tenant-aware crypto-shredding only after key ownership, legal-hold, backup and restore semantics exist.

#### Acceptance gate

The implementation packet must prove access/export, immediate restriction including races, legal-hold blocking, deletion/anonymization convergence, immutable-evidence preservation, restart recovery, cross-tenant denial, migration rollback/reapply and real-process behavior on one unchanged exact SHA.

The architecture-freeze PR itself claims no Protobuf, manifest, migration, production-route or runtime implementation. Contract expansion begins only after the freeze is accepted.

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

Every wave includes replay-safe contracts, authorization, tenant isolation, data/provenance expectations, credential boundaries, quotas, observability, recovery and synchronized governance state.

## 6. Later platform programs

### Phase 9 — AI-native CRM

AI remains an authenticated audited Actor using permission-scoped governed tools. It has no alternate mutation, Consent, identity-resolution, privacy or data-export path.

### Phase 10 — signed marketplace and sandbox

Untrusted extensions remain signed, permissioned and sandboxed with explicit capability/data/network/secret grants and lifecycle controls.

### Phase 11 — enterprise security, resilience and production proof

Enterprise hardening remains continuous and culminates in identity federation/provisioning, authorization, encryption, WORM audit export, privacy/legal hold, backup/restore, residency, supply-chain/security testing, load/chaos proof, SLOs, alerting, incident response and runbooks.

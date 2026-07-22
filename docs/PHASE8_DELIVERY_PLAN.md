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

Frozen production inventory:

- 6 public mutations;
- 6 permission-aware queries;
- 5 activation-gated worker-only coordinates;
- no completed Customer Enrichment non-runtime coordinates.

Accepted behavior includes immutable provider/mapping/request/response/conflict/suggestion/review/usage/application evidence, exact transport and secret isolation, quota/circuit controls, commit-before-I/O, independent live worker authorization, replay/reconciliation/recovery, governed Party owner-capability application, permission-aware reads, transaction-scoped reference guards, FORCE RLS and permanent real-process acceptance.

### 8A.11 — Customer Privacy Lifecycle — In progress

Issue: #126  
Architecture and foundation PRs: #140–#145  
Accepted production coordinates: PRs #146–#150  
Merged production inventory: 4 mutations + 1 query + 11 public non-runtime coordinates + 0 Customer Privacy workers  
Active bounded candidate: draft PR #152 / `customer_privacy.case.list@1.0.0`

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

#### Merged foundation

- PR #140 — architecture and guardrail freeze;
- PR #141 — owner foundation;
- PR #142 — deterministic pure-domain lifecycles;
- PR #143 — canonical private persistence;
- PR #144 — immutable public Protobuf contracts;
- PR #145 — FORCE RLS persistence proof.

PR #145 accepted source `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merge `721a1cf185ffbdea309bd1199c6c4568cf82d7a1` prove clean migrations, FORCE RLS under the non-privileged runtime role, tenant isolation/concealment, rollback/schema removal/reapply and strict record-envelope rehydration.

#### Bounded packet 8A.11.1 — `case.create` — Complete

PR #146 accepted unchanged source `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`.

Accepted: deterministic tenant/idempotency case identity, confidential Draft/version-1 state, exact predecessor lineage validation, replay/conflict, one atomic record/event/audit/idempotency batch, generic ingress, live authorization/activation, FORCE RLS and permanent real-process acceptance.

#### Bounded packet 8A.11.2 — `case.submit` — Complete

PR #147 accepted unchanged source `8b41e8420b1a897777596c68cb615e2b8bf80c34`, passed all 18 permanent workflows and merged as `0eba56084405301eb667f2173b3aef6565b95f87`.

Accepted: strict canonical rehydration, optimistic `Draft -> Submitted`, atomic status evidence, exact replay, incompatible replay, stale/wrong-state/cross-tenant/malformed rollback, generic ingress and clean/reapplied FORCE-RLS process proof.

#### Bounded packet 8A.11.3 — `case.subject.verify` — Complete

PR #148 accepted unchanged source `118327e09a6e31ba87b02bdab99289035b572ed9`, passed all 18 permanent workflows and merged as `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`.

Accepted: authoritative Party visibility, canonical redirect/active merge lineage, monotonic topology generation, shared topology/canonical-subject locks, atomic `Submitted N -> SubjectVerified N + 1`, replay/conflict/concealment/malformed/lock-contention proof and generic real HTTP/gRPC ingress.

#### Bounded packet 8A.11.4 — `case.get` — Complete

PR #149 accepted unchanged post-sync source `5a47318b24007cd534434ff6bac33fbd59215d38`, passed all 18 permanent workflows and merged as `5d580a7c253bcfa6c2dd981100612b222fd26825`.

Accepted: exact permission-aware query contract, FORCE-RLS lookup, strict aggregate rehydration, live case/canonical-Party visibility, concealment, field redaction, generic ingress and zero query-side writes.

#### Bounded packet 8A.11.5 — `case.cancel` — Complete

PR #150 accepted unchanged post-sync source `be05e874b21ab33cb8b6a84fbcefc3c025aa88cb`, passed all 18 permanent workflows and was squash-merged as `2a4c34727e9d7bf8ed51b6411b7ab9c76c109671`.

Accepted: exact optimistic terminal cancellation, immutable lineage preservation, sorted/deduplicated subject locks before a retained final case-row `FOR UPDATE`, direct row serialization for unbound cases, retryable TOCTOU denial, exact replay/conflict and permanent real-process proof. Merged production parity is four mutations, one query and eleven non-runtime public coordinates.

#### Bounded packet 8A.11.6 — `case.list` — Gate review

Draft PR #152 promotes exactly one additional public coordinate:

`customer_privacy.case.list@1.0.0`

Candidate packet:

- requires a canonical Party reference and accepts optional kind/status filters;
- defaults page size to 50 and caps it at 100;
- signs a cursor bound to tenant, actor, exact capability/version, Party, filters, updated-at sort and page size;
- uses the shared read-only FORCE-RLS keyset scan and fails retryably after 4096 scanned candidates;
- strictly rehydrates every candidate and matches only authoritative verified canonical subject bindings;
- never treats an unbound case or pending-rescope target as disclosure authority;
- applies live Party visibility before scan and live case visibility plus field redaction before output;
- uses generic application query composition with live authorization and module activation;
- writes no records, events, audits, outbox, idempotency or business transactions;
- permanently proves real HTTP/gRPC pagination, no duplicates, subject/kind/status filtering, cursor tamper and filter rebinding denial, cross-tenant empty concealment, redaction, suspension and absent grants;
- changes candidate route parity to four runtime mutations, two runtime queries, ten non-runtime public coordinates and zero Customer Privacy workers.

Explicit exclusions:

- `case.approve`;
- `case.plan.get` and `case.owner_outcomes.list`;
- restriction routes;
- legal-hold routes;
- worker/internal coordinates;
- owner execution;
- crypto-shred.

#### Remaining required behavior

- access/export assembly using governed Customer Data Operations disclosure and artifact controls;
- immediate processing/communication restriction using the accepted subject lock;
- deterministic owner/data-class deletion or anonymization plans;
- explicit retention and legal-hold precedence/conflict evidence;
- resumable per-owner execution with deterministic idempotency and no duplicate effects;
- plan/outcome disclosure with bounded visibility;
- search/projection/cache tombstone or rebuild convergence;
- preservation of audit, merge lineage, Consent, provenance and legal evidence where deletion is prohibited;
- non-reusable erased Party tombstones and no orphan references;
- tenant-aware crypto-shredding only after key ownership, legal-hold, backup and restore semantics exist.

#### Completion rule

Acceptance of the five merged coordinates and candidate `case.list` does not complete Phase 8A.11. Each later coordinate or tightly coupled lifecycle slice requires its own bounded production proof and exact route reclassification. Phase 8A.11 completes only after the full privacy lifecycle and worker/convergence acceptance is merged.

### Phase 8A completion gate

Phase 8A is complete only when merged `main` proves canonical customer identity/reference ownership, permission-aware Customer 360, authoritative Consent, reversible Identity Resolution, deterministic import/export/Data Quality/Enrichment evidence, complete privacy access/export/restriction/deletion/legal-hold interactions, tenant isolation, migration safety and production acceptance.

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

# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Customer Privacy packet: #126  
Commercial follow-on: #29  
Architecture/developer-experience program: #194  
Delivery governance: `DELIVERY_GOVERNANCE.md`  
Functional scope guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Architecture scalability guardrail: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`

## 1. Goal

Build the expert CRM domain layer on the completed governed platform without collapsing ownership into Sales or weakening compatibility, tenant isolation, authorization, audit, rollback, operations and exact-SHA evidence.

Every packet ends at a natural owner boundary with explicit contracts, persistence, activation, authorization, recovery, product experience and real-process acceptance where runtime behavior is introduced.

Phase 8 must also prove that product breadth can grow without proportional growth in crates, central manual composition and unrelated CI cost.

## 2. Packet contract

Every Phase 8 packet defines:

- authoritative owner and stable resource identity;
- public, worker and non-runtime classifications;
- persistence, tenant and authorization boundaries;
- audit, idempotency, approval and retention implications;
- projection/search/cache and compatibility consequences;
- exact production contribution and activation path;
- exact acceptance gates and rollback evidence;
- architecture complexity impact;
- frontend/product and operational impact where applicable.

A packet is **Complete** only after merge to `main`. Every later source or documentation change invalidates earlier exact-head evidence until applicable checks rerun.

## 3. Architecture rules for Phase 8

1. A normal capability added to an existing owner creates zero new crates.
2. A new owner domain targets three to five technical packages.
3. New crates require a real dependency, trust, reuse, process, lifecycle or extraction boundary.
4. Generic router and worker algorithms do not change merely to register one owner capability.
5. Every owner converges on one module-owned production contribution entry point.
6. Feature implementation and physical crate consolidation use separate PRs.
7. Shared behavior is extracted only after contrasting implementations prove it.
8. Iterative affected-scope checks may be focused; final exact-head acceptance remains complete.
9. Each packet reports files/packages touched, new crates, contribution/runtime impact and required workflows.
10. Phase 8B and later waves must demonstrate that architecture cost remains bounded as module count grows.

## 4. Wave 8A — canonical customer master, identity, consent and customer-data lifecycle

### 8A.1–8A.6 — Complete

Delivered customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent/Communication Authorization and explainable reversible Identity Resolution.

### 8A.7 — Customer Import — Complete

Delivered immutable sources, parsing/validation, deterministic row identity, resumable Party capability execution and crash/retry recovery.

### 8A.8 — Customer Export — Complete

Delivered immutable export specifications/manifests, governed Party reads, deterministic artifacts, reconciliation and both crash-window recoveries.

### 8A.9 — Customer Data Quality — Complete

Delivered immutable Party rule/completeness definitions, exact-version evaluation, findings/observations, stewardship, governed remediation, FORCE RLS and recovery.

### 8A.10 — Governed Customer Enrichment and Provenance — Complete

Issue #125 / PR #137 / merge `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

Frozen production inventory:

- six public mutations;
- six permission-aware queries;
- five activation-gated worker-only coordinates;
- accepted contract-only Customer Privacy owner contribution.

### 8A.11 — Customer Privacy Lifecycle — In progress

Issue: #126

Merged runtime inventory:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

All nine owner-scope implementations are accepted:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Post-merge documentation synchronization was accepted through PR #193 / merge `e09d3152c886386c2168f0b49e46d47cc44ed041`.

The owner implementation lane is complete. No accepted owner may be described as unstarted.

All nine coordinates remain contract-only/non-runtime. They add no public ingress, Customer Privacy worker, production discovery, planning or owner action execution.

## 5. Next bounded packet — Scope discovery and immutable snapshot

State: **Ready for packet freeze and implementation; production discovery is not implemented**.

Before runtime work, inspect:

- existing Customer Privacy schema and migrations;
- case aggregate and lifecycle;
- Protobuf contracts and route classifications;
- persistence adapter and transaction semantics;
- identity topology and canonical Party binding;
- module registry/version/digest authority;
- authorization, visibility, audit and RLS boundaries;
- all nine accepted owner contribution contracts.

Freeze and prove:

1. deterministic invocation of all nine exact owner coordinates;
2. exact registry version/digest and owner-coordinate compatibility binding;
3. fail-closed behavior when an owner is unavailable, disabled, stale or incompatible;
4. immutable snapshot identity bound to tenant, privacy case, canonical Party, topology generation, registry digest, purpose and effective request time;
5. bounded deterministic aggregation with exact owner/resource/data-class ordering;
6. snapshot, page and cursor digest contracts and terminal-completeness proof;
7. no resource payload disclosure;
8. no owner mutation, provider call, restriction, legal-hold decision, retention decision or destructive action;
9. permission-aware snapshot reads and exact audit evidence;
10. idempotency, replay, retry, registry/topology drift and crash-window semantics;
11. clean PostgreSQL, complete rollback/schema removal, reapply and repeated acceptance;
12. real-process acceptance before deterministic planning begins.

Planning, approval, restrictions, legal holds, execution, access/export, deletion/anonymization and convergence remain incomplete.

## 6. Customer Privacy packaging constraint

Do not implement discovery/snapshot as one new crate per command, query, worker, reader or composition fragment.

Target convergence:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

Rules:

- freeze the feature contract first;
- identify existing transitional crates and real dependency boundaries;
- perform consolidation only in a separate behavior-neutral PR;
- keep pure domain free of SQLx, transport and other owners' internals;
- keep owner-specific SQL and classifications owner-specific;
- use shared owner-scope support only for proven common protocol behavior;
- contribute routes/queries/workers through one Customer Privacy production entry point;
- do not add business-ID switches to generic runtime or worker algorithms.

## 7. Sequence after discovery acceptance

1. deterministic planning and permission-aware plan/outcome reads;
2. approval and immediate deny-only restrictions using final subject locks;
3. legal-hold and mandatory-retention precedence;
4. replay-safe resumable owner execution and crash-window recovery;
5. governed access/export assembly and artifact disclosure;
6. owner-specific deletion/anonymization;
7. Party tombstone and no-orphan proof;
8. projection/search/cache convergence;
9. disable/uninstall fail-closed worker behavior;
10. complete process and end-to-end lifecycle acceptance.

## 8. Frozen Customer Privacy ownership

`crm.customer-privacy` owns:

- privacy cases and verified subject binding;
- immutable scope snapshots;
- current restrictions;
- customer-data legal holds;
- retention decisions;
- deterministic plans;
- per-owner attempts/outcomes;
- orchestration checkpoints;
- governed export references;
- convergence evidence.

It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Customer Data Operations, Data Quality or Enrichment storage.

Critical precedence:

```text
legal hold > mandatory retention > approved privacy action > ordinary retention
```

Restriction is deny-only and never grants processing. Destructive actions are exact, owner-owned, replay-safe and resumable.

## 9. Phase 8A closure

Phase 8A remains **In progress**.

It closes only after discovery, immutable snapshots, planning, restrictions, legal holds, execution, access/export, deletion/anonymization, tombstone/no-orphan behavior, convergence and complete worker/process acceptance are merged.

Closure also requires:

- Customer Privacy contribution aggregation no longer grows generic runtime;
- no unjustified capability-specific crates introduced by Phase 8A.11;
- dependency and affected-scope reports are available for the final packet;
- documentation and issue #126 match exact merged behavior.

## 10. Wave 8B — Product Catalog, Pricing, CPQ and Quote-to-Revenue

State: **Planned; blocked on completed Phase 8A baseline**.

Required independent owner domains:

- Product Catalog;
- Price Books and Pricing;
- CPQ and immutable quote revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage;
- governed billing/ERP/payment/tax/fulfillment boundaries.

These domains must not be absorbed into Sales.

Phase 8B must use the golden owner package model from issue #194 and demonstrate that:

- normal capabilities add zero crates;
- owner contributions do not grow generic runtime;
- affected-scope iteration remains bounded;
- frontend and backend are delivered as complete vertical workflows.

## 11. Completion rule

Current product-complete expert modules: **0**.

A backend slice, crate, contract or migration is not product-complete without required domain breadth, governed APIs, persistence, authorization, audit, product workflow, frontend experience and production/operational proof.

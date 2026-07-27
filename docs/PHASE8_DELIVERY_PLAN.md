# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Commercial follow-on: #29  
Delivery governance: `DELIVERY_GOVERNANCE.md`  
Functional scope guardrail: `CRM_CAPABILITY_COVERAGE.md`

## 1. Goal

Build the expert CRM domain layer on the completed governed platform without collapsing ownership into Sales or weakening compatibility, tenant isolation, authorization, audit, rollback and exact-SHA evidence.

Every packet ends at a natural owner boundary with explicit contracts, persistence, activation, authorization, recovery and real-process acceptance where runtime behavior is introduced.

## 2. Packet contract

Every Phase 8 packet defines:

- authoritative owner and stable resource identity;
- public, worker and non-runtime classifications;
- persistence, tenant and authorization boundaries;
- audit, idempotency, approval and retention implications;
- projection/search/cache and compatibility consequences;
- exact acceptance gates and rollback evidence.

A packet is **Complete** only after merge to `main`. Every later source or documentation change invalidates earlier exact-head evidence until applicable checks rerun.

## 3. Wave 8A — canonical customer master, identity, consent and customer-data lifecycle

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
- zero completed non-runtime coordinates outside the accepted privacy owner contribution.

### 8A.11 — Customer Privacy Lifecycle — In progress

Issue: #126  
Merged runtime inventory: four mutations + two permission-aware queries + ten public non-runtime Customer Privacy coordinates + zero Customer Privacy workers.

Accepted production coordinates:

- `case.create`;
- `case.submit`;
- `case.subject.verify`;
- `case.cancel`;
- permission-aware `case.get`;
- subject-scoped permission-aware `case.list`.

Nine owner-scope coordinates are published and remain contract-only/non-runtime. All nine authoritative owner implementations are accepted:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Customer Data Operations remains non-runtime. Its accepted gate proves bounded four-family owner discovery, strict persisted-contract rehydration, alias-aware canonical Party relevance, exact selection-to-stage/outcome association, deterministic pagination, reference-only output, no writes, clean PostgreSQL, complete rollback/schema removal, reapply and repeated acceptance.

Data Quality remains non-runtime. Its accepted gate proves strict nine-type rehydration, exclusion of shared definitions, seven-family Party evidence, exact association integrity, alias-aware relevance, deterministic pagination, minimized output, no writes, primary-key access-path proof and repeated clean PostgreSQL acceptance.

Customer Enrichment remains non-runtime. PR #192 proves typed request/Party relationship-rooted discovery, strict nine-type rehydration, exclusion of shared provider-profile/mapping definitions, exact seven-family descendant lineage, alias-aware relevance, deterministic pagination, minimized output, relationship/record primary-key access paths, zero writes and repeated clean PostgreSQL acceptance.

## 4. Nine-owner set complete

The owner implementation lane is complete. No owner contribution is next and no accepted owner may be described as unstarted.

The nine accepted coordinates remain contract-only/non-runtime. They add no Customer Privacy worker, public ingress, application registration, production discovery, planning or owner action execution.

### Accepted historical packet: Customer Data Operations

`CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md` is an accepted historical contract for PR #188. It contributed only:

- `customer_data.import_row`;
- `customer_data.export_selection_item`;
- `customer_data.export_execution_stage`;
- `customer_data.export_execution_outcome`.

Import/export jobs, selection boundaries/progress, complete artifacts and other multi-subject containers remain excluded from automatic Party scope.

### Accepted historical packet: Data Quality

`DATA_QUALITY_PRIVACY_SCOPE_PACKET.md` is an accepted historical contract for PR #190. It contributes only seven direct Party-bearing families while shared rule-set/profile definitions remain strict owner validation dependencies and are excluded from subject evidence.

### Accepted historical packet: Customer Enrichment

`CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PACKET.md` is an accepted historical contract for PR #192. It contributes only:

- `customer_enrichment.request`;
- `customer_enrichment.provider_response_receipt`;
- `customer_enrichment.provider_response_conflict`;
- `customer_enrichment.suggestion`;
- `customer_enrichment.review_decision`;
- `customer_enrichment.application_attempt`;
- `customer_enrichment.provider_usage_entry`.

The request is the sole relationship-proven subject-discovery root. Shared provider-profile and mapping definitions remain strict validation dependencies and are excluded from Party evidence.

## 5. Next bounded packet — Scope discovery and immutable snapshot

State: **Ready for bounded packet definition and implementation; production discovery is not implemented**.

Do not begin runtime discovery, planning or owner execution until this packet's schemas, contracts, case lifecycle, persistence and authoritative governance have been inspected and its acceptance boundary is frozen.

The packet must define and prove:

1. deterministic invocation of all nine exact owner coordinates;
2. exact registry version/digest and owner-coordinate compatibility binding;
3. fail-closed behavior when an owner is unavailable, disabled, stale or incompatible;
4. immutable scope snapshot identity bound to tenant, privacy case, canonical Party, topology generation, registry digest, purpose and effective request time;
5. bounded deterministic aggregation with exact owner/resource/data-class ordering;
6. snapshot, page and cursor digest contracts and terminal-completeness proof;
7. no resource payload disclosure;
8. no owner mutation, provider call, restriction, legal-hold, retention decision or destructive action during discovery;
9. permission-aware snapshot reads and exact audit evidence;
10. idempotency, replay, retry, registry/topology drift and crash-window semantics;
11. clean PostgreSQL, complete rollback/schema removal, reapply and repeated acceptance;
12. real-process acceptance before deterministic planning begins.

Planning, approval, restrictions, legal holds, owner execution, access/export, deletion/anonymization and convergence remain incomplete and prohibited from being represented as implemented.

## 6. Lifecycle sequence after discovery acceptance

After scope discovery and immutable snapshot are accepted, deliver in order:

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

## 7. Frozen Customer Privacy ownership

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

It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Customer Data Operations, Data Quality or Enrichment storage. Those owners participate only through exact module-owned privacy capabilities.

Critical precedence:

`legal hold > mandatory retention > approved privacy action > ordinary retention`.

Restriction is deny-only and never grants processing. Destructive actions must be exact, owner-owned, replay-safe and resumable.

## 8. Phase 8A closure

Phase 8A remains **In progress**. It closes only when the full privacy/customer-master interaction baseline is merged, including discovery, immutable snapshots, planning, restrictions, legal holds, execution, access/export, deletion/anonymization, tombstone/no-orphan behavior, convergence and worker-process acceptance.

## 9. Wave 8B — Product Catalog, Pricing, CPQ and Quote-to-Revenue

State: **Planned; blocked on completed Phase 8A baseline**.

Required domains:

- Product Catalog;
- Price Books and Pricing;
- CPQ and immutable quote revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage;
- governed billing/ERP/payment/tax/fulfillment boundaries.

These domains remain independent owner domains and must not be absorbed into Sales.

## 10. Completion rule

Current product-complete expert modules: **0**.

A backend slice, crate, contract or migration is not product-complete without required domain breadth, governed APIs, persistence, authorization, audit, product workflow and production/operational proof.

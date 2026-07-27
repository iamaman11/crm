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
- zero completed non-runtime coordinates.

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

Nine owner-scope coordinates are published and remain contract-only/non-runtime.

Accepted owner implementations:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows.

Customer Data Operations remains non-runtime. Its accepted gate proves bounded four-family owner discovery, strict persisted-contract rehydration, alias-aware canonical Party relevance, exact selection-to-stage/outcome association, deterministic pagination, reference-only output, no writes, clean PostgreSQL, complete rollback/schema removal, reapply and repeated acceptance. The shared Identity Resolution read-only snapshot topology path also passed its permanent workflow on the same exact head.

## 4. Current owner sequence

### Next bounded owner packet: Data Quality

Coordinate: `data_quality.privacy.scope.contribute@1.0.0`  
State: **Inspection and bounded-entry freeze next; adapter implementation is not yet accepted**

The packet must inspect and freeze:

- authoritative Data Quality record families;
- Party-level relevance through direct Party references and indirect provenance references;
- rule, evaluation run, finding, observation, completeness, stewardship, remediation and evidence relationships;
- shared or multi-subject containers that must not be emitted for one subject;
- strict owner decoders and malformed-state fail-closed semantics;
- alias-aware relevance under the accepted topology generation;
- deterministic family order, same-tenant keyset pagination and terminal `page_size + 1` proof;
- raw scanned-row accounting and owner-wide scan/rehydration/relationship/canonical-resolution bounds;
- retention and minimized reference-only evidence boundaries;
- PostgreSQL indexes needed for bounded proof, including rollback and reapply behavior;
- contract-only implementation with no route, worker, application composition or runtime promotion.

Implementation must not start until ambiguous record-family or subject-relevance semantics are resolved in the entry packet.

### Following owner packets

1. Data Quality privacy-scope implementation after entry freeze;
2. Customer Enrichment privacy-scope contribution;
3. complete nine-owner set.

Only one owner packet is active at a time.

### Accepted historical packet: Customer Data Operations

`CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md` is an accepted historical contract for PR #188. It contributed only:

- `customer_data.import_row`;
- `customer_data.export_selection_item`;
- `customer_data.export_execution_stage`;
- `customer_data.export_execution_outcome`.

Import/export jobs, selection boundaries/progress, complete artifacts and other multi-subject containers remain excluded from automatic Party scope.

## 5. Discovery and lifecycle sequence after all owners

Production discovery does not begin until all nine owner implementations are accepted.

Then deliver, in order:

1. complete owner discovery with unavailable/stale owner fail-closed behavior;
2. immutable scope snapshot and deterministic owner/data-class plan;
3. permission-aware plan and owner-outcome reads;
4. approval and immediate deny-only restrictions using final subject locks;
5. legal-hold and mandatory-retention precedence;
6. replay-safe resumable owner execution and crash-window recovery;
7. governed access/export assembly and artifact disclosure;
8. owner-specific deletion/anonymization;
9. Party tombstone and no-orphan proof;
10. projection/search/cache convergence;
11. disable/uninstall fail-closed worker behavior;
12. complete process and end-to-end lifecycle acceptance.

## 6. Frozen Customer Privacy ownership

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

## 7. Phase 8A closure

Phase 8A closes only when the full privacy/customer-master interaction baseline is merged, including all nine owners, discovery, planning, restrictions, legal holds, execution, access/export, deletion/anonymization, tombstone/no-orphan behavior, convergence and worker-process acceptance.

## 8. Wave 8B — Product Catalog, Pricing, CPQ and Quote-to-Revenue

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

## 9. Completion rule

Current product-complete expert modules: **0**.

A backend slice, crate, contract or migration is not product-complete without required domain breadth, governed APIs, persistence, authorization, audit, product workflow and production/operational proof.

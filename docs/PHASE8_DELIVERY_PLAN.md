# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Customer Privacy packet: #126  
Commercial follow-on: #29  
Architecture/developer-experience program: #194  
Delivery governance: `DELIVERY_GOVERNANCE.md`  
Architecture guardrail: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`  
Measured baseline: `WORKSPACE_COMPLEXITY_BASELINE.md`

## 1. Goal and packet contract

Build expert CRM domains on the governed platform without collapsing ownership into Sales or weakening compatibility, tenant isolation, authorization, audit, rollback, operations or exact-head evidence.

Every packet defines:

- authoritative owner and stable resource identity;
- public, worker and non-runtime coordinates;
- persistence, tenant, authorization and audit boundaries;
- idempotency, recovery, retention and projection consequences;
- exact production contribution and activation path;
- architecture-complexity impact;
- focused, PostgreSQL, process and rollback acceptance where applicable.

A packet is **Complete** only after merge to `main`. Source or documentation changes invalidate earlier exact-SHA evidence until applicable checks rerun.

## 2. Phase 8 architecture rules

1. An ordinary capability added to an owner creates zero new crates.
2. A new owner targets three to five technical packages.
3. New crates require a real dependency, trust, reuse, process, lifecycle or extraction boundary.
4. Generic router and worker algorithms do not change merely to register one owner capability.
5. Owners converge on one module-owned production contribution entry point.
6. Feature implementation and physical consolidation use separate PRs.
7. Shared behavior is extracted only after contrasting implementations prove it.
8. Iterative affected-scope checks may be focused; final exact-head acceptance remains complete.
9. Product contract freeze may prepare a packet while an architecture prerequisite closes, but runtime implementation waits for the accepted package boundary.

## 3. Phase 8A completed work

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** immutable import sources, deterministic row identity, resumable Party import and recovery.
- **8A.8 — Complete:** governed Party export, immutable specifications/manifests, deterministic artifacts and recovery.
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship.
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance.

## 4. Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Merged public runtime inventory remains:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

All nine authoritative owner implementations are accepted:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Their coordinates remain non-public owner-owned reads. PR #206 composes them into trusted-internal production discovery without public ingress, a Customer Privacy worker, planning or owner action execution.

## 5. Nine-owner set complete

The owner implementation lane is complete. No accepted owner may be described as unstarted, and no additional owner contribution is the next packet.

Current exact owner evidence remains recorded in `PROJECT_STATUS.md`, `IMPLEMENTATION_ROADMAP.md`, `MODULE_CATALOG.md` and issue #126.

## 6. Accepted architecture prerequisites

Issue #194 runs alongside Phase 8A.

Accepted prerequisites:

1. PR #197 — reproducible workspace/dependency/public-surface/CI baseline and exception governance.
2. PR #199 — business-module dependency inheritance.
3. PR #200 — privacy-scope adapter dependency inheritance.
4. PR #203 — repository-wide root-family dependency no-growth.
5. PR #204 — frozen discovery/snapshot contract and acceptance semantics.
6. PR #205 — behavior-neutral Customer Privacy golden packages.

Stage B no-growth and Stage C packaging are complete. Existing dependency debt may only shrink through bounded role-based cohorts. Workspace package count is `113`.

## 7. Ordered continuation

### Packet A — Stage B no-growth closure

State: **Complete through PR #203**.

The accepted gate freezes the remaining non-inheriting root-family inventory, blocks new version/feature/source drift outside an explicit expiring exception and permits monotonic debt reduction only.

### Packet B — Scope discovery and immutable snapshot freeze

State: **Complete as the preserved historical freeze through PR #204**.

The freeze defines:

1. deterministic invocation of all nine exact owner coordinates;
2. registry version/digest and owner compatibility binding;
3. fail-closed unavailable, disabled, stale and incompatible-owner behavior;
4. snapshot identity bound to tenant, case, canonical Party, topology generation, registry digest, purpose and effective request time;
5. bounded owner/resource/data-class ordering;
6. snapshot, page, cursor and terminal-completeness digests;
7. no owner resource payload disclosure;
8. no owner state change, provider call, restriction, legal-hold/retention decision or execution during discovery;
9. permission-aware reads and exact audit evidence;
10. replay, retry, registry/topology drift and crash-window semantics;
11. clean PostgreSQL, complete schema removal, reapply and repeated acceptance;
12. real production-composition acceptance before planning begins.

The historical fields that described runtime as not started remain historical evidence and are not rewritten by later implementation.

### Packet C — Stage C Customer Privacy golden-package pilot

State: **Complete through PR #205**.

The accepted boundary is:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

The pilot preserved routes, workers and behavior while establishing the package boundary used by Packet D.

### Packet D — Scope discovery and immutable snapshot implementation

State: **Implemented in PR #206; exact-head gate review pending**.

PR #206 implements the frozen Packet B semantics inside the accepted application/PostgreSQL/production packages and records later evidence without rewriting the historical freeze.

Preserved architecture guardrails:

- Do not implement discovery/snapshot as one new crate per command, query, worker, reader or composition fragment.
- If consolidation is required, perform consolidation only in a separate behavior-neutral PR.

Implemented boundary:

- trusted-internal activation-gated exact-nine invocation;
- descriptor, registry and immutable-lineage validation;
- default page size `64`, maximum page size `128`, maximum cursor `2048` bytes and terminal completeness;
- durable append-only pages and contiguous checkpoints;
- deterministic safe resource aggregation and conflicting-duplicate fail closed;
- immutable snapshot id/binding and strict rehydration with digest recomputation;
- idempotent replay and page/checkpoint/finalization crash-window recovery;
- permission-aware internal snapshot reads and safe audit;
- FORCE-RLS PostgreSQL persistence, cross-tenant negatives, rollback/schema removal, reapply and repeated acceptance;
- production composition through the owner package without public route or worker registration.

Preserved metrics:

- workspace packages `113 → 113`;
- public Customer Privacy mutations `4 → 4`;
- permission-aware public queries `2 → 2`;
- Customer Privacy workers `0 → 0`;
- public discovery routes added `0`.

Forbidden and still unimplemented:

- action planning and plan/outcome reads;
- owner mutations and provider calls;
- restrictions;
- legal-hold and mandatory-retention decisions;
- owner action execution;
- access/export assembly;
- deletion/anonymization;
- Party tombstone and convergence;
- Phase 8B.

Acceptance requires every applicable permanent workflow to pass on one unchanged exact source SHA, followed by squash merge with expected-head protection.

## 8. Sequence after discovery acceptance

1. deterministic planning and permission-aware plan/outcome reads;
2. approval and immediate deny-only restrictions using final subject locks;
3. legal-hold and mandatory-retention precedence;
4. replay-safe resumable owner execution and crash recovery;
5. governed access/export assembly;
6. owner-specific deletion/anonymization;
7. Party tombstone and no-orphan proof;
8. projection/search/cache convergence;
9. disable/uninstall fail-closed worker behavior;
10. complete worker-process and end-to-end lifecycle acceptance.

## 9. Frozen Customer Privacy ownership

`crm.customer-privacy` owns privacy cases, verified subject binding, immutable scope snapshots, restrictions, customer-data legal holds, retention decisions, deterministic plans, per-owner attempts/outcomes, checkpoints, governed export references and convergence evidence.

It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Customer Data Operations, Data Quality or Enrichment storage.

```text
legal hold > mandatory retention > approved privacy action > ordinary retention
```

Restriction is deny-only and never grants processing. Owner actions remain exact, owner-owned, replay-safe and resumable.

## 10. Phase 8A closure

Phase 8A remains **In progress**.

It closes only after discovery, immutable snapshots, planning, restrictions, legal holds, execution, access/export, deletion/anonymization, tombstone/no-orphan behavior, convergence and complete worker/process acceptance are merged.

Closure also requires contribution aggregation that does not grow generic runtime, no unjustified capability-specific crates, synchronized documentation/issues and full exact-head evidence.

## 11. Phase 8B

Product Catalog, Pricing, CPQ, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment boundaries remain **Planned and blocked on completed Phase 8A**. They remain independent owner domains and must not be absorbed into Sales.

## 12. Completion rule

Current product-complete expert modules: **0**.

A backend slice, crate, contract or migration is not product-complete without required domain breadth, governed APIs, persistence, authorization, audit, product workflow, frontend experience and production/operational proof.

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

Merged runtime inventory:

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

Post-merge owner documentation synchronization was accepted through PR #193 / merge `e09d3152c886386c2168f0b49e46d47cc44ed041`.

All nine coordinates remain contract-only/non-runtime. They add no public ingress, Customer Privacy worker, production discovery, planning or owner action execution.

## 5. Nine-owner set complete

The owner implementation lane is complete. No accepted owner may be described as unstarted, and no additional owner contribution is the next packet.

Current exact owner evidence remains recorded in `PROJECT_STATUS.md`, `IMPLEMENTATION_ROADMAP.md`, `MODULE_CATALOG.md` and issue #126.

## 6. Accepted architecture prerequisites

Issue #194 runs alongside Phase 8A.

Accepted Stage B foundation:

1. PR #197 / merge `dbd7f6646f255b5f654060a045e26f99fc12c1f9` — reproducible 110-package complexity baseline, new-crate justification and machine-readable exception governance.
2. PR #199 / accepted source `2335ea00bb73d875c291b4a7668921beaec87adc` / merge `cbcce5f18f3b08851ad781d13bc3fe01c2eeb62c` — 13 business-module manifests, 39 inherited declarations, zero violations.
3. PR #200 / accepted source `31b3ab09caa4eccaba76a34c7d2211622830115f` / merge `aec7130bd48302d20bf821a617c339b2a9d755cf` — nine privacy-scope adapter manifests, 20 inherited declarations, zero violations.

Root workspace dependency families are `prost`, `serde`, `serde_json` and `sha2`. Remaining direct non-inheriting consumers are `prost` 53, `serde` 15, `serde_json` 23 and `sha2` 16.

Stage B remains in progress until new direct declarations are blocked outside an explicit owned exception. Existing consumers migrate only through bounded role-based cohorts.

## 7. Ordered continuation

### Packet A — Stage B no-growth closure

State: **Next cross-cutting implementation packet**.

Required result:

- freeze the exact remaining non-inheriting consumer inventory;
- block new direct version, feature or source declarations for root dependency families outside a machine-readable owned exception;
- reject growth in the accepted debt inventory;
- preserve dependency, package, public-surface and fan-out reports;
- make no Customer Privacy runtime change;
- avoid a repository-wide manifest rewrite.

This closes the dependency prerequisite for Stage C.

### Packet B — Scope discovery and immutable snapshot freeze

State: **Ready after documentation synchronization; production discovery is not implemented**.

Inspect the existing Customer Privacy aggregate, schema, migrations, Protobuf contracts, persistence/transaction semantics, topology and canonical Party binding, registry/version/digest authority, authorization/RLS/audit boundaries, all nine owner contracts and current crate boundaries.

Freeze:

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
12. real-process acceptance before planning begins.

### Packet C — Stage C Customer Privacy golden-package pilot

State: **Blocked on Packet A and the frozen Packet B boundary**.

Current repository fact:

- `modules/crm-customer-privacy` exists;
- `crm-customer-privacy-application`, `crm-customer-privacy-postgres` and `crm-customer-privacy-production` do not yet exist;
- current behavior is split across persistence, create/submit/subject/cancel, query and composition crates.

Target:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

The pilot is behavior-neutral. It preserves coordinates, routes, activation, authorization, RLS, audit, idempotency and process behavior; consolidates only across valid dependency seams; adds no discovery behavior; reports before/after package count, edges, fan-out, public surface and build/test effect; and exposes one Customer Privacy production contribution entry point without generic-runtime business switches.

If consolidation is required, perform consolidation only in a separate behavior-neutral PR.

### Packet D — Scope discovery and immutable snapshot implementation

State: **Blocked on accepted Packet C packaging**.

Implement the frozen Packet B semantics inside the accepted application/PostgreSQL/production packages.

Do not implement discovery/snapshot as one new crate per command, query, worker, reader or composition fragment.

Forbidden:

- direct owner storage access;
- generic runtime or worker branching on Customer Privacy IDs;
- planning, restrictions, legal-hold/retention decisions or owner action execution in this packet.

Acceptance requires focused tests, applicable generic conformance, FORCE RLS and cross-tenant negatives, complete schema removal/reapply, retry/replay/drift proof, permission-aware query/audit proof and real HTTP/gRPC process evidence on one unchanged exact head.

## 8. Sequence after discovery acceptance

1. deterministic planning and permission-aware plan/outcome reads;
2. approval and immediate deny-only restrictions using final subject locks;
3. legal-hold and mandatory-retention precedence;
4. replay-safe resumable owner execution and crash-window recovery;
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

Closure also requires:

- Customer Privacy contribution aggregation no longer grows generic runtime;
- no unjustified capability-specific crates introduced by Phase 8A.11;
- dependency and affected-scope reports for the final packet;
- synchronized documentation and issue #126 evidence.

## 11. Phase 8B

Product Catalog, Pricing, CPQ, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment boundaries remain **Planned and blocked on completed Phase 8A**. They remain independent owner domains and must not be absorbed into Sales.

Phase 8B must prove that normal capabilities add zero crates, owner contributions do not grow generic runtime, affected iteration stays bounded and frontend/backend ship as complete vertical workflows.

## 12. Completion rule

Current product-complete expert modules: **0**.

A backend slice, crate, contract or migration is not product-complete without required domain breadth, governed APIs, persistence, authorization, audit, product workflow, frontend experience and production/operational proof.

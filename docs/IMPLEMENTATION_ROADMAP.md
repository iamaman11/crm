# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Architecture/developer-experience program: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` / issue #194  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose

This roadmap defines dependency order for a universal modular expert CRM platform. It is not a feature wishlist, architecture essay or historical status log.

A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by unchanged exact-head evidence.

Cross-cutting architecture work must preserve product delivery and cannot use a big-bang rewrite as a substitute for bounded implementation.

## 2. Delivery rules

1. Deliver coherent reviewable packets linked to roadmap or cross-cutting issues.
2. Preserve one authoritative owner for every mutable aggregate.
3. Enter state-changing behavior through exact versioned capabilities with typed audit evidence.
4. Never access another module's storage or internals directly.
5. Treat security, privacy, tenant isolation, compatibility, rollback and operations as implementation requirements.
6. Require real composition, persistence and process evidence before runtime claims.
7. Invalidate exact-SHA evidence after every source or documentation change until applicable checks rerun.
8. Synchronize roadmap, phase plan, status, catalog, issues and PR descriptions.
9. Do not mark the universal CRM product complete while required capability families remain incomplete.
10. A normal capability added to an existing owner creates zero new crates by default.
11. Generic router and worker algorithms must not change merely to register one owner capability.
12. Feature behavior and physical crate consolidation are separate delivery packets.
13. Shared abstractions are extracted only after contrasting real implementations prove common behavior.
14. Iterative affected-scope CI may reduce feedback cost but never weakens final exact-head acceptance.

## 3. Work states

- Planned
- Ready
- In progress
- Gate review
- Complete
- Blocked
- Superseded

Only merged `main` work may be represented as **Complete**.

## 4. Product phase map

| Phase | Issue | Primary result | State | Depends on |
|---|---:|---|---|---|
| 0.1 | #3 | Repository hardening and executable roadmap | **Complete** | Governance v1 |
| 1 | #4 | Typed Module Manifest IR and deterministic identity | **Complete** | #3 |
| 2 | #5 | Governed Module SDK and test harness | **Complete** | #4 |
| 3 | #6 | Module lifecycle and registry runtime | **Complete** | #4, #5 |
| 4 | #7 | PostgreSQL tenant, record, artifact, outbox and audit foundation | **Complete** | #6 |
| 5 | #8 | Capability execution gateway | **Complete** | #5, #7 |
| 6 | #9 | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | #10 | Search, generalized projections, product shell, Admin Studio and UI-extension isolation | **Complete** | #9 |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** | #5, #9, #10 |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** | #9, #10 |
| 8B | #29 | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | completed 8A baseline |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** | mature domain capabilities |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** | #6, #8, #10 |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous** | all critical phases |

## 5. Cross-cutting architecture 10/10 program

Issue #194 is **Open** and runs alongside product delivery.

It is complete only when the measurable criteria in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` are implemented.

Required order:

1. documentation and complexity baseline;
2. workspace dependency and new-crate governance;
3. golden owner domain/application/PostgreSQL/production package model;
4. module-owned contribution aggregation;
5. affected-scope CI;
6. generic conformance suites;
7. measured transitional crate consolidation;
8. repository navigation tooling;
9. frontend and operational parity.

The program must not block active product delivery with a repository-wide rewrite. Improvements are applied at natural packet boundaries and structural changes remain behavior-neutral.

## 6. Completed foundation

### Phases 0.1–5 — Complete

Repository governance, immutable module identity, governed Module SDK, lifecycle, PostgreSQL tenant/RLS/record/artifact/idempotency/outbox/audit foundations and exact-version capability execution are merged.

### Phase 6 — Complete

Independent Sales and Activities owners, optional governed link, projections and deployable application composition are merged.

### Phase 7 — Complete

Generalized projections, permission-aware search, typed product shell, metadata/Admin Studio and trusted UI-extension isolation are merged.

### Native application-composition integrity — Complete

Issue #134 / PR #135 established module-owned exact-coordinate routing, tenant activation, pre-authorization cross-owner semantics, deterministic worker contributions and production-route parity.

## 7. Phase 8A — canonical customer master and governed customer-data lifecycle

State: **In progress**  
Parent issue: #28

Completed packets:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import sources, parsing/validation, resumable Party import and recovery;
- **8A.8** — governed Party export, immutable selection/manifests, deterministic artifacts and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

### 7.1 Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Merged runtime inventory:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

All nine privacy owner-scope implementations are accepted. The coordinates remain contract-only/non-runtime and add no Customer Privacy discovery, planning or execution runtime.

Accepted owner evidence:

1. Parties — PR #156 / merge `4368b8c3710e05137b71ba999bf7f3497c0801c8`;
2. Consents — PR #175 / merge `039d6461803208f6cb70ce0fbcfcaffaf59d7125`;
3. Customer Accounts — PR #179 / merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`;
4. Contact Points — PR #181 / merge `96cd0cf548310592a0718c97242a724a29717a72`;
5. Party Relationships — PR #183 / merge `9ad2aa91321e9edb54cab98218f93143923ef33f`;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

### 7.2 Active sequence

1. **Scope discovery and immutable snapshot.**
2. **Deterministic planning and permission-aware plan/outcome reads.**
3. **Approval and immediate deny-only restrictions.**
4. **Legal hold and mandatory-retention precedence.**
5. **Replay-safe resumable owner execution and crash recovery.**
6. **Governed access/export and owner-specific deletion/anonymization.**
7. **Party tombstone, no-orphan proof and projection/search/cache convergence.**
8. **Worker and complete end-to-end lifecycle acceptance.**
9. **Phase 8A closure.**
10. **Phase 8B only from the completed Phase 8A baseline.**

### 7.3 Next bounded packet — scope discovery and immutable snapshot

State: **Ready; implementation not started**.

Before runtime implementation:

- inspect existing schemas, contracts, case lifecycle, persistence and governance;
- freeze snapshot identity, registry binding, ordering, digests, replay and failure semantics;
- prohibit planning, owner mutation, provider calls, restrictions, legal-hold decisions, retention decisions and destructive actions;
- define permission-aware snapshot reads and exact audit evidence;
- define clean PostgreSQL, rollback/reapply, crash/retry and real-process acceptance.

Architecture guardrail:

- do not add one crate per command, query, worker or composition fragment;
- target Customer Privacy domain/application/PostgreSQL/production packaging;
- keep any consolidation behavior-neutral and separate from feature implementation;
- do not modify generic router or worker algorithms to register the packet.

## 8. Phase 8B — Product Catalog, Pricing, CPQ and Quote-to-Revenue

State: **Planned; blocked on completed Phase 8A baseline**.

Required independent owner domains include:

- Product Catalog;
- Price Books and Pricing;
- CPQ and immutable quote revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage;
- governed billing/ERP/payment/tax/fulfillment boundaries.

These domains must not be absorbed into Sales.

Phase 8B is also the first broad proof that the issue #194 architecture program keeps extension cost bounded across a new expert domain wave.

## 9. Later expert domains

Planned work includes broader Sales/Activities, omnichannel, Marketing, Service/Knowledge/Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

## 10. Completion rule

Current product-complete expert modules: **0**.

A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists. Completion requires defined domain breadth, governed APIs, persistence, authorization, audit, product workflow, frontend experience and production/operational evidence.

# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Architecture/developer-experience program and repository order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` / issue #194  
Accepted Rust boundary: `RUST_TOOLCHAIN_AND_LINT_BASELINE.md` / `rust-governance-policy.json`  
Measured architecture baseline: `WORKSPACE_COMPLEXITY_BASELINE.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose and delivery rules

This roadmap defines dependency order for a universal modular expert CRM platform. A phase or packet is complete only when implemented, merged and backed by unchanged exact-head evidence.

1. Preserve one authoritative owner for every mutable aggregate.
2. Enter state-changing behavior through exact versioned capabilities with typed audit evidence.
3. Never access another module's storage or internals directly.
4. Treat security, privacy, tenant isolation, rollback and operations as implementation requirements.
5. Require real composition, persistence and process evidence before runtime claims.
6. Invalidate old exact-SHA evidence after every source or documentation change.
7. Synchronize roadmap, phase plan, status, catalog, issues and PR descriptions.
8. An ordinary capability added to an existing owner creates zero new crates by default.
9. Generic router and worker algorithms do not change merely to register one owner capability.
10. Feature behavior and physical crate consolidation remain separate packets.
11. Repository implementation is strictly sequential: only the first unfinished item in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 may start.
12. Only one implementation packet may be active; evidence synchronization closes the accepted packet before the next implementation begins.

For the active Customer Privacy lane, **do not modify generic router or worker algorithms** merely to register one owner capability.

Only merged `main` work may be represented as **Complete**.

## 2. Product phase map

| Phase | Issue | Primary result | State |
|---|---:|---|---|
| 0.1–7 | #3–#10 | Governed platform, Sales/Activities proof, search, product shell and native composition | **Complete** |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** |
| 8B | #29 | Product Catalog, CPQ and quote-to-revenue lifecycle | **Planned; blocked on repository steps 1–19 and completed 8A** |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous** |

## 3. Cross-cutting architecture 10/10 program

Issue #194 remains **Open**.

- Stage A documentation/navigation baseline — **Complete**.
- Stage B dependency, crate and exception governance — **In progress; Rust toolchain and measured lint prerequisite accepted through PR #218**.
- Stage C Customer Privacy golden owner packages — **Complete** through PR #205.
- Stages D–I — broader contribution aggregation, affected-scope expansion, conformance, consolidation, reproducible environment, frontend and operations parity remain open.

Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205 and PR #218. Current workspace package count remains `113`; root dependencies remain `prost`, `serde`, `serde_json`, `sha2`; exact Rust `1.97.1`, workspace `rust-version = "1.97.1"` and zero-warning Rust/Clippy budgets are enforced. Three historical direct `too_many_arguments` lint tables remain exact, expiring, no-growth exceptions.

The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1 and 2 are complete through PR #218 and PR #220; the current next packet is repository step 3: bounded contribution aggregation without behavior change.

## 4. Phase 8A completed foundation

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import and recovery;
- **8A.8** — governed deterministic export and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

## 5. Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Latest accepted runtime inventory is five public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #220. `customer_privacy.plan.build@1.0.0` remains accepted trusted-internal runtime without public ingress.

All nine privacy owner-scope implementations are accepted:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

### 5.1 Accepted scope discovery and immutable snapshot

PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows implements trusted-internal exact-nine discovery, immutable snapshot lineage, bounded durable pages/checkpoints, strict rehydration, replay/crash recovery, permission-aware reads, audit, FORCE RLS, cross-tenant concealment, rollback/reapply and repeated acceptance.

PR #207 synchronized its machine-readable and human-readable evidence without changing the historical PR #204 freeze.

### 5.2 Accepted deterministic planning freeze and runtime

PR #208 / accepted source `d16a42551918ac6142d7a57cbeb7802f8f162fb9` / merge `bbdbc12ed139367efe75033c2a7e7ddb3eaec59d` / 16 of 16 permanent workflows froze immutable planning lineage, exact actions, ordering, digests, strict rehydration, unsupported crypto-shred failure and permission-aware read boundaries.

PR #209 / accepted source `b97fd9bb4537c14df4497ad7b737d0f0a64c4f3b` / merge `30621ffff5c1e07e1275cc80fee3f1297a91f49e` / 29 of 29 permanent workflows implements trusted-internal activation-gated deterministic planning inside the accepted Customer Privacy packages.

It verifies the exact case, immutable scope snapshot, Party/Identity Resolution binding, policy and jurisdiction lineage; builds one immutable action plan; transitions `Scoped → Planned` or `Scoped → AwaitingApproval`; persists append-only replay and audit evidence; preserves FORCE RLS, canonical `tenant_isolation`, cross-tenant concealment, rollback/reapply and unchanged 113-package / 4-mutation / 2-query / 0-worker historical boundary.

### 5.3 Accepted permission-aware plan and outcome reads

PR #211 / accepted source `933fa4b502d60a23b83de9ccee279cc6517b5cba` / merge `a1f3a60a6d8e8bba7bda50f936c57a61bc3521f7` / 32 of 32 permanent workflows promotes only `customer_privacy.case.plan.get@1.0.0` and `customer_privacy.case.owner_outcomes.list@1.0.0` through existing Customer Privacy packages.

The packet requires module activation, live visibility, tenant-bound reads, strict case↔snapshot↔plan↔replay evidence, payload-safe plan summaries, append-only safe read audit and concealed unauthorized/cross-tenant existence. Owner outcomes remain an empty deterministic terminal page with bounded request validation, stable page/terminal digests, no synthetic records and no outcome persistence.

It adds no crate, dependency family, mutation, worker, owner mutation, approval, restriction, hold/retention adjudication or destructive execution.

### 5.4 Accepted approval runtime

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows completes repository step 2.

`customer_privacy.case.approve@1.0.0` is public, activation-gated, live-authorized, tenant-bound and idempotent. It permits only `AwaitingApproval → Planned`, enforces expected-version concurrency, locks the case and immutable evidence, strictly validates case↔subject↔snapshot↔plan lineage, records immutable actor/time and atomically persists status, event, audit, idempotency and business evidence. Exact replay succeeds and conflicting replay or corrupt evidence fails closed.

The packet adds no restriction placement/release, hold/retention adjudication, owner execution/outcomes, access/export assembly, destructive action, worker, crate, dependency family or generic-runtime business switch.

### 5.5 Accepted Rust prerequisite

PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 permanent workflows completes repository step 1 without changing Customer Privacy behavior, dependencies, `Cargo.lock` or the 113-package workspace.

The accepted boundary is exact Rust `1.97.1`, root workspace `rust-version = "1.97.1"`, zero measured Rust/Clippy warnings and errors, and three exact expiring no-growth direct-lint exceptions.

## 6. Binding active sequence

The only permitted current sequence is:

1. **Repository step 1 — supported Rust toolchain, workspace `rust-version` and measured lint baseline — Complete through PR #218.**
2. **Repository step 2 — Customer Privacy approval runtime — Complete through PR #220.**
3. **Repository step 3 — bounded contribution aggregation without behavior change — Next.**
4. **Repository step 4 — immediate deny-only processing restrictions with final subject locks.**
5. **Repository step 5 — `explain`, `packet-check` and generated navigation.**
6. **Repository step 6 — legal-hold and mandatory-retention precedence.**
7. **Repository steps 7–19 — continue exactly as numbered in the architecture plan.**
8. **Repository step 20 — first Phase 8B packet.**

Bounded contribution aggregation is now the next permitted repository packet. It must remain behavior-neutral: no route, coordinate, persistence, authorization, dependency, public-inventory or Customer Privacy product-semantic changes are permitted. Immediate deny-only restrictions remain repository step 4.

## 7. Phase 8B and later expert domains

Phase 8B remains planned and blocked on repository steps 1–19 and completed Phase 8A. Independent owner domains include Product Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment boundaries. They must not be absorbed into Sales.

Later planned work includes broader Sales/Activities, omnichannel, Marketing, Service/Knowledge/Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

## 8. Completion rule

Current product-complete expert modules: **0**.

A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists. Completion requires defined domain breadth, governed APIs, persistence, authorization, audit, product workflow, frontend experience and production/operational evidence.

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
- Stage B dependency, crate and exception governance — **In progress; Rust toolchain/lint prerequisite accepted through PR #218 and lockfile-preserving Rust workflows accepted through PR #232**.
- Stage C Customer Privacy golden owner packages — **In progress**: package baseline complete through PR #205, final customer-subject policy prerequisite accepted through PR #224, immediate deny-only restriction placement/final owner guard accepted through PR #226, legal-hold/mandatory-retention precedence accepted through PR #230, and durable replay-safe owner execution/outcomes accepted through PR #237.
- Stage D contribution aggregation — **In progress; first bounded Customer Accounts registration-inventory aggregation accepted through PR #222**.
- Stage E — **In progress**: real-diff packet-check and broadened Rust closure are accepted; database/process/product/frontend/operations selection remains open.
- Stage H — **In progress**: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands remain open.
- Stage F — **In progress**: reusable generic mutation/query conformance is accepted through PR #235; worker conformance and contract lifecycle enforcement remain open.
- Stages G and I — measured consolidation, frontend and operations parity remain open.

Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228, PR #232, PR #230, PR #235 and PR #237. Current workspace package count remains `113`; root dependencies remain `prost`, `serde`, `serde_json`, `sha2`; exact Rust `1.97.1`, workspace `rust-version = "1.97.1"` and zero-warning Rust/Clippy budgets are enforced. Three historical direct `too_many_arguments` lint tables remain exact, expiring, no-growth exceptions.

The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–8 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235 and PR #237; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — is the current next packet.

## 4. Phase 8A completed foundation

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import and recovery;
- **8A.8** — governed deterministic export and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

## 5. Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #237. `customer_privacy.plan.build@1.0.0` and `customer_privacy.retention.evaluate@1.0.0` remain accepted trusted-internal runtime without public ingress; repository-step-8 owner execution is also trusted-internal and registers no public route or worker.

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

The packet requires module activation, live visibility, tenant-bound reads, strict case↔snapshot↔plan↔replay evidence, payload-safe plan summaries, append-only safe read audit and concealed unauthorized/cross-tenant existence. Owner outcomes remained an empty deterministic terminal page at the historical PR #211 boundary. PR #237 later accepted durable owner outcomes and real permission-aware bounded pagination without changing the public query coordinate.

It adds no crate, dependency family, mutation, worker, owner mutation, approval, restriction, hold/retention adjudication or destructive execution.

### 5.4 Accepted approval runtime

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows completes repository step 2.

`customer_privacy.case.approve@1.0.0` is public, activation-gated, live-authorized, tenant-bound and idempotent. It permits only `AwaitingApproval → Planned`, enforces expected-version concurrency, locks the case and immutable evidence, strictly validates case↔subject↔snapshot↔plan lineage, records immutable actor/time and atomically persists status, event, audit, idempotency and business evidence. Exact replay succeeds and conflicting replay or corrupt evidence fails closed.

The packet adds no restriction placement/release, hold/retention adjudication, owner execution/outcomes, access/export assembly, destructive action, worker, crate, dependency family or generic-runtime business switch.

### 5.5 Accepted Rust prerequisite

PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 permanent workflows completes repository step 1 without changing Customer Privacy behavior, dependencies, `Cargo.lock` or the 113-package workspace.

The accepted boundary is exact Rust `1.97.1`, root workspace `rust-version = "1.97.1"`, zero measured Rust/Clippy warnings and errors, and three exact expiring no-growth direct-lint exceptions.

### 5.6 Accepted bounded contribution aggregation

PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows completes repository step 3 on one unchanged exact head.

The packet exposes the exact Customer Accounts data-only mutation/query definition factories from the existing owner composition package, re-exports them through `crm-first-party-modules`, and replaces the selected direct generic-runtime inventory imports with the first-party facade. Exact mutation/query ordering, deterministic Account-before-Consents registration and activation semantics remain unchanged.

It changes no route, coordinate, public inventory, persistence, migration, tenant isolation, authorization, audit, idempotency, dependency family, manifest, `Cargo.lock`, workspace package count, worker, Customer Privacy product behavior or generic business dispatch algorithm. The workspace remains at 113 packages.

### 5.7 Accepted final customer-subject policy prerequisite

PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts the smallest architecture prerequisite required by repository step 4.

The existing `crm-core-data` package now exposes a transaction-scoped `TransactionalCustomerSubjectPolicyPort` and deterministic `TransactionalAggregateGuardChain`. Future Customer Privacy production policy must acquire the shared tenant + canonical Party lock and make a live decision in the owner transaction immediately before protected persistence or I/O. Unavailable, stale, corrupt and cross-tenant decisions must fail closed. No allow-all implementation is provided.

The prerequisite changes no route, coordinate, public inventory, restriction runtime, owner integration, persistence, migration, dependency family, manifest, `Cargo.lock`, workspace package count, worker or product behavior. Those historical prerequisite non-effects remain accepted.

### 5.8 Accepted immediate deny-only restrictions

PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows completes repository step 4.

The packet promotes `customer_privacy.restriction.place@1.0.0`, implements an authoritative tenant-bound FORCE-RLS decision over bounded strictly rehydrated state, shares the canonical Party lock between placement and protected owner execution, and integrates the final guard into `contact-points.contact-point.create@1.0.0` immediately before persistence. Exact Personal contracts, deterministic tenant/idempotency identity, live authorization, audit, idempotency and atomic business evidence remain enforced.

Permanent PostgreSQL and real-process acceptance proves pre-restriction owner success, public placement, active denial without side effects, unrelated-Party isolation, malformed/cross-tenant fail-closed behavior, retained lock, complete rollback/reapply and repeated acceptance. Restriction release/reads, legal holds, retention decisions, owner execution, destructive behavior and workers remain non-runtime.

### 5.9 Accepted repository explanation and generated navigation

PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows completes repository step 5 on one unchanged source-authored head.

The packet adds deterministic exact module/capability explanation, fail-closed packet validation, real-diff Affected Scope enforcement, generated active-packet/repository-map navigation and permanent freshness tests. It records 113 packages, 14 manifests, 119 capabilities and 70 events without changing product runtime, contracts, persistence, migrations, dependencies, `Cargo.lock` or workspace package count.

### 5.10 Accepted lockfile-preserving Rust workflow prerequisite

PR #232 / accepted source `3f09dcc595f79d633915e4a67117aedc59ed2499` / squash merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 applicable permanent workflows accepts the smallest repository-step-6 architecture prerequisite. Rust Generated Sync and Rust CI now verify the committed dependency graph with locked Cargo commands, preserve `Cargo.lock` byte-for-byte on ordinary packets and cannot auto-commit registry drift. Intentional lockfile refresh remains explicit through `python scripts/repo.py lock` inside a bounded packet. The six-file change adds no product behavior, contract, manifest, dependency, package, persistence or migration change.

### 5.11 Accepted legal-hold and mandatory-retention precedence

Repository step 6 is accepted through PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows on one unchanged source-authored head. It promotes `customer_privacy.legal_hold.place@1.0.0`, preserves the shared tenant + canonical Party lock, strictly rehydrates bounded FORCE-RLS legal-hold state and evaluates immutable plan items with precedence active legal hold → mandatory retention → approved privacy action. Public placement is activation-gated, live-authorized, tenant-bound, idempotent and atomic; malformed, unavailable, stale, over-bound and cross-tenant evidence fails closed. Clean PostgreSQL, real `crm-api`, rollback/reapply, replay and repeated acceptance are proven. The accepted inventory is 7 mutations / 4 permission-aware queries / 0 workers, with no owner execution, outcome persistence, export assembly, destructive action, dependency, `Cargo.lock`, manifest or workspace-package change.

### 5.12 Accepted reusable generic mutation/query conformance

Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.

### 5.13 Accepted replay-safe resumable owner execution

Repository step 8 is accepted through PR #237 / accepted source `f926ece93dc2b24683f982828e72bf9170dc123a` / squash merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 applicable permanent workflows on one unchanged source-authored head. It persists deterministic tenant-bound owner execution attempts, checkpoints and safe outcomes with immutable action-plan and retention-decision lineage; composes exactly nine canonical owner endpoints through trusted-internal production wiring; recovers from pre-invocation, post-owner-result and post-outcome/pre-checkpoint crash windows without duplicate owner invocation; and makes `customer_privacy.case.owner_outcomes.list@1.0.0` paginate real persisted payload-safe outcomes. Activation, registered initiating-capability attribution, FORCE RLS, strict rehydration, idempotency, audit, clean apply, rollback, reapply and repeated PostgreSQL acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route, worker, access/export assembly, destructive owner execution, crate, dependency, `Cargo.lock`, workspace-package or generic-runtime algorithm change was introduced.

## 6. Binding active sequence

The only permitted current sequence is:

1. **Repository step 1 — supported Rust toolchain, workspace `rust-version` and measured lint baseline — Complete through PR #218.**
2. **Repository step 2 — Customer Privacy approval runtime — Complete through PR #220.**
3. **Repository step 3 — bounded contribution aggregation without behavior change — Complete through PR #222.**
3a. **Inserted repository step 4 prerequisite — final customer-subject policy port and deterministic guard composition — Complete through PR #224.**
4. **Repository step 4 — immediate deny-only processing restrictions with final subject locks — Complete through PR #226.**
5. **Repository step 5 — `explain`, `packet-check` and generated navigation — Complete through PR #228.**
5a. **Inserted lockfile-preservation prerequisite before repository step 6 — Complete through PR #232.**
6. **Repository step 6 — legal-hold and mandatory-retention precedence — Complete through PR #230.**
7. **Repository step 7 — reusable generic mutation and query conformance — Complete through PR #235.**
8. **Repository step 8 — replay-safe resumable Customer Privacy owner execution and crash-window recovery — Complete through PR #237.**
9. **Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — Next.**
10–19. **Repository steps 10–19 — continue exactly as numbered in the architecture plan.**
20. **Repository step 20 — first Phase 8B packet.**

The inserted prerequisites did not renumber the master sequence. Repository step 9 is now the only next permitted implementation packet.

## 7. Phase 8B and later expert domains

Phase 8B remains planned and blocked on repository steps 1–19 and completed Phase 8A. Independent owner domains include Product Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment boundaries. They must not be absorbed into Sales.

Later planned work includes broader Sales/Activities, omnichannel, Marketing, Service/Knowledge/Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

## 8. Completion rule

Current product-complete expert modules: **0**.

A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists. Completion requires defined domain breadth, governed APIs, persistence, authorization, audit, product UX and production/operational evidence.

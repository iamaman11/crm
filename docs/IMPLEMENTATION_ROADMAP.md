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
| 8B | #29 | Product Catalog, CPQ and quote-to-revenue lifecycle | **Planned; blocked on repository steps 1–22 and completed 8A; first extension wave is step 23** |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous** |

## 3. Cross-cutting architecture 10/10 program

Issue #194 remains **Open**.

- Stage A documentation/navigation baseline — **Complete**.
- Stage B dependency, crate and exception governance — **Complete through PR #257: exact Rust/toolchain governance, lockfile-preserving workflows, blocking suppression governance, zero direct lint tables, section-aware process-host non-growth and representative change-cost/dependency-version-feature budgets are enforced**.
- Stage C Customer Privacy golden owner packages — **In progress**: package baseline complete through PR #205, final customer-subject policy prerequisite accepted through PR #224, immediate deny-only restriction placement/final owner guard accepted through PR #226, legal-hold/mandatory-retention precedence accepted through PR #230, durable replay-safe owner execution/outcomes accepted through PR #237, and governed access/export assembly accepted through PR #241, and authoritative exact-nine owner-specific anonymization/deletion execution accepted through PR #244.
- Stage D contribution aggregation — **Complete through PR #249; every currently active first-party owner exposes an owner-owned production contribution boundary aggregated through `crm-first-party-modules`, while generic native composition retains platform-level composition only**.
- Stage E — **Complete through PR #239**: deterministic Rust closure, exact eight-category repository ownership, contract/Protobuf/API/migration/PostgreSQL/process/product/frontend/operations selection, live workflow-filter compatibility, exact-head evidence and unknown-path fail-closed enforcement are accepted.
- Stage H — **In progress**: deterministic explain, packet-check and generated navigation are accepted through PR #228; repository step 18 owns local lifecycle completion.
- Stage F — **In progress**: reusable generic mutation/query conformance is accepted through PR #235; repository step 16 owns worker conformance, step 17 owns contract lifecycle enforcement and step 19 proves real worker adoption.
- Stage G — **Not started**: repository step 14 owns the first measured behavior-neutral consolidation.
- Stage I — **Not started as a complete stage**: repository step 20 owns frontend/operations evidence and step 21 proves Phase 8A closure.

Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228, PR #232, PR #230, PR #235, PR #237, PR #239, PR #241, PR #244, PR #246, PR #248, PR #249, PR #253, PR #255 and PR #257. Current workspace package count remains `113`; root dependencies remain `prost`, `serde`, `serde_json`, `sha2`; exact Rust `1.97.1`, workspace `rust-version = "1.97.1"` and zero-warning Rust/Clippy budgets are enforced. The accepted suppression multiset is blocking, all three historical direct lint tables and matching governance exceptions are removed, and no source-level `allow` or `expect` replaces them.

The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–12 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235, PR #237, PR #239, PR #241, PR #244, PR #246, PR #248 and PR #249; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 11 is accepted through PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / squash merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 applicable permanent workflows on one unchanged exact head. It executes approved owner-specific anonymization and supported deletion through the exact nine authoritative owner boundaries, binds every call to canonical immutable case/snapshot/plan/retention/attempt lineage, and persists replay-safe tenant-bound mutation, idempotency, business transaction, audit and outbox evidence atomically under FORCE RLS. Real Parties acceptance proves mutation, exact replay, stale and cross-tenant rejection, clean PostgreSQL, rollback/reapply and repeated execution. Unsupported owner/action combinations and unavailable crypto-shred fail closed before mutation. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no crate, dependency, contract, migration, Cargo.lock, workspace-package or generic-runtime business-switch change was introduced.

Repository step 12 batch 1 is accepted through PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows on one unchanged exact head. It moves Parties, Consents, Contact Points and Party Relationships exact mutation/query inventories and activation-gated contribution builders behind `crm-first-party-modules`, preserves the already aggregated Customer Accounts contribution, exact public coordinates and ordering, activation, authorization, Party-reference validation, persistence, workers, package count and external dependency versions, and removes their ordinary registration/inventory bypasses from generic native composition. The exact native-composition guard path is classified under the existing operations scope while unknown sibling scripts remain fail closed. Repository step 12 is complete through the three accepted batches ending in PR #249; repository step 13 is complete through PRs #253, #255 and #257.

Repository step 13 is complete through PRs #253, #255 and #257. This exact-head documentation packet synchronizes the accepted closure evidence across all live normative sources. Repository step 14 is the next permitted implementation step and remains not started until this packet is accepted and merged.


### 3.2 Accepted repository step 13 closure evidence

PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / 7 of 7 applicable permanent workflows established the exact baseline: 113 workspace packages, 841 internal dependency edges, maximum dependency depth 18, maximum direct dependents 105, maximum transitive reverse impact 106, conservative public Rust surface 5,377, 40 permanent workflows, 41 jobs, 1,712 path-filter entries, 31 PostgreSQL workflows and 94 equivalent suppression occurrences across 66 stable keys.

PR #255 / accepted source `4c80546283af9c869a28c2da9c8697b203d0c327` / squash merge `393b60bdcfad6e92fc37eacabe0920645d530f6b` / 21 of 21 applicable permanent workflows registers that historical multiset with explicit policy metadata, blocks new stable keys and occurrence growth while allowing reductions and line movement, enforces canonical formatting, removes all three direct lint tables and matching architecture exceptions, moves the affected packages to workspace lint inheritance, and activates calibrated blocking dependency/public-surface/central-LOC/reverse-impact/change-cost governance. Exact-head Rust compile, Clippy, workspace tests, generated sync, affected scope, database and applicable process/privacy workflows all passed.

PR #257 / accepted source `6cde72d7fc9a442018c51fd6e6772e626b26e307` / squash merge `10516e84ea3c2d0fa8ee0c61c9eeec7e96a6273c` / 7 of 7 applicable permanent workflows on one unchanged exact head completes the remaining ADR-031 blocking exit evidence. It freezes reduction-only workspace and role-aware central-system budgets; separately proves `crm-api` remains production-thin at one runtime internal dependency plus eighteen acceptance-only dev dependencies and `crm-application-runtime` remains at sixty-two runtime plus one dev internal dependency; and blocks unmeasured process-host, representative change-cost, dependency-version/feature, heavy-feature, declaration and workspace-centralization growth while permitting reductions. The nine-file packet changes no product/runtime source, Cargo manifest, dependency declaration, package, route, contract, schema, migration, persistence or worker behavior.

Repository step 13 is complete through PRs #253, #255 and #257. This exact-head documentation packet synchronizes the accepted closure evidence across all live normative sources. Repository step 14 is the next permitted implementation step and remains not started until this packet is accepted and merged. PRs #253, #255 and #257 change no product behavior, dependency family, package count, route, contract, schema, migration, persistence semantics or worker behavior.

### 3.1 Remaining stage-to-step ownership

The stage labels are accounting only; the architecture master sequence remains authoritative.

| Remaining step | Primary stage | Supporting stages |
|---|---|---|
| 12 — complete contribution aggregation — **Complete through PR #249** | D | B, E |
| 13 — dependency/public-surface/fan-out/exception governance completion | B | A, E |
| 14 — measured consolidation | G | B, D |
| 15 — tombstone/no-orphan/convergence | C | F |
| 16 — generic worker conformance | F | E |
| 17 — contract lifecycle enforcement | F | A, E |
| 18 — local lifecycle commands | H | A |
| 19 — Customer Privacy worker and full E2E | C, F | D, H |
| 20 — frontend and operations evidence | I | E |
| 21 — Phase 8A closure | C, F, I | A, E |
| 22 — Phase 8A architecture remeasurement | all stages | measurement only; no automatic 10/10 claim |
| 23–24 — contrasting later expert-domain waves | C, D | B, E, F, H, I |
| 25 — final 10/10 closure review | all stages | succeeds only if every normative criterion is proven |

## 4. Phase 8A completed foundation

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import and recovery;
- **8A.8** — governed deterministic export and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

## 5. Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #244. `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution remain accepted trusted-internal runtime without public ingress or a Customer Privacy worker.

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

The accepted boundary is exact Rust `1.97.1`, root workspace `rust-version = "1.97.1"` and zero measured Rust/Clippy warnings and errors. PR #255 later removed all three historical direct-lint exceptions and their matching governance entries without source-level replacements.

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

### 5.14 Accepted multi-plane affected-scope enforcement

Repository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.

### 5.15 Accepted governed access/export assembly

Repository step 10 is accepted through PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head. It implements trusted-internal, replay-safe Customer Privacy access/export assembly through `customer_privacy.access_export.request@1.0.0` and the exact `customer_data.export.privacy.request@1.0.0` Customer Data Operations boundary. Customer Privacy persists an immutable strictly rehydrated manifest and stable job/artifact references before I/O; Customer Data Operations remains the durable job and immutable artifact owner. Deterministic identities recover pre-target and finalized-artifact/pre-link crash windows without a second logical job or artifact. Activation, exact case/snapshot/plan/checkpoint lineage, tenant and canonical-Party locking, registered initiating-capability provenance, FORCE RLS, transaction/outbox/audit/idempotency evidence, clean PostgreSQL, rollback/reapply and repeated acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route or alternate download endpoint, destructive action, crate, dependency, `Cargo.lock`, Protobuf contract, migration, workspace package or generic-runtime business switch was introduced.

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
9. **Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — Complete through PR #239.**
10. **Repository step 10 — governed Customer Privacy access/export assembly — Complete through PR #241.**
11. **Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — Complete through PR #244.**
12. **Repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — Complete through PR #249.**
13. **Repository step 13 — calibrated dependency, Rust public-surface, reverse-fan-out and exception governance — Complete through PR #257.**
14. **Repository step 14 — first measured behavior-neutral transitional domain-cluster consolidation — Next, not started.**
15. **Repository step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence.**
16. **Repository step 16 — reusable generic worker conformance.**
17. **Repository step 17 — contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement.**
18. **Repository step 18 — deterministic local lifecycle commands.**
19. **Repository step 19 — Customer Privacy worker, disable/uninstall fail-closed semantics and full process acceptance.**
20. **Repository step 20 — Phase 8A frontend and operations evidence.**
21. **Repository step 21 — Phase 8A closure.**
22. **Repository step 22 — Phase 8A architecture remeasurement; not a final 10/10 declaration.**
23. **Repository step 23 — first Phase 8B expert-domain wave.**
24. **Repository step 24 — second contrasting expert-domain wave.**
25. **Repository step 25 — final architecture 10/10 closure review only after every criterion is mechanically proven.**

The inserted prerequisites did not renumber the master sequence. Repository step 13 is complete through PR #257 after this synchronization. Repository step 14 is the next implementation step and remains not started.

## 7. Phase 8B and later expert domains

Phase 8B remains planned and blocked on repository steps 1–22 and completed Phase 8A. Repository step 23 begins the first measured extension wave; step 24 must provide a contrasting second wave before the final step 25 architecture closure review. Independent owner domains include Product Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment boundaries. They must not be absorbed into Sales.

Later planned work includes broader Sales/Activities, omnichannel, Marketing, Service/Knowledge/Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

## 8. Completion rule

Current product-complete expert modules: **0**.

Repository step 22 remeasures the Phase 8A architecture but cannot itself claim architecture 10/10. Final closure is no earlier than step 25 and requires the two later expert-domain waves plus every architecture completion criterion.

A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists. Completion requires defined domain breadth, governed APIs, persistence, authorization, audit, product UX and production/operational evidence.

## Repository step 12 completion evidence

Repository step 12 and Stage D — contribution aggregation are **complete**. All currently active first-party owners now expose owner-owned production contribution boundaries aggregated through `crm-first-party-modules`; generic native composition retains platform-level composition only.

Accepted implementation evidence:

- PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows — Parties, Consents, Contact Points and Party Relationships, preserving the already aggregated Customer Accounts owner;
- PR #248 / accepted source `b15482361ab2b322591d488843ab9b46ff676dba` / squash merge `b4222364c21cb74127834f5ff4f0739343d26379` / 37 of 37 applicable permanent workflows — Identity Resolution, Customer Data Operations and Data Quality;
- PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / squash merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 applicable permanent workflows — Sales/Activities, Customer 360 and Customer Enrichment.

The accepted batches are behavior-neutral: public coordinates and ordering, tenant activation, authorization, governed Party/Consent reads, persistence, projections and workers remain unchanged; workspace package count and external dependency versions remain unchanged.

Repository step 13 is **in progress** after accepted PR #253 measurement and PR #255 blocking suppression/direct-lint enforcement. Its next bounded implementation packet remeasures accepted `main` against the remaining ADR-031 exit criteria and performs only evidence-required remediation. No later repository step may start before step 13 is complete and synchronized. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**.

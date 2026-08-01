# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Customer Privacy packet: #126  
Commercial follow-on: #29  
Architecture/developer-experience program: #194  
Architecture guardrail and repository order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4  
Accepted Rust boundary: `RUST_TOOLCHAIN_AND_LINT_BASELINE.md` / `rust-governance-policy.json`  
Delivery governance: `DELIVERY_GOVERNANCE.md`

## 1. Packet contract

Every Phase 8 packet defines authoritative ownership, stable identity, exact coordinates, persistence, tenant/authorization/audit boundaries, recovery, architecture impact and focused/process/rollback acceptance. A packet is complete only after merge to `main` with unchanged exact-head evidence.

Ordinary capabilities add zero crates, generic router/worker algorithms do not grow owner-specific switches, feature implementation and physical consolidation remain separate, and frozen historical evidence is not rewritten by later runtime acceptance.

**Do not implement discovery/snapshot as one new crate per command**, query, worker, reader or composition fragment. If consolidation is required, **perform consolidation only in a separate behavior-neutral PR**.

Repository implementation is strictly sequential. Only the first unfinished item in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 may start, and only one implementation packet may be active.

## 2. Phase 8A completed work

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed import and recovery.
- **8A.8 — Complete:** governed export and recovery.
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship.
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance.

## 3. Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #244. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution have no public ingress.

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

## 4. Nine-owner set complete

The owner implementation lane is complete. No accepted owner may be described as unstarted and no additional owner contribution is the next packet.

## 5. Accepted architecture prerequisites and current gate

PR #197, PR #199, PR #200 and PR #203 established reproducible architecture/dependency no-growth controls. PR #204 froze scope discovery and immutable snapshot semantics. PR #205 accepted the Customer Privacy domain/application/PostgreSQL/production package boundary. Workspace packages remain `113`.

PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 permanent workflows completes repository step 1. Exact Rust `1.97.1`, root workspace `rust-version = "1.97.1"`, zero measured Rust/Clippy warnings and errors, unchanged `Cargo.lock` and unchanged 113 packages are accepted. PR #255 later removed all three historical direct-lint exceptions and matching governance entries without source-level replacements.

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows completes repository step 2.

PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows completes repository step 3 without behavior changes.

PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts the smallest inserted prerequisite required by repository step 4: a transaction-scoped customer-subject policy port and deterministic final guard composition.

PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows completes repository step 4. Immediate deny-only restriction placement and the first complete protected-owner boundary are accepted.

PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows completes repository step 5. Deterministic explanation, packet validation, real-diff affected enforcement and generated navigation are accepted.

PR #232 / accepted source `3f09dcc595f79d633915e4a67117aedc59ed2499` / squash merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 applicable permanent workflows accepts the smallest repository-step-6 architecture prerequisite. Rust Generated Sync and Rust CI now verify the committed dependency graph with locked Cargo commands, preserve `Cargo.lock` byte-for-byte on ordinary packets and cannot auto-commit registry drift. Intentional lockfile refresh remains explicit through `python scripts/repo.py lock` inside a bounded packet. The six-file change adds no product behavior, contract, manifest, dependency, package, persistence or migration change.

Repository step 6 is accepted through PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows on one unchanged source-authored head. It promotes `customer_privacy.legal_hold.place@1.0.0`, preserves the shared tenant + canonical Party lock, strictly rehydrates bounded FORCE-RLS legal-hold state and evaluates immutable plan items with precedence active legal hold → mandatory retention → approved privacy action. Public placement is activation-gated, live-authorized, tenant-bound, idempotent and atomic; malformed, unavailable, stale, over-bound and cross-tenant evidence fails closed. Clean PostgreSQL, real `crm-api`, rollback/reapply, replay and repeated acceptance are proven. The accepted inventory is 7 mutations / 4 permission-aware queries / 0 workers, with no owner execution, outcome persistence, export assembly, destructive action, dependency, `Cargo.lock`, manifest or workspace-package change.

Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.

Repository step 8 is accepted through PR #237 / accepted source `f926ece93dc2b24683f982828e72bf9170dc123a` / squash merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 applicable permanent workflows on one unchanged source-authored head. It persists deterministic tenant-bound owner execution attempts, checkpoints and safe outcomes with immutable action-plan and retention-decision lineage; composes exactly nine canonical owner endpoints through trusted-internal production wiring; recovers from pre-invocation, post-owner-result and post-outcome/pre-checkpoint crash windows without duplicate owner invocation; and makes `customer_privacy.case.owner_outcomes.list@1.0.0` paginate real persisted payload-safe outcomes. Activation, registered initiating-capability attribution, FORCE RLS, strict rehydration, idempotency, audit, clean apply, rollback, reapply and repeated PostgreSQL acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route, worker, access/export assembly, destructive owner execution, crate, dependency, `Cargo.lock`, workspace-package or generic-runtime algorithm change was introduced.

Repository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.

Repository step 10 is accepted through PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head. It implements trusted-internal, replay-safe Customer Privacy access/export assembly through `customer_privacy.access_export.request@1.0.0` and the exact `customer_data.export.privacy.request@1.0.0` Customer Data Operations boundary. Customer Privacy persists an immutable strictly rehydrated manifest and stable job/artifact references before I/O; Customer Data Operations remains the durable job and immutable artifact owner. Deterministic identities recover pre-target and finalized-artifact/pre-link crash windows without a second logical job or artifact. Activation, exact case/snapshot/plan/checkpoint lineage, tenant and canonical-Party locking, registered initiating-capability provenance, FORCE RLS, transaction/outbox/audit/idempotency evidence, clean PostgreSQL, rollback/reapply and repeated acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route or alternate download endpoint, destructive action, crate, dependency, `Cargo.lock`, Protobuf contract, migration, workspace package or generic-runtime business switch was introduced.

Repository step 11 is accepted through PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / squash merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 applicable permanent workflows on one unchanged exact head. It executes approved owner-specific anonymization and supported deletion through the exact nine authoritative owner boundaries, binds every call to canonical immutable case/snapshot/plan/retention/attempt lineage, and persists replay-safe tenant-bound mutation, idempotency, business transaction, audit and outbox evidence atomically under FORCE RLS. Real Parties acceptance proves mutation, exact replay, stale and cross-tenant rejection, clean PostgreSQL, rollback/reapply and repeated execution. Unsupported owner/action combinations and unavailable crypto-shred fail closed before mutation. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no crate, dependency, contract, migration, Cargo.lock, workspace-package or generic-runtime business-switch change was introduced.

Repository step 12 batch 1 is accepted through PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows on one unchanged exact head. It moves Parties, Consents, Contact Points and Party Relationships exact mutation/query inventories and activation-gated contribution builders behind `crm-first-party-modules`, preserves the already aggregated Customer Accounts contribution, exact public coordinates and ordering, activation, authorization, Party-reference validation, persistence, workers, package count and external dependency versions, and removes their ordinary registration/inventory bypasses from generic native composition. Repository step 12 is complete through the three accepted batches ending in PR #249; repository step 13 is in progress after accepted PRs #253 and #255.

Repository step 13 remains in progress after accepted suppression enforcement and direct-lint removal. The next permitted implementation packet remeasures accepted `main` against the remaining ADR-031 exit criteria, proves central process-host dependency reduction or justified non-growth and representative change-cost/dependency-version-feature budgets, and performs only evidence-required bounded remediation. Repository step 14 remains blocked.

## 5.1 Accepted repository step 13 measurement and suppression enforcement

PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / 7 of 7 applicable permanent workflows established the exact baseline: 113 workspace packages, 841 internal dependency edges, maximum dependency depth 18, maximum direct dependents 105, maximum transitive reverse impact 106, conservative public Rust surface 5,377, 40 permanent workflows, 41 jobs, 1,712 path-filter entries, 31 PostgreSQL workflows and 94 equivalent suppression occurrences across 66 stable keys.

PR #255 / accepted source `4c80546283af9c869a28c2da9c8697b203d0c327` / squash merge `393b60bdcfad6e92fc37eacabe0920645d530f6b` / 21 of 21 applicable permanent workflows registers that historical multiset, blocks new stable keys and occurrence growth while allowing reductions and line movement, enforces canonical formatting, removes all three direct lint tables and matching architecture exceptions, moves the affected packages to workspace lint inheritance, and activates calibrated blocking governance. Exact-head Rust compile, Clippy, workspace tests, generated sync, affected scope, database and applicable process/privacy workflows all passed.

Repository step 13 remains in progress. Its next bounded packet closes only the remaining ADR-031 exit-evidence gaps. PR #255 changes no Customer Privacy behavior, public inventory, worker, route, contract, persistence semantics or Phase 8A readiness; repository step 14 remains blocked.

## 6. Accepted scope discovery and immutable snapshot

State: **Accepted through PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows**.

PR #206 implements exact-nine trusted-internal discovery, immutable tenant/case/Party/topology/registry/purpose/effective-time lineage, bounded owner pagination, durable pages/checkpoints, safe reference-only aggregation, strict snapshot rehydration, replay/crash recovery, permission-aware internal reads, safe audit, FORCE RLS, cross-tenant concealment, rollback/reapply and repeated acceptance.

PR #207 synchronized accepted evidence. The historical PR #204 freeze remains unchanged.

## 7. Accepted deterministic planning freeze and runtime

PR #208 / source `d16a42551918ac6142d7a57cbeb7802f8f162fb9` / merge `bbdbc12ed139367efe75033c2a7e7ddb3eaec59d` / 16 of 16 permanent workflows froze immutable plan lineage, exact actions, deterministic ordering, contiguous sequence, lineage/item/plan digests, strict canonical rehydration, unsupported crypto-shred failure and permission-aware plan/outcome read boundaries.

PR #209 / source `b97fd9bb4537c14df4497ad7b737d0f0a64c4f3b` / merge `30621ffff5c1e07e1275cc80fee3f1297a91f49e` / 29 of 29 permanent workflows accepted trusted-internal activation-gated planning.

Implemented behavior:

- exact case, snapshot, Party/Identity Resolution, policy and jurisdiction lineage validation;
- deterministic immutable action plan using `Retain`, `RestrictOnly`, `Anonymize`, `Delete`, supported `CryptoShred` and reserved `NoOpAlreadyCompliant`;
- atomic `Scoped → Planned` or `Scoped → AwaitingApproval` transition;
- strict rehydration, replay/conflict detection and append-only case/snapshot/plan/audit evidence;
- FORCE RLS, canonical `tenant_isolation`, cross-tenant concealment, clean PostgreSQL, rollback/reapply and repeated acceptance;
- unchanged 113 packages, 4 mutations, 2 queries and 0 workers at the PR #209 boundary.

## 8. Accepted permission-aware reads

PR #211 / accepted source `933fa4b502d60a23b83de9ccee279cc6517b5cba` / merge `a1f3a60a6d8e8bba7bda50f936c57a61bc3521f7` / 32 of 32 permanent workflows promotes only:

1. `customer_privacy.case.plan.get@1.0.0` with module activation, live permission/visibility, tenant-bound reads, strict case↔snapshot↔plan↔replay evidence, payload-safe summary, audited read and concealed unauthorized/cross-tenant existence;
2. `customer_privacy.case.owner_outcomes.list@1.0.0` with bounded validation and a deterministic empty terminal page (`items = []`) at the historical PR #211 boundary. PR #237 later accepted durable safe outcomes and real permission-aware pagination without changing this public coordinate.

The packet adds one append-only FORCE-RLS safe read-audit table and no owner-outcome table. Stable page/terminal digests are durable audit evidence; no synthetic outcome is returned.

No new crate, generic-runtime switch, mutation, worker, owner mutation, approval, restriction, hold/retention decision or destructive execution is included.

### 8.1 Accepted approval runtime

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows accepts `customer_privacy.case.approve@1.0.0` through the existing Customer Privacy packages.

The mutation is activation-gated, live-authorized, tenant-bound, expected-version protected and idempotent. It permits only `AwaitingApproval → Planned`, locks the case and immutable scope/plan evidence, validates exact case↔subject↔snapshot↔plan lineage, records immutable approval actor/time and atomically persists status, event, audit, idempotency and business evidence. Exact replay succeeds; conflicting replay, corrupt evidence, unauthorized access and cross-tenant existence fail closed.

No restriction placement/release, legal-hold/mandatory-retention decision, owner execution/outcome persistence, access/export assembly, destructive action, worker, crate, dependency family or generic-runtime business switch was added by approval.

### 8.2 Accepted bounded contribution aggregation

PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows accepts the first behavior-neutral contribution-aggregation packet.

Customer Accounts data-only mutation/query definition factories remain owned by its existing production composition package and are re-exported through `crm-first-party-modules`. Generic application runtime consumes those selected inventories through the aggregate while retaining exact public inventory order, deterministic Account-before-Consents contribution order and existing module activation behavior.

The packet changes no Customer Privacy route, coordinate, persistence, migration, authorization, tenant, audit, idempotency, public inventory or product behavior. It adds no dependency family, manifest, `Cargo.lock`, workspace package, crate consolidation, worker or generic business dispatch switch.

### 8.3 Accepted final customer-subject policy prerequisite

PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts the minimal prerequisite required before restriction placement can be safely promoted.

`TransactionalCustomerSubjectPolicyPort` is transaction-scoped and requires the shared tenant + canonical Party lock plus an authoritative live decision before protected persistence or I/O. `TransactionalAggregateGuardChain` composes owner-specific and Customer Privacy final guards deterministically without changing the generic executor algorithm. Processing and communication operation classes are explicit. Unavailable, stale, corrupt or cross-tenant decisions must fail closed; no allow-all production implementation exists.

The prerequisite changes no route, coordinate, public inventory, restriction runtime, owner integration, persistence, migration, dependency family, manifest, `Cargo.lock`, workspace package, worker or product behavior. Those historical prerequisite non-effects remain accepted.

### 8.4 Accepted immediate deny-only restrictions

PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows accepts repository step 4.

`customer_privacy.restriction.place@1.0.0` is promoted through the generic ingress with a strict high-risk Personal contract, deterministic tenant/idempotency identity, canonical topology proof, shared Party lock and atomic record/event/audit/idempotency/business evidence. The PostgreSQL final decision evaluates bounded strictly rehydrated FORCE-RLS state under the same lock and fails closed on unavailable, malformed, over-bound and cross-tenant evidence.

`contact-points.contact-point.create@1.0.0` is the first complete protected-owner path. Real-process acceptance proves placement, active denial immediately before persistence, zero denied side effects, unrelated-Party isolation, rollback/reapply and repeated acceptance. Restriction release/reads, legal holds, retention decisions, owner execution, access/export, destructive actions and workers remain later work.

### 8.5 Accepted repository explanation and generated navigation

PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows accepts repository step 5.

The packet provides deterministic module/capability explanation, exact baseline and path-policy packet checking, real pull-request diff enforcement, generated active-packet/repository-map navigation and freshness-checked conformance. It changes no Customer Privacy runtime, route, contract, manifest, persistence, migration, dependency, `Cargo.lock`, package or worker.

### 8.6 Accepted legal-hold and mandatory-retention precedence

Repository step 6 is accepted through PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows on one unchanged source-authored head. It promotes `customer_privacy.legal_hold.place@1.0.0`, preserves the shared tenant + canonical Party lock, strictly rehydrates bounded FORCE-RLS legal-hold state and evaluates immutable plan items with precedence active legal hold → mandatory retention → approved privacy action. Public placement is activation-gated, live-authorized, tenant-bound, idempotent and atomic; malformed, unavailable, stale, over-bound and cross-tenant evidence fails closed. Clean PostgreSQL, real `crm-api`, rollback/reapply, replay and repeated acceptance are proven. The accepted inventory is 7 mutations / 4 permission-aware queries / 0 workers, with no owner execution, outcome persistence, export assembly, destructive action, dependency, `Cargo.lock`, manifest or workspace-package change.

### 8.7 Accepted reusable generic mutation/query conformance

Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.

### 8.8 Accepted replay-safe resumable owner execution

Repository step 8 is accepted through PR #237 / accepted source `f926ece93dc2b24683f982828e72bf9170dc123a` / squash merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 applicable permanent workflows on one unchanged source-authored head. It persists deterministic tenant-bound owner execution attempts, checkpoints and safe outcomes with immutable action-plan and retention-decision lineage; composes exactly nine canonical owner endpoints through trusted-internal production wiring; recovers from pre-invocation, post-owner-result and post-outcome/pre-checkpoint crash windows without duplicate owner invocation; and makes `customer_privacy.case.owner_outcomes.list@1.0.0` paginate real persisted payload-safe outcomes. Activation, registered initiating-capability attribution, FORCE RLS, strict rehydration, idempotency, audit, clean apply, rollback, reapply and repeated PostgreSQL acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route, worker, access/export assembly, destructive owner execution, crate, dependency, `Cargo.lock`, workspace-package or generic-runtime algorithm change was introduced.

Repository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.

## 9. Binding repository continuation

The complete order is maintained in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4. The Phase 8 positions are:

1. repository step 1 — supported Rust toolchain, workspace `rust-version` and measured lint baseline — **complete through PR #218**;
2. repository step 2 — Customer Privacy approval runtime — **complete through PR #220**;
3. repository step 3 — bounded contribution aggregation without behavior change — **complete through PR #222**;
3a. inserted prerequisite for repository step 4 — final customer-subject policy port and deterministic guard composition — **complete through PR #224**;
4. repository step 4 — immediate deny-only processing restrictions using final subject locks — **complete through PR #226**;
5. repository step 5 — `explain`, `packet-check` and generated navigation — **complete through PR #228**;
5a. inserted lockfile-preservation prerequisite before repository step 6 — **complete through PR #232**;
6. repository step 6 — legal-hold and mandatory-retention precedence — **complete through PR #230**;
7. repository step 7 — reusable generic mutation/query conformance — **complete through PR #235**;
8. repository step 8 — replay-safe resumable owner execution and crash-window recovery — **complete through PR #237**;
9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **complete through PR #239**;
10. repository step 10 — governed access/export assembly — **complete through PR #241**;
11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **complete through PR #244**;
12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **complete through PR #249**;
13. repository step 13 — calibrated dependency, Rust public-surface, reverse-fan-out and exception governance — **in progress: measurement accepted through PR #253 and blocking suppression/direct-lint enforcement accepted through PR #255; remaining ADR-031 exit evidence is next**;
14. repository step 14 — first measured behavior-neutral consolidation;
15. repository step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence;
16. repository step 16 — reusable generic worker conformance;
17. repository step 17 — contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement;
18. repository step 18 — deterministic local lifecycle commands;
19. repository step 19 — Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
20. repository step 20 — Phase 8A frontend and operations evidence;
21. repository step 21 — Phase 8A closure;
22. repository step 22 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;
23. repository step 23 — first Phase 8B expert-domain wave;
24. repository step 24 — second contrasting expert-domain wave;
25. repository step 25 — final architecture 10/10 closure review only after every criterion is mechanically proven.

The inserted prerequisites did not renumber the normative master sequence. Repository step 13 remains the only active implementation step; its next bounded packet closes only the remaining ADR-031 exit-evidence gaps. Repository step 14 remains blocked.


### 9.1 Stage accountability for the remaining Phase 8 sequence

| Step | Primary stage responsibility |
|---|---|
| 11 | C — destructive owner execution inside the golden owner model |
| 12 | D — complete contribution aggregation for current first-party owners |
| 13 | B — dependency/public-surface/fan-out/exception governance completion |
| 14 | G — measured behavior-neutral consolidation |
| 15 | C — tombstone, no-orphan and convergence persistence model |
| 16 | F — reusable worker conformance |
| 17 | F — contract compatibility/deprecation/retirement enforcement |
| 18 | H — reproducible local lifecycle |
| 19 | C + F — real Customer Privacy worker and lifecycle proof |
| 20 | I — frontend and operations parity |
| 21 | C + F + I — Phase 8A closure |
| 22 | all stages — measurement checkpoint only |
| 23–24 | later-domain proof that extension cost remains bounded |
| 25 | final architecture closure review |

Step 12 is architecture refactoring only and does not change Customer Privacy product behavior. Step 13 is a separate governance program: PR #253 accepted measurement and PR #255 accepted blocking suppression/direct-lint enforcement; only the remaining ADR-031 exit evidence may continue before step 14. Step 14 is physical consolidation only and remains separate from feature behavior. Step 17 separately enforces contract lifecycle. Step 22 cannot close issue #194 or declare 10/10 before steps 23 and 24 provide contrasting later-domain evidence.

## 10. Frozen ownership

`crm.customer-privacy` owns privacy cases, immutable scope snapshots, deterministic plans, restrictions, customer-data legal holds, retention decisions, per-owner attempts/outcomes, checkpoints, governed export references and convergence evidence. It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Customer Data Operations, Data Quality or Customer Enrichment storage.

```text
legal hold > mandatory retention > approved privacy action > ordinary retention
```

## 11. Phase 8A closure

Phase 8A remains **In progress**.

It closes only after restrictions and legal holds are extended with release/read lifecycle where required, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle, frontend/operations evidence and full process acceptance are merged in the binding repository order.

## 12. Phase 8B and completion rule

Product Catalog, Pricing, CPQ, Orders, Contracts, Subscriptions and Billing remain blocked until repository step 22 and Phase 8A closure permit step 23. Two contrasting later expert-domain waves at steps 23 and 24 are required before the step 25 final architecture 10/10 review.

Current product-complete expert modules: **0**.

## Repository step 12 completion evidence

Repository step 12 and Stage D — contribution aggregation are **complete**. All currently active first-party owners now expose owner-owned production contribution boundaries aggregated through `crm-first-party-modules`; generic native composition retains platform-level composition only.

Accepted implementation evidence:

- PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows — Parties, Consents, Contact Points and Party Relationships, preserving the already aggregated Customer Accounts owner;
- PR #248 / accepted source `b15482361ab2b322591d488843ab9b46ff676dba` / squash merge `b4222364c21cb74127834f5ff4f0739343d26379` / 37 of 37 applicable permanent workflows — Identity Resolution, Customer Data Operations and Data Quality;
- PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / squash merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 applicable permanent workflows — Sales/Activities, Customer 360 and Customer Enrichment.

The accepted batches are behavior-neutral: public coordinates and ordering, tenant activation, authorization, governed Party/Consent reads, persistence, projections and workers remain unchanged; workspace package count and external dependency versions remain unchanged.

Repository step 13 is **in progress** after accepted PR #253 measurement and PR #255 blocking suppression/direct-lint enforcement. Its next bounded implementation packet remeasures accepted `main` against the remaining ADR-031 exit criteria and performs only evidence-required remediation. No later repository step may start before step 13 is complete and synchronized. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**.

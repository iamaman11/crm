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

Latest accepted public runtime inventory is six mutations, four permission-aware public queries and zero Customer Privacy workers through PR #226. Trusted-internal `customer_privacy.plan.build@1.0.0` still has no public ingress.

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

PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 permanent workflows completes repository step 1. Exact Rust `1.97.1`, root workspace `rust-version = "1.97.1"`, zero measured Rust/Clippy warnings and errors, unchanged `Cargo.lock`, unchanged 113 packages and three exact expiring no-growth direct-lint exceptions are accepted.

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows completes repository step 2.

PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows completes repository step 3 without behavior changes.

PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts the smallest inserted prerequisite required by repository step 4: a transaction-scoped customer-subject policy port and deterministic final guard composition.

PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows completes repository step 4. Immediate deny-only restriction placement and the first complete protected-owner boundary are accepted; repository step 5 is now the next permitted implementation packet.

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
2. `customer_privacy.case.owner_outcomes.list@1.0.0` with bounded validation and a deterministic empty terminal page (`items = []`) until owner execution and outcome persistence exist.

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

## 9. Binding repository continuation

The complete order is maintained in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4. The Phase 8 positions are:

1. repository step 1 — supported Rust toolchain, workspace `rust-version` and measured lint baseline — **complete through PR #218**;
2. repository step 2 — Customer Privacy approval runtime — **complete through PR #220**;
3. repository step 3 — bounded contribution aggregation without behavior change — **complete through PR #222**;
3a. inserted prerequisite for repository step 4 — final customer-subject policy port and deterministic guard composition — **complete through PR #224**;
4. repository step 4 — immediate deny-only processing restrictions using final subject locks — **complete through PR #226**;
5. repository step 5 — `explain`, `packet-check` and generated navigation — **next**;
6. repository step 6 — legal-hold and mandatory-retention precedence;
7. repository step 7 — reusable generic mutation/query conformance;
8. repository step 8 — replay-safe resumable owner execution and crash-window recovery;
9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks;
10. repository step 10 — governed access/export assembly;
11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution;
12. repository step 12 — first measured behavior-neutral consolidation;
13. repository step 13 — Party tombstone, no-orphan proof and projection/search/cache convergence;
14. repository step 14 — reusable generic worker conformance;
15. repository step 15 — deterministic local lifecycle commands;
16. repository step 16 — Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
17. repository step 17 — Phase 8A frontend and operations evidence;
18. repository step 18 — Phase 8A closure;
19. repository step 19 — architecture remeasurement and publication of the next numbered order;
20. repository step 20 — first Phase 8B packet.

The inserted prerequisite did not renumber the normative master sequence. A later step must not start while repository step 5 is unfinished.

## 10. Frozen ownership

`crm.customer-privacy` owns privacy cases, immutable scope snapshots, deterministic plans, restrictions, customer-data legal holds, retention decisions, per-owner attempts/outcomes, checkpoints, governed export references and convergence evidence. It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Customer Data Operations, Data Quality or Customer Enrichment storage.

```text
legal hold > mandatory retention > approved privacy action > ordinary retention
```

## 11. Phase 8A closure

Phase 8A remains **In progress**.

It closes only after restrictions are extended with release/read lifecycle where required, holds/retention, owner execution, access/export, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle, frontend/operations evidence and full process acceptance are merged in the binding repository order.

## 12. Phase 8B and completion rule

Product Catalog, Pricing, CPQ, Orders, Contracts, Subscriptions and Billing remain blocked until the repository order and Phase 8A closure permit them.

Current product-complete expert modules: **0**.

# Ultimate CRM — Project Status

Status date: 2026-07-29

This is the concise current-state snapshot. Normative product dependencies remain in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`; the single repository execution order is in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4; module readiness remains in `MODULE_CATALOG.md`.

## Authoritative references

- `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `DELIVERY_GOVERNANCE.md`;
- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 for the only permitted repository packet order;
- `RUST_TOOLCHAIN_AND_LINT_BASELINE.md` and `rust-governance-policy.json` for the accepted Rust boundary and measured lint cohorts;
- `IMPLEMENTATION_ROADMAP.md`, `PHASE8_DELIVERY_PLAN.md`, `MODULE_CATALOG.md`;
- `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_FREEZE.md` and `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_IMPLEMENTATION.md`;
- `CUSTOMER_PRIVACY_PLANNING_FREEZE.md` and `CUSTOMER_PRIVACY_PLANNING_IMPLEMENTATION.md`;
- `CUSTOMER_PRIVACY_PLAN_READS_IMPLEMENTATION.md` and its machine-readable evidence;
- `CUSTOMER_PRIVACY_APPROVAL_IMPLEMENTATION.md` and `contracts/customer-privacy-approval-implementation.json`;
- `CRM_CAPABILITY_COVERAGE.md` and the accepted historical owner packet documents.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

Latest accepted Customer Privacy runtime baseline is PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows.

Latest accepted repository architecture/developer-experience packet is PR #232 / accepted source `3f09dcc595f79d633915e4a67117aedc59ed2499` / squash merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 applicable permanent workflows.

The accepted inventory is seven public mutations (`case.create`, `case.submit`, `case.subject.verify`, `case.cancel`, `case.approve`, `restriction.place`, `legal_hold.place`), four permission-aware public queries (`case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`) and zero Customer Privacy workers. `customer_privacy.plan.build@1.0.0` remains trusted-internal runtime with no public route.

**All nine authoritative owner implementations are accepted:**

1. Parties — PR #156 / merge `4368b8c3710e05137b71ba999bf7f3497c0801c8`;
2. Consents — PR #175 / merge `039d6461803208f6cb70ce0fbcfcaffaf59d7125`;
3. Customer Accounts — PR #179 / merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`;
4. Contact Points — PR #181 / merge `96cd0cf548310592a0718c97242a724a29717a72`;
5. Party Relationships — PR #183 / merge `9ad2aa91321e9edb54cab98218f93143923ef33f`;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

## Accepted scope discovery and immutable snapshot

Accepted through PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows. The packet accepted trusted-internal exact-nine discovery, immutable snapshot lineage, bounded pages/checkpoints, strict rehydration, replay/crash recovery, permission-aware internal reads, safe audit, FORCE RLS, cross-tenant concealment, rollback, reapply and repeated acceptance.

The public inventory and workspace package count remained unchanged. PR #207 synchronized its post-merge evidence.

## Accepted deterministic planning runtime

PR #208 froze the deterministic action-plan and permission-aware read semantics. Its historical status text says **runtime implementation not started** and remains immutable historical evidence.

PR #209 later accepted the trusted-internal activation-gated planning runtime. It verifies the exact case, immutable scope snapshot, Party/Identity Resolution binding, policy and jurisdiction lineage; builds deterministic `Retain`, `RestrictOnly`, `Anonymize`, `Delete` or supported `CryptoShred` items; reserves `NoOpAlreadyCompliant`; strictly rehydrates the plan; atomically persists case/snapshot/plan replay evidence and transitions `Scoped → Planned` or `Scoped → AwaitingApproval`.

Accepted controls include canonical owner/resource order, contiguous sequence, lineage/item/plan digests, unsupported crypto-shred fail closed, idempotent replay/conflict detection, append-only evidence, FORCE RLS, canonical `tenant_isolation`, cross-tenant concealment, clean PostgreSQL, rollback/reapply and repeated acceptance. PR #209 historical inventory remains 4 mutations / 2 queries / 0 workers.

## Accepted permission-aware read packet

PR #211 / accepted source `933fa4b502d60a23b83de9ccee279cc6517b5cba` / merge `a1f3a60a6d8e8bba7bda50f936c57a61bc3521f7` / 32 of 32 permanent workflows promotes only `customer_privacy.case.plan.get@1.0.0` and `customer_privacy.case.owner_outcomes.list@1.0.0` through the existing Customer Privacy application/PostgreSQL/production packages.

`case.plan.get` is activation-gated, tenant-bound and live permission-aware. It strictly validates the case, immutable snapshot, immutable plan and durable replay link before returning a payload-safe summary without owner resource payloads.

`case.owner_outcomes.list` validates bounded page/cursor input and returns a deterministic empty terminal page (`items = []`, empty terminal cursor) because owner execution and outcome persistence remain absent. Stable page/terminal digests and safe allow/deny evidence are append-only in a FORCE-RLS audit table. No outcome table, synthetic outcomes, mutation or worker is added.

## Accepted approval runtime

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows completes repository step 2. `customer_privacy.case.approve@1.0.0` is activation-gated, live-authorized, tenant-bound and optimistic-concurrency protected. It permits only `AwaitingApproval → Planned`, locks the case and immutable snapshot/plan evidence, preserves exact case↔subject↔snapshot↔plan lineage, records immutable approval actor/time, and atomically persists status, event, audit, idempotency and business evidence. Exact replay succeeds; conflicting replay, corrupt evidence, unauthorized access and cross-tenant existence fail closed.

The packet adds no crate, dependency family, worker, restriction placement/release, legal-hold or mandatory-retention adjudication, owner execution/outcome persistence, access/export assembly or destructive action.

## Accepted Rust toolchain and lint governance

Repository step 1 is accepted through PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 applicable permanent workflows.

The repository now supports exact Rust `1.97.1`; root workspace `rust-version` is `1.97.1`; Rust and Clippy warning/error budgets are zero and measured from JSON compiler output; `Cargo.lock`, all 113 workspace packages, dependencies and product behavior remain unchanged. Three historical direct `clippy.too_many_arguments = "allow"` tables are exact no-growth exceptions with named owners, compensating checks, removal conditions and expiry on `2027-01-31`.

## Accepted bounded contribution aggregation

Repository step 3 is accepted through PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows on one unchanged exact head.

The Customer Accounts owner composition now exposes its exact data-only mutation/query definition factories, and `crm-first-party-modules` re-exports them beside the already accepted owner contribution builder. Generic application runtime consumes those selected inventory factories through the first-party facade while preserving the exact mutation/query order, deterministic Account-before-Consents contribution order and existing activation gates.

The five-file packet changed no route, coordinate, public inventory, persistence, migration, tenant isolation, authorization, audit, idempotency, product behavior, dependency family, manifest, `Cargo.lock`, workspace member, worker or Customer Privacy behavior. Workspace package count remains 113.

## Accepted final customer-subject policy prerequisite

PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts the smallest architecture prerequisite required by repository step 4.

`crm-core-data` now exposes a transaction-scoped `TransactionalCustomerSubjectPolicyPort` for authoritative live processing/communication decisions in the caller's PostgreSQL transaction and a deterministic `TransactionalAggregateGuardChain` for ordered final guards. The port contract requires the shared tenant + canonical Party lock and bounded denial on unavailable, stale, corrupt or cross-tenant decisions. No no-op or allow-all production implementation exists.

The three-file prerequisite added no route, coordinate, public inventory, restriction runtime, owner integration, persistence, migration, dependency family, manifest, `Cargo.lock`, workspace package, worker or product behavior. Those historical prerequisite non-effects remain accepted.

## Accepted immediate deny-only restriction runtime

Repository step 4 is accepted through PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows on one unchanged source-authored head.

`customer_privacy.restriction.place@1.0.0` is public, activation-gated, live-authorized, tenant-bound and idempotent. Placement proves the Party is currently canonical, acquires the shared tenant + Party lock, validates a strict high-risk Personal contract, persists record/event/audit/idempotency/business evidence and uses deterministic tenant/idempotency-bound identity.

The authoritative PostgreSQL final decision reads bounded FORCE-RLS state under the same lock and fails closed on unavailable, malformed, over-bound or cross-tenant evidence. `contact-points.contact-point.create@1.0.0` is the first complete protected-owner boundary and checks the final policy immediately before persistence. Permanent real-process acceptance proves pre-restriction success, public placement, active denial without side effects, unrelated-Party isolation, full rollback/reapply and repeated acceptance.

Restriction release/reads, owner execution/outcomes, access/export assembly, destructive actions and Customer Privacy workers remain non-runtime.

## Accepted repository explanation and generated navigation

Repository step 5 is accepted through PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows on one unchanged source-authored head.

The repository now has deterministic `repo.py explain` for exact module/capability ownership and route classification, fail-closed `repo.py packet-check` for exact baseline/path/affected/workflow/freshness validation, generated `docs/ACTIVE_PACKET.md`, and generated `docs/generated/REPOSITORY_MAP.md` with source digests. Affected Scope CI executes packet-check against the real pull-request diff before structural, Clippy and test closures.

The accepted generated inventory is 113 workspace packages, 14 business manifests, 119 published capability coordinates, 70 published event coordinates, 7 platform runtime routes, 5 worker runtime routes, 17 non-runtime contract routes and one route-less module. Product runtime, contracts, manifests, migrations, dependencies, `Cargo.lock`, package count and Customer Privacy behavior are unchanged.

## Accepted lockfile-preserving Rust workflow prerequisite

PR #232 / accepted source `3f09dcc595f79d633915e4a67117aedc59ed2499` / squash merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 applicable permanent workflows accepts the smallest repository-step-6 architecture prerequisite. Rust Generated Sync and Rust CI now verify the committed dependency graph with locked Cargo commands, preserve `Cargo.lock` byte-for-byte on ordinary packets and cannot auto-commit registry drift. Intentional lockfile refresh remains explicit through `python scripts/repo.py lock` inside a bounded packet. The six-file change adds no product behavior, contract, manifest, dependency, package, persistence or migration change.

## Accepted legal-hold and mandatory-retention precedence

Repository step 6 is accepted through PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows on one unchanged source-authored head. It promotes `customer_privacy.legal_hold.place@1.0.0`, preserves the shared tenant + canonical Party lock, strictly rehydrates bounded FORCE-RLS legal-hold state and evaluates immutable plan items with precedence active legal hold → mandatory retention → approved privacy action. Public placement is activation-gated, live-authorized, tenant-bound, idempotent and atomic; malformed, unavailable, stale, over-bound and cross-tenant evidence fails closed. Clean PostgreSQL, real `crm-api`, rollback/reapply, replay and repeated acceptance are proven. The accepted inventory is 7 mutations / 4 permission-aware queries / 0 workers, with no owner execution, outcome persistence, export assembly, destructive action, dependency, `Cargo.lock`, manifest or workspace-package change.

## Next permitted repository packet

Repository step 7 is reusable generic mutation and query conformance.

## Following permitted repository packet

Repository step 8 is replay-safe resumable Customer Privacy owner execution and crash-window recovery.

## Architecture and developer-experience 10/10 checkpoint

Issue #194 remains open.

- Stage A documentation/source hierarchy and stable navigation are complete.
- Stage B dependency/crate/exception governance is in progress: reproducible metrics, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1`, root `rust-version`, measured zero-warning Rust/Clippy governance, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions are accepted; broader dependency/public-surface calibration and exception removal remain.
- Stage C is in progress: the Customer Privacy golden package model, final customer-subject policy prerequisite, authoritative restriction decision, public restriction/legal-hold placement, retention adjudication and first protected-owner integration are accepted; broader owner adoption and migration/visibility generalization remain.
- Stage D is in progress: the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222, but the generic runtime still imports many other concrete owner adapters and remaining owners have not migrated.
- Stage E has a working foundation but is incomplete: real-diff packet validation and Rust broadening are accepted, while database/process/product/frontend/operations selection remains incomplete.
- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands remain repository step 15.
- Stages F, G and I remain foundation-only or unstarted; generic conformance/lifecycle, measured consolidation, frontend and operations parity are not complete.

## Repository continuation order

Only one implementation packet may be active. The current order begins:

```text
1. supported Rust toolchain / rust-version / measured lint baseline — complete through PR #218
-> 2. Customer Privacy approval runtime — complete through PR #220
-> 3. bounded contribution aggregation — complete through PR #222
-> 3a. final customer-subject policy port prerequisite — complete through PR #224
-> 4. immediate deny-only restrictions — complete through PR #226
-> 5. explain / packet-check / generated active packet and repository map — complete through PR #228
-> 5a. lockfile-preserving Rust workflow prerequisite — complete through PR #232
-> 6. legal-hold and mandatory-retention precedence — complete through PR #230
-> 7. reusable generic mutation/query conformance — next
```

The complete binding order through Phase 8B entry is `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4. No later item may start while an earlier item is unfinished.

Phase 8A closure does not make the universal CRM complete. Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing and the wider expert CRM domains remain planned or incomplete.

Current product-complete expert modules: **0**.

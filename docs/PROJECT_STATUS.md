# Ultimate CRM — Project Status

Status date: 2026-07-28

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

Latest accepted Customer Privacy runtime baseline is PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows.

Latest accepted repository architecture prerequisite is PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows.

The accepted inventory is five public mutations (`case.create`, `case.submit`, `case.subject.verify`, `case.cancel`, `case.approve`), four permission-aware public queries (`case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`) and zero Customer Privacy workers. `customer_privacy.plan.build@1.0.0` remains trusted-internal runtime with no public route.

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

The three-file packet adds no route, coordinate, public inventory, restriction runtime, owner integration, persistence, migration, dependency family, manifest, `Cargo.lock`, workspace package, worker or product behavior. Restrictions remain unimplemented.

## Next permitted repository packet

Repository step 4 remains immediate deny-only Customer Privacy processing restrictions using final subject locks. It must implement the authoritative Customer Privacy policy decision and integrate the final guard into the bounded protected owner paths before public promotion. Legal-hold/mandatory-retention adjudication, owner execution, destructive actions and workers remain later packets.

## Following permitted repository packet

Repository step 5 is `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and the generated repository map.

## Architecture and developer-experience 10/10 checkpoint

Issue #194 remains open.

- Stage A documentation/source hierarchy and stable navigation are complete.
- Stage B dependency/crate/exception governance is in progress: reproducible metrics, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1`, root `rust-version`, measured zero-warning Rust/Clippy governance and three exact expiring legacy lint exceptions are accepted; broader dependency/public-surface calibration and exception removal remain.
- Stage C is in progress: the Customer Privacy domain/application/postgres/production golden pilot and the final customer-subject policy port/guard-chain prerequisite are accepted, but the authoritative restriction decision and protected-owner integration are not implemented.
- Stage D is in progress: the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222, but the generic runtime still imports many other concrete owner adapters and remaining owners have not migrated.
- Stage E has a working foundation but is incomplete: affected-scope selection is not yet complete across every database/process/product/frontend/operations dimension.
- Stages F–I remain foundation-only or unstarted; generic conformance/lifecycle, measured consolidation, reproducible local environment, generated navigation, frontend and operations parity are not complete.

## Repository continuation order

Only one implementation packet may be active. The current order begins:

```text
1. supported Rust toolchain / rust-version / measured lint baseline — complete through PR #218
-> 2. Customer Privacy approval runtime — complete through PR #220
-> 3. bounded contribution aggregation — complete through PR #222
-> 3a. final customer-subject policy port prerequisite — complete through PR #224
-> 4. immediate deny-only restrictions — next
-> 5. explain / packet-check / generated active packet and repository map
-> 6. legal-hold and mandatory-retention precedence
```

The complete binding order through Phase 8B entry is `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4. No later item may start while an earlier item is unfinished. The prerequisite did not renumber the master sequence; it was inserted under the existing minimal-prerequisite rule and work now returns to repository step 4.

Phase 8A closure does not make the universal CRM complete. Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing and the wider expert CRM domains remain planned or incomplete.

Current product-complete expert modules: **0**.

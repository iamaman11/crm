# Ultimate CRM — Project Status

Status date: 2026-07-28

This is the concise current-state snapshot. Normative order remains in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`; module readiness remains in `MODULE_CATALOG.md`; issue #194 is governed by `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`.

## Authoritative references

- `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `DELIVERY_GOVERNANCE.md`;
- `IMPLEMENTATION_ROADMAP.md`, `PHASE8_DELIVERY_PLAN.md`, `MODULE_CATALOG.md`;
- `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_FREEZE.md` and `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_IMPLEMENTATION.md`;
- `CUSTOMER_PRIVACY_PLANNING_FREEZE.md` and `CUSTOMER_PRIVACY_PLANNING_IMPLEMENTATION.md`;
- `CUSTOMER_PRIVACY_PLAN_READS_IMPLEMENTATION.md` and its machine-readable evidence;
- `CRM_CAPABILITY_COVERAGE.md` and the accepted historical owner packet documents.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

Latest accepted Customer Privacy runtime baseline is PR #211 / accepted source `933fa4b502d60a23b83de9ccee279cc6517b5cba` / merge `a1f3a60a6d8e8bba7bda50f936c57a61bc3521f7` / 32 of 32 permanent workflows.

The accepted inventory is four mutations (`case.create`, `case.submit`, `case.subject.verify`, `case.cancel`), four permission-aware queries (`case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`) and zero Customer Privacy workers. `customer_privacy.plan.build@1.0.0` remains trusted-internal runtime with no public route.

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

## Next bounded packet

Implement approval runtime only. Immediate deny-only restrictions, legal-hold/mandatory-retention precedence, owner execution, destructive actions and workers remain later separate packets.

## Continuation order

```text
accepted scope discovery and immutable snapshot
-> accepted deterministic planning runtime
-> accepted permission-aware plan/outcome reads
-> approval runtime
-> immediate deny-only restrictions
-> legal-hold and mandatory-retention precedence
-> replay-safe resumable owner execution and crash recovery
-> governed access/export and deletion/anonymization/crypto-shred
-> Party tombstone, no-orphan proof and projection/search/cache convergence
-> Customer Privacy worker and full lifecycle acceptance
-> Phase 8A closure
-> Phase 8B / issue #29
```

Issue #194 remains open. Stage A, the Stage B dependency no-growth prerequisite and the Stage C Customer Privacy golden-package pilot are accepted; broader architecture/developer-experience stages remain incomplete.

Phase 8A closure does not make the universal CRM complete. Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing and the wider expert CRM domains remain planned or incomplete.

Current product-complete expert modules: **0**.

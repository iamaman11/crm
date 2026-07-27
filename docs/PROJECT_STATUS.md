# Ultimate CRM — Project Status

Status date: 2026-07-27

This document is the concise human-readable snapshot. Normative delivery order remains in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`; business-module readiness remains in `MODULE_CATALOG.md`; the cross-cutting architecture/developer-experience program remains in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`.

## Authoritative references

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `APPLICATION_ARCHITECTURE.md` — stable layer and composition model.
3. `ARCHITECTURE_READINESS.md` — accepted native-composition baseline.
4. `DELIVERY_GOVERNANCE.md` — packet state, exact-head evidence and synchronization policy.
5. `IMPLEMENTATION_ROADMAP.md` — normative product phase order.
6. `PHASE8_DELIVERY_PLAN.md` — detailed active Phase 8 sequence.
7. `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` — issue #194 architecture and developer-experience 10/10 execution program.
8. `WORKSPACE_COMPLEXITY_BASELINE.md` — reproducible Stage B dependency, packaging, public-surface and CI measurements.
9. `STAGE_B_DEPENDENCY_NO_GROWTH.md` — machine-enforced root dependency debt closure and continuation boundary.
10. `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_FREEZE.md` — exact discovery lineage, immutable snapshot and later runtime acceptance boundary.
11. `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_IMPLEMENTATION.md` — production discovery, persistence and non-effects evidence.
12. `CRM_CAPABILITY_COVERAGE.md` — product-completeness guardrail.
13. `MODULE_CATALOG.md` — business ownership and readiness accounting.
14. accepted owner packet documents — historical owner-scope acceptance boundaries.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

Latest accepted Customer Privacy runtime baseline: PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows. PR #205 / merge `f0f46238cf103f6e36487f599181e83849342021` remains the accepted package baseline.

Merged Customer Privacy runtime inventory remains:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

Nine owner-scope coordinates are published as non-public owner-owned reads. All nine authoritative owner implementations are accepted:

1. Parties — PR #156 / merge `4368b8c3710e05137b71ba999bf7f3497c0801c8`;
2. Consents — PR #175 / merge `039d6461803208f6cb70ce0fbcfcaffaf59d7125`;
3. Customer Accounts — PR #179 / merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`;
4. Contact Points — PR #181 / merge `96cd0cf548310592a0718c97242a724a29717a72`;
5. Party Relationships — PR #183 / merge `9ad2aa91321e9edb54cab98218f93143923ef33f`;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Post-merge owner documentation synchronization was accepted through PR #193 / merge `e09d3152c886386c2168f0b49e46d47cc44ed041`.

PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` composes the accepted owner executors into trusted-internal discovery. It adds no Customer Privacy worker, public ingress, planning or owner action execution.

## Active product dependency lane

```text
Deterministic Customer Privacy planning and permission-aware plan/outcome reads
-> deterministic planning and permission-aware plan/outcome reads
-> approval, immediate deny-only restrictions and legal-hold/retention precedence
-> replay-safe resumable owner execution and crash-window recovery
-> governed access/export and deletion/anonymization
-> Party tombstone, no-orphan proof and projection/search/cache convergence
-> full lifecycle and worker-process acceptance
-> Phase 8A closure
-> Phase 8B Product Catalog, Pricing, CPQ and Quote-to-Revenue
```

## Accepted product packet — Production scope discovery and immutable snapshot

State: **Accepted through PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows**.

The implementation preserves the PR #204 freeze and records later evidence in `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_IMPLEMENTATION.md` and `contracts/customer-privacy-discovery-snapshot-implementation.json`.

It provides:

- deterministic activation-gated invocation of all nine exact owner contributions;
- immutable tenant/case/Party/topology/registry/purpose/effective-time lineage;
- bounded owner pagination, durable page receipts and contiguous checkpoints;
- safe reference-only aggregation with duplicate conflict detection;
- deterministic immutable snapshot identity and strict rehydration;
- idempotent replay and all three frozen crash-window recovery paths;
- permission-aware internal snapshot reads and safe audit evidence;
- FORCE-RLS PostgreSQL persistence, cross-tenant denial, rollback/schema removal, reapply and repeated acceptance.

Public routes remain unchanged, Customer Privacy workers remain zero, mutations remain four, queries remain two and workspace packages remain 113.

Planning, retention decisions, restrictions, legal holds and owner action execution remain prohibited and unimplemented.

## Next bounded product packet — Deterministic planning and permission-aware plan/outcome reads

State: **Ready; implementation not started**.

The next packet may consume the immutable discovery snapshot but must not silently rediscover, rebase lineage, weaken authorization or begin restrictions, holds, retention decisions or owner execution.

## Cross-cutting architecture and developer-experience lane

Issue #194 is **Open**. Stage A, Stage B no-growth and the Stage C Customer Privacy golden-package pilot are complete; broader issue #194 stages and later natural-boundary calibration remain open.

Accepted Stage B foundation and inheritance packets:

1. PR #197 — reproducible workspace/dependency/public-surface/CI baseline, machine-readable new-crate justification and expiring exception governance; merge `dbd7f6646f255b5f654060a045e26f99fc12c1f9`.
2. PR #199 — all 13 business modules inherit root `serde`, `serde_json` and `sha2`; accepted source `2335ea00bb73d875c291b4a7668921beaec87adc`; merge `cbcce5f18f3b08851ad781d13bc3fe01c2eeb62c`; 26 of 26 applicable workflows.
3. PR #200 — all nine owner privacy-scope adapters inherit root `prost` and `sha2`, with Customer Enrichment also inheriting `serde` and `serde_json`; accepted source `31b3ab09caa4eccaba76a34c7d2211622830115f`; merge `aec7130bd48302d20bf821a617c339b2a9d755cf`; 15 of 15 applicable workflows.
4. PR #203 — every remaining direct/non-inheriting root-family consumer is frozen; accepted source `37cec8e2e68c42e85468cea83b31dcf3ba4138d4`; merge `6a445cd4cb9f423561f834fd7f291635f82eb464`; 4 of 4 applicable workflows.
5. PR #205 — Customer Privacy golden packages accepted without behavior change; accepted source `18c3e991454241f7ee3b02884345eac462bb6c04`; merge `f0f46238cf103f6e36487f599181e83849342021`; 29 of 29 applicable workflows.
6. PR #206 — production discovery and immutable snapshot accepted; source `086b17a95058eee285fcb67a903bd21d9263d357`; merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`; 31 of 31 permanent workflows.

Current measured architecture boundary:

- 113 effective workspace packages: 99 technical crates, 13 business modules and one deployable service;
- root `[workspace.dependencies]`: `prost`, `serde`, `serde_json`, `sha2`;
- accepted direct/non-inheriting debt inventory remains monotonic no-growth;
- zero registered temporary architecture exceptions;
- PR #206 added no package, no external dependency family/version/feature/source drift and no unjustified lockfile growth.

The closure prevents new dependency debt without forcing a mass manifest migration. Rust/toolchain policy, workspace lints, broader public-surface/fan-out calibration and later issue #194 stages remain natural-boundary work.

## Correct continuation order

1. preserve the accepted freeze and Stage C golden packages;
2. preserve accepted PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`;
3. continue deterministic planning and permission-aware plan/outcome reads;
4. continue restrictions, holds/retention, execution and convergence only through later bounded packets;
5. apply residual architecture calibration only at natural packet boundaries unless it blocks correctness.

## Guardrail for the active Customer Privacy packet

Do not add one new crate for each discovery command, query, worker or composition fragment. Generic router/worker algorithms must not grow Customer Privacy branches, and frozen discovery evidence must not be weakened to simplify implementation.

## Remaining product work

Phase 8A closure does not make the universal CRM complete. Major planned domains still include Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing, broader Sales and Activities, omnichannel, Marketing, Service and Knowledge, Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

Current product-complete expert modules: **0**.

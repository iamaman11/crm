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
9. `CRM_CAPABILITY_COVERAGE.md` — product-completeness guardrail.
10. `MODULE_CATALOG.md` — business ownership and readiness accounting.
11. accepted owner packet documents — historical owner-scope acceptance boundaries.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

Latest behavior-affecting architecture baseline: `aec7130bd48302d20bf821a617c339b2a9d755cf`. Later documentation-only merges do not change the runtime, packaging or dependency baseline.

Merged Customer Privacy runtime inventory:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

Nine owner-scope coordinates are published and remain contract-only/non-runtime. All nine authoritative owner implementations are accepted:

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

All nine contributions remain contract-only/non-runtime. They add no Customer Privacy worker, public ingress, production discovery, planning or owner action execution.

## Active product dependency lane

```text
Customer Privacy scope discovery and immutable snapshot
-> deterministic planning and permission-aware plan/outcome reads
-> approval, immediate deny-only restrictions and legal-hold/retention precedence
-> replay-safe resumable owner execution and crash-window recovery
-> governed access/export and deletion/anonymization
-> Party tombstone, no-orphan proof and projection/search/cache convergence
-> full lifecycle and worker-process acceptance
-> Phase 8A closure
-> Phase 8B Product Catalog, Pricing, CPQ and Quote-to-Revenue
```

## Next bounded product packet — Scope discovery and immutable snapshot

State: **Ready for contract/acceptance freeze; production implementation not started**.

The packet must prove:

- deterministic invocation and completeness of all nine accepted owner contributions;
- fail-closed behavior for unavailable, stale, disabled or incompatible owners;
- one immutable scope snapshot bound to tenant, privacy case, canonical Party, topology generation, registry digest, purpose and effective request time;
- deterministic owner/resource/data-class ordering without payload disclosure;
- replay-stable page, cursor and snapshot digests;
- no owner mutation, restriction, legal-hold, retention or destructive action during discovery;
- permission-aware snapshot reads and exact audit evidence;
- clean PostgreSQL, rollback/reapply, crash/retry and real-process acceptance.

Planning and action execution remain prohibited until discovery and immutable snapshot are accepted.

## Cross-cutting architecture and developer-experience lane

Issue #194 is **Open**. Stage A is complete. Stage B is **in progress with its foundational governance accepted**.

Accepted Stage B packets:

1. PR #197 — reproducible workspace/dependency/public-surface/CI baseline, machine-readable new-crate justification and expiring exception governance; merge `dbd7f6646f255b5f654060a045e26f99fc12c1f9`.
2. PR #199 — all 13 business modules inherit root `serde`, `serde_json` and `sha2`; accepted source `2335ea00bb73d875c291b4a7668921beaec87adc`; merge `cbcce5f18f3b08851ad781d13bc3fe01c2eeb62c`; 26 of 26 applicable workflows.
3. PR #200 — all nine owner privacy-scope adapters inherit root `prost` and `sha2`, with Customer Enrichment also inheriting `serde` and `serde_json`; accepted source `31b3ab09caa4eccaba76a34c7d2211622830115f`; merge `aec7130bd48302d20bf821a617c339b2a9d755cf`; 15 of 15 applicable workflows.

Current measured architecture baseline:

- 110 effective workspace packages: 96 technical crates, 13 business modules and one deployable service;
- root `[workspace.dependencies]`: `prost`, `serde`, `serde_json`, `sha2`;
- two calibrated blocking inheritance policies;
- owner-module policy: 13 manifests, 39 governed declarations, zero violations;
- privacy-scope adapter policy: nine manifests, 20 governed declarations, zero violations;
- remaining non-inheriting consumers: `prost` 53, `serde` 15, `serde_json` 23, `sha2` 16;
- 268 external direct dependency declarations and 773 internal workspace edges;
- maximum dependency depth 15 and maximum transitive reverse impact 103;
- conservative public Rust surface 4,283 items;
- zero registered temporary architecture exceptions;
- no lockfile change from the three dependency-inheritance waves.

Stage B is not complete because remaining direct consumers can still grow outside the two calibrated cohorts, public-surface/fan-out deltas are still measurement-only, and supported Rust/toolchain and workspace lint policy are not yet frozen.

## Correct continuation order

1. **Synchronize current documentation and issue evidence.** Do not begin another behavior packet from stale counts or stale Stage B claims.
2. **Complete the minimal Stage B no-growth closure.** Freeze the remaining non-inheriting consumer inventory for root dependency families, block new direct declarations outside an explicit owned exception, and keep existing consumers scheduled for bounded role-based migration rather than a big-bang rewrite.
3. **Freeze Customer Privacy discovery/snapshot contracts and acceptance semantics.** Define exact identity, registry/topology binding, ordering, digests, replay, authorization, failure and PostgreSQL/process proof before runtime code.
4. **Run the Stage C golden-package pilot on Customer Privacy as a separate behavior-neutral PR.** The target `crm-customer-privacy-application`, `crm-customer-privacy-postgres` and `crm-customer-privacy-production` packages do not yet exist; current behavior is split across capability-specific persistence, mutation, subject, cancel, query and composition crates. Consolidate only after the frozen feature boundary identifies real dependency seams.
5. **Implement scope discovery and immutable snapshot inside the accepted target packages.** Add no command/query/worker/composition-fragment crate and do not grow generic runtime business switches.
6. **After discovery acceptance, continue deterministic planning and the remaining privacy lifecycle sequence.** Apply later Stage B calibration, exact Rust/toolchain policy, workspace lints and further dependency cohorts only at natural packet boundaries unless they block the active product packet.

This order preserves the issue #194 stage dependency, prevents new architecture debt before Stage C, and avoids delaying Phase 8A with an unbounded repository-wide cleanup.

## Guardrail for the active Customer Privacy packet

Do not add one new crate for each discovery command, query, worker or composition fragment.

Required sequence:

1. freeze the discovery/snapshot contract and acceptance boundary;
2. identify the real Customer Privacy application/PostgreSQL/production ownership and dependency seams;
3. perform necessary consolidation as a separate behavior-neutral Stage C pilot;
4. implement discovery/snapshot in the accepted target packages;
5. preserve focused, PostgreSQL, rollback/reapply and real-process acceptance.

## Remaining product work

Phase 8A closure does not make the universal CRM complete. Major planned domains still include Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing, broader Sales and Activities, omnichannel, Marketing, Service and Knowledge, Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

Current product-complete expert modules: **0**.

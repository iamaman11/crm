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
11. `CRM_CAPABILITY_COVERAGE.md` — product-completeness guardrail.
12. `MODULE_CATALOG.md` — business ownership and readiness accounting.
13. accepted owner packet documents — historical owner-scope acceptance boundaries.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

Latest production-runtime-affecting architecture baseline: `aec7130bd48302d20bf821a617c339b2a9d755cf`. The Stage B no-growth closure and the discovery/snapshot freeze are non-runtime architecture work and do not add production routes, workers or dependency resolution changes.

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

## Frozen product packet — Scope discovery and immutable snapshot

State: **Contract and acceptance semantics frozen; production implementation not started**.

The frozen boundary defines:

- deterministic invocation and terminal completeness of all nine exact owner contributions;
- exact registry version/digest and owner-coordinate compatibility binding;
- fail-closed unavailable, disabled, stale or incompatible owners;
- one immutable lineage bound to tenant, privacy case, canonical Party, topology generation, registry version/digest, purpose and effective request time;
- purpose/effective-time-bound authoritative snapshot identity around the existing deterministic aggregation;
- bounded pagination, page/cursor receipts, deterministic owner/resource/data-class ordering and no payload disclosure;
- replay, retry, registry/topology drift and crash-window semantics;
- permission-aware snapshot reads and exact audit evidence;
- clean PostgreSQL, FORCE RLS, rollback/schema removal, reapply, repeated and real-process acceptance required before runtime discovery can be accepted.

The pure Customer Privacy domain now exposes strict non-runtime lineage, bound-owner contribution and immutable discovery-snapshot contracts with canonical rehydration. It does not register the discovery coordinate in production.

Planning, retention decisions, restrictions, legal holds and owner action execution remain prohibited until discovery and immutable snapshot runtime acceptance is complete.

## Next bounded product packet — Stage C Customer Privacy golden-package pilot

State: **Ready after acceptance of the freeze packet; behavior-neutral only**.

The pilot must establish the accepted Customer Privacy application/PostgreSQL/production ownership boundary without adding discovery behavior:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

It must preserve every existing coordinate, route, activation, authorization, RLS, audit, idempotency and process behavior; add no generic-runtime business switch; and report package, dependency, fan-out, public-surface and build/test effects. Runtime discovery starts only after this separate behavior-neutral pilot is accepted.

## Cross-cutting architecture and developer-experience lane

Issue #194 is **Open**. Stage A is complete. The bounded **Stage B no-growth closure is complete**; broader issue #194 stages and later natural-boundary calibration remain open.

Accepted Stage B foundation and inheritance packets:

1. PR #197 — reproducible workspace/dependency/public-surface/CI baseline, machine-readable new-crate justification and expiring exception governance; merge `dbd7f6646f255b5f654060a045e26f99fc12c1f9`.
2. PR #199 — all 13 business modules inherit root `serde`, `serde_json` and `sha2`; accepted source `2335ea00bb73d875c291b4a7668921beaec87adc`; merge `cbcce5f18f3b08851ad781d13bc3fe01c2eeb62c`; 26 of 26 applicable workflows.
3. PR #200 — all nine owner privacy-scope adapters inherit root `prost` and `sha2`, with Customer Enrichment also inheriting `serde` and `serde_json`; accepted source `31b3ab09caa4eccaba76a34c7d2211622830115f`; merge `aec7130bd48302d20bf821a617c339b2a9d755cf`; 15 of 15 applicable workflows.
4. PR #203 — every remaining direct/non-inheriting root-family consumer is frozen; accepted source `37cec8e2e68c42e85468cea83b31dcf3ba4138d4`; merge `6a445cd4cb9f423561f834fd7f291635f82eb464`; 4 of 4 applicable workflows.

Current measured architecture baseline and closure boundary:

- 110 effective workspace packages: 96 technical crates, 13 business modules and one deployable service;
- root `[workspace.dependencies]`: `prost`, `serde`, `serde_json`, `sha2`;
- two calibrated blocking inheritance policies plus one repository-wide root-family no-growth policy;
- accepted direct/non-inheriting debt inventory: `prost` 53, `serde` 15, `serde_json` 23, `sha2` 16, total 107 family-manifest entries;
- current debt must remain an exact subset, so reduction needs no baseline edit and growth fails closed;
- 268 external direct dependency declarations and 773 internal workspace edges;
- maximum dependency depth 15 and maximum transitive reverse impact 103;
- conservative public Rust surface 4,283 items;
- zero registered temporary architecture exceptions;
- package count and `Cargo.lock` unchanged by the no-growth closure.

The closure prevents new dependency debt before Stage C without forcing a mass manifest migration. Rust/toolchain policy, workspace lints, broader public-surface/fan-out calibration and later dependency cohorts remain issue #194 work at natural packet boundaries.

## Correct continuation order

1. accept the discovery/snapshot contract and acceptance freeze on one unchanged exact head;
2. run the Stage C behavior-neutral Customer Privacy golden-package pilot;
3. implement discovery/snapshot inside the accepted application/PostgreSQL/production packages;
4. after discovery acceptance, continue deterministic planning and the remaining privacy lifecycle sequence;
5. apply residual architecture calibration only at natural packet boundaries unless it blocks correctness.

## Guardrail for the active Customer Privacy packet

Do not add one new crate for each discovery command, query, worker or composition fragment. Feature behavior and packaging consolidation remain separate PRs, and generic router/worker algorithms must not grow Customer Privacy branches.

## Remaining product work

Phase 8A closure does not make the universal CRM complete. Major planned domains still include Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing, broader Sales and Activities, omnichannel, Marketing, Service and Knowledge, Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

Current product-complete expert modules: **0**.

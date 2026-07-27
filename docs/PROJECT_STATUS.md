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
8. `CRM_CAPABILITY_COVERAGE.md` — product-completeness guardrail.
9. `MODULE_CATALOG.md` — business ownership and readiness accounting.
10. accepted owner packet documents — historical owner-scope acceptance boundaries.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

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

Post-merge governance synchronization was accepted through PR #193 / merge `e09d3152c886386c2168f0b49e46d47cc44ed041`.

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

## Next bounded packet — Scope discovery and immutable snapshot

State: **Ready for bounded implementation; not started**.

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

Issue #194 is **Open**.

The foundational modular architecture remains accepted. The active improvement program addresses accidental complexity without weakening ownership or runtime safety.

Current major gaps:

- 109 root Cargo workspace members;
- capability-specific crate proliferation;
- many concrete domain dependencies in `crm-application-runtime`;
- partial first-party contribution aggregation;
- no root workspace dependency inheritance baseline;
- CI/workflow fan-out growing with owner count;
- `repo.py explain`, `repo.py packet-check`, generated active packet and repository map not yet complete;
- README/status drift was possible before this governance packet;
- frontend and operational evidence remain below backend architecture maturity.

Required direction:

- normal capability: zero new crates;
- normal owner: three to five technical packages;
- module-owned `build_contribution` for every owner;
- stable generic runtime and worker algorithms;
- centralized dependency versions/features;
- affected-scope iterative CI plus unchanged exact-head final proof;
- generic conformance plus owner-specific semantic tests;
- measured behavior-neutral consolidation;
- mechanical repository navigation and documentation consistency;
- frontend/browser/accessibility and restore/SLO/security parity.

The full sequence, metrics and completion criteria are in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`.

## Guardrail for the active Customer Privacy packet

Do not add one new crate for each discovery command, query, worker or composition fragment.

Required sequence:

1. freeze the discovery/snapshot contract;
2. identify the target Customer Privacy application/PostgreSQL/production ownership;
3. perform any necessary consolidation as a separate behavior-neutral packet;
4. implement discovery/snapshot in the target packages;
5. preserve focused, PostgreSQL, rollback/reapply and real-process acceptance.

## Remaining product work

Phase 8A closure does not make the universal CRM complete. Major planned domains still include Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing, broader Sales and Activities, omnichannel, Marketing, Service and Knowledge, Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

Current product-complete expert modules: **0**.

# Architecture Readiness Baseline

Status: **Ready for modular product development**
Accepted implementation: issue #134 / PR #135
Merged baseline: `023fa5ef1d510d5bcc32222c739e6d58e5696fb8`
Exact verification head: `c73a3eb830893477a4d535acc9383e006d74367d`

This document records the accepted non-regression baseline for continued CRM development. It does not claim that the universal CRM product is complete; it confirms that the production architecture is prepared for further owner-domain and link-module delivery without returning to central business routing or hidden lifecycle bypasses.

## Proven guarantees

The accepted baseline provides:

- deterministic module-owned mutation and query contributions;
- generic exact-coordinate routing with startup rejection for duplicates, owner mismatches, route-kind mismatches and incomplete handlers;
- tenant route and worker activation backed by durable `crm.module_installations` state;
- pre-authorization cross-owner semantic validation before final live authorization;
- deterministic, phase-ordered, activation-gated background worker contributions;
- declarative bootstrap visibility contributions;
- exact manifest/binding/production-route parity with individually reasoned classifications;
- immutable module publication compatibility enforcement;
- golden scaffolding with a mandatory `production/CONTRIBUTION.md` boundary;
- structural readiness checks that reject legacy central routers, fixed worker wiring and lifecycle bypasses.

All 15 applicable workflows passed together on the unchanged human-authored verification head before PR #135 merged.

## Non-regression contract

Future changes MUST preserve all of the following:

1. Business behavior enters production through explicit module-owned contributions, never a central capability-ID or concrete-adapter switch.
2. Public mutation/query dispatch uses exact owner, identifier, version and route-kind coordinates.
3. Durable tenant installation state is the runtime authority for module activation; bootstrap may provision that state but MUST NOT bypass it.
4. Cross-owner reads needed to establish request semantics occur before final authorization; authoritative executors do not perform unrelated awaited validation after authorization and before their side effect.
5. Background workers are contributed by modules, assigned deterministic phases and activation-gated per tenant.
6. Every governed contract coordinate has exactly one production route or one exact documented non-runtime classification; owner-wide and pattern allowlists are forbidden.
7. New modules use the golden scaffold and do not require edits to generic router or worker algorithms.
8. Any source or documentation change invalidates prior exact-SHA evidence until all applicable checks pass again.

## Required development entry point

Every new delivery packet must begin from current `main`, read `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `DEVELOPMENT_WORKFLOW.md`, `MODULE_DEVELOPMENT.md` and its active issue, then identify:

- authoritative ownership and stable references;
- exact public contract coordinates;
- module-owned routes, validators and workers;
- durable installation/disable/uninstall behavior;
- persistence, authorization, audit, idempotency and rollback semantics;
- route-parity/classification impact;
- required unit, PostgreSQL, process, browser and operational acceptance.

Use `python scripts/repo.py conformance` for the permanent local architecture preflight and run all specialized GitHub workflows affected by the packet. Architecture readiness remains valid only while these mechanical gates stay green.

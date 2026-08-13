# Repository Step 22I — Consents query-adapter fan-in reduction

## Status

Active bounded ADR-032 remediation. This packet removes one redundant direct production dependency from `crm-application-runtime`; it does not complete Repository Step 22 and does not claim architecture 10/10.

## Baseline

- Exact baseline: `fab78b95afb66344af2dccbff87121cd24ba3cd5` (merged Step 22H Identity Resolution fan-in reduction / PR #309).
- Accepted Step 22A inventory remains immutable at **63 total / 62 production / 1 test-only** runtime direct internal dependencies.
- Step 22H state before this packet: **53 total / 52 production / 1 test-only**, **27 final / 10 removed / 36 unresolved**.

## Bounded decision

`crm-application-runtime` still declared a direct production dependency on `crm-consents-query-adapter`, even though generic production composition already enters Consents through `crm-first-party-modules` and the existing owner boundary `crm-consents-capability-composition`.

`crm-consents-capability-composition` retains both `crm-consents-capability-adapter` and `crm-consents-query-adapter`, publishes the exact Consents mutation/query definition inventories, and constructs the complete Consents mutation/query production contribution. `crm-first-party-modules` retains that composition boundary, merges its contribution, and re-exports those inventories for generic application composition.

The direct `crm-consents-capability-adapter` runtime dependency is intentionally **not** removed in this packet. Bootstrap visibility still consumes its `MODULE_ID` and `RECORD_TYPE` constants, so that edge remains a real current production dependency rather than a stale manifest edge.

Step 22I therefore:

1. removes only the redundant direct `crm-consents-query-adapter` edge from `crm-application-runtime`;
2. preserves `crm-consents-capability-adapter` as a direct runtime dependency;
3. keeps the query adapter owned and consumed by `crm-consents-capability-composition`;
4. keeps Consents production contribution and mutation/query inventories entering generic application composition through `crm-first-party-modules`;
5. changes no runtime Rust source and adds no public accessor, constant, type, function, trait or re-export.

## Exact dependency evidence

Before Step 22I:

- runtime direct internal dependencies: **53**;
- production: **52**;
- test-only: **1**.
- exact prior runtime state: **53 total / 52 production / 1 test-only**.

After Step 22I:

- runtime direct internal dependencies: **52**;
- production: **51**;
- test-only: **1**.
- exact runtime state: **52 total / 51 production / 1 test-only**.

The cumulative removed stable-ID set becomes exactly eleven entries, adding:

- `crm-application-runtime::dependencies::crm-consents-query-adapter`.

The decision ledger becomes **28 final / 16 platform-generic / 1 test-only / 11 removed / 0 owner-specific-unavoidable / 35 unresolved**.

## Owner-boundary evidence

`crm-consents-capability-composition` retains direct dependencies on `crm-consents-capability-adapter` and `crm-consents-query-adapter`. Its `mutation_capability_definitions()` and `query_capability_definitions()` functions publish the owner inventories, and `build_contribution()` constructs the mutation validators/executor plus `ConsentQueryAdapter` query execution behind the owner composition boundary.

`crm-first-party-modules` retains `crm-consents-capability-composition`, merges `build_consents_contribution`, and re-exports the existing Consents mutation/query definition inventories. `crm-application-runtime/src/native_composition.rs` consumes those first-party aggregation APIs rather than `crm-consents-query-adapter` directly.

Bootstrap visibility continues to consume `MODULE_ID` and `RECORD_TYPE` from `crm-consents-capability-adapter`, so that capability-adapter edge is deliberately left unresolved/direct for a later evidence-backed decision.

No new public item is introduced by this packet.

## Lockfile invariant

Relative to baseline `fab78b95afb66344af2dccbff87121cd24ba3cd5`, the `crm-application-runtime` package dependency list in `Cargo.lock` drops exactly `crm-consents-query-adapter`. The `crm-consents-query-adapter` package record remains, and the `crm-consents-capability-composition` package continues to retain `crm-consents-query-adapter` internally.

## Public surface invariant

The conservative public Rust surface remains exactly **5,377**. This packet changes no Rust public API.

## Permanent guard

`scripts/check_step22_runtime_fanin_decisions.py` must fail if:

- the direct runtime manifest or lock edge to `crm-consents-query-adapter` returns;
- any Rust file under `crates/crm-application-runtime` directly references `crm_consents_query_adapter`;
- `crm-consents-capability-composition` stops retaining the query adapter internally or stops publishing/constructing the accepted Consents mutation/query contribution;
- `crm-first-party-modules` stops aggregating Consents through the owner composition boundary;
- `crm-consents-capability-adapter` disappears while bootstrap visibility still consumes its module/record constants;
- cumulative fan-in or decision-ledger counts differ from the exact Step 22I state.

`tests/test_workspace_analysis.py` independently reproduces the current runtime fan-in from the workspace and must agree with the same **52 / 51 / 1** state and eleven cumulative removals while the accepted Step 22A inventory stays immutable.

## Explicit non-goals

This packet does not:

- remove or replace the direct Consents capability-adapter edge used by bootstrap visibility;
- change Consents mutation, query, authorization, visibility, persistence, reference-validation or business semantics;
- modify Consents owner composition, query adapter, capability adapter or first-party aggregation implementation;
- add a new public Consents accessor solely to reduce generic runtime fan-in;
- remediate or classify Contact Points, Customer Data Operations, Customer Enrichment, Identity Resolution, Party Relationships or another unresolved dependency cohort;
- change permanent workflow/job dispositions;
- rewrite the accepted Step 22A inventory;
- complete all runtime classifications;
- complete Repository Step 22, begin Phase 8B, raise an architecture score or claim architecture 10/10.

## Acceptance

Acceptance requires one unchanged human-authored head to pass every applicable permanent pull-request workflow, including at minimum:

- Affected Scope CI;
- Application Runtime CI;
- Complexity Baseline CI;
- Consents Privacy Scope CI;
- Governance CI;
- Rust Generated Sync;
- Rust CI.

The accepted head must have zero unresolved PR comments, reviews or review threads. After merge, the merge tree must be bit-for-bit identical to the exact-green candidate tree and every push-triggered workflow on the merge SHA must succeed before another Step 22 dependency cohort begins.

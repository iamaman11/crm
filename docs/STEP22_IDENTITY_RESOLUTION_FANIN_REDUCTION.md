# Repository Step 22H — Identity Resolution fan-in reduction

## Status

Active bounded ADR-032 remediation. This packet removes four redundant direct production dependencies from `crm-application-runtime`; it does not complete Repository Step 22 and does not claim architecture 10/10.

## Baseline

- Exact baseline: `48e3f2f3e0049505b92a08ac56320ce18a1a41d1` (merged Step 22G Data Quality fan-in reduction / PR #307).
- Accepted Step 22A inventory remains immutable at **63 total / 62 production / 1 test-only** runtime direct internal dependencies.
- Step 22G state before this packet: **57 total / 56 production / 1 test-only**, **23 final / 6 removed / 40 unresolved**.

## Bounded decision

`crm-application-runtime` still declared four redundant Identity Resolution production dependencies even though generic production composition already enters Identity Resolution through `crm-first-party-modules` and the existing owner boundary `crm-identity-resolution-capability-composition`:

- `crm-identity-resolution-capability-composition`;
- `crm-identity-resolution-merge-composition`;
- `crm-identity-resolution-query-adapter`;
- `crm-identity-resolution-merge-query-adapter`.

`crm-identity-resolution-capability-composition` already owns the candidate and merge mutation/query production assembly. It directly retains the merge composition and both query adapters, publishes the combined mutation/query definition inventories, and is itself retained by `crm-first-party-modules`, which merges the owner contribution for the generic process host.

The direct `crm-identity-resolution-capability-adapter` runtime dependency is intentionally **not** removed in this packet. Bootstrap visibility still consumes its `MODULE_ID`, `RECORD_TYPE`, and `MERGE_OPERATION_RECORD_TYPE` constants, so that edge remains a real current production dependency rather than a stale manifest edge.

Step 22H therefore:

1. removes only the four redundant composition/query edges listed above from `crm-application-runtime`;
2. preserves `crm-identity-resolution-capability-adapter` as a direct runtime dependency;
3. leaves Identity Resolution owner, merge, query, authorization, visibility, persistence and business semantics unchanged;
4. adds no public accessor, constant, type, function, trait or re-export;
5. changes no runtime Rust source because existing first-party/owner boundaries already provide the production seam.

## Exact dependency evidence

Before Step 22H:

- runtime direct internal dependencies: **57**;
- production: **56**;
- test-only: **1**.

After Step 22H:

- runtime direct internal dependencies: **53**;
- production: **52**;
- test-only: **1**.

The cumulative removed stable-ID set becomes exactly ten entries, adding:

- `crm-application-runtime::dependencies::crm-identity-resolution-capability-composition`;
- `crm-application-runtime::dependencies::crm-identity-resolution-merge-composition`;
- `crm-application-runtime::dependencies::crm-identity-resolution-merge-query-adapter`;
- `crm-application-runtime::dependencies::crm-identity-resolution-query-adapter`.

The decision ledger becomes **27 final / 16 platform-generic / 1 test-only / 10 removed / 0 owner-specific-unavoidable / 36 unresolved**.

## Owner-boundary evidence

`crm-identity-resolution-capability-composition` retains direct dependencies on the capability adapter, merge composition, candidate query adapter and merge query adapter. Its production contribution builds candidate and merge mutation execution plus candidate and merge permission-aware query execution.

`crm-first-party-modules` retains `crm-identity-resolution-capability-composition`, merges its production contribution, and re-exports the existing Identity Resolution mutation/query definition inventories. `crm-application-runtime/src/native_composition.rs` consumes those first-party aggregation APIs rather than the four removed packages directly.

No new public item is introduced by this packet.

## Lockfile invariant

Relative to baseline `48e3f2f3e0049505b92a08ac56320ce18a1a41d1`, the `crm-application-runtime` package dependency list in `Cargo.lock` drops exactly the four direct Identity Resolution entries removed by this packet. All four package records remain, and the existing owner/aggregation package records retain their internal dependencies.

## Public surface invariant

The conservative public Rust surface remains exactly **5,377**. This packet changes no Rust public API.

## Permanent guard

`scripts/check_step22_runtime_fanin_decisions.py` must fail if:

- any of the four direct runtime manifest or lock edges returns;
- any Rust file under `crates/crm-application-runtime` directly references one of the four removed Identity Resolution packages;
- `crm-identity-resolution-capability-composition` stops retaining its merge composition or query adapters internally;
- `crm-first-party-modules` stops aggregating Identity Resolution through the owner composition boundary;
- the direct `crm-identity-resolution-capability-adapter` edge disappears while bootstrap visibility still consumes its constants;
- cumulative fan-in or decision-ledger counts differ from the exact Step 22H state.

`tests/test_workspace_analysis.py` independently reproduces the current runtime fan-in from the workspace and must agree with the same **53 / 52 / 1** state and ten cumulative removals while the accepted Step 22A inventory stays immutable.

## Explicit non-goals

This packet does not:

- remove the direct Identity Resolution capability-adapter edge used by bootstrap visibility;
- modify Identity Resolution owner/composition/query implementation;
- change Identity Resolution production semantics;
- add a new owner accessor or increase public surface;
- remediate another unresolved dependency cohort;
- change permanent workflow/job dispositions;
- rewrite the accepted Step 22A inventory;
- complete all runtime classifications;
- complete Repository Step 22, begin Phase 8B, raise an architecture score or claim architecture 10/10.

## Acceptance

Acceptance requires one unchanged human-authored head to pass every applicable permanent pull-request workflow, including at minimum:

- Affected Scope CI;
- Application Runtime CI;
- Complexity Baseline CI;
- Governance CI;
- Identity Resolution Privacy Scope CI;
- Rust Generated Sync;
- Rust CI.

The accepted head must have zero unresolved PR comments, reviews or review threads. After merge, the merge tree must be bit-for-bit identical to the exact-green candidate tree and every push-triggered workflow on the merge SHA must succeed before another Step 22 dependency cohort begins.

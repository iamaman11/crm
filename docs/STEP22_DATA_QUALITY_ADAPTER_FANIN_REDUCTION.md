# Repository Step 22G — Data Quality adapter fan-in reduction

## Status

Active bounded ADR-032 remediation. This packet removes two redundant direct production dependencies from `crm-application-runtime`; it does not complete Repository Step 22 and does not claim architecture 10/10.

## Baseline

- Exact baseline: `7c714fe4edac2c382a38957506acd149416d4464` (merged Step 22F post-merge lifecycle synchronization / PR #306).
- Accepted Step 22A inventory remains immutable at **63 total / 62 production / 1 test-only** runtime direct internal dependencies.
- Step 22F current state before this packet: **59 total / 58 production / 1 test-only**, **21 final / 4 removed / 42 unresolved**.

## Bounded decision

`crm-application-runtime` still declared direct production dependencies on `crm-data-quality-capability-adapter` and `crm-data-quality-query-adapter`, but production composition no longer consumes either adapter directly. Data Quality production mutation/query assembly is already owned by `crm-data-quality-source-composition`, which retains both adapters internally, and `crm-first-party-modules` already aggregates that owner contribution and re-exports the owner definition inventories consumed by generic application composition.

The only direct Data Quality adapter source use found inside `crm-application-runtime` is the integration registration test importing capability-adapter constants. That test is moved to compare the same two publication coordinates against the existing owner composition mutation definition inventory; no new public owner API is added.

Step 22G therefore:

1. removes only the direct `crm-data-quality-capability-adapter` and `crm-data-quality-query-adapter` edges from `crm-application-runtime`;
2. keeps both adapter packages and all owner-internal uses unchanged;
3. keeps Data Quality production mutation/query construction behind `crm-data-quality-source-composition` and `crm-first-party-modules`;
4. routes the registration test through the existing owner composition definition inventory rather than adapter constants while preserving its exact two-publication-coordinate contract;
5. changes no Data Quality mutation, query, authorization, visibility, persistence, worker, contract, route or schema behavior.

## Exact dependency evidence

Before Step 22G:

- runtime direct internal dependencies: **59**;
- production: **58**;
- test-only: **1**.

After Step 22G, current runtime direct fan-in is exactly **57 total / 56 production / 1 test-only**:

- runtime direct internal dependencies: **57**;
- production: **56**;
- test-only: **1**.

The cumulative removed stable-ID set becomes exactly six entries, adding:

- `crm-application-runtime::dependencies::crm-data-quality-capability-adapter`;
- `crm-application-runtime::dependencies::crm-data-quality-query-adapter`.

The decision ledger becomes **23 final / 16 platform-generic / 1 test-only / 6 removed / 0 owner-specific-unavoidable / 40 unresolved**.

## Owner-boundary evidence

`crm-data-quality-source-composition` retains direct dependencies on both Data Quality adapters and uses them to publish the Data Quality mutation/query inventories and construct the production contribution. `crm-first-party-modules` depends on that composition boundary, merges its production contribution, and re-exports its mutation/query definition inventories. Generic application composition consumes those existing aggregation APIs.

No new public item is introduced by this packet.

## Lockfile invariant

Relative to baseline `7c714fe4edac2c382a38957506acd149416d4464`, the `crm-application-runtime` package dependency list in `Cargo.lock` drops exactly the two direct Data Quality adapter entries. Both adapter package records remain, and the `crm-data-quality-source-composition` package retains both adapter dependencies.

## Public surface invariant

The conservative public Rust surface remains exactly **5,377**. This packet adds no public function, type, trait, constant or re-export.

## Permanent guard

`scripts/check_step22_runtime_fanin_decisions.py` and the current-state workspace-analysis guard must fail if:

- either direct runtime manifest or lock edge returns;
- any Rust file under `crates/crm-application-runtime` directly references either removed Data Quality adapter;
- the owner composition stops retaining either adapter internally;
- `crm-first-party-modules` stops aggregating Data Quality through the owner composition boundary;
- the registration test regains a direct Data Quality adapter reference or ceases to verify the same two publication coordinates;
- cumulative fan-in or decision-ledger counts differ from the exact Step 22G state.

## Explicit non-goals

This packet does not:

- remove either Data Quality adapter package from the workspace or owner composition;
- change Data Quality production semantics;
- add a new owner accessor or increase public surface;
- remediate another unresolved dependency cohort;
- change permanent workflow/job dispositions;
- rewrite the accepted Step 22A inventory;
- complete all runtime classifications;
- complete Repository Step 22, begin Phase 8B, raise an architecture score or claim architecture 10/10.

## Acceptance

Acceptance requires one unchanged human-authored head to pass every applicable permanent workflow, including at minimum:

- Affected Scope CI;
- Application Runtime CI;
- Complexity Baseline CI;
- Data Quality Privacy Scope CI;
- Data Quality Process Runtime CI;
- Governance CI;
- Rust Generated Sync;
- Rust CI.

The accepted head must also have zero unresolved PR comments, reviews or review threads. After merge, every push-triggered workflow on the merge SHA must succeed before the next Step 22 cohort begins.

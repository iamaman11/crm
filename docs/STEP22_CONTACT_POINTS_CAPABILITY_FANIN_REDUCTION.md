# Repository Step 22F — Contact Points capability-adapter fan-in reduction

## Status

Active bounded ADR-032 remediation. This packet removes one redundant direct production dependency from `crm-application-runtime`; it does not complete Repository Step 22 and does not claim architecture 10/10.

## Baseline

- Exact baseline: `17985f32806b239f6063159113f72d2f561c6c5a` (merged Repository Step 22E / PR #303).
- Accepted Step 22A inventory remains immutable at **63 total / 62 production / 1 test-only** runtime direct internal dependencies.
- Step 22E current state before this packet: **60 total / 59 production / 1 test-only**, **20 final / 3 removed / 43 unresolved**.

## Bounded decision

`crm-application-runtime` used `crm-contact-points-capability-adapter` directly only to obtain stable Contact Points module and record identity for generic bootstrap visibility. The complete Contact Points mutation/query production contribution already belongs to `crm-contact-points-capability-composition`, which already depends on and retains the capability adapter internally.

Step 22F therefore:

1. replaces the unused public `CRATE_NAME` marker in `crm-contact-points-capability-composition` with `contact_points_runtime_identity()` returning the exact owner module and record identity;
2. makes generic bootstrap visibility consume that owner-composition identity;
3. removes only the direct `crm-contact-points-capability-adapter` edge from `crm-application-runtime`;
4. keeps the adapter package and all owner-internal uses unchanged;
5. keeps Contact Points mutation, query, Party-reference validation, persistence, authorization, tenant isolation and Customer 360 visibility behavior unchanged.

## Exact dependency evidence

Before Step 22F:

- runtime direct internal dependencies: **60**;
- production: **59**;
- test-only: **1**.

After Step 22F:

- runtime direct internal dependencies: **59**;
- production: **58**;
- test-only: **1**.

The cumulative removed stable-ID set becomes exactly:

1. `crm-application-runtime::dependencies::crm-customer-privacy-query-adapter`;
2. `crm-application-runtime::dependencies::crm-customer-360-query-adapter`;
3. `crm-application-runtime::dependencies::crm-parties-capability-adapter`;
4. `crm-application-runtime::dependencies::crm-contact-points-capability-adapter`.

The decision ledger becomes **21 final / 16 platform-generic / 1 test-only / 4 removed / 0 owner-specific-unavoidable / 42 unresolved**.

## Lockfile invariant

Relative to baseline `17985f32806b239f6063159113f72d2f561c6c5a`, `Cargo.lock` may differ by exactly one line: deletion of `crm-contact-points-capability-adapter` from the `crm-application-runtime` package dependency list. The adapter package record and the `crm-contact-points-capability-composition` owner-internal dependency remain present.

## Public surface invariant

The conservative public Rust surface remains exactly **5,377**. The packet does not add a net public item: it replaces the unused public `crm-contact-points-capability-composition::CRATE_NAME` marker with the consumed public `contact_points_runtime_identity()` accessor.

## Permanent guard

`scripts/check_step22_runtime_fanin_decisions.py` must fail if:

- the direct runtime manifest or lock edge returns;
- any Rust file under `crates/crm-application-runtime` directly references `crm_contact_points_capability_adapter`;
- the owner composition stops retaining the adapter internally;
- bootstrap visibility stops consuming `contact_points_runtime_identity()`;
- the owner composition loses the identity accessor or restores the retired `CRATE_NAME` marker;
- cumulative fan-in or decision-ledger counts differ from the exact Step 22F state.

## Explicit non-goals

This packet does not:

- remove `crm-contact-points-capability-adapter` from the workspace or owner composition;
- classify `crm-contact-points-capability-composition` as unavoidable generic-host fan-in;
- remediate any other runtime dependency;
- change contracts, schemas, migrations, routes, workers or permanent workflow/job dispositions;
- rewrite the accepted Step 22A inventory;
- complete all runtime classifications;
- complete Repository Step 22, begin Phase 8B, raise an architecture score or claim architecture 10/10.

## Acceptance

Acceptance requires one unchanged human-authored head to pass every applicable permanent workflow, including at minimum:

- Affected Scope CI;
- Application Runtime CI;
- Complexity Baseline CI;
- Governance CI;
- Rust Generated Sync;
- Rust CI.

The accepted head must also have zero unresolved PR comments, reviews or review threads.

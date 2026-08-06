# Repository Step 22D — Customer 360 Query Fan-In Reduction

Status: **Active bounded remediation packet**  
Tracking issue: #194  
Binding decision: `docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Baseline: PR #300 squash merge `9b2495c9a594f5539aa586f6d775a8ea12442a48`

## Purpose

Step 22A measured 63 internal direct dependencies of `crm-application-runtime`. Step 22C removed the Customer Privacy query-adapter edge and reduced the current surface to 62. Step 22D removes one more redundant owner-adapter edge.

`crm-first-party-modules` already owns `crm-customer-360-query-adapter`, builds its production contribution and exposes its query inventory. The generic runtime used the adapter directly only to read `MODULE_ID` while registering bootstrap visibility.

Step 22D exposes that module identity through the existing first-party boundary and removes the direct process-host dependency.

## Exact before and after

| Metric | Step 22C merged state | Step 22D candidate | Delta |
|---|---:|---:|---:|
| Internal direct dependencies | 62 | 61 | -1 |
| Production internal direct dependencies | 61 | 60 | -1 |
| Test-only internal direct dependencies | 1 | 1 | 0 |
| Conservative public Rust surface | 5,377 | 5,377 | 0 |
| Final ADR-032 classifications | 18 | 19 | +1 |
| Cumulative `removed` | 1 | 2 | +1 |
| Unresolved accepted inventory dependencies | 45 | 44 | -1 |

Removed stable ID:

`crm-application-runtime::dependencies::crm-customer-360-query-adapter`

Replacement boundary:

`crm-first-party-modules`

## Boundary-preserving implementation

1. `crm-first-party-modules` re-exports `MODULE_ID` as `CUSTOMER_360_MODULE_ID` alongside its existing Customer 360 query inventory export.
2. `bootstrap_visibility/registry.rs` imports that constant through `crm_first_party_modules`.
3. `crm-customer-360-query-adapter` is removed only from `crm-application-runtime/Cargo.toml`.
4. `crm-first-party-modules` retains the adapter internally and continues to build the same production contribution.
5. The required module-identity export replaces the unused `CRATE_NAME` marker while the existing query-inventory export stays unchanged, keeping both non-comment LOC and the conservative public Rust surface at their accepted baselines.

No capability coordinate, query inventory, visibility resource, persistence path, authorization rule or runtime route changes.

## Exact lockfile synchronization

The lockfile proof is pinned to immutable baseline `9b2495c9a594f5539aa586f6d775a8ea12442a48`. The only accepted change is deletion of:

`"crm-customer-360-query-adapter",`

from the `crm-application-runtime` dependency list. Registry package versions, sources, checksums and all other package records must remain byte-identical to the baseline.

## Mechanical proof

Run:

```bash
python scripts/check_step22_runtime_fanin_decisions.py
```

The validator requires the current direct dependency set to equal the accepted 63-row Step 22A inventory minus exactly the Customer Privacy and Customer 360 query-adapter edges. Current counts must be 61 total, 60 production and 1 test-only. Both adapter packages must remain present behind their owner boundaries.

## Decision boundary

This packet does not remove either adapter package, remediate another dependency, classify any owner-specific edge as unavoidable, change a workflow or gate disposition, complete Repository Step 22, start Phase 8B or declare architecture 10/10.

## Next Step 22 work

After acceptance, 44 dependencies from the original inventory remain unresolved. Each further reduction remains a separate measured packet.

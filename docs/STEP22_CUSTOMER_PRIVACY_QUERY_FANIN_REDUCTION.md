# Repository Step 22C — Customer Privacy Query Fan-In Reduction

Status: **Active bounded remediation packet**  
Tracking issue: #194  
Binding decision: `docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Baseline: PR #299 squash merge `6fe0e8e7702b01a78f5db3f174c09b686de27402`

## Purpose

Step 22A measured 63 internal direct dependencies of `crm-application-runtime`. Step 22B finalized 16 generic platform boundaries and one test-only edge while leaving all owner-specific edges unresolved.

This packet performs one conclusive safe reduction. `crm-customer-privacy-production` already owns the Customer Privacy control-query adapter, constructs its executor and validator, and is documented as the only supported owner production entry point. The generic runtime nevertheless depended directly on `crm-customer-privacy-query-adapter` only to read `control_query_capability_definitions`.

Step 22C exposes that inventory through the owner production package and removes the redundant direct process-host edge.

## Exact before and after

| Metric | Accepted Step 22A baseline | Step 22C candidate | Delta |
|---|---:|---:|---:|
| Internal direct dependencies | 63 | 62 | -1 |
| Production internal direct dependencies | 62 | 61 | -1 |
| Test-only internal direct dependencies | 1 | 1 | 0 |
| Conservative public Rust surface | 5,377 | 5,377 | 0 |
| Final ADR-032 classifications | 17 | 18 | +1 |
| `removed` | 0 | 1 | +1 |
| Unresolved accepted inventory dependencies | 46 | 45 | -1 |

Removed stable ID:

`crm-application-runtime::dependencies::crm-customer-privacy-query-adapter`

Replacement boundary:

`crm-customer-privacy-production`

## Boundary-preserving implementation

The remediation is deliberately narrow:

1. `crm-customer-privacy-production` publicly re-exports `control_query_capability_definitions` from its existing internal query-adapter dependency.
2. `crm-application-runtime` imports the function through `crm_customer_privacy_production`.
3. `crm-customer-privacy-query-adapter` is removed only from `crm-application-runtime/Cargo.toml`.
4. The production package keeps its existing query-adapter dependency and remains responsible for construction of the control-query adapter, validator and executor.
5. Existing owner-package re-exports are grouped canonically so the conservative public Rust surface remains exactly 5,377 rather than growing for a syntax-only boundary exposure.

No capability ID, schema, public query inventory, route, persistence behavior, visibility policy, cursor behavior or PostgreSQL ownership changes.

## Packet path guard

The packet permits the exact nested manifest `crates/crm-application-runtime/Cargo.toml` and does not permit the workspace-root `Cargo.toml`. The root manifest therefore remains fail-closed as a path outside `allowed_paths`; it does not need a basename-only forbidden pattern that would also match the explicitly allowed nested manifest.

All other broad forbidden path families remain unchanged, and forbidden matches continue to take precedence over allowed matches.

## Exact lockfile synchronization

The lockfile proof is pinned to immutable baseline commit `6fe0e8e7702b01a78f5db3f174c09b686de27402`; it does not read a moving `main` or `origin/main` ref.

The baseline already contains Cargo's canonical `prost` reference. Therefore the only accepted `Cargo.lock` change is deletion of exactly one line from the `crm-application-runtime` dependency list:

`"crm-customer-privacy-query-adapter",`

The one-shot proof requires the resulting lockfile to equal the immutable baseline byte-for-byte after that single deletion and then requires `cargo metadata --locked` to accept it. Parsed comparison additionally requires:

- identical package count and order;
- identical package names, versions, sources and checksums;
- every package record outside `crm-application-runtime` to remain equal;
- every other field and dependency in the runtime package record to remain equal;
- the Customer Privacy production record to retain its internal query-adapter dependency;
- the query-adapter package record to remain present.

No moving-ref drift or broader Cargo-generated lockfile normalization is accepted.

## Mechanical proof

Run:

```bash
python scripts/check_step22_runtime_fanin_decisions.py
```

The permanent validator now fails closed unless:

- the current direct runtime stable-ID set equals the accepted 63-row inventory minus exactly the removed Customer Privacy query-adapter edge;
- current counts are exactly 62 total, 61 production and 1 test-only;
- the generic runtime manifest no longer names `crm-customer-privacy-query-adapter`;
- the generic runtime source contains no `crm_customer_privacy_query_adapter` import;
- the generic runtime consumes `control_query_capability_definitions` through `crm_customer_privacy_production`;
- the owner production manifest still retains the adapter internally;
- the owner production source publicly exposes the inventory function;
- the runtime lock record no longer contains the adapter edge and retains canonical `prost` spelling;
- the owner lock record and adapter package record remain present;
- the decision ledger records exactly 18 final classifications, 1 removal and 45 unresolved dependencies;
- no `owner-specific-unavoidable` decision, gate disposition or Step 22 closure is claimed.

Focused Rust tests and all affected process/PostgreSQL acceptance remain authoritative for behavior preservation.

## Decision boundary

This packet does **not**:

- remove the Customer Privacy query adapter package;
- change the owner production package's internal dependency graph;
- classify `crm-customer-privacy-production` as unavoidable;
- remediate any other owner-specific dependency;
- change any other lockfile byte, package identity, resolved version, source or checksum;
- add, remove or change a permanent workflow or job;
- assign a permanent-gate value disposition;
- complete runtime fan-in classification or Repository Step 22;
- start Phase 8B or declare architecture 10/10.

## Next Step 22 work

After acceptance, 45 dependencies from the original inventory remain unresolved. Further owner/process reductions must remain separate measured packets. An `owner-specific-unavoidable` classification remains prohibited until every ADR-032 evidence field is satisfied, including representative ordinary-owner change-cost proof. The permanent-gate value ledger remains a separate obligation.

# Repository Step 22E — Parties Capability-Adapter Fan-In Reduction

Status: **Active bounded remediation packet**  
Tracking issue: #194  
Binding decision: `docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Baseline: PR #301 squash merge `eac6707e6799f74e761ede39d852bf8de7ac6a77`

## Purpose

Step 22A froze 63 direct internal dependencies of `crm-application-runtime`. Step 22C removed the Customer Privacy query-adapter edge and Step 22D removed the Customer 360 query-adapter edge, leaving 61 current direct internal dependencies: 60 production and 1 test-only.

`crm-party-reference-composition` already owns the complete Parties production/reference composition boundary. It retains `crm-parties-capability-adapter` internally, builds Parties mutation/query contributions, validates Party references and centralizes Party transaction/reference semantics. The generic process runtime nevertheless still depended directly on `crm-parties-capability-adapter` only to obtain stable Parties bootstrap identity metadata: `MODULE_ID`, `RECORD_TYPE`, `CREATE_CAPABILITY` and `UPDATE_CAPABILITY`. It did not require Parties business execution semantics from that adapter.

Step 22E exposes that exact stable Parties runtime identity through the existing Parties production/reference boundary and removes the redundant adapter dependency and direct source imports from the generic runtime.

## Exact before and after

| Metric | Step 22D merged state | Step 22E candidate | Delta |
|---|---:|---:|---:|
| Internal direct dependencies | 61 | 60 | -1 |
| Production internal direct dependencies | 60 | 59 | -1 |
| Test-only internal direct dependencies | 1 | 1 | 0 |
| Conservative public Rust surface | 5,377 | 5,377 | 0 |
| Final ADR-032 classifications | 19 | 20 | +1 |
| Cumulative `removed` | 2 | 3 | +1 |
| Unresolved accepted-inventory dependencies | 44 | 43 | -1 |

Removed stable ID:

```text
crm-application-runtime::dependencies::crm-parties-capability-adapter
```

The immutable Step 22A 63-row inventory is not rewritten. Current state is proven as the accepted inventory minus exactly the three cumulative removed stable IDs.

## Boundary change

Before:

```text
crm-application-runtime
  -> crm-parties-capability-adapter
       -> MODULE_ID / RECORD_TYPE / CREATE_CAPABILITY / UPDATE_CAPABILITY

crm-application-runtime
  -> crm-party-reference-composition
       -> crm-parties-capability-adapter
       -> complete Parties production/reference composition
```

After:

```text
crm-application-runtime
  -> crm-party-reference-composition
       -> parties_runtime_identity()
       -> crm-parties-capability-adapter (owner-internal)
```

The runtime no longer knows the capability adapter as a direct dependency or direct source import. The existing owner boundary remains responsible for the adapter and exposes only the minimal stable identity required for bootstrap composition.

## Public-surface neutrality

Adding `parties_runtime_identity()` would otherwise increase the conservative public Rust surface. This packet therefore retires the unused architecture marker:

```text
crm_party_reference_composition::CRATE_NAME
```

and replaces it with the one public runtime-identity accessor actually consumed by production composition. The target public-surface count remains exactly 5,377.

## Preserved behavior

This packet changes no Parties capability ID, query ID, Protobuf schema, persisted-state schema, record type, database migration, tenant/RLS semantics, authorization rule, idempotency behavior, audit/event behavior, Party reference validation, Customer 360 visibility meaning or Search visibility meaning.

Bootstrap visibility and worker bootstrap authorization still use exactly the same Parties module, record and mutation capability identifiers. Only their package-level source is moved behind the existing owner production/reference boundary.

## Lockfile contract

`Cargo.lock` must remain byte-for-byte equal to the PR #301 baseline except for deletion of exactly the direct `crm-parties-capability-adapter` dependency line from the `crm-application-runtime` package record. The adapter package itself remains present because the owner/reference composition and other accepted consumers still depend on it.

## Validation

Permanent validation must prove all of the following:

- the runtime manifest no longer directly depends on `crm-parties-capability-adapter`;
- no `crm-application-runtime` production source directly imports `crm_parties_capability_adapter`;
- `crm-party-reference-composition` still depends on and owns the adapter internally;
- the owner boundary exports `parties_runtime_identity()`;
- the runtime consumes that exact boundary for Parties module, record and mutation capability identity;
- the current direct runtime set is exactly the immutable Step 22A set minus the three accepted removals;
- counts are exactly 60 total / 59 production / 1 test-only;
- the decision ledger is exactly 20 final / 3 removed / 43 unresolved;
- public Rust surface remains 5,377;
- Customer Privacy and Customer 360 prior removals remain enforced;
- generated contracts, schemas, migrations, routes, workers and permanent workflow topology do not change.

## Explicit non-goals

This packet does not remove `crm-parties-capability-adapter` from the workspace or from owner-internal consumers, does not remediate another runtime dependency, does not classify any owner-specific dependency as unavoidable, does not assign permanent-gate dispositions, does not complete Repository Step 22, does not start Phase 8B and does not declare architecture 10/10.

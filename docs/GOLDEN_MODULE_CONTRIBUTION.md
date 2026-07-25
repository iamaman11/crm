# Golden Module Contribution — Customer Accounts

Status: first bounded production-contribution pilot

Customer Accounts is the first owner migrated from concrete construction in the generic application runtime to a module-owned contribution entry point.

## Why this owner

Customer Accounts has a stable but representative production surface:

- mutation definitions and an aggregate planner;
- live Party reference validation;
- permission-aware get/list queries;
- activation gating;
- PostgreSQL persistence and cursor configuration.

It is complex enough to prove the boundary while avoiding the exceptional orchestration of Identity Resolution, Data Operations or Customer Enrichment.

## Accepted shape

The existing `crm-customer-accounts-capability-composition` package now exposes:

```rust
pub fn build_contribution(
    dependencies: CustomerAccountsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError>;
```

The owner package constructs its planner, semantic validator, query adapter and activation gates internally. The generic application runtime supplies only production context and merges the resulting contribution set.

No new crate is introduced. The transitional composition package is used as the owner production boundary for this pilot.

## Invariants

- public capability coordinates and contracts remain unchanged;
- data-only definition inventory remains available for grants and parity checks;
- final assembly still rejects duplicate routes, owner mismatch and route-kind mismatch;
- the application runtime no longer constructs Customer Accounts planners, validators or query adapters;
- full exact-head acceptance remains mandatory;
- this pilot does not yet introduce the first-party aggregate package or affected-scope skipping.

## Mechanical guard

`check_native_module_composition.py` rejects reintroduction of concrete Customer Accounts planner, validator or query-adapter construction in `crm-application-runtime/src/native_composition.rs`.

## Acceptance boundary

The one-time construction job ran focused composition tests and `cargo check --workspace` before producing the source commit. That staging proof is not final acceptance and is not reusable after any commit. The pilot is accepted only when all applicable permanent workflows pass on one unchanged user-authored candidate SHA after the one-time workflow and patch script are absent from the branch.

## Next comparison

After this pilot is accepted, migrate a second contrasting owner through the same generic merge seam. Only then stabilize a reusable first-party aggregate and decide whether naming/package consolidation is warranted.

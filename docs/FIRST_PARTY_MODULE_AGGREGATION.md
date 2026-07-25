# First-Party Module Contribution Aggregation

Status: bounded Phase C/D aggregation candidate

Customer Accounts and Consents have completed two contrasting module-owned production-contribution proofs. This packet introduces the narrow aggregate required to keep the generic process host independent from a growing list of owner packages.

## New crate justification

- **Protected boundary:** the generic application runtime depends on one first-party aggregate rather than concrete owner composition packages.
- **Isolated dependencies:** owner production-package dependencies remain behind `crm-first-party-modules`.
- **Expected consumers:** `crm-application-runtime` is the first consumer; process-test composition, packaging and future extraction tooling may consume the same stable boundary.
- **Why an internal module is insufficient:** an internal runtime module would not mechanically prevent the process host crate from importing each owner directly.
- **Lifecycle/extraction seam:** the architecture complexity plan explicitly requires a first-party module bundle before affected-scope CI and broader owner migration.
- **Build/test fan-out:** the runtime replaces two direct owner composition dependencies with one aggregate dependency; future migrated owners should change the aggregate rather than generic runtime imports.

## Source-of-truth rule

The aggregate stores no module identifier list and no capability coordinates. It calls owner-provided `build_contribution` functions and merges their `ModuleContributionSet` values. Owner definitions remain authoritative, and final `ApplicationComposition` assembly still rejects duplicates, owner mismatch and route-kind mismatch.

## Initial proven owners

- Customer Accounts — simple Party-reference validation plus mutation/query routes.
- Consents — multi-record Party/Contact Point validation, owner-specific executor wrapping and mutation/query routes.

Other owners remain in their current production wiring until migrated by separately proven packets. Adding a capability to either migrated owner must not require editing the generic application runtime.

## Acceptance boundary

This packet changes composition only. It introduces no public route, worker, contract, migration or business behavior. Application Runtime process acceptance and full unchanged exact-head acceptance remain mandatory.

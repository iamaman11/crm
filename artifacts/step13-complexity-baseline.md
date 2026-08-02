# Repository Step 13 Current-Main Complexity Baseline

Commit: `6a6e0870fb29e9a25dd9083130808b13e4160672`

> ADR-031 measurement and governance calibration only. No structural remediation is authorized.

## Headline

| Metric | Value |
|---|---:|
| Workspace packages | 112 |
| Internal dependency edges | 835 |
| Maximum dependency depth | 18 |
| Maximum direct dependents | 104 |
| Maximum transitive reverse impact | 105 |
| Conservative public Rust items | 5377 |
| Suppression/bypass entries | 91 |
| Permanent workflows | 42 |
| Workflow jobs | 43 |
| Workflow path-filter entries | 1746 |
| PostgreSQL workflows | 31 |

## Central systems

| Package | Role | Direct deps | Direct consumers | Reverse impact | Depth | Public items | Non-comment LOC |
|---|---|---:|---:|---:|---:|---:|---:|
| `crm-module-sdk` | sdk-ports | 0 | 104 | 105 | 0 | 91 | 1372 |
| `crm-core-contracts` | stable-contracts | 0 | 15 | 91 | 0 | 31 | 250 |
| `crm-proto-contracts` | stable-contracts | 0 | 69 | 79 | 0 | 3 | 207 |
| `crm-capability-runtime` | generic-runtime | 1 | 74 | 82 | 1 | 36 | 605 |
| `crm-query-runtime` | generic-runtime | 2 | 46 | 80 | 2 | 37 | 985 |
| `crm-application-composition` | generic-composition | 3 | 18 | 40 | 3 | 60 | 1239 |
| `crm-core-data` | infrastructure-ports | 11 | 70 | 75 | 5 | 176 | 9922 |
| `crm-first-party-modules` | first-party-aggregation | 16 | 1 | 2 | 15 | 14 | 204 |
| `crm-application-runtime` | process-composition | 63 | 1 | 1 | 17 | 129 | 7266 |
| `crm-api` | process-host | 19 | 0 | 0 | 18 | 2 | 9 |

## Suppression and bypass inventory

| Kind | Count |
|---|---:|
| `ignored-test` | 4 |
| `rust-allow` | 87 |

## Representative change cost

| Exemplar | Kind | Files | Packages | Central files | Workflow files |
|---|---|---:|---:|---:|---:|
| `ordinary-owner-capability` | ordinary-capability | 35 | 10 | 6 | 2 |
| `new-owner-production-wave` | new-owner | 206 | 22 | 52 | 4 |
| `owner-contribution-aggregation` | composition-remediation | 21 | 5 | 5 | 0 |

## Candidate-only thin wrappers

| Package | Sole consumer | LOC | Public items |
|---|---|---:|---:|
| `crm-customer-enrichment-visibility` | `crm-application-runtime` | 232 | 5 |
| `crm-first-party-modules` | `crm-application-runtime` | 204 | 14 |
| `crm-party-relationships-capability-composition` | `crm-first-party-modules` | 172 | 7 |

## Calibration

- `ci_regression_blocking_percent`: `25`
- `ci_regression_warning_percent`: `15`
- `expected_workspace_packages`: `112`
- `expired_architecture_exceptions`: `0`
- `generic_composition_loc_growth_requires_rationale`: `0`
- `implementation_reverse_impact_growth_requires_rationale`: `0`
- `new_workspace_package_growth`: `0`
- `ordinary_capability_generic_runtime_files`: `0`
- `ordinary_capability_new_crates`: `0`
- `ordinary_capability_unrelated_owner_packages`: `0`
- `process_host_owner_dependency_growth`: `0`
- `public_surface_growth_requires_rationale`: `0`
- `thin_wrapper_maximum_non_comment_loc`: `250`
- `unregistered_suppressions_after_baseline`: `0`

## Limitations

- Public item counts are conservative source-text measurements.
- Historical change costs do not infer elapsed developer time.
- This first packet inventories suppressions; the accepted baseline is registered and enforced in the next bounded step-13 packet.
- Thin-wrapper results are candidates only.

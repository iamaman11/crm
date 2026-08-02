# Workspace and CI Complexity Baseline

Commit: `6a6e0870fb29e9a25dd9083130808b13e4160672`

> Measurement-only baseline. No thresholds in this report are blocking.

## Headline metrics

| Metric | Value |
|---|---:|
| Workspace packages | 112 |
| Technical crates | 98 |
| Business modules | 13 |
| Internal dependency edges | 835 |
| Maximum direct dependents | 104 |
| Maximum transitive reverse impact | 105 |
| One-consumer packages | 26 |
| Duplicate dependency families | 11 |
| Permanent workflows | 42 |
| Workflow jobs | 43 |
| Workflow path-filter entries | 1746 |
| Workflows using PostgreSQL services | 31 |
| Pull-request workflows | 41 |
| Workflows with concurrency control | 41 |
| Main-only push workflows | 36 |
| Application composition non-comment LOC | 1239 |
| Application runtime non-comment LOC | 7266 |

## Highest reverse-impact packages

| Package | Category | Direct dependents | Transitive reverse impact |
|---|---|---:|---:|
| `crm-module-sdk` | technical-crate | 104 | 105 |
| `crm-core-contracts` | technical-crate | 15 | 91 |
| `crm-capability-runtime` | technical-crate | 74 | 82 |
| `crm-query-runtime` | technical-crate | 46 | 80 |
| `crm-metadata-runtime` | technical-crate | 4 | 79 |
| `crm-proto-contracts` | technical-crate | 69 | 79 |
| `crm-core-events` | technical-crate | 11 | 78 |
| `crm-metadata-schema` | technical-crate | 2 | 78 |
| `crm-metadata-api-adapter` | technical-crate | 3 | 77 |
| `crm-projection-runtime` | technical-crate | 5 | 77 |
| `crm-core-files` | technical-crate | 4 | 76 |
| `crm-metadata-query-adapter` | technical-crate | 2 | 76 |
| `crm-search-runtime` | technical-crate | 5 | 76 |
| `crm-core-data` | technical-crate | 70 | 75 |
| `crm-capability-plan-support` | technical-crate | 58 | 71 |

## Duplicate dependency families

| Dependency | Versions |
|---|---|
| `block-buffer` | 0.10.4, 0.12.1 |
| `cpufeatures` | 0.2.17, 0.3.0 |
| `crypto-common` | 0.1.7, 0.2.2 |
| `digest` | 0.10.7, 0.11.3 |
| `foldhash` | 0.1.5, 0.2.0 |
| `getrandom` | 0.2.17, 0.4.3 |
| `hashbrown` | 0.15.5, 0.16.1, 0.17.1 |
| `hmac` | 0.12.1, 0.13.0 |
| `sha2` | 0.10.9, 0.11.0 |
| `syn` | 2.0.119, 3.0.3 |
| `windows-sys` | 0.52.0, 0.61.2 |

## Workflow summary

| Workflow | Jobs | Paths | Timeout | PostgreSQL |
|---|---:|---:|---:|:---:|
| Affected Scope CI | 1 | 0 | 45 | no |
| Application Runtime CI | 1 | 118 | 70 | yes |
| CI Telemetry Baseline | 1 | 8 | 12 | no |
| Complexity Baseline CI | 1 | 52 | 10 | no |
| Consents Privacy Scope CI | 1 | 40 | 40 | yes |
| Contact Points Privacy Scope CI | 1 | 46 | 50 | yes |
| Contract CI | 1 | 46 | 15 | no |
| Customer Accounts Privacy Scope CI | 1 | 46 | 50 | yes |
| Customer Data Operations Privacy Scope CI | 1 | 52 | 60 | yes |
| Customer Enrichment Privacy Scope CI | 1 | 64 | 60 | yes |
| Customer Enrichment Review Process Runtime CI | 1 | 48 | 30 | yes |
| Customer Enrichment Worker Process Runtime CI | 1 | 46 | 40 | yes |
| Customer Privacy Access Export CI | 1 | 16 | 55 | yes |
| Customer Privacy Approval CI | 1 | 58 | 50 | yes |
| Customer Privacy Discovery CI | 1 | 34 | 45 | yes |
| Customer Privacy Hold Retention CI | 1 | 46 | 50 | yes |
| Customer Privacy Owner Execution CI | 1 | 26 | 60 | yes |
| Customer Privacy Persistence CI | 1 | 48 | 50 | yes |
| Customer Privacy Planning CI | 1 | 42 | 45 | yes |
| Customer Privacy Restriction Policy CI | 1 | 54 | 45 | yes |
| Data Quality Privacy Scope CI | 1 | 54 | 60 | yes |
| Data Quality Process Runtime CI | 1 | 36 | 35 | yes |
| Database CI | 2 | 60 | 40 | yes |
| Event Runtime CI | 1 | 24 | 35 | yes |
| Export Process Runtime CI | 1 | 42 | 45 | yes |
| Generic Mutation Query Conformance CI | 1 | 44 | 50 | yes |
| Governance CI | 1 | 146 | 15 | no |
| Identity Resolution Privacy Scope CI | 1 | 44 | 60 | yes |
| Import Process Runtime CI | 1 | 42 | 35 | yes |
| Import Retryable Process Runtime CI | 1 | 42 | 35 | yes |
| Metadata Runtime CI | 1 | 22 | 25 | yes |
| Parties Privacy Scope CI | 1 | 34 | 35 | yes |
| Party Relationships Privacy Scope CI | 1 | 46 | 55 | yes |
| One-shot PostgreSQL adapter Clippy repair | 1 | 1 | 10 | no |
| PostgreSQL Process Isolation Pilot | 1 | 56 | 45 | yes |
| Product Plane CI | 1 | 46 | 60 | no |
| Projection Runtime CI | 1 | 40 | 35 | yes |
| Rust Generated Sync | 1 | 29 | 20 | no |
| Rust CI | 1 | 0 | 15 | no |
| Search Runtime CI | 1 | 44 | 35 | yes |
| Step 17 Budget Package | 1 | 2 | 30 | no |
| Step 17 Telemetry Budget Refactor | 1 | 2 | 30 | no |

## Limitations

- Build and test durations require repeated runtime telemetry and are not inferred from timeout values.
- Current values are measurement-only and do not establish blocking budgets.
- Reverse impact is computed from declared direct workspace dependencies in cargo metadata.

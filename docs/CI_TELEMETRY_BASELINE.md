# CI Runtime Telemetry Baseline

Snapshot source head: `bcb61db6f43f50f408dda92324bf5d32b840eb33`

Generated at: `2026-07-25T20:02:42.930601+00:00`

Sample window: `2026-07-25T19:22:46+00:00` through `2026-07-25T20:00:20+00:00`

> Measurement-only baseline. Queue, runtime and compute values are historical observations, not blocking budgets. The first snapshot captures an unusually active implementation burst and must not be interpreted as a long-term reliability or cost profile.

## Headline metrics

| Metric | Value |
|---|---:|
| Completed workflow runs sampled | 100 |
| Pull-request runs | 78 |
| Push runs | 22 |
| Jobs with duration data | 62 |
| Successful runs | 52 |
| Failed runs | 9 |
| Cancelled runs | 39 |
| Cancelled pull-request runs | 39 |
| Queue p50 | 0 s |
| Queue p95 | 0 s |
| Execution p50 | 85 s |
| Execution p95 | 232 s |
| Total p50 | 85 s |
| Total p95 | 232 s |
| Sampled runner compute | 94.68 min |

The zero-second queue observation reflects GitHub API timestamp granularity and the sampled period. It does not prove that queueing is permanently absent.

## Highest observed workflow execution p95

| Workflow | Samples | Success | Cancelled | Execution p50/p95 | Sampled runner compute |
|---|---:|---:|---:|---:|---:|
| Rust CI | 16 | 3 | 13 | 109/332 s | 23.57 min |
| Governance CI | 17 | 6 | 11 | 104/319 s | 17.53 min |
| Customer Enrichment Worker Process Runtime CI | 3 | 2 | 1 | 214/232 s | 3.53 min |
| Customer Privacy Persistence CI | 3 | 2 | 1 | 204/218 s | 3.33 min |
| Product Plane CI | 3 | 2 | 1 | 85/210 s | 1.37 min |
| Export Process Runtime CI | 3 | 2 | 1 | 193/203 s | 3.17 min |
| Application Runtime CI | 4 | 2 | 1 | 58/199 s | 2.78 min |
| Rust Generated Sync | 2 | 1 | 1 | 45/180 s | 2.90 min |
| Customer Enrichment Review Process Runtime CI | 3 | 2 | 1 | 144/176 s | 4.97 min |
| Data Quality Process Runtime CI | 3 | 2 | 1 | 159/171 s | 5.08 min |
| Import Process Runtime CI | 3 | 2 | 1 | 162/167 s | 5.05 min |
| Import Retryable Process Runtime CI | 3 | 2 | 1 | 111/140 s | 3.80 min |
| Database CI | 3 | 2 | 1 | 108/114 s | 4.40 min |
| Projection Runtime CI | 3 | 2 | 1 | 97/103 s | 2.97 min |
| Event Runtime CI | 3 | 2 | 1 | 81/100 s | 2.65 min |
| Metadata Runtime CI | 4 | 3 | 1 | 75/98 s | 2.50 min |
| Search Runtime CI | 3 | 2 | 1 | 86/94 s | 2.80 min |
| Contract CI | 4 | 4 | 0 | 19/19 s | 0.53 min |

Cancelled runs are included in the sample counts and duration percentiles. Consequently, the raw success ratio in this burst is not a product-quality success rate.

## Longest sampled completed runs

| Workflow | Event | Conclusion | Execution | Head |
|---|---|---|---:|---|
| Rust CI | pull_request | success | 332 s | `755f91b25c36` |
| Governance CI | pull_request | cancelled | 319 s | `4f97595b42ae` |
| Rust CI | pull_request | success | 311 s | `7bcc0b37cd32` |
| Rust CI | push | success | 299 s | `f89b3238a05e` |
| Governance CI | pull_request | success | 269 s | `7bcc0b37cd32` |
| Customer Enrichment Worker Process Runtime CI | pull_request | success | 232 s | `7bcc0b37cd32` |
| Customer Privacy Persistence CI | pull_request | success | 218 s | `7bcc0b37cd32` |
| Customer Enrichment Worker Process Runtime CI | push | success | 214 s | `f89b3238a05e` |
| Product Plane CI | pull_request | success | 210 s | `7bcc0b37cd32` |
| Customer Privacy Persistence CI | push | success | 204 s | `f89b3238a05e` |
| Export Process Runtime CI | pull_request | success | 203 s | `7bcc0b37cd32` |
| Application Runtime CI | pull_request | success | 199 s | `7bcc0b37cd32` |
| Rust CI | pull_request | cancelled | 194 s | `8c3ce42a30c5` |
| Export Process Runtime CI | push | success | 193 s | `f89b3238a05e` |
| Rust Generated Sync | pull_request | success | 180 s | `7bcc0b37cd32` |

## Evidence-based observations

1. Rust CI and Governance CI are the two longest broad checks in this sample and together account for 41.10 sampled runner minutes. They are the first candidates for cache and critical-path experiments.
2. The 39 cancelled pull-request runs confirm that the newly merged concurrency policy is actively removing superseded work. The snapshot does not calculate avoided compute because a cancelled run may already have consumed runner time.
3. Several PostgreSQL process suites cluster around 2.5–4 minutes in successful runs. This is enough to justify instrumenting migration/setup time separately before changing database architecture.
4. Contract CI remains a fast structural gate at approximately 19 seconds in the sampled runs.
5. Larger runners are not justified by this snapshot. The data does not yet separate compilation, migration, database reset and test execution time or establish CPU saturation.

## Interpretation limits

- `updated_at - run_started_at` is used as workflow execution duration.
- GitHub timestamps have limited precision; short queue intervals may appear as zero.
- Job execution durations are summed only for the configured recent job sample, not every run in the 100-run window.
- The sample covers less than 40 minutes of unusually frequent repository updates.
- One-time and maintenance workflows present in recent history are included in the generated artifact.
- Cancelled pull-request runs are observable; whether each was superseded is not inferred without change-lineage correlation.
- GitHub-hosted runner hardware and queue conditions may change between samples.
- This report does not establish performance budgets, failure-rate targets or a business case for larger runners.

## Next measurements

The permanent `CI Telemetry Baseline` workflow runs daily and can be dispatched manually. Stable Phase B evidence should add:

1. rolling 7-day and 30-day windows;
2. separation of cancelled runs from decisive success/failure rates;
3. correlation of each run with changed paths and affected package closure;
4. job and high-value step durations, especially Rust compile/test and PostgreSQL migration/reset phases;
5. cache hit, restore and save durations after a controlled cache pilot;
6. flake/retry counts and repeated-attempt lineage;
7. estimated compute avoided by superseded-run cancellation.

Only repeated telemetry across representative leaf, owner, shared-core and workflow-only changes may be used to propose relative warnings, cache policy or runner-size experiments.

# CI Runtime Telemetry Baseline

Snapshot source head: `bcb61db6f43f50f408dda92324bf5d32b840eb33`

Generated at: `2026-07-25T20:02:42.930601+00:00`

Sample window: `2026-07-25T19:22:46+00:00` through `2026-07-25T20:00:20+00:00`

> Measurement-only baseline. Queue, runtime, compute and step values are historical observations, not blocking budgets. The first snapshots capture an unusually active implementation burst and must not be interpreted as a long-term reliability or cost profile.

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

## Step-level supplement

Supplement source head: `9ebd8748a3f6d440ff0ab3a6ca63ddbeccbc804f`

Generated at: `2026-07-25T20:28:24.317381+00:00`

The v2 analyzer sampled 100 completed runs, 60 jobs and 556 non-internal workflow steps. GitHub internal `Set up job`, `Complete job` and `Post ...` steps are excluded from the ranked table; action setup, container initialization and repository commands remain visible.

| Workflow | Step | Samples | Success/Failure/Cancelled/Skipped/Other | Execution p50/p95/max |
|---|---|---:|---:|---:|
| Rust CI | Run workspace tests | 17 | 6/0/7/4/0 | 83/222/222 s |
| Governance CI | Run permanent native architecture conformance preflight | 17 | 10/0/7/0/0 | 79/94/94 s |
| Rust CI | Run Clippy | 17 | 13/0/4/0/0 | 71/81/81 s |
| Metadata Runtime CI | Verify durable tenant-scoped metadata publication runtime | 1 | 1/0/0/0/0 | 33/33/33 s |
| CI Telemetry Baseline | Collect measurement-only CI telemetry | 6 | 6/0/0/0/0 | 20/28/28 s |
| Product Plane CI | Verify generated contract artifacts are up-to-date | 1 | 1/0/0/0/0 | 27/27/27 s |
| Product Plane CI | Run Web typecheck | 1 | 1/0/0/0/0 | 10/10/10 s |
| Product Plane CI | Run E2E Integration Suite | 1 | 1/0/0/0/0 | 7/7/7 s |

`Success/Failure/Cancelled/Skipped/Other` is shown explicitly so conclusion counts reconcile with the sample count. Skipped steps normally result from an earlier failure or cancellation and must not be interpreted as zero-cost successful executions.

### Step-level observations

1. The Rust workspace-test step is the dominant measured Rust step: p50/p95 is 83/222 seconds, compared with 71/81 seconds for Clippy.
2. A cache experiment should therefore target reusable Rust compilation state shared by Clippy and workspace tests, while continuing to measure the two steps separately.
3. Step timing alone does not distinguish dependency compilation, local-crate compilation, linking and actual test execution inside `cargo test`. Cache gains must be proven by warm/cold comparisons and restore/save timing rather than inferred from the current 222-second p95.
4. Governance conformance is a separate broad Python/Rust/contract path at 79/94 seconds and should not be assumed to benefit materially from the Rust target cache.
5. PostgreSQL step coverage in this 60-job sample is sparse and uneven. Daily reports must accumulate more migration, reset, process-test and container-setup samples before database sharding or template-database decisions.

## Evidence-based observations

1. Rust CI and Governance CI are the two longest broad checks in the initial sample. They remain the first candidates for critical-path experiments, but they require different optimizations.
2. The cancelled pull-request runs confirm that the merged concurrency policy is actively removing superseded work. The snapshots do not calculate avoided compute because a cancelled run may already have consumed runner time.
3. Several PostgreSQL process suites cluster around 2.5–4 minutes in successful runs. Step-level coverage must grow before changing database architecture.
4. Contract CI remains a fast structural gate at approximately 19 seconds in the sampled runs.
5. Larger runners are not justified by these snapshots. The data does not establish CPU saturation or linear parallel scalability.
6. The next controlled optimization should be a Rust cache pilot with exact restore/save telemetry, trusted-write boundaries and unchanged full acceptance.

## Interpretation limits

- `updated_at - run_started_at` is used as workflow execution duration.
- GitHub timestamps have limited precision; short queue intervals may appear as zero.
- Job and step execution durations are collected only for the configured recent job sample, not every run in the 100-run window.
- Step timestamps include the whole named command/action and may combine compilation, linking, process startup and execution.
- The samples cover unusually frequent repository updates rather than a stable multi-day period.
- One-time and maintenance workflows present in recent history are included in generated artifacts.
- Cancelled pull-request runs are observable; whether each was superseded is not inferred without change-lineage correlation.
- GitHub-hosted runner hardware and queue conditions may change between samples.
- This report does not establish performance budgets, failure-rate targets or a business case for larger runners.

## Next measurements

The permanent `CI Telemetry Baseline` workflow runs daily and can be dispatched manually. Stable Phase B evidence should add:

1. rolling 7-day and 30-day windows;
2. separation of cancelled runs from decisive success/failure rates;
3. correlation of each run with changed paths and affected package closure;
4. broader PostgreSQL migration/reset/process step samples;
5. cache hit, restore and save durations from a controlled Rust cache pilot;
6. Rust cold/warm Clippy and workspace-test comparisons;
7. flake/retry counts and repeated-attempt lineage;
8. estimated compute avoided by superseded-run cancellation.

Only repeated telemetry across representative leaf, owner, shared-core and workflow-only changes may be used to propose relative warnings, cache policy or runner-size experiments.

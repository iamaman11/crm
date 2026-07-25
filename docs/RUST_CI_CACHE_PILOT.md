# Rust CI Cache Pilot — Concluded

Status: concluded; no Rust cache retained

This Phase B experiment evaluated whether GitHub Actions cache restore/save could reduce the unchanged Rust CI critical path without weakening exact-head acceptance, test scope or cache trust.

The final decision is to retain **no Rust cache** in the permanent workflow. Both tested scopes failed to demonstrate a stable benefit that justified their transfer cost and operational complexity.

## Pre-cache baseline

The accepted step-level telemetry baseline measured:

- `Run workspace tests`: p50/p95/max `83/222/222` seconds;
- `Run Clippy`: p50/p95/max `71/81/81` seconds;
- combined Clippy plus tests: p50-derived `154` seconds and p95-derived `303` seconds;
- full Rust workflow execution p95 approximately `332` seconds in the initial burst sample.

A Cargo step combines dependency resolution, compilation, linking and test execution. The experiments were therefore evaluated by the complete measured path, not by cache-hit status alone.

## Trust model used during the experiment

The temporary pilot used split `actions/cache/restore` and `actions/cache/save`, pinned to official `v5.0.3` commit `cdf6c1fa76f9f475f3d7449005a359c84ca0f306`.

During both variants:

1. pull-request runs could restore but could not publish entries;
2. only a successful `main` push could publish after architecture, lockfile, formatting, Clippy and full workspace tests passed;
3. misses were non-blocking and executed the full cold path;
4. every validation run still executed the complete quality suite;
5. machine-readable artifacts recorded exact hit, keys, restored footprint, restore duration, Clippy/test durations and publish outcome.

These controls proved that the performance conclusion was not obtained by reducing coverage or allowing pull requests to populate trusted entries.

## Experiment 1 — complete build-output cache

The `rust-quality-v1` variant stored Cargo dependency data plus the complete `target/` directory.

An exact trusted hit on SHA `fdb51ca8591a306899bb24941e46441bf8585166` measured:

- restored footprint: `21,638,228,034` bytes (`20.15 GiB`);
- restore duration: `86,274` ms;
- Clippy duration: `39,945` ms;
- workspace-test duration: `194,318` ms;
- combined restore plus Clippy plus tests: `320,537` ms;
- pull-request publish outcome: `skipped`.

The archive reduced Clippy time but cost too much to restore and did not materially reduce the test step. The complete warm path was worse than the p50-derived pre-cache path and slightly worse than the p95-derived path.

**Decision:** full `target/` caching was rejected immediately. Its unused key epoch will expire naturally.

## Experiment 2 — dependency-only cache

The `rust-deps-v2` variant stored only Cargo registry index/archive data and Cargo git database data. It deliberately excluded `target/`.

### Cold exact-head sample

SHA `75a6f610eef60c08edb7a188ec5036de7149a42b` passed Complexity Baseline CI, Governance CI and the complete Rust CI suite.

Measured values:

- exact hit: `false`;
- restored footprint: `0` bytes;
- restore duration: `278` ms;
- Clippy duration: `71,985` ms;
- workspace-test duration: `196,992` ms;
- combined restore plus Clippy plus tests: `269,255` ms;
- pull-request publish outcome: `skipped`.

### Warm exact-head sample 1

SHA `3273ed1e33f8cd6e5c92ece4d0fa6a0026f26d72` proved an exact trusted dependency hit:

- restored footprint: `74,881,152` bytes (`71.41 MiB`);
- restore duration: `2,307` ms;
- Clippy duration: `57,719` ms;
- workspace-test duration: `197,931` ms;
- combined restore plus Clippy plus tests: `257,957` ms;
- delta from cold: `-11,298` ms (`-4.2%`);
- pull-request publish outcome: `skipped`.

### Warm exact-head sample 2

SHA `2bd93b4fc43fb441b8a5b44bf378ea70b11355f2` repeated the exact trusted hit:

- restored footprint: `74,881,152` bytes (`71.41 MiB`);
- restore duration: `882` ms;
- Clippy duration: `71,434` ms;
- workspace-test duration: `208,626` ms;
- combined restore plus Clippy plus tests: `280,942` ms;
- delta from cold: `+11,687` ms (`+4.3%`);
- pull-request publish outcome: `skipped`.

The mean of the two warm samples is `269,449.5` ms, compared with `269,255` ms for the exact cold sample: approximately `+0.07%`. The observed result is neutral and slightly negative, while the cache adds Action dependencies, key maintenance, storage, publication logic and another failure surface.

## Final decision

The Rust cache pilot is concluded with no cache retained:

- all `actions/cache` restore/save steps are removed from `Rust CI`;
- the temporary cache-policy checker and its tests are removed;
- Rust CI returns to the simpler cold workflow;
- exact-head acceptance, architecture checks, lockfile freshness, formatting, Clippy and full workspace tests remain unchanged;
- historical entries under `rust-quality-v1` and `rust-deps-v2` are no longer referenced and may expire naturally.

A future cache proposal must start as a new, separately gated experiment with a new key epoch and representative repeated measurements. It must demonstrate a stable total-runtime or compute-cost improvement rather than a single favorable hit.

This experiment introduced no permanent performance budget and no production behavior change.

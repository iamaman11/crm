# Trusted Rust CI Cache Pilot

Status: controlled Phase B pilot

This packet introduces one bounded cache experiment in `Rust CI`. It does not change test scope, runner size, exact-head acceptance, dependency resolution or production behavior.

## Baseline

The accepted step-level telemetry baseline measured:

- `Run workspace tests`: p50/p95/max `83/222/222` seconds;
- `Run Clippy`: p50/p95/max `71/81/81` seconds;
- full Rust workflow execution p95 approximately `332` seconds in the initial burst sample.

A Cargo step includes dependency compilation, local compilation, linking and execution. Therefore the pilot must compare observed step durations and cache restore/save telemetry; it must not attribute every duration change to compilation.

## Trust model

The cache policy is:

1. Pull-request runs may restore cache entries but never save them.
2. Only a successful `push` run on `refs/heads/main` may save a cache entry.
3. The save action runs only after architecture, lockfile, formatting, Clippy and full workspace tests succeed.
4. A cache miss is non-blocking and executes the complete cold path.
5. Cache content is limited to Cargo registry/git data and `target/` build output. Credentials, SSH material and environment files are forbidden.
6. Restore and save use the same immutable `actions/cache` commit and the same path set.

The Action is pinned to official `actions/cache` tag `v5.0.3`, resolved to commit `cdf6c1fa76f9f475f3d7449005a359c84ca0f306`. The split `restore` and `save` actions make write eligibility explicit instead of relying on a post-job implicit save.

## Key model

The primary key includes:

- runner operating system;
- runner architecture;
- a SHA-256 digest of `rustc -Vv`;
- `Cargo.lock` content hash;
- an explicit policy epoch (`rust-quality-v1`).

A restore prefix may reuse an older cache only within the same operating system, architecture and exact Rust toolchain identity. Cargo remains responsible for validating and rebuilding stale artifacts.

## Measurement

`CI Telemetry Baseline` already records individual step durations. The pilot adds named steps for:

- Rust cache identity resolution;
- trusted cache restore;
- restore outcome reporting;
- trusted cache save.

The Rust job summary records exact hit, primary key, matched key and whether the run is eligible to write.

## Acceptance sequence

1. The pilot PR must pass its complete cold/miss exact-head gate without saving from the PR.
2. After merge, the successful `main` Rust run may create the trusted cache.
3. A separate follow-up PR must prove a warm restore while still executing the full architecture, lockfile, formatting, Clippy and workspace-test suite.
4. Cache restore/save cost and Clippy/test duration must be compared with the recorded baseline.
5. The cache remains a pilot until representative repeated samples justify retaining, revising or removing it.

No absolute performance budget is introduced by this document.

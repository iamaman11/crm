# Trusted Rust CI Cache Pilot

Status: dependency-only Phase B pilot

This packet evaluates one bounded cache experiment in `Rust CI`. It does not change test scope, runner size, exact-head acceptance, dependency resolution or production behavior.

## Accepted pre-cache baseline

The step-level telemetry baseline measured:

- `Run workspace tests`: p50/p95/max `83/222/222` seconds;
- `Run Clippy`: p50/p95/max `71/81/81` seconds;
- combined Clippy plus tests: p50-derived `154` seconds and p95-derived `303` seconds;
- full Rust workflow execution p95 approximately `332` seconds in the initial burst sample.

A Cargo step includes dependency compilation, local compilation, linking and execution. Cache effectiveness must therefore be judged by the complete measured path, not by cache-hit state alone.

## Trust and key model

Pull requests may restore but never publish an entry. Only a successful `main` push may publish after architecture, lockfile, formatting, Clippy and full workspace tests pass. A miss remains non-blocking and executes the complete cold path.

The Action is pinned to official `actions/cache` tag `v5.0.3`, resolved to commit `cdf6c1fa76f9f475f3d7449005a359c84ca0f306`.

The active key contains:

- policy epoch `rust-deps-v2`;
- runner operating system and architecture;
- a SHA-256 digest of `rustc -Vv`;
- `Cargo.lock` content hash.

## Rejected full-target experiment

The initial `rust-quality-v1` experiment included Cargo dependency data plus the complete `target/` directory. A successful `main` run created the entry, and a later pull-request run proved an exact trusted hit with identical primary and matched keys while remaining write-ineligible.

Machine-readable telemetry on exact-head SHA `fdb51ca8591a306899bb24941e46441bf8585166` measured:

- exact hit: `true`;
- main write eligible: `false`;
- restored footprint: `21,638,228,034` bytes (`20.15 GiB`);
- restore duration: `86,274` ms;
- Clippy duration: `39,945` ms;
- workspace-test duration: `194,318` ms;
- combined restore plus Clippy plus tests: `320,537` ms;
- save outcome: `skipped`.

The complete warm path was worse than the p50-derived pre-cache path (`154` seconds) and slightly worse than the p95-derived path (`303` seconds). The cache reduced Clippy time but transferred an excessive archive and did not reduce test execution enough to recover the restore cost.

**Decision:** the complete `target/` cache is rejected. Its `rust-quality-v1` key epoch is no longer referenced and will expire naturally. `target/` is now forbidden by the permanent cache-policy check.

## Dependency-only revision

The active `rust-deps-v2` experiment stores only Cargo registry index/archive data and Cargo git database data. It deliberately excludes `target/`. Clippy and tests continue to build and execute from a clean build-output directory, while repeated dependency downloads may be avoided.

Every Rust run publishes `rust-cache-telemetry.json` with cache scope, exact candidate SHA, hit/key information, publish eligibility, restore duration and footprint, and Clippy/test durations and outcomes.

## Acceptance sequence

1. The dependency-only revision must pass a complete cold/miss exact-head pull-request gate and prove that publishing remains skipped.
2. After merge, a successful `main` Rust run may create the dependency entry.
3. A separate exact-head pull request must prove a warm dependency restore while executing the unchanged full quality suite.
4. Restore cost and Clippy/test duration must be compared with both the pre-cache baseline and the rejected full-target experiment.
5. The dependency cache is retained only if repeated evidence shows neutral or positive total runtime without weakening correctness.

No absolute performance budget is introduced by this document.

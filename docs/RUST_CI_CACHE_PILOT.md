# Trusted Rust CI Cache Pilot

Status: accepted dependency-only Phase B pilot

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

**Decision:** the complete `target/` cache is rejected. Its `rust-quality-v1` key epoch is no longer referenced and will expire naturally. `target/` is forbidden by the permanent cache-policy check.

## Dependency-only experiment

The active `rust-deps-v2` entry stores only Cargo registry index/archive data and Cargo git database data. It deliberately excludes `target/`. Clippy and tests continue to build and execute from a clean build-output directory, while repeated dependency downloads are avoided.

Every Rust run publishes `rust-cache-telemetry.json` with cache scope, exact candidate SHA, hit/key information, publish eligibility, restore duration and footprint, and Clippy/test durations and outcomes.

### Cold acceptance

Exact-head SHA `75a6f610eef60c08edb7a188ec5036de7149a42b` passed Complexity Baseline CI, Governance CI and the complete Rust CI suite. Its pull-request telemetry measured:

- exact hit: `false` under the new epoch;
- restored footprint: `0` bytes;
- restore duration: `278` ms;
- Clippy duration: `71,985` ms;
- workspace-test duration: `196,992` ms;
- combined restore plus Clippy plus tests: `269,255` ms;
- main write eligible: `false`;
- save outcome: `skipped`.

This proved the cold path, full quality scope and pull-request publication denial.

### Premature validation miss

The first documentation-only validation run on SHA `70367ac41fcaf6d935e1b040c061a1786424f259` started before the merged cold `main` run had completed publication. It remained a valid cold/miss execution and passed the full Rust quality suite, but did not satisfy warm acceptance.

### Warm acceptance

Exact-head SHA `3273ed1e33f8cd6e5c92ece4d0fa6a0026f26d72` proved the entry published by `main`:

- cache scope: `cargo-dependencies`;
- exact hit: `true`;
- primary and matched keys: identical;
- restored footprint: `74,881,152` bytes (`71.41 MiB`);
- restore duration: `2,307` ms;
- Clippy duration: `57,719` ms;
- workspace-test duration: `197,931` ms;
- combined restore plus Clippy plus tests: `257,957` ms;
- main write eligible: `false`;
- save outcome: `skipped`;
- architecture, lockfile freshness, formatting, Clippy and full workspace tests: successful.

Against the exact cold dependency-only sample, the warm path reduced the measured total by `11,298` ms (`4.2%`). Restore overhead remained small, the entry was approximately 71 MiB, and pull-request publication remained impossible.

## Decision and continued measurement

The dependency-only cache is accepted as a bounded pilot because it produced a modest positive exact-sample result without caching build outputs or weakening any quality gate.

It is not treated as a permanent performance guarantee. Daily CI telemetry must continue to observe restore duration, cache footprint, Clippy/test duration, misses and key churn. The policy epoch should be revised or the cache removed if repeated representative samples become neutral or negative.

No absolute performance budget is introduced by this document.

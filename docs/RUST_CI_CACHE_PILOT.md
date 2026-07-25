# Trusted Rust CI Cache Pilot

Status: dependency-only warm validation

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

This proved the cold path, full quality scope and pull-request publication denial. The merged `main` run is permitted to create the first dependency-only entry only after the same full suite passes.

### First validation attempt

The first documentation-only validation run on SHA `70367ac41fcaf6d935e1b040c061a1786424f259` started before the merged cold `main` run had completed publication. It remained a valid cold/miss execution:

- exact hit: `false`;
- restored footprint: `0` bytes;
- restore duration: `297` ms;
- Clippy duration: `73,437` ms;
- workspace-test duration: `211,504` ms;
- combined restore plus Clippy plus tests: `285,238` ms;
- main write eligible: `false`;
- save outcome: `skipped`;
- full Rust quality suite: successful.

This attempt is retained as miss-tolerance evidence but does not satisfy warm acceptance. A newer exact head must prove the published dependency entry.

## Warm acceptance requirement

This documentation-only branch intentionally leaves cache logic unchanged. Its final exact-head Rust run must prove:

1. `cache_scope` is `cargo-dependencies`;
2. exact hit is `true` with identical primary and matched keys;
3. main write eligibility is `false` and save outcome is `skipped`;
4. architecture, lockfile freshness, formatting, Clippy and full workspace tests all pass;
5. restore plus Clippy plus test duration is compared with the `269,255` ms cold measurement and the rejected `320,537` ms full-target result.

The dependency cache is retained only if repeated evidence shows neutral or positive total runtime without weakening correctness. No absolute performance budget is introduced by this document.

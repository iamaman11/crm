# CI Scalability, Test Isolation and Device-Lab Architecture Plan

Status: **Normative companion to `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`**

This document refines the primary architecture scalability plan where verification fan-out, workflow orchestration, PostgreSQL process acceptance, build caching, supply-chain controls and real-device execution require concrete operational rules.

It does not replace:

- `SYSTEM_INVARIANTS.md`;
- `ARCHITECTURE_READINESS.md`;
- `DELIVERY_GOVERNANCE.md`;
- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`;
- the exact-head acceptance contract.

Where this document and a lower-level workflow implementation disagree, this document is authoritative until the normative architecture documents are explicitly amended.

## 1. Verified repository baseline

The current repository demonstrates strong final quality discipline but an increasingly expensive iteration model:

- PR #155 contained 58 commits, 23 changed files, 927 additions and 104 deletions;
- its accepted candidate SHA passed 15 applicable workflows;
- Rust CI runs for every push and pull request without path filtering;
- Rust CI performs architecture validation, lockfile generation/freshness, full formatting, workspace-wide Clippy for all targets/features and workspace-wide tests for all features;
- Rust CI has no explicit Cargo build cache or `sccache` configuration;
- Application Runtime CI uses a 70-minute timeout and repeatedly resets the schema, reapplies all migrations and then runs one real-process E2E scenario at a time;
- long path lists are duplicated manually between `push` and `pull_request` blocks in permanent workflows;
- `PROJECT_STATUS.md` has already lagged accepted merges.

The principal scaling problem is therefore not free runner-minute availability. It is the widening and manually maintained graph that decides which checks run, how often they repeat, and whether expensive tests are isolated enough to execute concurrently.

## 2. Objectives

The verification system must:

1. preserve all accepted architecture, persistence, RLS, process and exact-head guarantees;
2. make ordinary iteration cost proportional to the explainable affected dependency closure;
3. prevent duplicate equivalent workflow runs for the same feature-branch commit;
4. cancel obsolete iterative work without cancelling authoritative acceptance evidence;
5. centralize workflow setup and check-selection metadata without creating a hidden central business router;
6. isolate PostgreSQL process tests so independent suites can run concurrently;
7. treat build caches and third-party Actions as supply-chain boundaries;
8. prevent public-repository code from reaching persistent personal or device-connected runners;
9. measure the CI critical path before purchasing larger runners;
10. make every skipped or selected workflow machine-explainable.

## 3. Non-goals

This plan does not authorize:

- weakening the final unchanged exact-head gate;
- replacing full phase-closure, nightly or release regression matrices with affected-scope checks;
- trusting caches as correctness evidence;
- moving domain-specific acceptance into generic infrastructure tests;
- connecting a personal workstation as a normal persistent self-hosted runner;
- migrating source control providers merely to obtain a different free-minute allowance;
- buying large runners before the critical path is measured;
- parallelizing tests that share mutable database, process, port, advisory-lock or artifact state.

## 4. Workflow event architecture

### 4.1 Required trigger model

Ordinary feature-branch verification should run from `pull_request`.

Permanent workflows should use `push` only for:

- `main`;
- explicitly governed release branches;
- release tags;
- a documented branch-specific responsibility that is not equivalent to its PR run.

A permanent workflow must not run equivalent full checks for both feature-branch `push` and `pull_request` unless the PR explains why the two executions provide different acceptance evidence.

Recommended default:

```yaml
on:
  push:
    branches:
      - main
  pull_request:
```

### 4.2 Superseded PR cancellation

Iterative PR workflows should use a concurrency key based on workflow identity and PR number:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

Required behavior:

- a newer PR commit cancels an older in-progress iterative run for the same workflow and PR;
- `main`, release, phase-closure and final acceptance runs are not cancelled merely because a later run exists;
- once an exact candidate SHA is selected for Gate review, final evidence must come from a complete non-superseded run on that unchanged SHA;
- any source or documentation commit invalidates prior exact-head evidence.

Concurrency is an efficiency control only. It cannot redefine acceptance.

## 5. Check graph and path-filter governance

### 5.1 One governed source of truth

Workflow path filters and affected-scope selection must be generated from, or mechanically compared against:

- Cargo workspace metadata and reverse dependencies;
- module manifests;
- contract bindings;
- migration ownership;
- production route and worker inventories;
- frontend package ownership;
- architecture policy metadata;
- device-lab requirement metadata.

Hand-maintained path lists may remain during migration, but duplicated lists must have a parity test until they are generated.

### 5.2 Explainable selection

`python scripts/repo.py check-affected --base origin/main` must print:

- directly changed paths;
- affected Rust packages and reverse dependency closure;
- affected contracts and generated bindings;
- affected migrations and database suites;
- affected public routes, queries, workers and process suites;
- affected frontend packages;
- required workflows;
- why each workflow is required;
- why each skipped workflow is safe to skip;
- whether uncertainty widened the selection to a broader matrix.

Unknown impact defaults to broader verification.

### 5.3 Required execution layers

1. **Structural preflight** — architecture, manifests, contracts, generated freshness, formatting, dependency and documentation checks.
2. **Affected package closure** — focused Rust and static checks for changed packages and reverse dependencies.
3. **Domain acceptance** — owner-specific domain, application, PostgreSQL, RLS, idempotency and migration tests.
4. **Process acceptance** — real HTTP/gRPC/worker/process tests when runtime behavior is affected.
5. **Full matrix** — nightly, core/shared changes, phase closure, release, architecture-policy changes and uncertain impact.

The final Gate review still requires all applicable checks on one unchanged candidate SHA.

## 6. Rust build-cache policy

### 6.1 Measurement first

Before enabling a blocking cache standard, record:

- clean build duration;
- warm local build duration;
- dependency compilation time;
- crate compilation time;
- linking time;
- cache restore duration;
- cache save duration;
- cache hit rate;
- total workflow critical path.

### 6.2 Cache trust model

Correctness must never depend on a cache hit.

Required controls:

- cache misses and cache-service failures fall back to a correct clean build;
- cache identity includes OS, Rust toolchain, target triple, profile, `Cargo.lock` digest and material feature grouping;
- untrusted fork and PR contexts may restore approved cache content but must not publish into a shared trusted namespace;
- trusted `main` or dedicated maintenance jobs may publish shared caches;
- PR-specific cache publication, if used, remains isolated from trusted branches;
- cache contents are treated as untrusted input and never as acceptance evidence;
- cache growth, eviction and storage consumption are measured.

### 6.3 Cargo cache versus sccache pilot

Pilot both approaches on representative workflows:

- ordinary dependency/target caching;
- `sccache` with its Rust constraints and incremental-compilation implications.

Adopt the option that produces the best measured critical-path improvement without hiding an overly broad affected dependency closure.

## 7. GitHub Actions supply-chain policy

### 7.1 Immutable references

Every third-party Action in a permanent workflow must be pinned to a full commit SHA.

The human-readable release tag should remain in a comment:

```yaml
- uses: actions/checkout@<full-commit-sha> # v7
```

Moving references such as `@main`, `@master` and `@stable` are forbidden in permanent workflows.

### 7.2 Controlled updates

Action updates must:

- arrive through a bounded maintenance PR;
- show the old and new full SHA;
- identify the intended release/tag;
- run normal governance and affected checks;
- be compatible with an allowlist of approved publishers/repositories;
- preserve minimum `GITHUB_TOKEN` permissions.

Workflow, reusable-workflow and composite-action changes are production supply-chain changes.

## 8. Reusable workflow and composite-action boundary

Use reusable workflows for repeated job-level orchestration and composite Actions for repeated step sequences.

Standardize where useful:

- pinned checkout;
- Rust toolchain installation;
- Python and dependency setup;
- Buf setup;
- PostgreSQL client setup;
- cache bootstrap;
- diagnostics and artifact collection;
- common environment validation.

Keep domain-specific commands, tests and acceptance responsibility visible in the calling workflow.

Configuration reuse primarily reduces drift and maintenance risk. It does not itself eliminate setup time because each hosted job receives an isolated runner.

## 9. PostgreSQL E2E architecture

### 9.1 Separate migration correctness from process isolation

Repeatedly running:

```text
reset schema -> apply all migrations -> run one E2E
```

inside a single sequential job combines two different responsibilities.

Maintain two acceptance lanes.

### 9.2 Sequential migration lane

This lane proves:

- clean database/cluster setup;
- complete ordered migration application;
- rollback and reapply where supported;
- FORCE RLS behavior;
- cluster-wide role semantics;
- database grants and configuration;
- repeated application safety;
- irreversible migration evidence where applicable.

It remains sequential where ordering is part of the behavior under test.

### 9.3 Isolated parallel process lane

Independent process suites should receive independent database and artifact namespaces.

Supported models:

1. a migrated PostgreSQL template database cloned into a unique database per shard; or
2. a GitHub matrix where every shard receives its own PostgreSQL service container.

Every shard must have:

- a unique database name and `DATABASE_URL`;
- unique ports where multiple processes share a host;
- unique artifact/log paths;
- no shared mutable fixtures;
- no fixed advisory-lock namespace collision;
- deterministic cleanup.

Template cloning must account for:

- cluster-wide roles not being database-local;
- database-level grants/configuration that may require reapplication;
- prohibition on active connections to the template during cloning;
- fixed database-name assumptions in existing tests.

### 9.4 Rollout

Start with two independent suites, preferably Party and Account or another pair with low coupling.

Measure separately:

- migration/setup time;
- database clone/start time;
- Rust compilation time;
- process startup time;
- test execution time;
- cleanup time;
- total critical path.

Expand only after isolation, diagnostics and failure reproducibility are proven.

## 10. Device-lab security boundary

### 10.1 Prohibited topology

A public repository must not execute untrusted PR code on:

- a personal workstation;
- a persistent device-connected machine;
- an ordinary long-lived self-hosted runner containing reusable credentials or local state.

### 10.2 Required topology

Preferred architecture:

```text
public CRM repository
    -> trusted merged SHA or signed artifact
    -> private device-lab control repository or protected dispatcher
    -> manual/protected-environment approval
    -> ephemeral or JIT runner
    -> isolated real device
```

Required controls:

- dedicated machine, VM or isolated host without personal data;
- no fork PR execution;
- no unsafe `pull_request_target` checkout of untrusted code;
- only merged, signed or explicitly approved candidate inputs;
- minimal short-lived tokens;
- separate network placement;
- dedicated runner label such as `device-lab`;
- one job per ephemeral/JIT runner where practical;
- deterministic runner, workspace and device-state cleanup;
- externally retained diagnostics sufficient for incident investigation;
- explicit inventory of connected devices and firmware versions;
- manual recovery path when device reset fails.

Device tests supplement hosted CI. Hardware-independent protocol, persistence and process behavior must remain testable without the device.

## 11. Platform and larger-runner policy

GitHub remains the default SCM/CI platform while the public-repository and governance model remains suitable.

Provider pricing and free-minute quantities are dated operational inputs and must not be embedded as permanent architecture facts. Re-evaluate platform choice only when repository visibility, security requirements, integrations or measured total cost materially change.

A larger runner may be piloted only after cache, trigger and fan-out improvements when measurements prove that the critical path:

- remains CPU- or memory-bound;
- contains parallel work capable of using more resources;
- blocks normal delivery;
- produces a better cost/time result than check-graph or database-isolation improvements.

Begin with an 8–16 vCPU comparison. Do not assume 32–96 vCPU will linearly improve linking, migrations or single-process E2E execution.

## 12. Required CI observability

The complexity analyzer and CI reporting must include:

- number of workflows selected per PR;
- number of push and pull-request runs per PR;
- duplicate equivalent runs;
- cancelled superseded runs and estimated time saved;
- queue time;
- runner execution time;
- p50 and p95 duration by workflow;
- total compute per PR;
- exact-head acceptance critical path;
- cache hit rate and restore/save duration;
- Rust compile and link duration;
- migration/reset duration;
- database clone/service startup duration;
- E2E execution duration separate from setup;
- retry, rerun and flake rate;
- artifact and cache storage growth;
- failure category and first failing step.

A larger runner or major workflow redesign must cite before/after measurements.

## 13. Immediate controls

The following low-risk controls may land before or alongside the active Privacy adapter and Phase B measurement work:

1. restrict ordinary permanent `push` triggers to `main` and governed release refs;
2. add PR-scoped concurrency cancellation for superseded iterative runs;
3. pin permanent third-party Actions to full commit SHAs;
4. include every normative architecture/check-selection document in Governance CI coverage;
5. synchronize `PROJECT_STATUS.md`, roadmap, phase plan and active packet after accepted merges;
6. begin collecting workflow count, queue time, execution time and duplicate-run metrics.

Each control must be a bounded PR with its own unchanged exact-head evidence.

## 14. Phased delivery integration

### Phase B additions

Complexity measurement must include CI critical-path, duplicate-run, cache, migration/setup and flake metrics, not only crate/dependency metrics.

### Phase E additions

Affected-scope CI delivery includes:

1. event normalization;
2. concurrency cancellation for superseded PR iterations;
3. immutable Action pinning;
4. repeated setup centralization;
5. affected package/check calculation;
6. trusted cache pilot;
7. PostgreSQL isolation pilot;
8. generated or mechanically validated workflow filters;
9. retained full nightly, phase-closure and release matrices;
10. unchanged exact-head Gate rules.

### Phase I additions

Frontend and operational parity includes establishing the private, approval-gated, ephemeral/JIT device-lab contour before any real-device CI is accepted.

## 15. Recommended order

```text
active Privacy adapter hygiene and proof
-> immediate workflow event/concurrency/action-pinning and status-sync controls
-> complexity and CI critical-path baseline
-> golden domain production package
-> first-party contribution aggregation
-> affected-scope CI plus PostgreSQL isolation pilot
-> second Privacy owner implementation
-> shared Privacy protocol extraction
-> remaining owner contributions
-> gradual transitional-crate consolidation
-> frontend, operational and secure device-lab parity
```

This order preserves product delivery, removes obvious repeated work early and avoids using faster hardware to conceal an unbounded verification graph.

## 16. Hard invariants

The following should become immediately blocking once their checks are implemented:

- equivalent feature-branch `push` and `pull_request` verification without documented distinct responsibility;
- permanent third-party Action referenced by a moving tag or branch;
- untrusted PR context publishing into a shared trusted cache namespace;
- skipped affected-scope check without a machine-readable reason;
- path/check filters diverging from governed ownership metadata;
- public-repository PR code executing on a persistent personal or ordinary long-lived self-hosted runner;
- parallel E2E shards sharing mutable database, port, lock or artifact state;
- cancellation of `main`, release or selected exact-head acceptance evidence as ordinary superseded PR work;
- purchase or adoption of larger runners without measured critical-path justification.

## 17. Definition of done

This companion plan is implemented when:

- ordinary PR commits do not create duplicate equivalent push and PR verification;
- obsolete iterative runs are cancelled while authoritative gates remain complete;
- workflow selection is machine-explainable and based on governed metadata;
- Rust cache behavior is trusted, measured and non-authoritative;
- permanent external Actions are immutably pinned;
- independent PostgreSQL process suites run in isolated shards;
- full migration correctness remains separately proven;
- phase-closure and release matrices remain complete;
- the real-device contour is private, approval-gated, isolated and ephemeral/JIT;
- larger-runner decisions are based on before/after critical-path evidence;
- every accepted packet still has complete applicable checks on one unchanged head SHA.

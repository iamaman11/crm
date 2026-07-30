# Affected-Scope CI and Local Iteration

Status: **Repository step 9 implementation — deterministic multi-plane selection**

Affected-scope checks reduce ordinary iteration cost while preserving the unchanged exact-head acceptance contract. They do not replace permanent domain, process, database, phase-closure, nightly or release evidence.

## Commands

```bash
python scripts/repo.py affected --base origin/main
python scripts/repo.py affected --base origin/main --json
python scripts/repo.py check-affected --base origin/main
```

`affected` reports:

- directly changed paths;
- directly affected workspace packages;
- transitive reverse-dependent Rust packages;
- selected non-Rust repository scopes and their authoritative owners;
- required pull-request workflows and the exact matching or broadening reason;
- safely skipped workflows and why their governed path filters do not match;
- uncertainty that widened selection to the full workspace and every permanent pull-request workflow; known workflow under-coverage fails closed.

`check-affected` first runs the permanent structural conformance preflight and formatting check. It then runs Clippy and tests for the affected Rust package closure. Shared workspace/check-graph changes and unknown impact widen to workspace-wide Clippy and tests.

## Sources of truth

The analyzer reads:

- Cargo workspace metadata for Rust package ownership and dependency edges;
- `affected-scope-policy.json` for the exact repository-step-9 scope owners, path patterns and mandatory workflows;
- permanent GitHub Actions pull-request path filters for workflow selection;
- Git changed paths relative to the selected base;
- conservative broad-impact patterns for workspace, workflow, policy and shared-tooling changes.

It does not maintain a second module ID or capability-coordinate catalog.

## Authoritative repository scopes

`affected-scope-policy.json` owns exactly these categories:

- `contracts` — architecture governance;
- `protobuf_api_compatibility` — contract platform;
- `database_migrations` and `postgresql_acceptance` — data platform;
- `process_runtime_acceptance` — runtime platform;
- `product_plane` and `frontend` — product platform;
- `operations` — platform operations.

A path may select more than one scope. That is cumulative coverage, not an owner conflict: all matched scopes and all of their mandatory workflows are retained.

## Fail-closed rules

- Unknown non-package ownership widens the package and workflow selection.
- Root workspace, lockfile, workflow, affected-scope policy and shared-tooling changes widen to the full workspace and all permanent pull-request workflows.
- A selected scope whose mandatory workflow exists but is not selected by its current path filter is a configuration error and fails immediately.
- A selected scope that names a missing permanent pull-request workflow is a configuration error and fails immediately.
- A workflow without pull-request path filters remains selected.
- One-time workflows are excluded from permanent selection.
- A skipped workflow is reported only when none of the changed paths match its governed filters.
- The packet check consumes the same fail-closed analyzer, so invalid scope ownership or workflow coverage cannot be bypassed by a valid path allowlist.

## Permanent CI

`Affected Scope CI` runs on every pull request, tests the policy and analyzer, materializes a deterministic JSON report, executes the real packet check, and then runs structural, Clippy and test phases. The report is uploaded as a read-only diagnostic artifact.

Superseded runs are cancelled by pull-request number. Gate review still requires every applicable permanent workflow to pass on one unchanged candidate SHA. Affected-scope selection never weakens that rule.

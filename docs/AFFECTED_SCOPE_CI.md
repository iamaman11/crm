# Affected-Scope CI and Local Iteration

Status: Phase E analysis and execution pilot

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
- transitive reverse-dependent packages;
- required pull-request workflows and the matching paths;
- safely skipped workflows and why their governed path filters do not match;
- uncertainty that widened selection to the full workspace and all PR workflows.

`check-affected` first runs the permanent structural conformance preflight and formatting check. It then runs Clippy and tests for the affected package closure. Shared workspace/check-graph/contract changes and unknown impact widen to workspace-wide Clippy and tests.

## Sources of truth

The analyzer reads:

- Cargo workspace metadata for package ownership and dependency edges;
- permanent GitHub Actions pull-request path filters for workflow selection;
- Git changed paths relative to the selected base;
- conservative broad-impact patterns for workspace, workflow, contract and shared-policy changes.

It does not maintain a second module ID or capability-coordinate catalog.

## Fail-closed rules

- Unknown path ownership widens the package and workflow selection.
- Root workspace, lockfile, workflow, architecture-policy and contract/schema changes widen to the full workspace.
- A workflow without pull-request path filters remains selected.
- One-time workflows are excluded from permanent selection.
- A skipped workflow is reported only when none of the changed paths match its governed filters.

## CI pilot

`Affected Scope CI` runs on pull requests, publishes the complete reasoning in the job log and executes `check-affected`. Superseded runs are cancelled by PR number. The workflow is an iteration aid only; Gate review still requires every applicable permanent workflow to pass on one unchanged candidate SHA.

## Next boundary

After affected package/check selection is accepted, a separate PostgreSQL isolation pilot will shard two low-coupling real-process suites into independent database/service namespaces and measure setup versus execution time. It must not weaken the existing sequential migration lane.

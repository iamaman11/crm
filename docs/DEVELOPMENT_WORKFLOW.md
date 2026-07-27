# Ultimate CRM — Development Workflow

Status: **Normative contributor and coding-agent workflow**

This document defines how implementation is scoped, validated and merged. Use `docs/README.md` to locate task-specific guidance. Architecture invariants and published contracts always take precedence.

## 1. Unit of delivery

The default unit is a **coherent delivery packet**, not a PR per class, crate, file or mechanical step.

A packet produces one independently understandable result, such as:

- one owner capability with complete governed production path;
- one behavior-neutral structural consolidation;
- one platform runtime boundary;
- one rebuildable projection/search capability;
- one end-to-end product workflow;
- one dependency, CI or developer-experience improvement with measured evidence.

Cohesion, reviewability, rollback safety and invariant coverage—not line count—define the boundary.

## 2. Separate behavior from structure

Do not mix:

- new product behavior and crate consolidation;
- contract semantic changes and unrelated refactors;
- migration ownership changes and ordinary feature work;
- CI optimization and weakened acceptance;
- local-environment tooling and unrelated runtime changes.

A behavior-neutral structural PR must prove unchanged public contracts, routes, workers, activation, persistence behavior and applicable acceptance evidence.

## 3. Working branches and exact identity

A coherent packet uses one implementation branch. Temporary commits are allowed, but incomplete checkpoints remain clearly marked.

Do not create separate branches/PRs for formatting, lockfile refreshes, import ordering or one constructor field when they belong to the same packet.

When multiple contributors participate, overlapping code has one primary writer. Verification uses exact commit SHA, never a moving branch name.

## 4. Required architecture sequence

Unless an accepted ADR says otherwise:

```text
1. authoritative owner, invariants and exclusions
2. public contract or compatible new version
3. application commands/queries/workers and ports
4. persistence and migration ownership
5. pre-authorization semantic validation
6. owner-owned production contribution
7. exact route/worker registration and durable activation
8. focused, PostgreSQL and real-process acceptance
9. operational and documentation closure
```

Dependency direction remains:

```text
domain <- application <- adapters <- production composition <- delivery
```

## 5. Normal capability budget

An ordinary capability inside an existing owner should:

- create zero new crates;
- touch zero generic router/worker files;
- touch zero unrelated owners, migrations or workflows;
- reuse the owner application/postgres/production packages;
- extend the owner contribution rather than central composition;
- reuse generic conformance and add only owner-specific semantic tests;
- run an explainable affected closure.

A deviation requires an architecture justification and measured fan-out/change-locality impact.

## 6. Architecture checkpoints

### Checkpoint A — scope and structure

- one authoritative owner and explicit exclusions;
- dependency/source-boundary checks pass;
- public contract/version implications are identified;
- migration/storage ownership is explicit;
- exact route/worker and activation impacts are identified;
- no generic business switch or unjustified crate is introduced;
- affected scope is explainable.

### Checkpoint B — behavior

- focused domain/application tests pass;
- affected integration/PostgreSQL tests pass;
- tenant, authorization, idempotency/retry and failure behavior is covered;
- no-op, replay, conflict and crash windows are explicit where relevant;
- cross-tenant negative proof exists for changed storage/read boundaries.

### Checkpoint C — delivery

- formatting and Clippy pass for Rust scope;
- required workspace/affected tests pass;
- Contract, Governance, Database, process and frontend gates pass when applicable;
- rollback/reapply or compensation is proven;
- generated contracts/navigation are fresh when affected;
- roadmap/status/catalog/issue claims match actual behavior;
- all applicable workflows are green on one unchanged final review head.

## 7. Pull request policy

Open a PR at a coherent review boundary. Use multiple PRs only for a real boundary, for example:

- contract freeze;
- behavior-neutral consolidation;
- feature implementation;
- process-level acceptance and governance closure.

A PR description states:

- architecture result;
- owner and dependency boundaries;
- changed contracts, routes, workers, migrations and data classes;
- production path and contribution entry point;
- activation, authorization, tenant and failure behavior;
- rollback/reapply or compensation;
- before/after complexity for structural work;
- affected-scope reasoning and specialized gates;
- exact final head and acceptance evidence;
- explicit remaining scope.

Do not imply unrun checks have passed.

## 8. Commit policy

Commits are implementation tools; PRs are delivery artifacts.

Use iterative commits while working, then prefer a compact semantic history before merge. Every verification claim names the exact commit actually tested.

A new commit makes older evidence stale for checks not rerun.

## 9. Golden owner pattern

The conceptual owner layers are:

```text
manifest and ownership
published contracts
pure domain aggregates/value objects/policies
application commands/queries/workers and ports
pre-authorization validators
PostgreSQL/external adapters
module-owned production contribution
exact routes/workers and durable activation
focused/PostgreSQL/process acceptance
```

Target physical packaging is domain + application + postgres + production, with one optional real provider/process/trust boundary. Ordinary capabilities stay inside existing packages.

See `MODULE_DEVELOPMENT.md` for current scaffold limitations and target evolution.

## 10. Public contract lifecycle

Published versions are immutable.

A semantic change requires a new version, compatibility/impact report, parallel support window, consumer migration and explicit retirement gate. Deprecation includes owner, replacement, deadline, consumer inventory and removal condition.

Do not remove a supported coordinate merely because repository code no longer calls it directly.

## 11. Migration and data ownership

A migration belongs to the authoritative owner of the affected state.

- no cross-owner table edits;
- FORCE RLS and tenant context remain enforced;
- forward, rollback/schema-removal and reapply evidence is required where applicable;
- ownership transfer requires ADR, compatibility window, cutover and rollback;
- retention, legal-hold, export/deletion and restore consequences are explicit.

## 12. Test architecture

Use generic conformance for repeated platform behavior and owner suites for unique semantics.

During iteration:

```bash
python scripts/repo.py conformance
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
```

Then run all specialized gates selected by contracts, migrations, routes, workers, processes, frontend or security scope.

Affected-scope optimization changes iteration cost, not the final exact-head acceptance rule.

## 13. Local development and navigation

Use `docs/README.md` as the stable task map.

Currently implemented commands are defined by `scripts/repo.py`. Planned commands such as `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo`, `smoke`, `explain` and `packet-check` are part of issue #194 and must not be claimed as available before permanent tests exist.

The target local workflow is:

```text
doctor
→ bootstrap
→ dev-up
→ seed-demo
→ focused implementation
→ explain/affected/packet-check
→ smoke
→ exact-head gates
```

Generated active-packet and repository-map documents are reproducible navigation outputs, not sources of truth.

## 14. Multi-agent exact-SHA workflow

Default roles:

- **Architect / Implementer** — scope, architecture, primary implementation and fixes;
- **Local Integrator / Verifier** — independent exact-SHA build/test/reproduction, default `VERIFY_ONLY`;
- **GitHub CI** — final exact-head authority.

```text
planning
→ one primary writer
→ exact-SHA local handoff when useful
→ structured report
→ fixes
→ final unchanged review head
→ all applicable GitHub workflows
→ merge
```

Do not run hidden concurrent writers on overlapping code. A verifier writes only through explicit authorization or handoff.

## 15. Documentation closure

README and `AGENTS.md` are orientation. `docs/README.md` is the stable index. Current state belongs in `PROJECT_STATUS.md`, the roadmap/phase plan, module catalog and active issues.

Historical packet documents retain accepted boundaries and must not be edited into live status trackers.

Documentation changes invalidate exact-head evidence until applicable checks rerun.

## 16. Non-negotiable gates

Faster delivery never weakens:

- single owner per mutable aggregate;
- immutable versioned contracts;
- live authorization before side effects;
- tenant isolation and cross-tenant negative tests;
- idempotency, concurrency and crash recovery;
- atomic state/outbox/audit/idempotency evidence;
- rebuildable derived state;
- safe disable, upgrade, rollback and uninstall;
- module-owned routes/workers without central business switches;
- manifest/binding/route parity;
- exact-head acceptance.

The process is optimized for fewer decisions, faster feedback and local changes—not fewer correctness guarantees.

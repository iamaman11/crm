# Ultimate CRM — Development Workflow

Status: **Normative contributor and coding-agent workflow**

This document defines how implementation is scoped, validated and merged. Use `docs/README.md` for task navigation, `docs/ACTIVE_PACKET.md` for generated packet orientation and `docs/PROJECT_STATUS.md` for merged state. Architecture invariants and published contracts always take precedence.

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

## 2. Declare and explain the packet

Every bounded packet should have an explicit owner, objective, baseline, allowed paths, forbidden paths, acceptance and non-goals. When `repository-packet.json` is present, it is the machine-readable packet declaration and `docs/ACTIVE_PACKET.md` is generated orientation only.

Use:

```bash
python scripts/repo.py explain <module-or-coordinate>
python scripts/repo.py packet-check --base origin/main
python scripts/generate_repository_navigation.py --check
```

`explain` identifies exact owner, manifest, contract and route classification. `packet-check` fails closed on baseline mismatch, disallowed changed paths, inconsistent affected closure, missing required workflows or stale generated navigation.

## 3. Separate behavior from structure

Do not mix:

- new product behavior and crate consolidation;
- contract semantic changes and unrelated refactors;
- migration ownership changes and ordinary feature work;
- CI optimization and weakened acceptance;
- local-environment tooling and unrelated runtime changes.

A behavior-neutral structural PR must prove unchanged public contracts, routes, workers, activation, persistence behavior and applicable acceptance evidence.

## 4. Working branches and exact identity

A coherent packet uses one implementation branch. Temporary commits are allowed, but incomplete checkpoints remain clearly marked.

Do not create separate branches or PRs for formatting, lockfile refreshes, import ordering or one constructor field when they belong to the same packet.

When multiple contributors participate, overlapping code has one primary writer. Verification uses exact commit SHA, never a moving branch name.

## 5. Required architecture sequence

Unless an accepted ADR says otherwise:

```text
1. authoritative owner, invariants and exclusions
2. public contract or compatible new version
3. application commands/queries/workers and ports
4. persistence and migration ownership
5. pre-authorization semantic validation
6. module-owned production contribution
7. exact route/worker registration and durable activation
8. focused, PostgreSQL and real-process acceptance
9. operational and documentation closure
```

Dependency direction remains:

```text
domain <- application <- adapters <- production composition <- delivery
```

## 6. Normal capability budget

An ordinary capability inside an existing owner should:

- create zero new crates;
- touch zero generic router or worker algorithms;
- touch zero unrelated owners, migrations or workflows;
- reuse the owner application/postgres/production packages;
- extend the module-owned production contribution;
- reuse generic conformance and add only owner-specific semantic tests;
- run an explainable affected closure.

A deviation requires architecture justification and measured fan-out/change-locality impact.

## 7. Architecture checkpoints

### Checkpoint A — scope and structure

- exact owner and coordinate resolve through `repo.py explain`;
- explicit exclusions and packet path policy exist;
- dependency/source-boundary checks pass;
- contract/version implications are identified;
- migration/storage ownership is explicit;
- route/worker and activation impacts are identified;
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
- generated contracts and navigation are fresh;
- roadmap/status/catalog/issue claims match merged behavior;
- all applicable workflows are green on one unchanged final review head.

## 8. Pull request policy

Open a PR at a coherent review boundary. A PR description states:

- architecture result;
- owner and dependency boundaries;
- changed contracts, routes, workers, migrations and data classes;
- production path and contribution entry point;
- activation, authorization, tenant and failure behavior;
- rollback/reapply or compensation;
- before/after complexity for structural work;
- affected-scope and packet-check reasoning;
- specialized gates;
- exact final head and acceptance evidence;
- explicit remaining scope.

Do not imply unrun checks have passed. Keep draft status while generated outputs, documentation or exact-head acceptance remain incomplete.

## 9. Commit policy

Commits are implementation tools; PRs are delivery artifacts. Use iterative commits while working, then prefer compact semantic history before merge.

Every verification claim names the exact commit actually tested. A new commit makes older evidence stale for checks not rerun. Bot-authored generated-sync commits must be followed by a meaningful source-authored commit before final exact-head acceptance when repository policy requires human-authored review identity.

## 10. Golden owner pattern

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

## 11. Contract lifecycle

Published versions are immutable. A semantic change requires a new version, compatibility/impact report, parallel support window, consumer migration and explicit retirement gate.

Do not remove a supported coordinate merely because repository code no longer calls it directly. Generated contract registries must be regenerated through repository commands, never edited manually.

## 12. Migration and data ownership

A migration belongs to the authoritative owner of affected state.

- no cross-owner table edits;
- FORCE RLS and tenant context remain enforced;
- forward, rollback/schema-removal and reapply evidence is required where applicable;
- ownership transfer requires ADR, compatibility window, cutover and rollback;
- retention, legal-hold, export/deletion and restore consequences are explicit.

## 13. Test architecture

Use generic conformance for repeated platform behavior and owner suites for unique semantics.

During iteration:

```bash
python scripts/repo.py conformance
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
python scripts/repo.py packet-check --base origin/main
```

Then run every specialized gate selected by contracts, migrations, routes, workers, processes, frontend, security or operations scope. Affected-scope optimization changes iteration cost, not final exact-head acceptance.

## 14. Navigation and local development

Implemented navigation commands are defined by `scripts/repo.py` and permanently tested:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
python scripts/repo.py packet-check --base origin/main
python scripts/generate_repository_navigation.py --check
```

Generated active-packet and repository-map documents are reproducible navigation outputs, not sources of truth.

The deterministic local lifecycle starts with the implemented Step 18 commands:

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

Use:

```bash
python scripts/repo.py doctor
python scripts/repo.py doctor --profile bootstrap
python scripts/repo.py bootstrap --dry-run
python scripts/repo.py bootstrap
python scripts/repo.py dev-up --dry-run
python scripts/repo.py dev-up
python scripts/repo.py dev-reset --dry-run
python scripts/repo.py dev-reset
python scripts/repo.py seed-demo --dry-run
python scripts/repo.py seed-demo
python scripts/repo.py smoke --dry-run
python scripts/repo.py smoke
```

`doctor` reads tool requirements from committed repository configuration and changes nothing. The bootstrap profile validates dependency-preparation prerequisites without requiring Docker; the full profile additionally checks Docker CLI, Compose v2 and daemon availability. `bootstrap` creates an isolated `.venv`, installs committed Python constraints, uses locked Cargo and frozen pnpm dependency resolution, and verifies generated navigation. It does not silently install or globally switch system toolchains.

`dev-up` creates or reuses the exact checkout-owned PostgreSQL dependency plane from pinned repository migrations and fixtures. It validates ownership, image, port, volume and schema digest before reuse. `dev-reset` first validates immutable ownership labels, removes the container before the volume, and recreates the current clean database; dry-run performs inspection only. Neither command starts product processes or seeds a demo scenario.

`seed-demo` starts the real production composition through one locked Rust process target, applies the accepted Party production-adapter fixture, and creates or idempotently replays the versioned `local-demo-acme` organization only through `parties.party.create`. `smoke` starts a fresh real `crm-api` process, proves the authenticated Party query is denied without a live grant, then verifies the explicit bootstrap-granted read, missing-authentication denial and tenant-B non-disclosure. `kill_on_drop` plus graceful SIGINT cleanup prevents orphan local API processes.

This bounded packet completes the Repository Step 18 command surface. Step 18 remains in progress until merge and a separate exact evidence synchronization; frontend/browser acceptance and Repository Step 19 remain blocked.

## 15. Multi-agent exact-SHA workflow

Default roles:

- **Architect / Implementer** — scope, architecture, primary implementation and fixes;
- **Local Integrator / Verifier** — independent exact-SHA build/test/reproduction, default `VERIFY_ONLY`;
- **GitHub CI** — final exact-head authority.

Do not run hidden concurrent writers on overlapping code. A verifier writes only through explicit authorization or handoff.

## 16. Documentation closure

README and `AGENTS.md` are orientation. `docs/README.md` is the stable index. Current merged state belongs in `PROJECT_STATUS.md`, roadmap/phase plan, module catalog and active issues.

Historical packet documents retain accepted boundaries and must not become live trackers. Generated navigation must be deterministic and freshness-checked. Documentation changes invalidate exact-head evidence until applicable checks rerun.

## 17. Non-negotiable gates

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

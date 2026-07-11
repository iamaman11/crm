# Ultimate CRM — Development Workflow

Status: **Normative contributor and coding-agent workflow**

This document defines how implementation work is grouped, validated and merged. It complements `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `IMPLEMENTATION_ROADMAP.md` and `AGENTS.md`.

## 1. Unit of delivery

The default unit of delivery is a **coherent delivery packet**, not a pull request per class, crate, file or mechanical change.

A delivery packet must produce one independently understandable architecture result, for example:

- complete event-driven runtime support for one governed integration;
- a deployable application composition root;
- one bounded-context module with domain, contracts, capabilities, queries, persistence and acceptance evidence;
- one rebuildable projection/search capability;
- one complete frontend/backend vertical user workflow.

Line count is not the primary limit. Cohesion, reviewability, rollback safety and invariant coverage are the limits.

## 2. Working branches

A delivery packet uses one long-lived implementation branch.

Example:

```text
develop/phase6-runtime-completion
```

Inside the working branch:

- ordinary incremental and temporary commits are allowed;
- incomplete checkpoints must remain clearly marked and must not be merged;
- contributors may refactor adjacent code needed to preserve clean boundaries;
- documentation is synchronized at meaningful milestones rather than after every tiny edit;
- the final review history is reduced to a small number of semantic commits.

Do not create a new branch or pull request for formatting, lockfile refreshes, import ordering, a single constructor field or another mechanical sub-step.

## 3. Required architecture sequence

Implement each packet in this order unless an accepted ADR says otherwise:

```text
1. ownership and invariants
2. public contracts
3. application ports/use cases
4. infrastructure adapters
5. composition
6. acceptance tests
7. operational and documentation closure
```

The dependency direction remains:

```text
domain <- application <- adapters <- composition root
```

Business owner modules never import transport, PostgreSQL, brokers, secret stores, arbitrary HTTP clients, LLM providers or another business module's internals.

## 4. Checkpoints

Full repository CI is required at coherent checkpoints, not after every edit.

### Checkpoint A — architecture

- dependency and source-boundary checks pass;
- affected crates compile;
- published contracts and module manifests are internally consistent;
- no forbidden cross-module or infrastructure dependency is introduced.

### Checkpoint B — behavior

- focused domain/unit tests pass;
- affected integration tests pass;
- retry, idempotency, tenant and authorization behavior is covered;
- negative and failure paths are explicit.

### Checkpoint C — delivery

- `cargo fmt --all --check` passes;
- Clippy passes with warnings denied;
- workspace tests pass;
- Contract and Governance CI pass when applicable;
- Database CI passes for SQL, runtime, composition or PostgreSQL behavior;
- roadmap/status/catalog changes match actual merged behavior.

## 5. Pull request policy

Open a pull request when the packet has a coherent review boundary. A packet may use two or three pull requests only when there is a real architecture boundary, such as:

- reusable platform runtime;
- deployable application composition;
- complete process-level acceptance and closure.

A pull request description must state:

- the architecture result;
- ownership and dependency boundaries;
- exact production path;
- failure and rollback behavior;
- acceptance evidence;
- remaining scope not claimed by the PR.

## 6. Commit policy

Commits are internal working tools. Pull requests are delivery artifacts.

During implementation, commits may be small and iterative. Before merge, prefer a compact semantic history such as:

```text
1. add governed runtime contracts and state model
2. compose production adapters and process path
3. add acceptance evidence and synchronize documentation
```

Do not spend implementation time manufacturing a clean commit for every mechanical fix. Clean the history once the packet is ready for review.

## 7. Golden module pattern

Every business owner module should converge on the same conceptual layers:

```text
module manifest
published contracts
domain aggregates and value objects
application commands/queries and ports
capability/query adapters
persistence and external adapters
composition registration
unit/integration/PostgreSQL acceptance
```

Physical crate boundaries may vary, but ownership and dependency direction may not.

The repository should provide permanent scaffolding and validation commands tracked by issue #56 so future modules are generated from the proven pattern rather than copied manually.

## 8. Domain-wave development

After the first production runtime proof, build CRM breadth as domain waves tracked by issue #57:

1. Customer 360;
2. Revenue lifecycle;
3. Service and support;
4. Growth and marketing;
5. Product platform, analytics and automation.

Each wave includes backend ownership, contracts, persistence, projections, frontend surface and acceptance evidence. Backend and frontend evolve together after the product-shell foundation exists.

## 9. Non-negotiable gates

Faster delivery must never weaken:

- single owner per mutable aggregate;
- versioned capability/event/query contracts;
- live authorization before side effects;
- tenant isolation and cross-tenant negative tests;
- idempotency and optimistic concurrency;
- atomic state, outbox, audit and idempotency evidence;
- rebuildable non-authoritative projections;
- safe disable, upgrade, rollback and uninstall behavior;
- exact typed money, time, identity and lifecycle semantics.

The process is optimized for fewer coordination steps, not fewer correctness guarantees.

# Ultimate CRM — Agent and Contributor Operating Guide

This file is the default operating guide for humans and coding agents working in this repository.

## 1. Start with the smallest correct context

Before changing code:

1. Read `docs/README.md` and choose the path matching the task.
2. Read `docs/SYSTEM_INVARIANTS.md`.
3. Read `docs/PROJECT_STATUS.md`, `docs/ACTIVE_PACKET.md` and the active GitHub issue.
4. Use `python scripts/repo.py explain <module-or-coordinate>` to locate the authoritative owner and published coordinate.
5. Inspect only the contracts, manifest, implementation and tests selected by that path.
6. Run `python scripts/repo.py packet-check --base origin/main` before claiming packet closure.

For architecture-sensitive work also read:

- `docs/ARCHITECTURE_READINESS.md`;
- `docs/APPLICATION_ARCHITECTURE.md`;
- `docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`;
- relevant accepted ADRs under `docs/adr/`.

For delivery and module work read:

- `docs/DEVELOPMENT_WORKFLOW.md`;
- `docs/MODULE_DEVELOPMENT.md`;
- `docs/MULTI_AGENT_DEVELOPMENT.md` when more than one agent participates;
- `docs/CODEX_AGENT_QUALIFICATION.md` when a local Codex agent is used.

When descriptive documents disagree, precedence is:

`SYSTEM_INVARIANTS` → published contracts and accepted ADRs → architecture documents → normative execution plans → `PROJECT_STATUS` → active issue/PR → generated/orientation documents.

Generated `ACTIVE_PACKET` and repository-map files are navigation outputs, not sources of runtime or delivery truth. Completion requires merged implementation plus applicable exact-head acceptance evidence.

## 2. Repository concepts

- **Business module:** independently governed owner or link module under `modules/`; not necessarily a microservice.
- **Owner module:** the single authority for one mutable business domain.
- **Link module:** optional cross-domain coordination over published events/capabilities with its own private coordination state.
- **Platform crate:** reusable technical component under `crates/`; not counted as a business module.
- **Service:** deployable composition root; owns no business domain state.
- **Capability:** the supported state-changing business entry point.
- **Query:** permission-bound read path separate from mutation semantics.
- **Event:** immutable versioned integration evidence; consumers are idempotent.
- **Projection/read model:** rebuildable non-authoritative state.
- **Delivery packet:** one coherent architecture result with explicit ownership, acceptance and rollback/failure behavior.
- **Verification checkpoint:** immutable commit SHA plus a defined check set.

## 3. Non-negotiable dependency direction

```text
transport/service
  → ingress
    → generic application runtime
      → owner-owned production contribution
        → application use cases and ports
          → pure owner-domain module
            → stable contracts and governed SDK ports

infrastructure adapters
  → stable application/runtime ports
  → PostgreSQL / external systems
```

Forbidden shortcuts:

- business module → PostgreSQL, SQLx, broker, arbitrary HTTP, secrets or provider SDK;
- business module → another business module's Rust types, repositories or storage;
- public transport → domain or persistence implementation;
- query path → mutation idempotency/business-transaction semantics;
- projection/search/cache → authoritative business ownership;
- generic router/worker → switch on business capability, query, worker or module identifiers.

Do not weaken an architecture gate to make a feature pass.

## 4. Cost-of-change rule

A normal capability added to an existing owner:

- creates **zero new crates**;
- adds an internal command, query or worker to the existing application package;
- reuses existing adapters unless a real boundary changes;
- extends the module-owned production contribution;
- changes no generic router or worker algorithm;
- adds owner-specific tests and reuses generic conformance;
- runs the explainable affected closure plus required specialized gates.

A new crate is allowed only for a real dependency, trust, reuse, process, lifecycle or extraction boundary. A handler, planner, re-export or capability-specific composition function is not sufficient justification.

Feature behavior and crate consolidation must be separate PRs.

## 5. How to add or change a business feature

1. Resolve the exact owner and coordinate with `repo.py explain`.
2. Identify explicit out-of-scope domains.
3. Inspect the owner manifest, public contracts and existing application/postgres/production packages.
4. Change typed domain invariants without infrastructure access.
5. Evolve public contracts only when public meaning changes; preserve published versions.
6. Add application commands/queries/workers and stable ports.
7. Add persisted-state conversion separately from public wire contracts.
8. Add deterministic adapters and pre-authorization semantic validators.
9. Extend the module-owned production contribution.
10. Use durable tenant module-installation state; never add an activation bypass.
11. Add tenant, authorization, idempotency/retry, rollback and negative acceptance as applicable.
12. Update route classifications, generated registries and status/catalog sources when their scope changes.
13. Run `repo.py affected`, `check-affected`, `packet-check` and every selected specialized gate.

Do not start from a controller, table or UI component and invent ownership afterward.

## 6. Cross-domain behavior

Prefer an optional link module:

```text
source owner mutation
  → transactional outbox event
    → governed event delivery
      → link-owned deterministic coordination state
        → governed CapabilityClient
          → target owner capability
```

The link must be independently installable, disableable and uninstallable. It cannot read or mutate source/target storage directly.

## 7. Governed execution

Mutation:

```text
authentication
→ tenant/actor context
→ exact owner/capability/version route
→ durable activation
→ typed and pre-authorization semantic validation
→ rate/approval policy
→ final live authorization
→ deterministic planning and one governed transaction
```

Query:

```text
authentication
→ tenant/actor context
→ exact owner/query/version route
→ durable activation
→ typed and pre-authorization semantic validation
→ live authorization/visibility
→ authoritative tenant-scoped read and masking
```

Public transport must not contain domain decisions or persistence calls. Mutation-only idempotency or business-transaction fields never leak into query contracts.

## 8. Composition, persistence and Rust API rules

`services/crm-api` is the production composition root. Business functionality enters through exact module-owned contribution sets. Generic registries validate duplicate coordinates, owner/kind mismatch, missing handlers and deterministic worker phases.

Persistence rules:

- every owner has explicit storage namespaces and migration ownership;
- no cross-owner table edits without an accepted ownership-transfer ADR;
- tenant context and FORCE RLS remain database-enforced;
- forward, rollback/schema-removal, reapply and cross-tenant negative evidence are required when applicable;
- persisted envelopes are versioned independently from public wire contracts;
- destructive changes require retention, legal-hold, audit and recovery implications to be explicit.

Rust API rules:

- default implementation visibility is private or `pub(crate)`;
- cross-package APIs are limited to published contracts, stable ports and contribution interfaces;
- concrete adapters and repositories are not re-exported for convenience;
- shared DTOs do not replace owner-domain value objects;
- a new public symbol requires a consumer and compatibility rationale.

## 9. Repository navigation and commands

Implemented navigation:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
python scripts/repo.py packet-check --base origin/main
python scripts/generate_repository_navigation.py --check
```

Use `docs/generated/REPOSITORY_MAP.md` for deterministic workspace/module/route inventory. Unknown or ambiguous explain targets must fail closed.

Implemented validation and local lifecycle entry points include:

```bash
python scripts/repo.py doctor
python scripts/repo.py doctor --profile bootstrap
python scripts/repo.py bootstrap --dry-run
python scripts/repo.py bootstrap
python scripts/repo.py dev-up --dry-run
python scripts/repo.py dev-up
python scripts/repo.py dev-reset --dry-run
python scripts/repo.py dev-reset
python scripts/repo.py conformance
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
python scripts/repo.py packet-check --base origin/main
python scripts/repo.py quality
```

`doctor` reads repository-pinned tool requirements and never changes the machine. The bootstrap profile excludes Docker so dependency preparation can be diagnosed independently; the full profile checks Docker CLI, Compose v2 and daemon availability. `bootstrap` uses an isolated `.venv`, committed constraints and lockfiles, and does not silently install or globally switch system toolchains.

`dev-up` and `dev-reset` manage only the checkout-scoped PostgreSQL dependency plane. They use immutable image and schema-input digests, loopback-only publishing and ownership labels. Never manually relabel a lifecycle resource; when exact configuration or schema inputs change, inspect `dev-reset --dry-run` and reset the owned state.

Repository Step 18 remains in progress. `seed-demo` and `smoke` are not yet implemented or accepted; backend/frontend process startup remains outside this packet.

## 10. Required checks before completion

Run the checks selected by `packet-check`, then every applicable specialized gate:

- Contract CI for Protobuf or contract changes;
- Governance CI for manifests, normalized IR, architecture policy and repository governance;
- Rust CI for architecture, lockfile, formatting, Clippy and workspace tests;
- Database CI for SQL, migrations, persistence and PostgreSQL behavior;
- process/runtime/frontend/security/operations gates when their scopes are affected.

A packet is complete only when applicable workflows are green on one unchanged exact review head and documentation reflects merged behavior.

## 11. Multi-agent exact-SHA rule

```text
Architect / Implementer
  = scope, architecture, primary implementation and fixes

Local Integrator / Verifier
  = exact-SHA checkout, independent build/test and structured report

GitHub CI
  = final exact-head merge authority
```

Rules:

1. Overlapping code has one primary writer at a time.
2. Verification defaults to `VERIFY_ONLY` until an explicit handoff grants write authority.
3. Every handoff and report names the exact SHA, mode, affected scope and commands.
4. A new commit invalidates earlier evidence for checks not rerun.
5. Local green status accelerates feedback but never replaces applicable GitHub gates.
6. Do not rely on chat-only state when it can be committed or recorded in the issue/PR.

## 12. Change discipline

- Keep PRs coherent and reviewable.
- Do not mix unrelated refactors, consolidation and product behavior.
- Prefer measured incremental structural changes over a big-bang rewrite.
- Preserve compatibility, rollback and operational semantics.
- Treat stale documentation or generated navigation as a defect.
- Record temporary architecture exceptions with owner, reason, expiry, removal condition and compensating checks.
- Do not claim product completion while required owner domains or production gates remain open.

See `docs/PROJECT_STATUS.md` and active GitHub issues for the live product and architecture packets. Orientation documents must not independently change phase bookkeeping.

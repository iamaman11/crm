# Ultimate CRM — Agent and Contributor Operating Guide

This file is the default operating guide for humans and coding agents working in this repository.

## 1. Start with the smallest correct context

Before changing code:

1. Read `docs/README.md` and choose the path matching the task.
2. Read `docs/SYSTEM_INVARIANTS.md`.
3. Read `docs/PROJECT_STATUS.md` and the active GitHub issue.
4. Read only the task-specific architecture/workflow documents identified by the navigation index.
5. Inspect the authoritative contracts, owner manifest, implementation and affected tests before proposing changes.

For architecture-sensitive work also read:

- `docs/ARCHITECTURE_READINESS.md` — accepted native-composition non-regression baseline;
- `docs/APPLICATION_ARCHITECTURE.md` — layer and composition boundaries;
- `docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` — tracked 10/10 architecture and developer-experience program;
- relevant accepted ADRs under `docs/adr/`.

For delivery and module work read:

- `docs/DEVELOPMENT_WORKFLOW.md`;
- `docs/MODULE_DEVELOPMENT.md`;
- `docs/MULTI_AGENT_DEVELOPMENT.md` when more than one agent participates;
- `docs/CODEX_AGENT_QUALIFICATION.md` when a local Codex agent is used.

When descriptive documents disagree, the precedence is:

`SYSTEM_INVARIANTS` → published contracts and accepted ADRs → `ARCHITECTURE_READINESS` / `APPLICATION_ARCHITECTURE` → normative execution plans → `PROJECT_STATUS` → active issue/PR → orientation documents.

Do not infer completion from a directory name, manifest declaration or historical packet. Completion requires merged implementation plus applicable exact-head acceptance evidence.

## 2. Repository concepts

- **Business module:** independently governed owner or link module under `modules/`; not necessarily a microservice.
- **Owner module:** the single authority for one mutable business domain.
- **Link module:** optional cross-domain coordination over published events/capabilities with its own private deduplication/configuration state.
- **Platform crate:** reusable technical component under `crates/`; not counted as a business module.
- **Service:** deployable process under `services/`; composes governed functionality but owns no business domain state.
- **Capability:** the only supported state-changing business entry point.
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

The intended normal change is local to one authoritative owner.

A normal capability added to an existing owner:

- creates **zero new crates**;
- adds an internal command, query or worker to the existing application package;
- reuses the existing persistence/external adapters unless a real boundary changes;
- extends the existing owner-owned production contribution;
- changes no generic router or worker algorithm;
- adds owner-specific tests and reuses generic conformance;
- runs the explainable affected closure plus required specialized gates.

A new crate is allowed only for a real dependency, trust, reuse, process, lifecycle or extraction boundary. A handler, planner, thin re-export or capability-specific composition function is not sufficient justification.

Feature behavior and crate consolidation must be separate PRs.

## 5. How to add or change a business feature

Use the smallest vertical slice that preserves ownership:

1. Identify the single authoritative owner and explicit out-of-scope domains.
2. Inspect the owner manifest, public contracts and existing application/postgres/production packages.
3. Change typed domain invariants without infrastructure access.
4. Evolve Protobuf or other public contracts only when the public meaning changes; preserve published versions.
5. Add application commands/queries/workers and stable ports.
6. Add persisted-state conversion separately from public wire contracts.
7. Add deterministic adapters and pre-authorization semantic validators.
8. Extend the module-owned production contribution.
9. Use durable tenant module-installation state; never add an activation bypass.
10. Add tenant, authorization, idempotency/retry, rollback and negative acceptance as applicable.
11. Keep projections/search rebuildable and non-authoritative.
12. Update route classifications, generated registries and status/catalog sources when their scope changes.
13. Run `repo.py affected` / `check-affected` and every specialized gate selected by the change.

Do not start from a controller, table or UI component and invent ownership afterward.

## 6. How to add cross-domain behavior

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

## 7. Public endpoints and governed execution

A public endpoint terminates at governed ingress. Transport may authenticate, build tenant/actor context and map safe errors; it must not contain domain decisions or persistence calls.

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

Never share mutation-only idempotency or business-transaction fields with query contracts.

## 8. Application composition rule

`services/crm-api` is the production composition root. It owns process configuration, infrastructure construction, durable module activation, contribution aggregation, ingress, health, observability and shutdown.

Business functionality enters through exact module-owned contribution sets. Generic registries validate duplicate coordinates, owner/kind mismatch, missing handlers and deterministic worker phases. An ordinary capability must not add a direct domain dependency or dispatch branch to the generic application runtime.

## 9. Persistence and migration ownership

- Every owner has explicit storage namespaces and migration ownership.
- A migration may not modify another owner's authoritative tables without an accepted ownership migration ADR and coordinated compatibility plan.
- Tenant context and FORCE RLS remain database-enforced.
- Forward, rollback/schema-removal, reapply and cross-tenant negative evidence are required when applicable.
- Persisted envelopes are versioned independently from public wire contracts.
- Destructive data changes require retention, legal-hold, audit and recovery implications to be explicit.

## 10. Rust API surface

- Default implementation visibility is `pub(crate)` or private.
- Cross-package APIs are limited to published contracts, stable ports and production contribution interfaces.
- Concrete adapters and repositories are not re-exported as convenience APIs.
- Shared DTOs do not replace owner-domain value objects.
- A new public Rust symbol requires a consumer and compatibility rationale.

## 11. Required checks before completion

Start with:

```bash
python scripts/repo.py conformance
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
```

Then run every applicable specialized gate:

- Contract CI for Protobuf or contract changes;
- Governance CI for manifests, normalized IR, architecture policy and repository governance;
- Rust CI for architecture, lockfile, formatting, Clippy and workspace tests;
- Database CI for SQL, migrations, persistence and PostgreSQL behavior;
- process/runtime/frontend/security/operations gates when their scopes are affected.

A packet is complete only when applicable workflows are green on one unchanged exact review head and documentation reflects merged behavior.

## 12. Multi-agent exact-SHA rule

When a second agent participates, follow `docs/MULTI_AGENT_DEVELOPMENT.md`.

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

Useful coordination signals remain `SECOND_AGENT_NOT_NEEDED`, `CONNECT_SECOND_AGENT`, `SECOND_AGENT_REPORT_NEEDED` and `READY_FOR_EXACT_HEAD_CI`.

## 13. Navigation and local environment

Use `docs/README.md` as the stable task index. Live state remains in `PROJECT_STATUS.md` and active issues.

Currently implemented repository commands are listed in the root README and `scripts/repo.py`. Commands such as `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo`, `smoke`, `explain` and `packet-check` are planned under issue #194 and must not be represented as available until implemented and permanently tested.

Generated `ACTIVE_PACKET` and repository-map files are navigation outputs, not new sources of truth.

## 14. Change discipline

- Keep PRs coherent and reviewable.
- Do not mix unrelated refactors, consolidation and product behavior.
- Prefer measured incremental structural changes over a big-bang rewrite.
- Preserve compatibility, rollback and operational semantics.
- Treat stale documentation as a defect.
- Record temporary architecture exceptions with owner, reason, expiry, removal condition and compensating checks.
- Do not claim product completion while required owner domains or production gates remain open.

## 15. Current next step

See `docs/PROJECT_STATUS.md` and the active GitHub issues for the live product and architecture packets. Orientation documents must not independently change phase bookkeeping.

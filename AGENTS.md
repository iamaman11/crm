# Ultimate CRM — Agent and Contributor Operating Guide

This file is the default orientation guide for humans and coding agents working in this repository.

## 1. Read this first

Before changing code, read these sources in order:

1. `docs/SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `docs/ARCHITECTURE_READINESS.md` — accepted native-composition non-regression baseline.
3. `docs/IMPLEMENTATION_ROADMAP.md` — normative delivery sequence and current phase.
4. `docs/PROJECT_STATUS.md` — concise current state and next executable steps.
5. `docs/APPLICATION_ARCHITECTURE.md` — layer model, dependency direction and composition boundaries.
6. `docs/MODULE_CATALOG.md` — what counts as a module and which business domains exist or are planned.
7. `docs/DEVELOPMENT_WORKFLOW.md` — coherent delivery packets, checkpoints, PR and commit policy.
8. `docs/MULTI_AGENT_DEVELOPMENT.md` — exact-SHA handoff and independent local verification protocol when more than one agent participates.
9. `docs/CODEX_AGENT_QUALIFICATION.md` — persistent local checkout, responsibility levels and qualification rules for a Codex local agent when applicable.
10. `docs/MODULE_DEVELOPMENT.md` — golden owner/link module scaffolding and permanent repository commands.
11. Relevant accepted ADRs under `docs/adr/`.
12. The GitHub issue for the active slice.

When descriptive documents disagree, the precedence is:

`SYSTEM_INVARIANTS` → accepted ADRs and published contracts → `ARCHITECTURE_READINESS` → `IMPLEMENTATION_ROADMAP` → `PROJECT_STATUS` → issue text → README.

Process documents govern how work is performed but never override architecture invariants, published contracts or the accepted non-regression baseline.

Do not infer completion from a directory name, manifest declaration or old issue text. Completion requires merged implementation plus the phase acceptance gates.

## 2. Repository concepts

- **Business module**: an independently governed owner or link module under `modules/` with a manifest and lifecycle. A module is not necessarily a microservice.
- **Platform crate**: a reusable technical component under `crates/`. Platform crates are not counted as CRM business modules.
- **Service**: a deployable process under `services/` that composes governed runtime components. Services do not own business domain state.
- **Projection/read model**: rebuildable non-authoritative state. A projection is not automatically a module.
- **Capability**: the only supported state-changing business entry point.
- **Query**: a permission-bound read path that is structurally separate from mutation semantics.
- **Event**: immutable versioned integration evidence; consumers must be idempotent.
- **Delivery packet**: one coherent architecture result with explicit ownership, production path, acceptance evidence and rollback/failure behavior.
- **Verification checkpoint**: an immutable exact commit SHA plus a defined local check set; branch names alone are not verification identities.

## 3. Non-negotiable dependency direction

```text
transport/service
  → ingress
    → application runtime gateway
      → composition/adapters
        → owner-domain module
          → stable contracts and governed SDK ports

infrastructure adapters
  → stable runtime ports
  → PostgreSQL / external systems
```

Forbidden shortcuts:

- business module → PostgreSQL, SQLx, broker, arbitrary HTTP, secrets or LLM provider;
- business module → another business module's Rust types or storage;
- public transport → business module or persistence implementation;
- query path → mutation idempotency/business-transaction semantics;
- projection/search/cache → authoritative business ownership.

The architecture policy and CI must enforce these boundaries. Do not weaken a gate to make a feature pass.

## 4. How to add a business feature

Use the smallest vertical slice that preserves ownership:

1. Identify the single authoritative owner domain.
2. Update or create the module manifest only if ownership/contracts really change.
3. Add typed domain invariants with no infrastructure access.
4. Publish or evolve versioned Protobuf contracts compatibly.
5. Add persisted-state conversion separately from public wire contracts.
6. Add deterministic capability/query adapters and pre-authorization semantic validators.
7. Contribute exact versioned routes and workers through the module-owned production boundary.
8. Use durable tenant module-installation state for activation; never add a bootstrap bypass.
9. Compose through generic exact-coordinate gateway/ingress registries without business switches.
10. Add PostgreSQL and transport acceptance for tenant isolation, authorization, replay/conflict, disable/uninstall and rollback as applicable.
11. Add projection/search behavior only as rebuildable read state.
12. Update contract parity/classifications and roadmap/status/module catalog in the same PR when scope or completion changes.

Do not start from a controller, database table or UI component and then invent ownership afterward.

## 5. How to add cross-domain behavior

Prefer an optional link module:

```text
source owner event
  → governed event delivery
    → link module-owned deduplication state
      → governed CapabilityClient
        → target owner capability
```

The link module must be independently installable, disableable and uninstallable. It must not mutate source or target storage directly.

## 6. How to add a public endpoint

A public endpoint must terminate at a governed ingress boundary. Public transport code may parse transport metadata and map safe errors, but must not contain domain decisions or persistence calls.

Mutation path:

```text
authentication → tenant/actor context → exact module-owned capability/version route → durable module activation → typed and pre-authorization semantic validation → policy/approval → live authorization → transactional execution
```

Query path:

```text
authentication → tenant/actor context → exact module-owned query/version route → durable module activation → typed and pre-authorization semantic validation → live authorization/visibility → authoritative read
```

Never share mutation-only idempotency or business-transaction fields with query contracts.

## 7. Application composition rule

`services/crm-api` is the production composition root. It wires platform components, configuration and process lifecycle, but depends on business functionality through governed module-owned composition/runtime boundaries rather than owner-domain internals.

The executable service owns:

- configuration validation;
- PostgreSQL pool/runtime adapter construction;
- durable module publication/installation activation;
- exact mutation/query/visibility contribution assembly;
- deterministic activation-gated worker assembly;
- authentication and policy adapter composition;
- HTTP and gRPC server startup;
- health/readiness endpoints;
- observability and graceful shutdown.

Generic router and worker algorithms must not branch on business IDs or concrete adapters. Business rules remain outside the service. A new module may register a contribution but must not require a new central dispatch branch.

## 8. Required checks before claiming completion

Run `python scripts/repo.py conformance` as the permanent local architecture preflight, then run the applicable gates, not a hand-picked subset:

- Contract CI for Protobuf/contract changes;
- Governance CI for manifests, normalized IR, architecture policy or governed process changes;
- Rust CI for architecture, lockfile, formatting, Clippy and workspace tests;
- Database CI for SQL, migrations, persistence, production mutation/query/link/projection/search behavior;
- specialized runtime/process/frontend gates when their scopes are affected.

A phase or delivery packet is complete only when its acceptance evidence is green on the exact review head and documentation reflects the merged state.

Independent local verification may be required or useful before final CI, but local green status never replaces applicable GitHub gates.

## 9. Multi-agent exact-SHA operating rule

When a second agent participates in the same delivery packet, follow `docs/MULTI_AGENT_DEVELOPMENT.md`.

The baseline split is:

```text
Architect / Implementer
  = scope + architecture + primary implementation + fixes

Local Integrator / Verifier
  = exact-SHA checkout + full local build/test/integration + structured report

GitHub CI
  = final exact-head independent gate authority
```

This baseline is not a permanent ceiling. A ChatGPT Codex local agent is qualified under `docs/CODEX_AGENT_QUALIFICATION.md` and may be promoted per packet to Local Integrator, Co-Implementer or Delivery Packet Owner when its actual environment and demonstrated behavior support that responsibility.

Rules:

1. One primary writer owns overlapping code at a time.
2. The verifier defaults to `VERIFY_ONLY` until a handoff or qualification grants broader authority.
3. Every verification handoff names an exact SHA, mode, affected scope and required commands.
4. Every report names the exact SHA actually tested.
5. A new commit makes older green evidence stale for checks not rerun on the new SHA.
6. Architecture, contract, domain, authorization, tenant and persistence changes require an identified decision owner.
7. Mechanical verifier writes require explicit authorization; broader writes require a writer handoff or explicit non-overlapping workstream ownership.
8. A qualified local agent should keep a persistent checkout and report its real repository path, branch, HEAD and worktree state.
9. Final merge still requires applicable GitHub checks green on one exact review head.

Useful coordination signals:

- `SECOND_AGENT_NOT_NEEDED` — planning or active primary implementation;
- `CONNECT_SECOND_AGENT` — a verifier-ready exact-SHA handoff exists;
- `SECOND_AGENT_REPORT_NEEDED` — implementation is paused for the structured report;
- `READY_FOR_EXACT_HEAD_CI` — local verification requirements are satisfied for the named head.

Do not rely on chat-only instructions when the same information can be committed or recorded in the active issue/PR.

## 10. Change discipline

- Keep PRs reviewable and phase-scoped.
- Do not mix unrelated refactors with product behavior.
- Prefer additive structure over broad directory moves while the architecture is still evolving.
- Preserve compatibility and rollback semantics explicitly.
- Treat stale documentation as a defect: update the normative status sources in the same PR.
- Do not claim the complete CRM is finished while required owner domains or production gates remain open.
- Do not use a second agent as a hidden concurrent writer on overlapping scope.
- Do not transfer a green result from one SHA to another without rerunning the required check.

## 11. Current next step

See `docs/PROJECT_STATUS.md` for the live implementation packet and exact next executable steps. The roadmap/status/issues, not this orientation guide, are the authoritative place for changing phase bookkeeping.

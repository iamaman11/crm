# Ultimate CRM — Agent and Contributor Operating Guide

This file is the default orientation guide for humans and coding agents working in this repository.

## 1. Read this first

Before changing code, read these sources in order:

1. `docs/SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `docs/IMPLEMENTATION_ROADMAP.md` — normative delivery sequence and current phase.
3. `docs/PROJECT_STATUS.md` — concise current state and next executable steps.
4. `docs/APPLICATION_ARCHITECTURE.md` — layer model, dependency direction and composition boundaries.
5. `docs/MODULE_CATALOG.md` — what counts as a module and which business domains exist or are planned.
6. `docs/DEVELOPMENT_WORKFLOW.md` — coherent delivery packets, checkpoints, PR and commit policy.
7. `docs/MULTI_AGENT_DEVELOPMENT.md` — exact-SHA handoff and independent local verification protocol when more than one agent participates.
8. `docs/MODULE_DEVELOPMENT.md` — golden owner/link module scaffolding and permanent repository commands.
9. Relevant accepted ADRs under `docs/adr/`.
10. The GitHub issue for the active slice.

When descriptive documents disagree, the precedence is:

`SYSTEM_INVARIANTS` → accepted ADRs and published contracts → `IMPLEMENTATION_ROADMAP` → `PROJECT_STATUS` → issue text → README.

Process documents govern how work is performed but never override architecture invariants or published contracts.

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
6. Add deterministic capability/query adapters.
7. Compose through the governed gateway/ingress boundary.
8. Add PostgreSQL and transport acceptance for tenant isolation, authorization, replay/conflict and rollback as applicable.
9. Add projection/search behavior only as rebuildable read state.
10. Update roadmap/status/module catalog in the same PR when scope or completion changes.

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
authentication → tenant/actor context → exact capability → validation/policy → live authorization → transactional execution
```

Query path:

```text
authentication → tenant/actor context → exact query → validation → live authorization/visibility → authoritative read
```

Never share mutation-only idempotency or business-transaction fields with query contracts.

## 7. Application composition rule

`services/crm-api` is the production composition root. It may wire platform components, configuration and process lifecycle, but it must depend on business functionality through governed composition/runtime boundaries rather than importing owner-domain internals directly.

The executable service must eventually own:

- configuration validation;
- PostgreSQL pool/runtime adapter construction;
- module and capability/query catalog composition;
- authentication and policy adapter composition;
- HTTP and gRPC server startup;
- health/readiness endpoints;
- observability and graceful shutdown.

Business rules remain outside the service.

## 8. Required checks before claiming completion

Run the applicable gates, not a hand-picked subset:

- Contract CI for Protobuf/contract changes;
- Governance CI for manifests, normalized IR, architecture policy or governed process changes;
- Rust CI for architecture, lockfile, formatting, Clippy and workspace tests;
- Database CI for SQL, migrations, persistence, production mutation/query/link/projection/search behavior;
- specialized runtime/process/frontend gates when their scopes are affected.

A phase or delivery packet is complete only when its acceptance evidence is green on the exact review head and documentation reflects the merged state.

Independent local verification may be required or useful before final CI, but local green status never replaces applicable GitHub gates.

## 9. Multi-agent exact-SHA operating rule

When a second agent participates in the same delivery packet, follow `docs/MULTI_AGENT_DEVELOPMENT.md`.

Default role split:

```text
Architect / Implementer
  = scope + architecture + primary implementation + fixes

Local Integrator / Verifier
  = exact-SHA checkout + full local build/test/integration + structured report

GitHub CI
  = final exact-head independent gate authority
```

Rules:

1. One primary writer owns overlapping code at a time.
2. The verifier defaults to `VERIFY_ONLY`.
3. Every verification handoff names an exact SHA, mode, affected scope and required commands.
4. Every report names the exact SHA actually tested.
5. A new commit makes older green evidence stale for checks not rerun on the new SHA.
6. Architecture, contract, domain, authorization, tenant and persistence fixes return to the Architect / Implementer by default.
7. Mechanical verifier writes require explicit authorization; broader writes require a writer handoff.
8. Final merge still requires applicable GitHub checks green on one exact review head.

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

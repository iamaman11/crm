# Ultimate CRM Platform

Implementation repository for a universal modular, metadata-driven, production-grade CRM platform.

## Start here

For a new contributor or coding agent, read in this order:

1. [`AGENTS.md`](AGENTS.md) — repository operating model and change workflow.
2. [`docs/SYSTEM_INVARIANTS.md`](docs/SYSTEM_INVARIANTS.md) — absolute architecture rules.
3. [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md) — normative delivery sequence.
4. [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md) — concise current state and next step.
5. [`docs/APPLICATION_ARCHITECTURE.md`](docs/APPLICATION_ARCHITECTURE.md) — layer and composition skeleton.
6. [`docs/MODULE_CATALOG.md`](docs/MODULE_CATALOG.md) — business-module counting and planned owner domains.
7. [`docs/DEVELOPMENT_WORKFLOW.md`](docs/DEVELOPMENT_WORKFLOW.md) — coherent delivery packets and acceptance checkpoints.
8. [`docs/MULTI_AGENT_DEVELOPMENT.md`](docs/MULTI_AGENT_DEVELOPMENT.md) — exact-SHA two-agent implementation and local verification protocol.

Accepted ADRs and published contracts take precedence over descriptive prose unless they violate an absolute system invariant.

## Current state

**Phase 6 is complete** and Phase 7 is in progress.

The repository contains a production-composed modular CRM platform proof with:

- governed module manifests, SDK and lifecycle;
- PostgreSQL tenant/RLS, transaction, idempotency, outbox and audit foundations;
- authenticated capability mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task production vertical slices;
- governed event delivery and an independently lifecycle-managed Sales–Activities link module;
- rebuildable projections and a generalized projection runtime;
- a real `crm-api` application composition root with HTTP/gRPC ingress, health/readiness, background workers and graceful shutdown;
- permission-aware tenant-scoped global search with deterministic logical index generations and live visibility re-checking;
- golden module scaffolding and permanent repository validation commands.

The next product-plane packet is the typed web product shell: generated client boundary, authentication/session integration, permission-aware routing and the design-system baseline, followed by Admin Studio foundations and expert domain waves.

The complete CRM product is not finished. Customer master and identity resolution, consent, catalog/pricing/CPQ/commercial lifecycle, communications, service, marketing, broader expert domains, AI, marketplace and enterprise operational proof remain roadmap work.

See [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md) for the exact current state.

## Architectural model

The target is a **modular monolith with independently governed owner and link modules**, not a collection of accidental microservices.

Core invariants include:

- every mutable aggregate has one authoritative owner;
- actors mutate state only through versioned governed capabilities;
- business modules do not receive direct database, broker, object-storage, arbitrary HTTP, secret-store or LLM-provider clients;
- business modules do not import or mutate another business module's internals;
- cross-domain behavior uses versioned capabilities/events and optional link modules;
- state mutations atomically persist required idempotency, outbox and audit evidence;
- queries are permission-bound and structurally separate from mutation semantics;
- search, analytics, caches, timelines and projections are rebuildable and non-authoritative;
- live authorization runs immediately before side effects;
- AI and marketplace extensions have no alternate mutation or data-access path.

The complete normative rules are in [`docs/SYSTEM_INVARIANTS.md`](docs/SYSTEM_INVARIANTS.md).

## Authoritative specifications

The repository implements the following architecture documents, in precedence order:

1. [v2.2 Architecture Closure & Contract Specification](https://docs.google.com/document/d/1xUl7oGh3nrMzJ332mxtoZqRch_0O6wyj_wfUFGYnPrU/edit)
2. [v2.1 Implementation Readiness Addendum](https://docs.google.com/document/d/1fgCls9uumH_V0hMh0_aUEvvNWCIAsvRQSB0n-M5Ih0U/edit)
3. [v2.0 Production-Grade Architecture Blueprint](https://docs.google.com/document/d/1UF-VfjP6hpPr3qWh-b0djQErdWN8kkL9IDMCQVcHXmc/edit)

Repository invariants, accepted ADRs and compilable published contracts are the executable interpretation of those specifications.

## Repository layout

- `proto/` — authoritative RPC, command and event contract sources.
- `crates/` — platform core, governed runtimes and infrastructure adapters.
- `modules/` — independently governed business owner/link modules without raw infrastructure access.
- `services/` — deployable composition roots; `services/crm-api` is the production application process.
- `database/` — authoritative migrations and PostgreSQL acceptance assets.
- `schemas/` — strict authoring schemas compiled into typed runtime IR.
- `docs/adr/` — accepted architecture decisions.
- `scripts/` — architecture, contract and manifest enforcement.
- `.github/workflows/` — permanent conformance and acceptance gates.

The product plane is introduced only through an explicit Phase 7 delivery packet and remains constrained to governed mutation/query boundaries.

Generated `build/` content and workflow artifacts are reproducible outputs and are not authoritative source files.

## Local validation

Use the repository command surface where available:

```bash
python scripts/repo.py architecture
python scripts/repo.py manifests
python scripts/repo.py format --check
python scripts/repo.py quality
```

Underlying focused commands and specialized runtime/database gates remain available and mandatory when their scopes are affected. See [`docs/MODULE_DEVELOPMENT.md`](docs/MODULE_DEVELOPMENT.md) and the active delivery issue.

When an independent local verifier participates, every verification result must be attached to the exact commit SHA actually tested. See [`docs/MULTI_AGENT_DEVELOPMENT.md`](docs/MULTI_AGENT_DEVELOPMENT.md).

## Status synchronization rule

README is stable orientation, not a second roadmap. Detailed progress belongs in:

- `docs/IMPLEMENTATION_ROADMAP.md`;
- `docs/PROJECT_STATUS.md`;
- `docs/MODULE_CATALOG.md`;
- the active GitHub phase issue.

When scope or completion changes, update those sources together.

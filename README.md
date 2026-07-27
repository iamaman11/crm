# Ultimate CRM Platform

Implementation repository for a universal modular, metadata-driven, production-grade CRM platform.

## Start here

For a new contributor or coding agent, read in this order:

1. [`AGENTS.md`](AGENTS.md) — repository operating model and change workflow.
2. [`docs/SYSTEM_INVARIANTS.md`](docs/SYSTEM_INVARIANTS.md) — absolute architecture rules.
3. [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md) — concise current merged state and next packet.
4. [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md) — normative product delivery order.
5. [`docs/PHASE8_DELIVERY_PLAN.md`](docs/PHASE8_DELIVERY_PLAN.md) — active Phase 8 sequence.
6. [`docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`](docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md) — tracked 10/10 architecture and developer-experience program.
7. [`docs/APPLICATION_ARCHITECTURE.md`](docs/APPLICATION_ARCHITECTURE.md) — layer and composition model.
8. [`docs/MODULE_CATALOG.md`](docs/MODULE_CATALOG.md) — business ownership and product-completeness accounting.
9. [`docs/DEVELOPMENT_WORKFLOW.md`](docs/DEVELOPMENT_WORKFLOW.md) — coherent packet and exact-head acceptance workflow.
10. [`docs/MULTI_AGENT_DEVELOPMENT.md`](docs/MULTI_AGENT_DEVELOPMENT.md) — exact-SHA implementation and verification protocol.

Accepted ADRs and published contracts take precedence over descriptive prose unless they violate an absolute system invariant.

## Current state

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.11 / issue #126 is in progress.**

The current bounded product packet is **Customer Privacy scope discovery and immutable snapshot**. All nine authoritative privacy owner-scope implementations are accepted, but production discovery, planning and owner execution are not yet implemented.

The cross-cutting architecture and developer-experience program is tracked by **issue #194**. It preserves the existing modular architecture while reducing accidental crate proliferation, central manual composition, dependency drift, CI fan-out and repository navigation cost.

Current product-complete expert modules: **0**.

See [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md) for exact merged evidence and the next permitted implementation boundary.

## Architectural model

The target is a **modular monolith with independently governed owner and link modules**, not a collection of accidental microservices.

Core invariants:

- every mutable aggregate has one authoritative owner;
- actors mutate state only through exact versioned governed capabilities;
- business modules do not receive raw database, broker, object-storage, arbitrary HTTP, secret-store or LLM-provider clients;
- business modules do not import or mutate another business module's internals;
- cross-domain behavior uses versioned capabilities/events and optional link modules;
- state mutations atomically persist required idempotency, outbox and audit evidence;
- queries are permission-bound and structurally separate from mutation semantics;
- search, analytics, caches, timelines and projections are rebuildable and non-authoritative;
- live authorization runs immediately before side effects;
- AI and marketplace extensions have no alternate mutation or data-access path.

The complete normative rules are in [`docs/SYSTEM_INVARIANTS.md`](docs/SYSTEM_INVARIANTS.md).

## Extension rule

A normal capability added to an existing owner should create **zero new crates**. It belongs inside the existing owner application and production packages.

A new business module is justified only by a new authoritative mutable domain. Screens, reports, tables, projections and convenience groupings do not create owner modules.

Generic router and worker algorithms must not change merely to register one owner capability.

See [`docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`](docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md).

## Repository layout

- `proto/` — authoritative RPC, command and event contract sources.
- `crates/` — platform runtimes, application packages and infrastructure/production adapters.
- `modules/` — independently governed pure business owner/link modules.
- `services/` — deployable composition roots; `services/crm-api` is the production process.
- `database/` — authoritative migrations and PostgreSQL acceptance assets.
- `schemas/` — strict authoring schemas compiled into typed runtime IR.
- `docs/adr/` — accepted architecture decisions.
- `scripts/` — architecture, contract, manifest, affected-scope and repository tooling.
- `.github/workflows/` — permanent conformance and acceptance gates.

Generated `build/` content and workflow artifacts are reproducible outputs and are not authoritative source files.

## Local validation

Use the stable repository command surface:

```bash
python scripts/repo.py architecture
python scripts/repo.py manifests
python scripts/repo.py contracts
python scripts/repo.py conformance
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
python scripts/repo.py format --check
python scripts/repo.py quality
```

Specialized contract, database, process, migration and product-plane gates remain mandatory when their scopes are affected.

## Status synchronization rule

README is stable orientation, not a second roadmap. Detailed progress belongs in:

- `docs/PROJECT_STATUS.md`;
- `docs/IMPLEMENTATION_ROADMAP.md`;
- `docs/PHASE8_DELIVERY_PLAN.md`;
- `docs/MODULE_CATALOG.md`;
- active GitHub issues.

When scope or completion changes, update those sources together and rerun applicable exact-head checks.

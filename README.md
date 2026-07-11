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

Accepted ADRs and published contracts take precedence over descriptive prose unless they violate an absolute system invariant.

## Current state

The platform foundation through **Phase 6H** is complete:

- governed module manifests, SDK and lifecycle;
- PostgreSQL tenant/RLS, transaction, idempotency, outbox and audit foundation;
- authenticated capability mutation gateway;
- independent Sales Deal and Activities Task owner-domain slices;
- production PostgreSQL mutations through HTTP/gRPC ingress;
- permission-bound Deal/Task get/list queries with opaque cursor pagination;
- authenticated HTTP/gRPC query ingress with query-only execution context.

The current next slice is **Phase 6I — optional Sales–Activities link module**.

The complete CRM product is not finished. Search/Admin Studio, production application composition, frontend product shell, customer master, commercial lifecycle, expert modules, AI, marketplace and enterprise operational proof remain later roadmap work.

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
- `services/` — deployable composition roots; `services/crm-api` is the target production application process.
- `database/` — authoritative migrations and PostgreSQL acceptance assets.
- `schemas/` — strict authoring schemas compiled into typed runtime IR.
- `docs/adr/` — accepted architecture decisions.
- `scripts/` — architecture, contract and manifest enforcement.
- `.github/workflows/` — permanent conformance and acceptance gates.

Generated `build/` content and workflow artifacts are reproducible outputs and are not authoritative source files.

## Local validation

```bash
python -m pip install -r requirements-dev.txt
python scripts/validate_contracts.py
python scripts/validate_module_manifests.py
python scripts/compile_module_manifest_ir.py --output-dir build/module-ir
cargo run --quiet -p crm-module-manifest --bin validate-module-manifest -- build/module-ir/*.json
python -m unittest tests/test_module_manifest_validation.py
python scripts/check_architecture.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Database-affecting changes must also pass the permanent Database CI gates.

## Status synchronization rule

README is stable orientation, not a second roadmap. Detailed progress belongs in:

- `docs/IMPLEMENTATION_ROADMAP.md`;
- `docs/PROJECT_STATUS.md`;
- `docs/MODULE_CATALOG.md`;
- the active GitHub phase issue.

When scope or completion changes, update those sources together.
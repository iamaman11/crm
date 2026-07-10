# Ultimate CRM Platform

Private implementation repository for the Ultimate CRM architecture.

## Authoritative specifications

The repository implements the following Google Drive documents, in precedence order:

1. [v2.2 Architecture Closure & Contract Specification](https://docs.google.com/document/d/1xUl7oGh3nrMzJ332mxtoZqRch_0O6wyj_wfUFGYnPrU/edit)
2. [v2.1 Implementation Readiness Addendum](https://docs.google.com/document/d/1fgCls9uumH_V0hMh0_aUEvvNWCIAsvRQSB0n-M5Ih0U/edit)
3. [v2.0 Production-Grade Architecture Blueprint](https://docs.google.com/document/d/1UF-VfjP6hpPr3qWh-b0djQErdWN8kkL9IDMCQVcHXmc/edit)

ADR and compilable contracts take precedence over descriptive text unless they violate an absolute architecture invariant.

## Invariants

- Actors mutate state only through governed capabilities.
- Business modules do not receive direct DB, NATS, object-storage, secrets, or LLM-provider clients.
- State mutations atomically write transactional outbox events.
- Projections and search indexes are rebuildable and never sources of truth.
- Audit hashing uses a canonical audit envelope, not a re-encoded business event.
- Workflow and AI actions perform live authorization immediately before execution.
- Sensitive payloads are minimized, reference-based, encrypted, and deletable by key destruction where required.

The complete normative rules are in [`docs/SYSTEM_INVARIANTS.md`](docs/SYSTEM_INVARIANTS.md). Controlled data formats and canonicalization are governed by [`ADR-024`](docs/adr/ADR-024-controlled-data-formats-and-canonicalization.md).

## Repository layout

- `proto/` — Contract Pack v1.0 source of truth.
- `crates/` — platform core Rust crates.
- `modules/` — independently governed domain modules without infrastructure access.
- `services/` — executable service boundaries.
- `schemas/` — strict authoring schemas compiled into typed runtime IR.
- `docs/adr/` — accepted architecture decisions.
- `scripts/check_architecture.py` — dependency-boundary enforcement.
- `scripts/validate_module_manifests.py` — strict YAML, schema and module dependency enforcement.
- `.github/workflows/` — contract, governance and Rust CI.
- `artifacts/contracts/` — validated descriptor set and checksum.

## Local validation

```bash
python -m pip install -r requirements-dev.txt
python scripts/validate_contracts.py
python scripts/validate_module_manifests.py
python -m unittest tests/test_module_manifest_validation.py
python scripts/check_architecture.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Module authoring policy

A module is authored as `modules/<module>/module.yaml`, but raw YAML is never runtime truth. Governance CI requires a strict JSON-compatible YAML 1.2 subset, validates it against `schemas/module.schema.json`, performs semantic and dependency checks, and emits a `crm.cjson/v1` digest. Runtime installation will consume the future typed immutable Module Manifest IR rather than the source YAML.

## Current stage

Phase 0 repository skeleton and Contract Pack are present. System invariants and Module Manifest authoring governance are defined. Production readiness remains gated by implementation, benchmarks, security testing and restore drills.

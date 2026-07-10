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

## Repository layout

- `proto/` — Contract Pack v1.0 source of truth.
- `crates/` — platform core Rust crates.
- `modules/` — domain modules without infrastructure access.
- `services/` — executable service boundaries.
- `docs/adr/` — accepted architecture decisions.
- `scripts/check_architecture.py` — dependency-boundary enforcement.
- `.github/workflows/` — contract and Rust CI.
- `artifacts/contracts/` — validated descriptor set and checksum.

## Local validation

```bash
python -m pip install -r requirements-dev.txt
python scripts/validate_contracts.py
python scripts/check_architecture.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Current stage

Phase 0 repository skeleton and Contract Pack are present. Production readiness remains gated by implementation, benchmarks, security testing, and restore drills.

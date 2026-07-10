# Ultimate CRM Platform

Private implementation repository for the Ultimate CRM architecture.

## Authoritative specifications

The repository implements the following Google Drive documents, in precedence order:

1. [v2.2 Architecture Closure & Contract Specification](https://docs.google.com/document/d/1xUl7oGh3nrMzJ332mxtoZqRch_0O6wyj_wfUFGYnPrU/edit)
2. [v2.1 Implementation Readiness Addendum](https://docs.google.com/document/d/1fgCls9uumH_V0hMh0_aUEvvNWCIAsvRQSB0n-M5Ih0U/edit)
3. [v2.0 Production-Grade Architecture Blueprint](https://docs.google.com/document/d/1UF-VfjP6hpPr3qWh-b0djQErdWN8kkL9IDMCQVcHXmc/edit)

ADR and compilable contracts take precedence over descriptive text unless they violate an absolute architecture invariant.

## Delivery plan

The executable step-by-step delivery plan is [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md). GitHub issue [#2](https://github.com/iamaman11/crm/issues/2) is the parent epic; issues #3–#14 contain phase scope, dependencies and acceptance gates.

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
- `crates/` — platform core and governed runtime crates.
- `modules/` — independently governed domain modules without infrastructure access.
- `services/` — executable service boundaries.
- `schemas/` — strict authoring schemas compiled into typed runtime IR.
- `docs/adr/` — accepted architecture decisions.
- `scripts/check_architecture.py` — dependency-boundary enforcement.
- `scripts/validate_module_manifests.py` — strict YAML, schema and module dependency enforcement.
- `scripts/compile_module_manifest_ir.py` — normalized JSON IR and digest generation.
- `.github/workflows/` — contract, governance and Rust CI.
- `build/` and workflow artifacts — generated descriptors, normalized IR and diagnostics; they are reproducible outputs and are not authoritative source files.

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

## Module authoring policy

A module is authored as `modules/<module>/module.yaml`, but raw YAML is never runtime truth. Governance CI requires a strict JSON-compatible YAML 1.2 subset, validates it against `schemas/module.schema.json`, performs semantic and dependency checks, emits normalized JSON IR and a `crm.cjson/v1` digest, and verifies that the Rust runtime representation derives the same identity.

## Current stage

Governance Foundation v1 is complete. The executable roadmap and Typed Module Manifest IR are in active implementation under issues #3 and #4. Production readiness remains gated by the later platform, module, security, benchmark and restore-drill phases.

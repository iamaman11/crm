# Ultimate CRM — Golden Module Development

Status: **Normative golden-module workflow**
Foundation: issue #56 / Phase 7; native production contribution baseline: issue #134 / PR #135.

This guide defines the repository-supported starting point for new business owner modules and optional link modules. Scaffolding removes repetitive wiring; it does not decide domain ownership and never implies production readiness.

## 1. Decide ownership before generating code

Create an **owner module** only for a distinct authoritative mutable domain. Create a **link module** only for optional cross-domain coordination over published events/capabilities.

Do not create a module for a screen, table, team, projection, search index or convenience grouping.

Before running the generator, decide:

- immutable `module_id`;
- owner team and contact;
- authoritative object identifiers for an owner module;
- exact required source/target module dependencies for a link module;
- lifecycle and uninstall expectations.

## 2. Generate an owner module

Example:

```bash
python scripts/scaffold_module.py owner \
  --module-id crm.customer \
  --display-name "CRM Customer" \
  --team customer-platform \
  --contact crm-owner@example.com \
  --object customer.party \
  --object customer.contact_point
```

The command creates:

```text
modules/crm-customer/
  Cargo.toml
  module.yaml
  src/lib.rs
  contracts/README.md
  adapters/README.md
  production/CONTRIBUTION.md
  tests/acceptance.rs
  migrations/.gitkeep
  README.md
  ACCEPTANCE.md
  MODULE_CATALOG_ENTRY.md
```

and registers the crate in the root Cargo workspace.

Owner generation requires at least one explicit object. This prevents a new owner domain from being created before its authoritative boundary is named.

The generated contract and adapter directories are explicit TODO boundaries, not fake production implementations. Published Protobuf remains in the canonical repository contract tree, and production adapters remain outside the pure module core behind governed composition boundaries.

## 3. Generate a link module

Example:

```bash
python scripts/scaffold_module.py link \
  --module-id crm.customer-sales-link \
  --display-name "Customer Sales Link" \
  --team integration-platform \
  --contact crm-owner@example.com \
  --requires 'crm.customer@^0.1.0' \
  --requires 'crm.sales@^0.2.0'
```

A generated link module:

- owns no authoritative business records;
- requires explicit source/target module dependencies;
- receives only private state ownership for delivery/coordination state;
- defaults to `delete_private_state` on uninstall;
- must later define exact published source events, target capabilities, deterministic delivery identity and disable/uninstall behavior.

Dependency version ranges are validated before any files are written.

## 4. Preview safely

Use `--dry-run` to validate names and see every path that would be created without changing the repository:

```bash
python scripts/scaffold_module.py owner \
  --module-id crm.customer \
  --display-name "CRM Customer" \
  --team customer-platform \
  --contact crm-owner@example.com \
  --object customer.party \
  --dry-run
```

The generator refuses to overwrite an existing module directory or duplicate workspace member.

## 5. Permanent repository commands

Use the cross-platform command runner rather than memorizing CI internals:

```bash
# full native architecture conformance preflight
python scripts/repo.py conformance

# focused architecture dependency/source boundaries
python scripts/repo.py architecture

# module manifests, normalized IR and Rust digest parity
python scripts/repo.py manifests

# verify or regenerate module-to-Protobuf bindings
python scripts/repo.py contracts
python scripts/repo.py contracts --write

# format or check formatting
python scripts/repo.py format
python scripts/repo.py format --check

# synchronize Cargo.lock
python scripts/repo.py lock

# focused package tests
python scripts/repo.py test crm-sales
python scripts/repo.py test crm-core-data --test-target postgres_query

# full Rust workspace tests
python scripts/repo.py test-all

# architecture + formatting check + Clippy + full tests
python scripts/repo.py quality
```

Specialized Contract, Database, Event Runtime and Application Runtime gates remain mandatory when their scopes are affected. `repo.py quality` does not replace those specialized CI proofs.

## 6. Generator acceptance and generated readiness

Governance CI validates the generator itself. Its permanent scaffolding suite:

- generates owner and link manifests and checks them against the real manifest schema and semantic validator;
- proves overwrite protection and dry-run behavior;
- creates a fresh owner module in a temporary Cargo workspace;
- runs `cargo check --all-targets` so the generated library and ignored acceptance-test placeholder compile;
- compares generated dependencies with `architecture-policy.json` and rejects forbidden infrastructure or disallowed internal CRM dependencies.

Every generated module still starts at **Foundation only**.

`ACCEPTANCE.md` contains explicit incomplete gates for:

- ownership/invariants or link-delivery semantics;
- versioned published contracts;
- deterministic domain/application behavior;
- governed adapters;
- tenant, authorization, retry/idempotency and cross-tenant negative tests;
- platform-owned persistence/projection/search adapters;
- replacement of the ignored `tests/acceptance.rs` scaffold gate with real production-path evidence;
- production composition and end-to-end acceptance;
- rollback/disable/uninstall behavior;
- roadmap/status/catalog synchronization.

A generated directory, compiling crate or valid manifest is never evidence of a production vertical slice.

## 7. Dependency rules inherited by construction

Generated business modules depend only on stable platform boundaries:

- `crm-core-contracts`;
- `crm-module-sdk`.

The repository architecture policy rejects direct infrastructure dependencies such as SQLx/PostgreSQL clients, brokers, arbitrary HTTP clients, secret stores and LLM providers. It also rejects direct imports of another business module's internal crate.

Cross-domain work must remain:

```text
published source event
→ governed delivery
→ link-owned deterministic coordination state
→ CapabilityClient
→ target owner capability
```

## 8. From scaffold to production

After generation, follow `DEVELOPMENT_WORKFLOW.md`:

1. ownership and invariants;
2. public contracts;
3. application ports/use cases;
4. infrastructure adapters outside the module core;
5. production composition;
6. acceptance tests;
7. operational and documentation closure.

Move the generated `MODULE_CATALOG_ENTRY.md` content into the normative catalog only when the module identity and ownership decision are accepted. Update readiness only when the corresponding merged acceptance evidence exists.

## 9. Publish contract bindings without a second source of truth

Every item under `provides.capabilities` and `provides.events` must include its exact Protobuf binding. Capability entries use `kind: protobuf_rpc` with RPC/request/response names; event entries use `kind: protobuf_message` with the payload message.

The binding is authoring/build metadata. It is removed from normalized runtime module IR, so runtime lifecycle and installation identity stay independent from Protobuf repository organization.

Never edit `contracts/module-contract-bindings.json` directly. Generate it from all manifests and the compiled descriptor set:

```bash
python scripts/generate_contract_bindings.py --write
python scripts/generate_contract_bindings.py --check
```

`pnpm web:generate` performs the same generation together with browser clients and contract hashes. Contract CI requires exact module-set completeness, descriptor input/output parity and byte-for-byte generated-artifact freshness. See `CONTRACT_BINDING_REGISTRY.md` for the normative architecture and invariants.

## Production contribution boundary

Every generated module contains `production/CONTRIBUTION.md`. The pure module core does not wire itself into the process host. A separately owned adapter/composition crate contributes exact versioned mutation/query routes, pre-authorization semantic validation, activation-gated workers and deterministic worker phases. Adding a module must not require edits to generic router or worker algorithms.

Before a generated module can claim a production vertical slice, its contribution must prove:

1. exact owner/identifier/version/kind definitions;
2. complete validator and handler bindings with startup failure on mismatch;
3. durable tenant installation, disable and uninstall behavior;
4. pre-authorization cross-owner validation through governed ports only;
5. deterministic worker phases, bounded work and retry/idempotency where workers exist;
6. route-parity coverage or an exact reasoned non-runtime classification;
7. focused, PostgreSQL and real-process acceptance plus synchronized module/roadmap status.

Compiled production coordinates are checked against `contracts/module-contract-bindings.json`. Platform-owned routes and any intentionally non-runtime governed coordinate must be listed individually, with a reason, in `contracts/production-route-classifications.json`; owner-wide or pattern allowlists are forbidden.

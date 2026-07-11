# Ultimate CRM — Golden Module Development

Status: **Phase 7 foundation**  
Tracked by: issue #56 and parent Phase 7 issue #10.

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
# architecture dependency/source boundaries
python scripts/repo.py architecture

# module manifests, normalized IR and Rust digest parity
python scripts/repo.py manifests

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

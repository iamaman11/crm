# Ultimate CRM — Golden Module Development

Status: **Normative golden-module workflow**  
Foundation: issue #56 / Phase 7  
Architecture and developer-experience program: issue #194

This guide defines the supported path for new owner/link modules and ordinary capabilities. Scaffolding removes repetitive setup; generated navigation removes repository archaeology. Neither decides ownership or implies production readiness.

## 1. Decide ownership before generating code

Create an **owner module** only for a distinct authoritative mutable domain. Create a **link module** only for optional coordination over published events and capabilities.

Do not create a module for a screen, table, report, team, projection, search index or convenience grouping.

Before generating anything, decide:

- immutable `module_id`;
- owner team and contact;
- authoritative objects;
- exact dependencies for a link module;
- lifecycle, disable, rollback and uninstall expectations;
- storage and migration ownership;
- whether a real provider, trust, process or extraction boundary exists.

Use `python scripts/repo.py explain <module-or-coordinate>` before extending an existing owner. Unknown or ambiguous targets must fail closed instead of being guessed.

## 2. Ordinary capability rule

A normal capability added to an existing owner creates **zero new crates**.

Add it inside the existing owner structure:

```text
owner domain
→ application command/query/worker
→ existing or justified adapter
→ module-owned production contribution
→ focused owner tests and generic conformance
```

Do not create a crate for one handler, planner, query, worker, re-export or capability-specific composition function. Feature behavior and behavior-neutral crate consolidation are separate PRs.

## 3. Current scaffold versus 10/10 target

The current generator creates a **foundation owner/link crate** under `modules/`. It proves naming, manifest, lifecycle placeholders and pure dependency boundaries.

Current foundation output:

```text
modules/crm-<domain>/
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

The accepted Customer Privacy pilot proves the target technical package model:

```text
modules/crm-<domain>/                 # pure domain
crates/crm-<domain>-application/      # commands, queries, workers, validation, ports
crates/crm-<domain>-postgres/         # persistence and authoritative reads
crates/crm-<domain>-production/       # routes, workers, contribution, wiring
```

An optional fifth package is allowed only for a real provider, trust, process or extraction boundary.

Stage C is not complete merely because one pilot exists. Until scaffolding, migration ownership and visibility rules are generalized, evolve another owner only through an explicitly reviewed package-boundary packet. Never imitate the target by creating one new crate per use case.

## 4. Generate an owner foundation

```bash
python scripts/scaffold_module.py owner \
  --module-id crm.customer \
  --display-name "CRM Customer" \
  --team customer-platform \
  --contact crm-owner@example.com \
  --object customer.party \
  --object customer.contact_point
```

Owner generation requires at least one explicit authoritative object. The generator refuses existing directories and duplicate workspace members.

Generated contract/adapter directories are TODO boundaries, not production implementations. Published Protobuf remains in the canonical contract tree. Infrastructure remains outside the pure module core.

## 5. Generate a link foundation

```bash
python scripts/scaffold_module.py link \
  --module-id crm.customer-sales-link \
  --display-name "Customer Sales Link" \
  --team integration-platform \
  --contact crm-owner@example.com \
  --requires 'crm.customer@^0.1.0' \
  --requires 'crm.sales@^0.2.0'
```

A link module:

- owns no source/target authoritative records;
- declares exact source and target dependencies;
- owns only private coordination/deduplication/configuration state;
- consumes published events and invokes governed capabilities;
- remains independently installable, disableable and uninstallable.

Use `--dry-run` before generation.

## 6. Permanent repository commands

Use the stable command runner:

```bash
python scripts/repo.py conformance
python scripts/repo.py architecture
python scripts/repo.py manifests
python scripts/repo.py contracts
python scripts/repo.py contracts --write
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
python scripts/repo.py navigation --check
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
python scripts/repo.py packet-check --base origin/main
python scripts/repo.py format --check
python scripts/repo.py lock

python scripts/repo.py test --package crm-sales
python scripts/repo.py test --package crm-core-data --test-target postgres_query
python scripts/repo.py test-all
python scripts/repo.py quality
```

Specialized Contract, Database, Event Runtime, Application Runtime, process and frontend gates remain mandatory when affected. `quality` does not replace specialized acceptance.

Generated `docs/ACTIVE_PACKET.md` and `docs/generated/REPOSITORY_MAP.md` are deterministic orientation outputs. Never edit them manually; regenerate or freshness-check them through `repo.py navigation`.

Local lifecycle commands `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` remain future repository step 15 work and are not available until implemented and permanently tested.

## 7. From foundation to production

Follow `DEVELOPMENT_WORKFLOW.md`:

1. authoritative ownership, invariants and exclusions;
2. public contract or compatible new version;
3. application commands/queries/workers and ports;
4. persistence and migration ownership;
5. pre-authorization semantic validation;
6. module-owned production contribution;
7. exact route/worker registration and durable activation;
8. focused, PostgreSQL and process acceptance;
9. operational and documentation closure.

A generated directory, compiling crate or valid manifest is never evidence of a production vertical slice.

## 8. Package-boundary decision

Create a package only when it protects at least one real boundary:

- forbidden infrastructure dependency;
- provider SDK, arbitrary HTTP, secrets, broker or object storage;
- independent trust/security boundary;
- separately operating worker/process;
- multiple independent consumers;
- credible extraction seam documented by ADR;
- visibility that package boundaries must enforce.

Required review note:

```text
New crate justification:
- protected boundary:
- isolated dependencies:
- expected consumers:
- why an internal module is insufficient:
- lifecycle/extraction seam:
- expected fan-out:
- removal or consolidation condition:
```

## 9. Dependency and Rust visibility rules

Pure business modules depend only on stable platform contracts and governed SDK ports.

- no SQLx/PostgreSQL, broker, arbitrary HTTP, secret-store or provider dependencies;
- no direct imports of another business module's internals;
- implementation visibility defaults to private or `pub(crate)`;
- public Rust APIs are limited to contracts, stable ports and contribution interfaces;
- concrete adapters and repositories are not re-exported for convenience.

## 10. Contract publication and retirement

Every provided capability/event/query includes an exact binding to its authoritative Protobuf contract.

Never edit generated binding registries directly. Use:

```bash
python scripts/repo.py contracts --write
python scripts/repo.py contracts
```

Published versions are immutable. Semantic change creates a new version. Deprecation requires replacement, consumer inventory, deadline and explicit retirement criteria. Removal occurs only after supported consumers migrate and rollback/compatibility evidence exists.

## 11. Persistence and migration ownership

Each owner declares authoritative storage namespaces and migrations.

- do not alter another owner's tables;
- enforce tenant context and FORCE RLS;
- include cross-tenant negative tests;
- prove forward, rollback/schema-removal and reapply behavior where applicable;
- version persisted envelopes independently from public wire messages;
- use an ownership-transfer ADR for any authoritative data move.

## 12. Production contribution boundary

The pure module core does not wire itself into the process host. The owner production package contributes:

- exact mutation/query routes;
- pre-authorization semantic validators;
- planner/executor or query bindings;
- activation-gated workers with deterministic phases;
- module identity for durable activation.

Adding a capability must not modify generic router or worker algorithms.

Before production readiness, prove:

1. exact owner/identifier/version/kind definitions;
2. complete validator/handler bindings with startup failure on mismatch;
3. durable install/disable/uninstall behavior;
4. governed cross-owner reads only through stable ports;
5. worker boundedness, retry and crash recovery where applicable;
6. exact route parity or individually reasoned non-runtime classification;
7. focused, PostgreSQL and real-process acceptance;
8. synchronized status, roadmap and module catalog.

## 13. Navigation expectation

Use `docs/README.md` for task selection and `docs/generated/REPOSITORY_MAP.md` for inventory. `repo.py explain` makes each owner or capability path mechanical; `packet-check` makes declared scope and required CI explainable.

Any coordinate that cannot be resolved unambiguously is a defect. Fix the authoritative manifest/classification or navigation generator rather than adding a manual exception.
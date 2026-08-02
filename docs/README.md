# Ultimate CRM — Documentation Navigation

Status: **Stable navigation index; not a source of runtime or delivery truth**

This page answers one question: **where should a developer or coding agent go next?**

It does not duplicate live status, roadmap state, module inventories or accepted packet evidence. Those remain authoritative in the documents linked below.

## 1. Fastest correct entry path

Read only what is required for the current task:

1. `../AGENTS.md` — repository operating rules and change discipline.
2. `SYSTEM_INVARIANTS.md` — rules that may not be violated.
3. `PROJECT_STATUS.md` — exact current merged state and next permitted packet.
4. [`ACTIVE_PACKET.md`](ACTIVE_PACKET.md) — generated active packet, baseline and allowed/forbidden paths.
5. [`generated/REPOSITORY_MAP.md`](generated/REPOSITORY_MAP.md) — generated workspace/module/route inventory.
6. The active GitHub issue — executable scope, acceptance boundary and current work state.
7. The task-specific guide from the table below.

Do not reconstruct current status from historical packet documents, old PR descriptions or directory names.

## 2. Choose by task

| Task | Read first | Then inspect |
|---|---|---|
| Understand what is being built now | `PROJECT_STATUS.md`, `ACTIVE_PACKET.md` | `IMPLEMENTATION_ROADMAP.md`, `PHASE8_DELIVERY_PLAN.md`, active issue |
| Understand the complete future CRM product | `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md` | `CRM_CAPABILITY_COVERAGE.md`, `IMPLEMENTATION_ROADMAP.md`, `MODULE_CATALOG.md` |
| Understand automation, triggers, robots, pipelines or Kanban | `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md` sections 7–9 | `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, owner capability contracts |
| Understand architecture boundaries | `SYSTEM_INVARIANTS.md` | `APPLICATION_ARCHITECTURE.md`, accepted ADRs |
| Add or change a capability | `DEVELOPMENT_WORKFLOW.md` | `MODULE_DEVELOPMENT.md`, `repo.py explain`, owner manifest and generated map |
| Add a new owner domain | `MODULE_DEVELOPMENT.md` | `MODULE_CATALOG.md`, architecture plan, relevant ADRs |
| Add cross-domain behavior | `APPLICATION_ARCHITECTURE.md` | link-module rules in `AGENTS.md` and `MODULE_DEVELOPMENT.md` |
| Change Protobuf or public contracts | `SYSTEM_INVARIANTS.md` | contract registry docs, module manifest bindings, Contract CI |
| Change PostgreSQL schema or persistence | `SYSTEM_INVARIANTS.md` | owner migrations, Database CI, rollback/reapply acceptance |
| Change composition, routes or workers | `APPLICATION_ARCHITECTURE.md` | contribution package, route classifications, application-runtime parity tests |
| Improve architecture or developer experience | `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` | issue #194 and measured baseline |
| Prepare or review a PR | `DEVELOPMENT_WORKFLOW.md` | `DELIVERY_GOVERNANCE.md`, `repo.py packet-check`, affected-scope report |
| Coordinate multiple agents | `MULTI_AGENT_DEVELOPMENT.md` | `CODEX_AGENT_QUALIFICATION.md`, exact-SHA handoff |
| Check product completeness | `MODULE_CATALOG.md` | `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`, `CRM_CAPABILITY_COVERAGE.md`, roadmap and status |

## 3. Source-of-truth hierarchy

| Concern | Authoritative source |
|---|---|
| Absolute architecture and security rules | `SYSTEM_INVARIANTS.md` |
| Published machine contracts | Protobuf, schemas, manifests and accepted contract registries |
| Stable application layering and composition | `APPLICATION_ARCHITECTURE.md` and accepted ADRs |
| Single repository implementation order | `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 |
| Product delivery order | `IMPLEMENTATION_ROADMAP.md` |
| Complete product portfolio, automation semantics and 10/10 completion contract | `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md` |
| Functional no-omission baseline | `CRM_CAPABILITY_COVERAGE.md` |
| Active Phase 8 sequence | `PHASE8_DELIVERY_PLAN.md` |
| Current merged state and next packet | `PROJECT_STATUS.md` |
| Active packet declaration | `repository-packet.json` rendered as `ACTIVE_PACKET.md` |
| Architecture/developer-experience execution program | `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` and issue #194 |
| Business owner and completeness accounting | `MODULE_CATALOG.md` |
| Work-in-progress scope | active GitHub issue and PR |
| Historical acceptance boundary | accepted packet document and merged PR evidence |

When two descriptive documents disagree, follow the higher source and treat the lower document as stale. Generated navigation carries deterministic source digests but never overrides its authoritative inputs.

## 4. Normal feature navigation

The target path for an ordinary capability is:

```text
owner manifest / published contract
→ owner application command, query or worker
→ owner persistence or external port implementation
→ owner-owned production contribution
→ exact ingress or worker registration
→ focused owner tests
→ affected CI and required specialized gates
```

Trace that path before changing code:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
```

An ordinary capability must not require a new crate, a central business switch, direct edits to unrelated owners or copied platform test suites. Record any unresolved explain hop as developer-experience debt under issue #194.

## 5. Repository command surface

Currently available:

```bash
python scripts/repo.py architecture
python scripts/repo.py manifests
python scripts/repo.py contracts
python scripts/repo.py contracts --write
python scripts/repo.py conformance
python scripts/repo.py format --check
python scripts/repo.py lock
python scripts/repo.py test --package <package>
python scripts/repo.py test-all
python scripts/repo.py explain <module-or-capability@version>
python scripts/repo.py packet-check --base origin/main
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
python scripts/repo.py quality
```

Generated navigation:

```bash
python scripts/generate_repository_navigation.py --check
python scripts/generate_repository_navigation.py --write
```

`packet-check` validates the declared baseline, allowed/forbidden paths, affected packages, selected workflows, changed contracts/routes/workers/migrations and generated freshness. `conformance` rejects stale generated navigation.

Planned and required for the 10/10 target:

```bash
python scripts/repo.py doctor
python scripts/repo.py bootstrap
python scripts/repo.py dev-up
python scripts/repo.py dev-reset
python scripts/repo.py seed-demo
python scripts/repo.py smoke
```

A command listed as planned must not be represented as implemented until it exists and is covered by permanent tests.

## 6. Generated navigation

Implemented outputs:

- [`ACTIVE_PACKET.md`](ACTIVE_PACKET.md), generated from `repository-packet.json`;
- [`generated/REPOSITORY_MAP.md`](generated/REPOSITORY_MAP.md), generated from workspace members, all module manifests and production route classifications;
- `repo.py explain`, which resolves exact modules and capability coordinates and groups repository references by architecture layer;
- `repo.py packet-check`, which combines packet scope with the existing affected-scope analyzer.

Generated outputs use deterministic ordering and embedded SHA-256 source digests. Do not hand-edit them. Rust Generated Sync writes canonical output, and permanent conformance rejects stale output.

## 7. Documentation classes

- **Normative:** invariants, accepted ADRs, architecture, delivery governance, product portfolio and workflow rules.
- **Current state:** project status, active phase plan, module catalog and active issues.
- **Generated navigation:** active packet, repository map and explain/packet-check reports.
- **Historical:** accepted packet documents and merged PR evidence.
- **Orientation:** root README, `AGENTS.md` and this index.

Historical documents must not carry changing current-status claims. Orientation documents must link to status rather than copy detailed evidence.

## 8. Navigation quality bar

Navigation reaches 10/10 only when a new contributor can, without repository archaeology:

1. identify the authoritative owner;
2. locate contract, application, persistence and production contribution;
3. identify migrations and data-isolation rules;
4. determine required tests and workflows;
5. see the active packet and explicit exclusions;
6. reproduce the local environment;
7. obtain an explainable affected scope;
8. complete a representative leaf change without touching generic runtime or unrelated modules;
9. identify the product wave, user journey and product-completion evidence required for the feature.

The target is not more documentation. The target is fewer decisions, fewer contradictory entry points and a mechanically explainable path from intent to verified change.

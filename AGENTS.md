> **ПРИОРИТЕТНОЕ РЕШЕНИЕ ВЛАДЕЛЬЦА · 23.11.2026 — ENTERPRISE-PLUS, НЕ MVP.** Цель — полнофункциональная CRM с проверяемым превосходством над Salesforce/Dynamics/HubSpot по конкретным сценариям; [#484](https://github.com/iamaman11/crm/issues/484) — единственный живой продуктовый план и расширенная конкурентная матрица, [#487](https://github.com/iamaman11/crm/issues/487) — один текущий исполнительный курсор. **554 требований и 20 семейств — нижняя граница охвата, не предел.** Запрещено объявлять продукт готовым по узкому bounded-срезу или откладывать функции по причине «достаточно для MVP». C разрешён только при зафиксированной предметной зависимости/условии возврата; A сохраняет доказанные функции, B реализует полную полезную семантику, D относится только к ненужной legacy-инфраструктуре. **Один code slice — несколько связанных операций пользователя с полным Protobuf/Connect/Rust owner/D1/React/test путём, штатным Gate A и независимым LIVE Gate B; никаких микрокоммитов ради полей.** Сохранить строгую централизованную типизированную схему, compiled fail-closed Rust policy и единственный CI. Любые последующие исторические упоминания «малый MVP», «C для MVP», LATER/END или старых Markdown-планов не меняют этот курс; детали и актуальные статусы — только в Issues.

# Ultimate CRM — Agent and Contributor Operating Guide

> **АКТУАЛЬНЫЙ ПОРЯДОК РАБОТЫ CRM (issue-first).** Ветка `dev`, remote [iamaman11/crm](https://github.com/iamaman11/crm) — единственный код. **Продуктовый план, 20 семейств, список и порядок задач — [#484](https://github.com/iamaman11/crm/issues/484); текущая функция, решение A/B/C/D и доказательства двух Gate — [#487](https://github.com/iamaman11/crm/issues/487).** Не вести параллельных дорожных карт и текущих статусов в Markdown. Исторические полные MD/матрицы перемещены в [docs/archive](docs/archive/), их `docs/*.md` адреса — только указатели. Техническая архитектура остаётся [apps/cloudflare-crm/ARCHITECTURE.md](apps/cloudflare-crm/ARCHITECTURE.md). Все старые инструкции ниже, противоречащие этому правилу, не применяются к текущей CRM.


> **Текущий путь разработки CRM:** только `apps/cloudflare-crm` в канонической
> удалённой ветке `dev`. Действуют `apps/cloudflare-crm/AGENTS.md`,
> `apps/cloudflare-crm/ARCHITECTURE.md` и очередь [#484](https://github.com/iamaman11/crm/issues/484) со ссылкой на текущий курсор. Нижеследующие старые
> инструкции по PostgreSQL, RLS, packet/PR и дополнительным runtime
> относятся только к историческому product-plane, не к активной CRM.
> Не восстанавливать старые генераторы, CI или инфраструктуру.
> **Обязательное функциональное начало каждого следующего чата:**
> [docs/crm-functional-execution-matrix.md](docs/crm-functional-execution-matrix.md):
> **сверка полноты функционального скелета SK-00…SK-09**, сопоставление
> [20 групп целевой CRM](docs/CRM_CAPABILITY_COVERAGE.md) и
> [полного продуктового плана](docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md)
> с текущими действующими Rust owner/Protobuf/Worker/D1/React/двумя Gate.
> Исторические 39 MVP ID и R1–R7 — только часть продукта:
> **после принятого R1 не переходить механически к R2 (крупный импорт)**.
> Прорабатывать доказанные пробелы и продуктовые волны по очереди основного плана.
> Командное назначение и двухпользовательский пилот завершают очередь; не считать старые
> PostgreSQL/SQLx/main acceptance доказательством готовности Cloudflare CRM.
> **OPEN в исторической матрице — обязанность проверить, не команда реализовать.**
> Сначала определить реальное незакрытое действие пользователя и проверить действующие typed
> owner/RPC/UI/tests и ранее принятые evidence; только затем сопоставлять старые операции.
> **До написания любой функции открыть её исторический код и отрицательные тесты**, сверить
> с текущей реализацией, записать A/B/C/D и proof в текущем курсоре. Не повторять уже действующие RPC.
> Существующее доказанное A принимается без нового кода/CI (при необходимости добрать только
> независимый Gate B); только доказанный B создаёт кодовый срез. Недопустим обход единой
> Protobuf → generated Connect → compiled policy → original Rust owner → D1 схемы.
> **Перенос — не копирование legacy:** до B оценить функцию глазами конечного пользователя:
> кому нужна, для какого реального действия, понятен ли путь в UI, есть ли необходимые данные,
> что решает проще существующий сценарий и какую сложность добавит. Ненужное обоснованно
> классифицировать C для MVP, полезное перенести как целостный пользовательский сценарий,
> сохраняя бизнес-инварианты. Решение/причины зафиксировать в текущем курсоре до кода; третьего gate нет.
> Единица работы — **законченный крупный пользовательский срез** (S-M1-A → S-M1-B → S-M2),
> а не микроизменение. После каждого среза **оба gate обязательны**: Gate A — штатный CI,
> workerd/D1/Chromium + exact SHA deploy; Gate B — независимый LIVE Universal Browser MCP
> на опубликованном том же SHA. Оба PASS либо NO-GO; статус в текущем курсоре.
> Все **93/93 crates, 532/532 старых Rust-файла, 6633 распознанные функции**
> привязаны к функциональным ID или X-01 review в матрице; это контроль полноты учёта,
> не доказанная behavioral parity. Исторические исходники и тесты не удалять до пооперационного A/B/C/D.


This file is the default operating guide for humans and coding agents working in this repository.

For the Cloudflare application and its CI, follow `apps/cloudflare-crm/AGENTS.md` and
`apps/cloudflare-crm/ARCHITECTURE.md`. Issue #484 sets the product order and links to the current issue for acceptance evidence. The app’s direct-dev, single-pipeline, development-API policy supersedes the
historical PostgreSQL packet/PR/compatibility process below for that application.
App Markdown contains stable rules only; Markdown-only pushes do not build or deploy CRM.

The active CRM runtime is `apps/cloudflare-crm`: one Rust Workers Worker, Connect router and D1.
The former PostgreSQL process runtime (`services/crm-api`, `crates/crm-application-runtime`)
was retired from the active workspace and CI on 2026-11-20. Its source and PostgreSQL migrations
remain historical reference only; do not run, extend or restore that runtime. Shared Rust domain
modules remain active only where linked into the Cloudflare Worker.

## 1. Start with the smallest correct context

Before changing code:

1. Read `docs/README.md` and choose the path matching the task.
2. Read `docs/SYSTEM_INVARIANTS.md`.
3. Read `docs/PROJECT_STATUS.md`, `docs/ACTIVE_PACKET.md` and the active GitHub issue.
4. Use `python scripts/repo.py explain <module-or-coordinate>` to locate the authoritative owner and published coordinate.
5. Inspect only the contracts, manifest, implementation and tests selected by that path.
6. Run `python scripts/repo.py packet-check --base origin/main` before claiming packet closure.

For architecture-sensitive work also read:

- `docs/ARCHITECTURE_READINESS.md`;
- `docs/APPLICATION_ARCHITECTURE.md`;
- `docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`;
- relevant accepted ADRs under `docs/adr/`.

For delivery and module work read:

- `docs/DEVELOPMENT_WORKFLOW.md`;
- `docs/MODULE_DEVELOPMENT.md`;
- `docs/MULTI_AGENT_DEVELOPMENT.md` when more than one agent participates;
- `docs/CODEX_AGENT_QUALIFICATION.md` when a local Codex agent is used.

When descriptive documents disagree, precedence is:

`SYSTEM_INVARIANTS` → published contracts and accepted ADRs → architecture documents → normative execution plans → `PROJECT_STATUS` → active issue/PR → generated/orientation documents.

Generated `ACTIVE_PACKET` and repository-map files are navigation outputs, not sources of runtime or delivery truth. Completion requires merged implementation plus applicable exact-head acceptance evidence.

## 2. Repository concepts

- **Business module:** independently governed owner or link module under `modules/`; not necessarily a microservice.
- **Owner module:** the single authority for one mutable business domain.
- **Link module:** optional cross-domain coordination over published events/capabilities with its own private coordination state.
- **Platform crate:** reusable technical component under `crates/`; not counted as a business module.
- **Service:** deployable composition root; owns no business domain state.
- **Capability:** the supported state-changing business entry point.
- **Query:** permission-bound read path separate from mutation semantics.
- **Event:** immutable versioned integration evidence; consumers are idempotent.
- **Projection/read model:** rebuildable non-authoritative state.
- **Delivery packet:** one coherent architecture result with explicit ownership, acceptance and rollback/failure behavior.
- **Verification checkpoint:** immutable commit SHA plus a defined check set.

## 3. Non-negotiable dependency direction

```text
transport/service
  → ingress
    → generic application runtime
      → owner-owned production contribution
        → application use cases and ports
          → pure owner-domain module
            → stable contracts and governed SDK ports

infrastructure adapters
  → stable application/runtime ports
  → PostgreSQL / external systems
```

Forbidden shortcuts:

- business module → PostgreSQL, SQLx, broker, arbitrary HTTP, secrets or provider SDK;
- business module → another business module's Rust types, repositories or storage;
- public transport → domain or persistence implementation;
- query path → mutation idempotency/business-transaction semantics;
- projection/search/cache → authoritative business ownership;
- generic router/worker → switch on business capability, query, worker or module identifiers.

Do not weaken an architecture gate to make a feature pass.

## 4. Cost-of-change rule

A normal capability added to an existing owner:

- creates **zero new crates**;
- adds an internal command, query or worker to the existing application package;
- reuses existing adapters unless a real boundary changes;
- extends the module-owned production contribution;
- changes no generic router or worker algorithm;
- adds owner-specific tests and reuses generic conformance;
- runs the explainable affected closure plus required specialized gates.

A new crate is allowed only for a real dependency, trust, reuse, process, lifecycle or extraction boundary. A handler, planner, re-export or capability-specific composition function is not sufficient justification.

Feature behavior and crate consolidation must be separate PRs.

## 5. How to add or change a business feature

1. Resolve the exact owner and coordinate with `repo.py explain`.
2. Identify explicit out-of-scope domains.
3. Inspect the owner manifest, public contracts and existing application/postgres/production packages.
4. Change typed domain invariants without infrastructure access.
5. Evolve public contracts only when public meaning changes; preserve published versions.
6. Add application commands/queries/workers and stable ports.
7. Add persisted-state conversion separately from public wire contracts.
8. Add deterministic adapters and pre-authorization semantic validators.
9. Extend the module-owned production contribution.
10. Use durable tenant module-installation state; never add an activation bypass.
11. Add tenant, authorization, idempotency/retry, rollback and negative acceptance as applicable.
12. Update route classifications, generated registries and status/catalog sources when their scope changes.
13. Run `repo.py affected`, `check-affected`, `packet-check` and every selected specialized gate.

Do not start from a controller, table or UI component and invent ownership afterward.

## 6. Cross-domain behavior

Prefer an optional link module:

```text
source owner mutation
  → transactional outbox event
    → governed event delivery
      → link-owned deterministic coordination state
        → governed CapabilityClient
          → target owner capability
```

The link must be independently installable, disableable and uninstallable. It cannot read or mutate source/target storage directly.

## 7. Governed execution

Mutation:

```text
authentication
→ tenant/actor context
→ exact owner/capability/version route
→ durable activation
→ typed and pre-authorization semantic validation
→ rate/approval policy
→ final live authorization
→ deterministic planning and one governed transaction
```

Query:

```text
authentication
→ tenant/actor context
→ exact owner/query/version route
→ durable activation
→ typed and pre-authorization semantic validation
→ live authorization/visibility
→ authoritative tenant-scoped read and masking
```

Public transport must not contain domain decisions or persistence calls. Mutation-only idempotency or business-transaction fields never leak into query contracts.

## 8. Composition, persistence and Rust API rules

`apps/cloudflare-crm/rust-party` is the sole active CRM runtime and composition root. The former
`services/crm-api` / `crates/crm-application-runtime` PostgreSQL process host is retired and
excluded from the active workspace/CI. Business functionality enters through the Cloudflare
Worker and original Rust owner modules.

Persistence rules:

- every owner has explicit storage namespaces and migration ownership;
- no cross-owner table edits without an accepted ownership-transfer ADR;
- tenant context and FORCE RLS remain database-enforced;
- forward, rollback/schema-removal, reapply and cross-tenant negative evidence are required when applicable;
- persisted envelopes are versioned independently from public wire contracts;
- destructive changes require retention, legal-hold, audit and recovery implications to be explicit.

Rust API rules:

- default implementation visibility is private or `pub(crate)`;
- cross-package APIs are limited to published contracts, stable ports and contribution interfaces;
- concrete adapters and repositories are not re-exported for convenience;
- shared DTOs do not replace owner-domain value objects;
- a new public symbol requires a consumer and compatibility rationale.

## 9. Repository navigation and commands

Implemented navigation:

```bash
python scripts/repo.py explain crm.customer-privacy
python scripts/repo.py explain customer_privacy.case.submit@1.0.0
python scripts/repo.py packet-check --base origin/main
python scripts/generate_repository_navigation.py --check
```

Use `docs/generated/REPOSITORY_MAP.md` for deterministic workspace/module/route inventory. Unknown or ambiguous explain targets must fail closed.

Implemented validation and local lifecycle entry points include:

```bash
python scripts/repo.py doctor
python scripts/repo.py doctor --profile bootstrap
python scripts/repo.py bootstrap --dry-run
python scripts/repo.py bootstrap
python scripts/repo.py conformance
python scripts/repo.py affected --base origin/main
python scripts/repo.py check-affected --base origin/main
python scripts/repo.py packet-check --base origin/main
python scripts/repo.py quality
```

`doctor` reads repository-pinned tool requirements and never changes the machine. Neither profile requires Docker or a database service. `bootstrap` uses an isolated `.venv`, committed Python constraints, the active Worker Cargo lock and the pnpm lockfile; it does not silently install or globally switch system toolchains. The active runtime and its D1/workerd/Chromium acceptance are owned by the single Cloudflare CRM pipeline.

## 10. Required checks before completion

Run the checks selected by `packet-check`, then every applicable specialized gate:

- Contract CI for Protobuf or contract changes;
- Governance CI for manifests, normalized IR, architecture policy and repository governance;
- Rust CI for architecture, lockfile, formatting, Clippy and workspace tests;
- the Cloudflare CRM CI for Rust/WASM, workerd, D1, Chromium and protected release evidence;
- repository governance and contract checks when their scopes are affected.

A packet is complete only when applicable workflows are green on one unchanged exact review head and documentation reflects merged behavior.

## 11. Multi-agent exact-SHA rule

```text
Architect / Implementer
  = scope, architecture, primary implementation and fixes

Local Integrator / Verifier
  = exact-SHA checkout, independent build/test and structured report

GitHub CI
  = final exact-head merge authority
```

Rules:

1. Overlapping code has one primary writer at a time.
2. Verification defaults to `VERIFY_ONLY` until an explicit handoff grants write authority.
3. Every handoff and report names the exact SHA, mode, affected scope and commands.
4. A new commit invalidates earlier evidence for checks not rerun.
5. Local green status accelerates feedback but never replaces applicable GitHub gates.
6. Do not rely on chat-only state when it can be committed or recorded in the issue/PR.

## 12. Change discipline

- Keep PRs coherent and reviewable.
- Do not mix unrelated refactors, consolidation and product behavior.
- Prefer measured incremental structural changes over a big-bang rewrite.
- Preserve compatibility, rollback and operational semantics.
- Treat stale documentation or generated navigation as a defect.
- Record temporary architecture exceptions with owner, reason, expiry, removal condition and compensating checks.
- Do not claim product completion while required owner domains or production gates remain open.

See `docs/PROJECT_STATUS.md` and active GitHub issues for the live product and architecture packets. Orientation documents must not independently change phase bookkeeping.

# CRM #352 — функция → исторический Rust → действующий owner → приемка

**Назначение:** долговечная исполнимая очередь разработки тестовой CRM, а не просто индекс старых crate. Открывать при каждом следующем сеансе **до** выбора задач и изменения кода. Правило приоритетов: `M1 → M2 → NEXT (по доказанному спросу) → LATER → END`. Ни один пункт LATER/END не блокирует готовность MVP. **Первое действие: M1-01.** Нельзя превращать исходную бизнес-логику в отдельный старый runtime.

**Доказательная база:** [инвентаризация всех 93 crates](legacy-crate-semantic-matrix.md) · [все 532 Rust-файла и test/guard symbols](legacy-crate-operation-evidence.md) · [20 глубоких gap comparisons](legacy-crate-capability-audit.md). Исходный снимок [`a627e6a9f196`](https://github.com/iamaman11/crm/commit/a627e6a9f1966aaac5fcc0179c943811f879eb38); успешно проверенный основной CI+deploy [#44913591777](https://github.com/iamaman11/crm/actions/runs/44913591777). Поздние docs-only коммиты НЕ означают новый опубликованный Worker. Каждая из 39 строк содержит ссылки на реально существующие старые и нынешние файлы, проверенные по Git tree.

## Правило для следующего чата/исполнителя (обязательно, не рекомендация)

1. Прочитать [#352](https://github.com/iamaman11/crm/issues/352), `apps/cloudflare-crm/AGENTS.md`, **эту матрицу** и текущий `dev`. Выбрать первый **непринятый** ID в приоритетном порядке. Техническое наличие уже работающего метода НЕ значит принятую пользовательскую историю.
2. **До изменения кода обязательно открыть источники из поля «Legacy» выбранного ID**, связанные с ними негативные тесты, вложенные modules/re-exports и их consumers (по [evidence](legacy-crate-operation-evidence.md)). Записать инварианты/ветки: вход, успешный выход, actor/tenant, права, version/CAS, idempotency/replay, audit/receipt, side effects. Если старой реализации именно этой операции нет — так и записать, без домыслов.
3. **Сверить «Сейчас»**, один original Rust owner, действующий `.proto`, generated Rust/TS, RPC/manifest/compiled policy, D1/transaction, React и браузерные тесты. Поставить **по операции** вердикт `A` (эквивалентность доказана), `B` (бизнес-gap к реализации), `C` (не нужен сознательно), `D` (legacy infrastructure, когда доказаны инварианты и все consumers), иначе `OPEN`. Наличие legacy crate — это не команда перепортировать crate.
4. Делать только gap в существующей архитектуре: один Worker + D1 + Buf Protobuf + original owners + Connect/React + один штатный CI. Не новые генераторы, registry, worker, SQLx/PostgreSQL, generic framework и обратная совместимость тестовой БД.
5. После среза — единственный действующий CI с native workerd/D1/Chromium/Access/audit, exact SHA publish, применимый независимый Browser Gate B. В #352 записать **ID, исходный метод/отрицательный тест, точный B-gap либо решение C, SHA, CI/live/Gate B, доказательство и остаток**. Только после доказательств пометить ID как ACCEPTED. **Новый чат продолжает с первого непринятого ID, без повторных планов.**
6. Действующие auth, tenant, signed identity, policy, CAS/idempotency, audit, PII redaction и fail-closed действуют **сейчас**. Partial Privacy 2/9 `complete=false` остаётся закрыт для export/erasure. Полная 93-crate behavioral parity **ещё не доказана**, поэтому старый исходный код не удалять массово.

## Упорядоченная матрица бизнес-функций

| ID | Очередь | Функция | Уже имеется | Историческая реализация (вскрыть до работы) |
|---|---|---|---|---|
| [M1-01](#m1-01) | M1 | Выбор по имени вместо ID | UX GAP | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs) |
| [M1-02](#m1-02) | M1 | Люди и организации — CRUD | ACTIVE/VERIFY | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) |
| [M1-03](#m1-03) | M1 | Клиент (Account) из Party | ACTIVE/VERIFY | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs) |
| [M1-04](#m1-04) | M1 | Контактные точки / verified email | ACTIVE/UX GAP | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter/src/planner.rs) |
| [M1-05](#m1-05) | M1 | Связи Party ↔ Party | ACTIVE/UX GAP | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter/src/planner.rs) |
| [M1-06](#m1-06) | M1 | Навигация Account→Contacts/Deals/Tasks | PARTIAL/UX GAP | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs) |
| [M1-07](#m1-07) | M1 | Создание и изменение Deal | ACTIVE/VERIFY | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs) |
| [M1-08](#m1-08) | M1 | Задача — создать, изменить, выполнить | ACTIVE/UX GAP | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs) |
| [M1-09](#m1-09) | M1 | Sales→Task внутренняя связь | PARTIAL/PROOF | [src/link_event_processor.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/link_event_processor.rs) |
| [M1-10](#m1-10) | M1 | Единая приемка Party→Account→Deal→Task | NOT ACCEPTED | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) |
| [M1-11](#m1-11) | M1 | Архивирование и чистка тестовых записей | DECISION/CONDITIONAL | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) |
| [M2-01](#m2-01) | M2 | Sales pipeline, стадии, Kanban | ACTIVE/VERIFY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/lib.rs) |
| [M2-02](#m2-02) | M2 | Основной контакт Deal | ACTIVE/VERIFY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs) |
| [M2-03](#m2-03) | M2 | Сохраненные views и фильтры Sales | ACTIVE/VERIFY | [src/query_catalog.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/query_catalog.rs) |
| [M2-04](#m2-04) | M2 | Sales: прогноз, точная математика и конверсия | ACTIVE/VERIFY | [src/phase6_projections.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/phase6_projections.rs) |
| [M2-05](#m2-05) | M2 | Lead qualification как отдельная сущность? | NOT PROVEN/DECIDE | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs) |
| [N-01](#n-01) | NEXT | Bounded CSV import и ошибки строк | PARTIAL/ACTIVE | [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs) |
| [N-02](#n-02) | NEXT | Bounded CSV export | PARTIAL/ACTIVE | [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs) |
| [N-03](#n-03) | NEXT | Customer360 — недостающие связи карточки | PARTIAL/ACTIVE | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs) |
| [N-04](#n-04) | NEXT | Data Quality проверка и ручное исправление | PARTIAL/ACTIVE | [src/remediation_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs) |
| [N-05](#n-05) | NEXT | Дубликаты: поиск и сравнение | PARTIAL/READONLY | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/planner.rs) |
| [N-06](#n-06) | NEXT | Consent grant/withdraw/evaluate | ACTIVE/GUARD | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/planner.rs) |
| [L-01](#l-01) | LATER | Durable CSV import/checkpoints/rows | SOURCE ONLY | [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs) |
| [L-02](#l-02) | LATER | Durable export/artifact/recovery | SOURCE ONLY | [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs) |
| [L-03](#l-03) | LATER | Identity merge/unmerge and topology | SOURCE ONLY | [src/merge_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/merge_planner.rs) |
| [L-04](#l-04) | LATER | Data Quality findings and remediation | SOURCE ONLY | [src/finding_stewardship_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs) |
| [L-05](#l-05) | LATER | Enrichment provider dispatch/secret/quota | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-registry/src/lib.rs) |
| [L-06](#l-06) | LATER | Enrichment suggestion human review | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/src/lib.rs) |
| [L-07](#l-07) | LATER | Enrichment materialization to Party owner | SOURCE ONLY | [src/process.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/process.rs) |
| [L-08](#l-08) | LATER | Global search / index / reindex | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-global-search-composition/src/lib.rs) |
| [L-09](#l-09) | LATER | Relationship hierarchy projection | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs) |
| [E-01](#e-01) | END | Privacy subject verification and preview 2/9 | PARTIAL/PAUSED | [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs) |
| [E-02](#e-02) | END | Privacy remaining 7 owner scopes and freeze | SOURCE ONLY | [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs) |
| [E-03](#e-03) | END | Privacy planning/approval/legal hold/retention | SOURCE ONLY | [src/planning.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs) |
| [E-04](#e-04) | END | Privacy governed subject access export | SOURCE ONLY | [src/access_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/access_export.rs) |
| [E-05](#e-05) | END | Privacy owner erasure/execution/restriction | SOURCE ONLY | [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/execution.rs) |
| [E-06](#e-06) | END | Metadata / admin / module registry | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/lib.rs) |
| [E-07](#e-07) | END | Enterprise CPQ/service/AI/marketplace | NOT REQUIRED MVP | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-workflow/src/lib.rs) |
| [E-08](#e-08) | END | Production operations / DR / реальная приватность | OUTSIDE TEST MVP | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/lib.rs) |

## Карточки реализации (правило, прежняя реализация, текущий код, конкретная приёмка)

### M1-01

**Бизнес-функция:** Выбор по имени вместо ID. **Очередь:** M1. **Что уже есть:** UX GAP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ContactScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ContactScreen.tsx); [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx); [src/TaskScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/TaskScreen.tsx); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [src/Pickers.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/Pickers.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Использовать уже существующие PartyPicker/AccountPicker и текущие List RPC. Не создавать Search runtime.

**DONE только при:** На 4 экранах фильтры по имени вместо ввода ID; выбор, пустое состояние, cursor, refresh; tenant denied.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-02

**Бизнес-функция:** Люди и организации — CRUD. **Очередь:** M1. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-parties/src/domain.rs); [src/PartyScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/PartyScreen.tsx); [connect_sales/party_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не переносить уже существующий CRUD. Сверить правила типов, версии, auth/replay со старым planner.

**DONE только при:** Создать/найти/изменить Person и Organization; reload, stale version, tenant/actor denied, audit.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-03

**Бизнес-функция:** Клиент (Account) из Party. **Очередь:** M1. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-accounts/src/domain.rs); [src/AccountScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountScreen.tsx); [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [connect_sales/account_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить существующий AccountCreator/Editor, owner и Party reference; не вводить master модель.

**DONE только при:** Party→Account, роли и статус, изменение и карточка после reload; CAS, missing/cross-tenant ref denied.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-04

**Бизнес-функция:** Контактные точки / verified email. **Очередь:** M1. **Что уже есть:** ACTIVE/UX GAP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-contact-points/src/domain.rs); [src/ContactScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ContactScreen.tsx); [connect_sales/contact_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить нынешние Create/Update/Get/List/VerifyOwnEmail, сделать поиск Party по имени; не подменять signed human service principal.

**DONE только при:** Email/phone сохраняются, status/validity/verification правильны; чужой owner/tenant, stale, fake identity rejected.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-05

**Бизнес-функция:** Связи Party ↔ Party. **Очередь:** M1. **Что уже есть:** ACTIVE/UX GAP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-party-relationships/src/domain.rs); [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx); [connect_sales/relationship_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Показывать понятные имена вместо 2 ID; старые planner-инварианты ролей/направления сохранить.

**DONE только при:** Создать/редактировать связь по выбранным именам; роли, порядок, временные границы и tenant верны после reload.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-06

**Бизнес-функция:** Навигация Account→Contacts/Deals/Tasks. **Очередь:** M1. **Что уже есть:** PARTIAL/UX GAP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [src/routes.ts](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/routes.ts); [src/navigation.ts](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/navigation.ts); [connect_sales/overview_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/overview_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Использовать GetAccountOverview и owner reads; не переносить projection runtime.

**DONE только при:** Переходы и обратные ссылки по названиям, Back/refresh, правильный Account/tenant.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-07

**Бизнес-функция:** Создание и изменение Deal. **Очередь:** M1. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [src/DealRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DealRecord.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Переиспользовать existing Deal/Account/Contact pickers, exact money и оригинальный Sales owner.

**DONE только при:** Account→Deal, primaryContact, stage, amount; save/reload, currency precision, CAS/replay/audit.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-08

**Бизнес-функция:** Задача — создать, изменить, выполнить. **Очередь:** M1. **Что уже есть:** ACTIVE/UX GAP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-activities/src/domain.rs); [src/TaskScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/TaskScreen.tsx); [connect_sales/task_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не переписывать Create/Update/CompleteTask; поправить Account filter, статусы, сроки и понятные ссылки.

**DONE только при:** Из Account создать Task, изменить, выполнить; readback, stale CAS, tenant и idempotency.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-09

**Бизнес-функция:** Sales→Task внутренняя связь. **Очередь:** M1. **Что уже есть:** PARTIAL/PROOF. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/link_event_processor.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/link_event_processor.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales-activities-link/src/lib.rs); [src/DealRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DealRecord.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить реальный internal typed v1 consumer и атомарную D1 границу; не заменять новым orchestration.

**DONE только при:** Предусмотренная linked Task создаётся не более одного раза; failed write не даёт partial side effect; receipt/audit PASS.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-10

**Бизнес-функция:** Единая приемка Party→Account→Deal→Task. **Очередь:** M1. **Что уже есть:** NOT ACCEPTED. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [test/typed-browser.mjs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/test/typed-browser.mjs); [e2e/live.mjs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/e2e/live.mjs); [src/app.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/app.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не новый тестовый фреймворк: расширить штатный native workerd/Chromium и live/browser сценарий.

**DONE только при:** Один связанный пользовательский поток и negative tenant/CAS/replay; единственный CI+publish exact SHA, отдельный Gate B.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M1-11

**Бизнес-функция:** Архивирование и чистка тестовых записей. **Очередь:** M1. **Что уже есть:** DECISION/CONDITIONAL. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [src/ContactScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ContactScreen.tsx); [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сначала использовать уже существующий inactive/archive. Не считать Delete реализованным в старом CRUD; физическое удаление только при необходимости.

**DONE только при:** Документированное решение C или B; если B — owner-mediated archive/disable без нарушения ссылок/аудита.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M2-01

**Бизнес-функция:** Sales pipeline, стадии, Kanban. **Очередь:** M2. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/lib.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/pipeline.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/pipeline.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Существующие Create/Update/ListPipeline, ListStageDeals и AdvanceStage. DnD должен вызывать единственный путь.

**DONE только при:** Создание/изменение этапов, stage move, archive, reopen, no invalid stage, stale CAS denied.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M2-02

**Бизнес-функция:** Основной контакт Deal. **Очередь:** M2. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/DealRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DealRecord.tsx); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** DealContactPicker уже есть; проверка принадлежности активного контакта Account должна жить в Sales owner.

**DONE только при:** Чужой или inactive контакт не сохраняется; выбранный контакт и Account корректны после reload.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M2-03

**Бизнес-функция:** Сохраненные views и фильтры Sales. **Очередь:** M2. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/query_catalog.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/query_catalog.rs); [src/definitions.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/definitions.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/views.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/views.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Повторно использовать Create/Update/Get/ListSalesView, ListDeals, ListStageDeals, не generic filter engine.

**DONE только при:** Views/filters сохранены после reload; cursor, status, pipeline, account и tenant совпадают.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M2-04

**Бизнес-функция:** Sales: прогноз, точная математика и конверсия. **Очередь:** M2. **Что уже есть:** ACTIVE/VERIFY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/phase6_projections.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/phase6_projections.rs); [src/executor.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/executor.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** GetPipelineMetrics уже есть: проверять expected close/no date, cohort denominator, currency и probability.

**DONE только при:** Суммы и win rate совпадают с D1 owner, 0/100% и no-date не теряются; React не присваивает бизнес-решения.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### M2-05

**Бизнес-функция:** Lead qualification как отдельная сущность?. **Очередь:** M2. **Что уже есть:** NOT PROVEN/DECIDE. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Историческая отдельная Lead сущность здесь не доказана. Проверить, достаточна ли начальная стадия Deal; не строить новую сущность заранее.

**DONE только при:** Явно C (stage-based достаточно) либо B с конкретным контрактом/сценарием и proof; не принимать по названию roadmap.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-01

**Бизнес-функция:** Bounded CSV import и ошибки строк. **Очередь:** NEXT. **Что уже есть:** PARTIAL/ACTIVE. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx); [connect_sales/data_operations_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Уже работает preview/apply для ≤16KiB и ≤20 строк; улучшать только реальные UX ошибки, не вводить job framework.

**DONE только при:** Невалидные строки имеют причину, нет частичной записи, digest и idempotency корректны.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-02

**Бизнес-функция:** Bounded CSV export. **Очередь:** NEXT. **Что уже есть:** PARTIAL/ACTIVE. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs); [src/export_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx); [connect_sales/data_operations_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Preview и download уже существуют. Не путать с privacy-экспортом, не добавлять durable artifact без нужды.

**DONE только при:** Фильтр Person/Organization, корректный CSV, tenant authorization, version semantics.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-03

**Бизнес-функция:** Customer360 — недостающие связи карточки. **Очередь:** NEXT. **Что уже есть:** PARTIAL/ACTIVE. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [connect_sales/overview_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/overview_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** GetAccountOverview уже есть; добавлять только недостающие read-данные. Projection — только при доказанной необходимости.

**DONE только при:** Текущий linked summary после reload и permission-aware reads, нет чужих персональных полей.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-04

**Бизнес-функция:** Data Quality проверка и ручное исправление. **Очередь:** NEXT. **Что уже есть:** PARTIAL/ACTIVE. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/remediation_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs); [src/finding_stewardship_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ReviewScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ReviewScreen.tsx); [connect_sales/quality_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/quality_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** EvaluatePartyName уже работает; сперва исправить Party через его owner, не делать findings/job engine автоматически.

**DONE только при:** Плохие данные обнаружены и исправлены существующей записью; повторная проверка и права корректны.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-05

**Бизнес-функция:** Дубликаты: поиск и сравнение. **Очередь:** NEXT. **Что уже есть:** PARTIAL/READONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/planner.rs); [src/owner_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/owner_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ReviewScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ReviewScreen.tsx); [connect_sales/identity_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** FindSharedEmailCandidates/CompareParties доступны; не внедрять Merge на MVP.

**DONE только при:** Совпадение email — лишь кандидат, не identity proof; read tenant/PII boundaries.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-06

**Бизнес-функция:** Consent grant/withdraw/evaluate. **Очередь:** NEXT. **Что уже есть:** ACTIVE/GUARD. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/planner.rs); [src/owner_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/owner_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ConsentScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ConsentScreen.tsx); [connect_sales/consent_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить действующий consent guard; интеграцию с отправкой строить только при реальной отправке.

**DONE только при:** Отзыв разрешения блокирует применимый внешний эффект; signed actor, replay/tenant proofs.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-01

**Бизнес-функция:** Durable CSV import/checkpoints/rows. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-source-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-data-operations/src/execution.rs); [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Нужен только при объёмах за пределом bounded CSV; переносить инварианты, не PostgreSQL coordinator.

**DONE только при:** Resume после fault, row outcome/duplicates/replay, версионный progress в D1.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-02

**Бизнес-функция:** Durable export/artifact/recovery. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs); [src/privacy_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-data-operations/src/export.rs); [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только при реальном длительном или регулируемом экспорте; не путать с CSV.

**DONE только при:** Stable artifact digest, version binding, expiry, authorized download, replay proof.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-03

**Бизнес-функция:** Identity merge/unmerge and topology. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/merge_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/merge_planner.rs); [src/current_canonical.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition/src/current_canonical.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-merge-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/merge_lineage.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-identity-resolution/src/merge_lineage.rs); [connect_sales/identity_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сейчас read-only; сохранить reversible lineage и канонические ссылки, не переносить Postgres transaction.

**DONE только при:** Никаких потерянных links; stale merge denied, unmerge reversible, audit/replay.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-04

**Бизнес-функция:** Data Quality findings and remediation. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/finding_stewardship_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs); [src/remediation_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-data-quality/src/domain.rs); [src/ReviewScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ReviewScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только при конкретном процессе stewardship. Не generic job/rule framework.

**DONE только при:** Persist/assign/remediate/re-evaluate, stale evidence denied, owner-only mutation.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-05

**Бизнес-функция:** Enrichment provider dispatch/secret/quota. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-registry/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-worker-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-registry-http-transport/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/EnrichmentScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/EnrichmentScreen.tsx); [connect_sales/enrichment_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Активен лишь publish/get/list configuration; нужен выбранный реальный provider.

**DONE только при:** Endpoint/secret/consent/quota/timeout/circuit, sanitized result, D1 provenance.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-06

**Бизнес-функция:** Enrichment suggestion human review. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/lifecycle.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-enrichment/src/lifecycle.rs); [src/EnrichmentScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/EnrichmentScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только после L-05; не путать сохранённый mapping с результатом обогащения.

**DONE только при:** Approve/reject с версионированной policy, expiry и stale guard, no unwanted owner write.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-07

**Бизнес-функция:** Enrichment materialization to Party owner. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/process.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/process.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/materialization.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-enrichment/src/materialization.rs); [connect_sales/enrichment_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** После L-05/L-06; использовать existing Party mutation, не generic materialization engine.

**DONE только при:** Один approved suggestion→один owner UpdateParty; stale/CAS/replay/provenance.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-08

**Бизнес-функция:** Global search / index / reindex. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-global-search-composition/src/lib.rs); [src/generation.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-runtime/src/generation.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/SearchField.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SearchField.tsx); [connect_sales/party_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только когда current owner search недостаточен по измеримому сценарию.

**DONE только при:** Read-time ACL and hidden-field non-disclosure, tenant, rebuild/permission revoke.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-09

**Бизнес-функция:** Relationship hierarchy projection. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx); [connect_sales/relationship_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Прямые owner reads сначала; projection only with clear user scenario.

**DONE только при:** Directed/reverse/effective edges, tenant and rebuild state correct.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-01

**Бизнес-функция:** Privacy subject verification and preview 2/9. **Очередь:** END. **Что уже есть:** PARTIAL/PAUSED. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/PrivacyScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/PrivacyScreen.tsx); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить действующие VerifyOwnPrivacyCaseSubject + PreviewOwnPrivacyScope; не расширять сейчас.

**DONE только при:** Signed real human/tenant proof, service denied, 2/9 complete=false; export/erasure blocked.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-02

**Бизнес-функция:** Privacy remaining 7 owner scopes and freeze. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs); [src/planning.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/domain.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Когда будет требование реальных данных: all 9 real owner scans, no fake empty scopes.

**DONE только при:** 9/9 bounded scans, immutable snapshot, completeness & version digest, fail closed.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-03

**Бизнес-функция:** Privacy planning/approval/legal hold/retention. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planning.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs); [src/retention.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/retention.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/domain.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только после E-02 и требований реальных данных.

**DONE только при:** Trusted case+scope, approved plan, correct hold/retention, D1 audited decisions.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-04

**Бизнес-функция:** Privacy governed subject access export. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/access_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/access_export.rs); [src/privacy_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/access_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/access_export.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не подменять bounded CSV; без E-02/E-03 экспорта нет.

**DONE только при:** Signed human+complete scope, immutable versioned artifact, allowed recipient and audit.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-05

**Бизнес-функция:** Privacy owner erasure/execution/restriction. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/execution.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/execution.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не делать partial erasure; нужен весь workflow и legal conditions.

**DONE только при:** Approved complete scope, legal hold checks, idempotent owner checkpoint, audit.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-06

**Бизнес-функция:** Metadata / admin / module registry. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-registry/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [cloudflare-crm/ARCHITECTURE.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/ARCHITECTURE.md); [cloudflare-crm/AGENTS.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/AGENTS.md). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не подключать старый runtime без конкретного бизнес-требования.

**DONE только при:** Без второго источника схемы/registry/worker; explicit product decision.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-07

**Бизнес-функция:** Enterprise CPQ/service/AI/marketplace. **Очередь:** END. **Что уже есть:** NOT REQUIRED MVP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-workflow/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-events/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-integrations/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [cloudflare-crm/ARCHITECTURE.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/ARCHITECTURE.md). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Наличие планов/контрактов не доказывает старую работающую бизнес-реализацию.

**DONE только при:** Каждая новая область с отдельным владельцем и ценностью, не по инерции roadmap.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-08

**Бизнес-функция:** Production operations / DR / реальная приватность. **Очередь:** END. **Что уже есть:** OUTSIDE TEST MVP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [cloudflare-crm/AGENTS.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/AGENTS.md); [cloudflare-crm/README.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/README.md). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не возвращать PostgreSQL/SQLx process для production-гарантий.

**DONE только при:** Отдельные требования реальных пользователей, нагрузка/DR/право, без ложного production-ready.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

## Пределы этапов

**M1:** ID M1-01…M1-10 приняты по UI+owner+D1+signed CI/browser; M1-11 явно решён (B или C), не бесконечный спор об удалении. **M2:** M2-01…M2-04 доказаны; M2-05 имеет чёткое B/C решение, а не новый модуль по умолчанию. **MVP:** M1/M2 и только необходимые NEXT, без LATER/END. Полная **93/93** исходная карта существует в двух связанных документах, но не означает полную parity старых алгоритмов.

**Запрет на забывание:** новый чат начинается с чтения этого файла и #352. Если отсутствует сравнение с историческим кодом для выбранного ID, работа над ним **не может называться DONE**. Результат конкретной функции — строка `ID → source/tests → current mapping → A/B/C/D → exact evidence`, а не ещё одна глобальная матрица или промежуточный отчёт.

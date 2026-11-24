# CRM — документация и порядок работы

> **Единственный канонический репозиторий:** [iamaman11/crm](https://github.com/iamaman11/crm), ветка `dev`. **Текущие планы, решения и результаты ведутся в GitHub Issues.** Этот файл — только навигация, а не второй план.

## Текущая CRM

| Вопрос | Единственный действующий источник |
|---|---|
| Полный продукт, 20 семейств и очередность | [**Issue #484 — основной продуктовый план**](https://github.com/iamaman11/crm/issues/484) |
| Конкретный текущий срез, A/B/C/D, Gate A и Gate B | [**Issue #487 — единственный исполнительный курсор**](https://github.com/iamaman11/crm/issues/487) |
| Строгая техническая архитектура | [`apps/cloudflare-crm/ARCHITECTURE.md`](../apps/cloudflare-crm/ARCHITECTURE.md) |
| Правила разработки и проверки | [`apps/cloudflare-crm/AGENTS.md`](../apps/cloudflare-crm/AGENTS.md), [корневой `AGENTS.md`](../AGENTS.md) |
| Типизированная схема бизнеса | `proto/crm/**` + Buf, generated Connect Rust/TS, зарегистрированный Rust Router, одна fail-closed policy, исходный Rust owner и одна Cloudflare D1 |
| Доказательства ранее принятого | [Архивный issue #352](https://github.com/iamaman11/crm/issues/352); архитектурная приёмка [#471](https://github.com/iamaman11/crm/issues/471) |
| Технические открытые замечания | [#485](https://github.com/iamaman11/crm/issues/485), [#486](https://github.com/iamaman11/crm/issues/486) — **не продуктовые курсоры** |

**Один рабочий путь:** открыть #484 → увидеть ссылку на текущий #487 → проверить ранее принятое и действующий код → найти конкретный полезный пробел → сверить исторические source/tests по архивному приложению → A/B/C/D → при B сделать один связный typed срез → штатный Gate A + отдельный LIVE Gate B на одном опубликованном SHA → записать результат **только в issue**. Не создавать новое нормативное MD-ТЗ, второй roadmap, вручную ведённый реестр или систему CI.

## Исторические источники, не действующие планы

- [Замороженный каталог 554 требований / 20 семейств](archive/cloudflare-snapshots/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md) — **снимок исходной инвентаризации**, статусы и приоритеты больше **не обновлять здесь**. Действующее решение — #484.
- [Замороженная функциональная матрица и сопоставление 93 crates](archive/cloudflare-snapshots/crm-functional-execution-matrix.md) — свидетельства старого кода и негативных тестов, **не отдельная очередь**. Действующее решение — #487.
- [Архив старой PostgreSQL/main платформы](archive/legacy-main/) — прежние `PROJECT_STATUS`, `IMPLEMENTATION_ROADMAP`, `PHASE8_DELIVERY_PLAN`, архитектура `crm-api`, governance и другие планы **не управляют** текущей Cloudflare CRM.
- [Детальное исходное сопоставление 93 crates](legacy-crate-operation-evidence.md), [семантический индекс](legacy-crate-semantic-matrix.md) — статические исходные свидетельства, не текущая готовность.

Старые пути `docs/*.md` с именами удалённых из активной навигации планов оставлены **только короткими указателями** на архив и GitHub Issues, чтобы не ломать исторические ссылки. В архиве сохранены полные тексты; ни один архивный документ не может требовать `main`, PostgreSQL/SQLx, `crm-api`, legacy gateway, отдельного транспорта, ручных DTO или дополнительных CI.

Обязательный путь новой функции: **versioned .proto → Buf → generated Protobuf/Rust + TypeScript → binary Connect → единая Rust policy → original Rust domain owner → одна D1**; UI — React strict TSX через один generated client. Все данные `viewer` доступны в рамках tenant, запись только по ролям. Приёмка — штатный CI/workerd/D1/Chromium и **независимый Universal Browser MCP** после публикации точного SHA.

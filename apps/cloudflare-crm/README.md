# CRM Dev — нативное приложение Cloudflare

Единственный источник кода и состояния разработки — [iamaman11/crm](https://github.com/iamaman11/crm), актуальный план — [Issue #338](https://github.com/iamaman11/crm/issues/338).

## Как устроено приложение

```text
React 19 / Vite (Cloudflare Static Assets)
  → Cloudflare Access (подписанная личность)
  → один Worker workers-rs (Rust/WASM; один HTTP entrypoint)
  → исходные Rust-модули из modules/ (единственные владельцы бизнес-правил)
  → нативный D1 binding DB → одна база crm-dev
```

Без JS API-маршрутизатора, отдельного Worker, PostgreSQL, Docker и дополнительной базы данных. Worker проверяет подписанный Access JWT, сопоставляет пользователя с tenant/ролью в D1, маршрутизирует запросы и вызывает исходный Rust-домен. Только оператор с ролью admin изменяет данные, прочие зарегистрированные пользователи читают данные своего tenant. Никогда не доверяем клиентским заголовкам tenant или пользователя.

### Границы ответственности

- **UI** — `web/src/main.jsx`, `Parties.jsx`, `Customer360.jsx`. `App` владеет списками Party/Account/ContactPoint/Deal/Task; дочерние экраны получают данные через props. Формы передают идентификаторы, версии и параметры API, но не выбирают конечный результат доменных переходов.
- **HTTP, Access, диагностика** — `rust-party/src/lib.rs`, `access.rs`; **Party HTTP/D1** — `rust-party/src/parties.rs`. Стабильные `x-request-id`, `x-crm-module`, `x-crm-error-code` и `x-crm-source-sha`; облачные структурированные Logs без токенов и пользовательских данных.
- **Адаптеры оригинальных модулей к D1** — `rust-party/src/accounts.rs`, `contact_points.rs`, `sales_deals.rs`, `activity_tasks.rs`, `party_relationships.rs`, `consents.rs`. Это **не второй бизнес-домен**: обработчики разбирают JSON в Rust-типы, вызывают исходные агрегаты и сохраняют их штатными D1-запросами.
- **Оригинальные бизнес-правила** — `modules/crm-*`. Межмодульный `sales_activities_link.rs` использует исходный Rust link-модуль и типизированный Protobuf-адаптер, коммит Deal/Task/receipt выполняется одной атомарной операцией D1.
- **Карточка клиента** — `account_overview.rs`, read-only композиция состояний владельцев; не отдельная клиентская таблица и не владелец изменений.
- **База** — одна D1 из `wrangler.jsonc`; отдельные таблицы состояний и квитанций по владельцам, общая таблица аудита. Миграции `migrations/0001_*.sql`–`0010_*.sql`.

## Что реализовано и что пока отсутствует

В пользовательском интерфейсе: Parties (создать, изменить), **PartyRelationships (создать связь двух лиц/организаций, изменить статус)**, **Consents (проверить разрешение и записать операторский запрет, без выдачи Grant)**, Accounts (создать, изменить, карточка), ContactPoints (создать, изменить; пользователь не может присвоить себе `verified`), Sales Deals (создать, изменить, продвинуть этап, таблица/воронка), Activities Tasks (создать и завершить). Поиск и обновление списка; данные переживают перезагрузку. Действуют Access, tenant-разделение и контроль версии. **Универсального Delete для этих оригинальных агрегатов нет.**

В **Identity Resolution** добавлено только безопасное чтение двух Party через оригинальный `CanonicalPartyPair` и tenant-bound D1: React → GET `/api/identity-resolution/compare/:left/:right` → один Rust Worker → оригинальные Party. Результат показывает имена, типы и версии; **это не поиск кандидатов, не доказательство совпадения и не объединение**. Достоверный matcher, MatchEvidenceSnapshot и состояние дела пока **NOT_CONNECTED**; D1-таблица для несуществующего дела не создаётся. Остальные `crm-customer-data-operations`, `crm-data-quality`, `crm-customer-enrichment`, `crm-customer-privacy` пока только участвуют в WASM compile/link-проверках. **Consent Evaluate и operator Deny** подключены к оригинальному `crm-consents`: проверка разрешения только читает состояние, операторский запрет записывается в D1 через Access, показывается в UI и подтверждается физическим Chromium/D1/Logs-тестом. Consent Grant (достоверное согласие субъекта) и доверенная верификация ContactPoint пока **не подключены**; UI не имеет кнопки выдачи согласия.

`pilot.rs`, JS backend и дублированный JS Customer360 удалены. Старые тестовые таблицы могут оставаться в единой D1 ради отката прежнего Worker; активные Rust API их не используют. Обратная совместимость тестовых сущностей не требуется.

## Разработка, диагностика и публикация

- `npm run locate -- deals` / `contacts` / `INVALID_INPUT` определяет владеющий модуль и файлы по `shared/catalog.mjs`.
- `npm run doctor` — быстрая локальная **проверка кода**, не доказательство живого браузера или D1.
- Авторизованный `GET /api/doctor` — реальная версия Worker, источник сборки, Access, таблицы и tenant-readback. Внешнюю браузерную/Logs-приёмку он честно помечает как `EXTERNAL_PROTECTED_CI_CHECK_REQUIRED`.
- **Ежедневная работа:** постоянная ветка `dev`. Один `push dev` запускает штатный `Cloudflare CRM Spike CI`: проверки оригинальных Rust-модулей и Access/JWT, D1/`workerd`, React/Chromium, одну production-feature Rust/WASM + React сборку и SHA-256 готового артефакта. Не нужно создавать PR или объединять `main` для каждого исправления.
- **Публикация:** после полного PASS тот же workflow отправляет **те же файлы** в защищённый GitHub Environment `crm-dev` (один раз разрешаем постоянную ветку `dev`), через Wrangler обновляет один существующий Worker и выполняет физическую Chromium-проверку сайта, Access A/B, D1 audit и Cloudflare Logs по `request_id`. При FAIL прежний Worker восстанавливается; D1-миграции автоматически не откатываются.
- **`main` — принятые этапы:** по завершении проверенного функционального этапа PR `dev → main` запускает общие монорепозиторные проверки. `main` никогда автоматически не публикует `crm-dev`. В PR действуют CI проверки, но никакого второго deployment.
- **При потере контекста:** открыть `AGENTS.md`, Issue #338, текущий `dev` HEAD, последний CI для `dev`, Cloudflare active `crm-dev` GIT_SHA и единственную D1 binding. Если SHA кода и сайта различаются, это **не принятый релиз** до физического PASS.
- `wasm-bindgen-cli` и `worker-build` закреплены; GitHub Actions кеширует инструменты, но не случайные compiled outputs. Тестовый `native-ci` Worker после проверки заменяется сохранёнными точными production-byte файлами `native`, без повторной сборки.

Результат публикации определяется проверкой **активной версии, exact SHA и полной live-приёмкой**, а не названием ветки, локальным PASS или компиляцией всех 13 модулей.

Общие принципы оригинального модульного Rust-монолита изложены в `docs/APPLICATION_ARCHITECTURE.md`: описанные там `services/crm-api` / PostgreSQL относятся к другому историческому runtime репозитория, **не** к развёрнутому `apps/cloudflare-crm`. Для текущего Cloudflare-приложения руководствоваться этим README и `AGENTS.md`, не строить параллельный runtime.

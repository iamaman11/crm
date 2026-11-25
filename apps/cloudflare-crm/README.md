# Cloudflare CRM

Канонический source — [iamaman11/crm](https://github.com/iamaman11/crm), ветка `dev`.
[ARCHITECTURE.md](ARCHITECTURE.md) описывает схему, [AGENTS.md](AGENTS.md) — правила разработки.
**Единый полный продуктовый план и очередь — в [#484](https://github.com/iamaman11/crm/issues/484), единственный текущий исполнительный курсор, A/B/C/D и Gate A/B — в [#487](https://github.com/iamaman11/crm/issues/487). Статусы не обновляются в Markdown.** Архитектурные решения — #338,
история cutover — #350. Markdown не является журналом релизов.
Продолжение в другом чате: открыть **#484 → #487**, проверить актуальный код/принятые Gate, затем для конкретной функции сверить [сохранённую историческую Rust-матрицу](../../docs/archive/cloudflare-snapshots/crm-functional-execution-matrix.md) и исходные тесты. Архивный Markdown — только source evidence, **не второй план**.

```text
Protobuf → generated Rust/TypeScript
React + packages/ui → packages/client/cloudflare → binary Connect
→ Rust Access/policy → original Rust owner → owner SQL/CAS/receipt/audit → одна D1
```

Один workers-rs Worker и встроенный Connect router. Frontend находится в `web/src`;
wire/domain mappings — в `rust-party/src/connect_sales.rs` и его handler modules;
бизнес-инварианты — в исходных `modules/crm-*`. Не добавлять параллельные транспорт,
backend, UI kit или domain model.

Разработка идёт прямыми commits в `dev`; PR необязателен. Code push запускает один
существующий GitHub Actions workflow. Изменения только Markdown не собирают и не публикуют
приложение; code + Markdown проходят полный CI. Выполняющийся запуск заканчивается,
следующим проверяется последнее ожидающее состояние.

Markdown-only commit не меняет опубликованный Worker. Его source SHA остаётся SHA последней
прошедшей code-сборки, а не обязательно HEAD документации; опубликованную версию показывают
`/api/version` и соответствующий запуск CI. Для изменения текста пересборка не требуется.

Buf проверяет текущий контракт; генерация и компиляторы — Rust/TS; TypeScript AST — границы
клиента. Compiled router/policy сверяются с descriptors и исполненными workerd-сценариями.
Историческая совместимость собственного тестового CRM API не блокирует развитие.
Контракт, сервер, UI и тесты меняются согласованно. Другие потребители общих контрактов
сохраняют свои обязательства совместимости.

Зависимости устанавливаются с frozen lockfile; Rust — с --locked и Cargo cache.
Domain/workerd/D1/Chromium проверяют результат, права, tenant, CAS/replay/audit.
`rust-party/test/typed-browser.mjs` выполняет реальные действия через generated client;
`e2e/live.mts` проверяет опубликованное приложение через Access. Doctor — только read-only
диагностика, не замена приёмки.

Release использует artifact этого CI без пересборки. Production `native` и test-only
`native-ci` различаются. SHA/checksum и live readback подтверждают опубликованную версию.
Обычная эволюция D1 добавляет миграции через Wrangler ledger; reset тестовых данных выполняется
явно. Без новых миграций release не останавливает вход ради D1 apply. Ошибка live-приёмки
оставляет fail-closed; несовместимый старый Worker не откатывается автоматически.

В issue записываются проверенный source, ссылка на CI, live result и остаток работы.
Принятие функции требует содержательных тестов; строгие типы не доказывают все бизнес-правила.

### Проверки по назначению

- `cloudflare-crm-spike`: все архитектурные правила, типизированные Rust RPC и полный
  регрессионный сценарий `rust-party/test/typed-browser.mjs` на workerd/D1.
- `deploy-crm-dev`: точный опубликованный SHA, реальные Cloudflare Access и D1,
  короткий Chromium `e2e/published-smoke.mts`, независимый audit/Trace readback.
  Полный браузерный регрессионный сценарий **не повторяется** после публикации.
- **Gate B — отдельная независимая приёмка новой пользовательской функции в
  реальном браузере после попадания кода в dev**, не job и не CI smoke. Её
  evidence публикуется в текущем курсоре; успешный CI сам по себе не значит Gate B ACCEPTED.


# Cloudflare CRM — разработка в каноническом GitHub

**Репозиторий:** [iamaman11/crm](https://github.com/iamaman11/crm), единственная рабочая ветка `dev`. Разрабатывается Cloudflare CRM: **один Rust Worker, один бинарный типизированный Connect-транспорт, одна D1**. Не считать готовностью текущей CRM завершённость исторического PostgreSQL/main продукта.

## Единственный рабочий вход

| Задача | Источник истины |
|---|---|
| Полный продуктовый план, 20 семейств, 554 требования, очередь | [Issue #484 — единый продуктовый план](https://github.com/iamaman11/crm/issues/484) |
| Единственный текущий функциональный курсор, A/B/C/D, доказательства Gate A/B | [Issue #487 — SK-06 «Рабочий день»](https://github.com/iamaman11/crm/issues/487) |
| Техническая архитектура | [apps/cloudflare-crm/ARCHITECTURE.md](apps/cloudflare-crm/ARCHITECTURE.md) |
| Инструкции исполнителя | [apps/cloudflare-crm/AGENTS.md](apps/cloudflare-crm/AGENTS.md), [корневой AGENTS.md](AGENTS.md) |
| Объяснение архива и проверка исторической семантики | [docs/README.md](docs/README.md), [docs/archive](docs/archive/) |

**Никаких параллельных продуктовых дорожных карт в Markdown.** Исторические тексты сохранены в архиве как справочные снимки; их состояние, приоритеты, статус принятия и старые runtime/CI не управляют `dev`. Решения и изменения статусов вести **в Issues**.

## Строгая централизованная типизированная схема

```text
versioned .proto → Buf → generated Protobuf Rust/TypeScript
React strict TSX → packages/client/cloudflare → binary Connect
→ verified Cloudflare Access → one fail-closed Rust RPC policy
→ original Rust domain owner → one D1 with CAS/idempotency/receipt/audit
```

Нет отдельного business REST, JS backend, параллельных DTO, SQLx/PostgreSQL runtime, второго Worker/DB, второго CI или ручного обхода owner policy. Все бизнес-поля доступной записи одинаково читаются `viewer` и `admin`; запись ограничена ролью и tenant.

**Единица разработки:** реальный полезный крупный сценарий, сначала проверить уже принятый код и старые Rust source/tests, затем A/B/C/D. Только доказанный B требует код. **Gate A:** действующий GitHub Actions `cloudflare-crm-spike` с Buf/Rust/TS/workerd/D1/Chromium и публикацией exact SHA. **Gate B:** независимый реальный LIVE Universal Browser MCP на том же опубликованном SHA. Результат, оставшиеся пробелы и следующий шаг — только в [#487](https://github.com/iamaman11/crm/issues/487).

Тестовые данные не являются основанием переносить устаревшие runtime или поддерживать прежние несовместимые API. Для исторических методик и архитектурных решений использовать [архив](docs/archive/) без восстановления старых планов как нормативных.

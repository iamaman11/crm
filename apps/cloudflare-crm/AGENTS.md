# Cloudflare CRM — инструкции разработки

## Норма и область

Применяется к apps/cloudflare-crm/**. Канонический source — только iamaman11/crm.
[ARCHITECTURE.md](ARCHITECTURE.md) задаёт конечную схему; [Issue #350](https://github.com/iamaman11/crm/issues/350) — единственный подробный план жёсткого cutover; #338 — верхний план.

Решение владельца 12.11.2026 заменило прежние freeze/compatibility инструкции для одноразового cutover. Cutover принят на `272aba214114f3db6247cad115f86d54312e3030`, CI/live 43302137364 PASS. Теперь сохранять принятые контракты и историю migrations; несовместимая эволюция требует явного решения. Не повторять reset тестовых данных при обычной разработке. Пользовательские инструкции имеют приоритет.

Root AGENTS.md регулирует общие domain invariants и repository practices. Исторические PostgreSQL/gateway runtime instructions не предписывают Cloudflare runtime. Не добавлять PostgreSQL или второй backend для примирения старых документов.

## Рабочий процесс

1. Читать актуальный #350, этот файл, ARCHITECTURE.md, текущие PR/head и CI. Проверять реальный deployed SHA перед утверждениями о live.
2. PR #351 завершён. Продолжать в постоянной dev по #338, без новых branches/plans/workflows/Environment на каждый шаг. Принятая API/schema baseline фиксирована в существующем CI; не заменять её автоматически на HEAD.
3. Разработка через канонический GitHub; builds/tests выполняет GitHub Actions. Не поручать обычную работу локальному Windows агенту и не просить пользователя запускать команды.
4. Определить existing owner и точную capability до изменения UI/transport. Не угадывать ownership по имени пакета.
5. Неполные implementation commits допустимы, но конечные checks не ослабляются и не заменяются migration-phase PASS. Не публиковать частичные RPC.
6. Записывать tested SHA/tree/features, CI и evidence в #350; #338 синхронизировать при принятии. Не утверждать DONE по документации или compile-only.
7. Не merge/deploy до полного CI и независимой проверки кандидата. Деструктивный reset тестовых business data выполняется только как подготовленная операция финального переключения с проверенным binding.

## Обязательные ограничения

- Один workers-rs Worker, embedded Axum/Connect router, binary Connect-Web transport, native D1 и существующий Access.
- Protobuf — единственный wire contract; generated Rust/TS, original Rust owners и typed mappings.
- SQL/CAS/receipts/audit могут оставаться в existing owner-specific Worker files. Не строить generic CRUD/ORM/repository framework.
- Identity и request_id авторитетны только после Rust Access verification. Policy явная, fail-closed, одна. UI не дублирует правила домена.
- Использовать compiled Buf descriptors, Router::methods()/has_method(), выполнение Rust policy и TypeScript compiler/AST. Не разбирать семантику Rust/Protobuf regex и не считать source strings доказательством tests.
- Не создавать contract-coverage.json, второй ручной RPC registry или metadata generator.
- Existing packages/client/cloudflare и packages/ui обязательны. Не создавать второй web/src/ui design system; CSS Modules + semantic tokens + system/light/dark.
- Содержательные domain/workerd/D1/browser tests сохранять/переводить на новый API; source-only duplicate checks удалять. Не ослаблять auth, tenant, CAS, replay, audit.
- При committed write и failed refresh показывать сохранённый результат и устаревшее чтение, не «запись не удалась».
- В финальном runtime нет legacy JSX/business REST, temporary screen, Published/Candidate, migration-freeze/contract-only, codec bridges/stubs.
- CI: один pinned pnpm workspace/lock, frozen/locked builds, реальные compiler tests, native integration и browser generated-client scenarios. Missing mandatory evidence = FAIL.
- Test-only signed native-ci отличается от production features; не смешивать доказательства.
- Clean reset не означает удаление Access infrastructure и не выполняется на каждом deploy. Старый несовместимый Worker не откатывать автоматически после reset.
- Не ломать shared contracts других приложений без проверки consumers. После принятия закрепить новый immutable baseline, будущие breaking changes оформлять явно.

## Функциональные границы

Не создавать fake Delete для Party/Account, consent grant без доказательств, automatic identity merge, provider dispatch или privacy execution. Enrichment configuration и Privacy case intake — реальный ограниченный объём, остальное NOT_CONNECTED. Customer360 v2 Account-rooted, historical v1 Party-rooted. Sales–Activities — existing atomic typed flow, не выдуманный public API.

Количество modules/RPC не является константой. Будущая capability обязана расширять exact ownership, policy и executed scenario checks.

## Приёмка и governance

Сценарий засчитывается после assertions результата, прав/tenant, D1 и прочих обязательств. Node fetch не browser E2E; Chromium data: preflight не тест CRM. Фиксировать head_sha/tested_sha/tree/artifact/deployed_sha отдельно.

Сохраняется решение владельца: не включать branch protection/rulesets/required checks dev/main. CI блокирует штатный deployment, не direct push. Не обещать абсолютную автоматическую бизнес-корректность.

Полный DONE только по #350: конечное приложение, удалённый legacy, full CI, mandatory artifacts, независимая проверка и live Access A/B/UI/D1/Logs/readback.

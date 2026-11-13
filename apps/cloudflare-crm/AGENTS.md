# Cloudflare CRM — инструкции разработки

## Норма и область

Применяется к apps/cloudflare-crm/**. Канонический source — только iamaman11/crm.
[ARCHITECTURE.md](ARCHITECTURE.md) — нормативная схема; [#350](https://github.com/iamaman11/crm/issues/350) — завершённый cutover и его evidence; [#338](https://github.com/iamaman11/crm/issues/338) — архитектурная основа; [#352](https://github.com/iamaman11/crm/issues/352) — текущий план развития и приёмки новых функций.

Решение владельца 12.11.2026 заменило прежние freeze/compatibility инструкции для одноразового cutover. Cutover принят на `272aba214114f3db6247cad115f86d54312e3030`, CI/live 43302137364 PASS. Теперь сохранять принятые контракты и историю migrations; несовместимая эволюция требует явного решения. Не повторять reset тестовых данных при обычной разработке. Пользовательские инструкции имеют приоритет.

Root AGENTS.md регулирует общие domain invariants и repository practices. Исторические PostgreSQL/gateway runtime instructions не предписывают Cloudflare runtime. Не добавлять PostgreSQL или второй backend для примирения старых документов.

## Рабочий процесс

1. Читать актуальный #352, этот файл, ARCHITECTURE.md, принятые решения #338/#350, текущие PR/head и CI. Проверять реальный deployed SHA перед утверждениями о live.
2. PR #351 завершён. Разрабатывать каждый законченный срез в одной кандидатной ветке/PR канонического GitHub → `dev`, с архитектурой #338 и планом #352. Не добавлять новые CI workflow, Environment или runtime. Непроверенный кандидат не отправлять прямым push в `dev`. Допустимый прямой push не является свидетельством принятия.
3. Разработка через канонический GitHub; builds/tests выполняет GitHub Actions. Не поручать обычную работу локальному Windows агенту и не просить пользователя запускать команды.
4. Определить existing owner и точную capability до изменения UI/transport. Не угадывать ownership по имени пакета.
5. Неполные implementation commits допустимы **только в кандидатной ветке**. PR→dev прогоняет весь существующий CI; при новом commit или изменении dev/base PR требуется повторная применимая проверка. Не merge непроверенное. После merge dev выполняет тот же CI и только затем разрешает deploy/live.
6. Записывать tested SHA/tree/features, CI и evidence в implementation issue и #352; #338 остаётся архитектурной основой, #350 — завершённой исторической приёмкой. Не утверждать DONE по документации или compile-only.
7. Не merge/deploy до полного CI и независимой проверки кандидата. Защищены историческая `272aba214114f3db6247cad115f86d54312e3030` и последняя **live-принятая версия контрактов/миграций** `50370070d2f9bb147baa8cb712ab4f54d174f7b8` ([CI + live 43342945163](https://github.com/iamaman11/crm/actions/runs/43342945163)). `event.before` и PR base — только диагностические SHA разработки; не base breaking checks, пока они не приняты. После новой успешной CI+live приёмки закреплять следующий точный accepted SHA отдельным явным изменением pin с доказательствами, не автоматически по 'последнему зелёному'. Метаданные и документация без изменения контрактов/миграций не создают новое поколение принятой **схемы** и не требуют ещё одного pin.

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
- CI: один pinned pnpm workspace/lock, frozen/locked builds, реальные compiler tests, native integration и browser generated-client scenarios. Порядок: ранние проверки → Buf/TS → actual Rust/WASM check/Clippy → дальнейшая сборка → Chromium/workerd. Отчёт перечисляет source/checkout SHA, historical/last accepted и prior-development SHA и первую причину FAIL. Отсутствие обязательного evidence после выполненного browser test = FAIL, пропущенный browser этап не создаёт вторую ложную ошибку загрузки.
- Test-only signed native-ci отличается от production features; не смешивать доказательства.
- Clean reset не означает удаление Access infrastructure и не выполняется на каждом deploy. Старый несовместимый Worker не откатывать автоматически после reset.
- Не ломать shared contracts других приложений без проверки consumers. После принятия закрепить новый immutable baseline, будущие breaking changes оформлять явно.

## Функциональные границы

Не создавать fake Delete для Party/Account, consent grant без доказательств, automatic identity merge, provider dispatch или privacy execution. Enrichment configuration и Privacy case intake — реальный ограниченный объём, остальное NOT_CONNECTED. Customer360 v2 Account-rooted, historical v1 Party-rooted. Sales–Activities — existing atomic typed flow, не выдуманный public API.

Количество modules/RPC не является константой. Будущая capability обязана расширять exact ownership, policy и executed scenario checks.

## Приёмка и governance

Сценарий засчитывается после assertions результата, прав/tenant, D1 и прочих обязательств. Node fetch не browser E2E; Chromium data: preflight не тест CRM. Фиксировать head_sha/tested_sha/tree/artifact/deployed_sha отдельно.

Сохраняется решение владельца: не включать branch protection/rulesets/required checks dev/main. CI блокирует штатный deployment, не direct push. Не обещать абсолютную автоматическую бизнес-корректность.

Cutover DONE зафиксирован в #350; новые продуктовые срезы принимаются отдельно по #352: полный CI, mandatory artifacts, независимая проверка и live Access A/B/UI/D1/Logs/readback.

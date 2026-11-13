# Cloudflare CRM — нормативная архитектура

Здесь находятся устойчивые правила приложения. План, текущая функциональность, чекпойнты и
приёмка — в [#352](https://github.com/iamaman11/crm/issues/352). Архитектурные решения —
[#338](https://github.com/iamaman11/crm/issues/338); история cutover — [#350](https://github.com/iamaman11/crm/issues/350).

## 1. Состав

```text
versioned Protobuf → Buf descriptors → generated Rust/Buffa + TypeScript/Protobuf-ES
React strict TSX + packages/ui → packages/client/cloudflare → binary Connect
→ Rust Access verification → verified context + explicit policy → Connect router
→ typed wire/domain mapping → original Rust owner → owner SQL/CAS/receipt/audit → D1
```

Один workers-rs Worker и встроенный Axum/Connect router. Generated JavaScript — сборочная
оболочка, не второй backend. Один транспорт и одна D1. Не добавлять business REST, JS gateway,
ORM/generic CRUD, параллельный UI kit или второй runtime.

Оригинальные crates владеют бизнес-инвариантами. Owner-specific файлы Worker могут содержать
D1 SQL, CAS, receipts и audit и вызывать domain crates. Typed wire/domain conversion законна;
дублирование бизнес-модели и Buffa→bytes→Prost bridge — нет.

## 2. Авторитеты и проверки

| Сведения | Источник | Проверка |
|---|---|---|
| RPC, поля, presence, enums | versioned .proto | Buf build/lint/format, генерация, wire tests |
| Владелец RPC | существующий module.yaml и Cargo | manifest schema, exact binding/linkage |
| Регистрация | compiled Connect Router | Router::methods()/has_method() против descriptors |
| Доступ | одна typed Rust policy | compiled policy и реальные role/tenant tests |
| Семантика | original Rust owner | domain tests, workerd/D1 assertions |
| Клиент | packages/client/cloudflare | strict TypeScript и AST/module graph |
| UI | packages/ui + feature TSX | Chromium через generated client |
| Данные | канонические SQL migrations | создание с нуля, CAS/replay/atomicity/audit |
| Release | проверенный artifact | SHA/checksum и live readback |

Активные RPC из descriptors совпадают с registered RPC, policy RPC и исполненными обязательными
сценариями. Новый метод расширяет эту область. Отсутствующие owner, handler, policy или evidence —
FAIL. Исторические proto других потребителей не становятся Cloudflare API автоматически.
Число модулей/RPC не фиксируется константой.

Не использовать semantic regex для Rust/Protobuf, поиск строк как доказательство теста,
ручной каталог покрытия или второй metadata generator. Использовать компиляторы, descriptors,
штатный router и TypeScript AST. Временный CI-отчёт является результатом проверки.

## 3. Типы, права и поведение

Access проверяется до dispatch. Tenant/actor/role/server request_id не принимаются на доверии
от клиента. Один verified context передаётся сервисам. Unknown RPC, missing policy, invalid auth
и недостаточные права закрыты. Право определяется policy, не названием метода.

Ошибки используют стандартные Connect codes и typed domain details. Request_id связывает ответ,
D1 audit и логи. Generated client проверяет ConnectError.code; одного HTTP status мало.
Не логировать JWT, secrets или raw business payload.

Деньги — int64 minor units/bigint + currency/exponent, без floating point. Timestamp и date-only
различаются. Patch явно определяет absent/set/clear. Unknown enums, limits, pagination/filter/sort
и expected_version проверяются. Read-only RPC не получает mutation semantics.

Owner обеспечивает CAS, idempotent replay и атомарность state/receipt/audit. Ошибочная команда
не меняет агрегат частично. UI не копирует domain policy; после успешной записи ошибка refresh
означает сохранённый результат с устаревшим отображением, а не failed write.

React использует strict TSX, allowJs=false, noUncheckedIndexedAccess и exactOptionalPropertyTypes.
Не обходить типы через any/as unknown as или suppressed diagnostics. Общий client — единственный
business transport. Health/session/doctor/version и bounded CSV download — ограниченные исключения.
Общие controls/tokens принадлежат packages/ui; feature layouts используют CSS Modules.

## 4. Эволюция тестового приложения

Принимается целостность текущей версии, а не неизменность всех прошлых API. Собственные CRM
контракты, handlers, generated client, UI и тесты меняются согласованно. Совместимость старого
открытого UI не обещана; после несовместимого обновления он может требовать перезагрузки.
В development CI нет historical/accepted SHA pins, last-green поиска или buf breaking против
прошлых веток. Buf build/lint/format, генерация и проверки поведения обязательны.

Контракты других приложений сохраняют их требования совместимости и соответствующий CI.
Появление внешних клиентов или production-обязательств требует отдельного решения о версиях;
это решение не выводится из зелёного CI.

CI создаёт локальную D1 с нуля. Live D1 использует Wrangler migration ledger: обычная эволюция
добавляет миграцию, применённый файл не исполняется заново от редактирования. Тестовые данные
не являются обязательством сохранения. Намеренный reset выполняется явно, не удаляет Access
infrastructure и не нужен для каждого deploy.

## 5. Разработка, CI и release

Одна постоянная ветка dev, прямые commits, PR по необходимости для обсуждения. Branch protection
и обязательный двойной PR/push CI не требуются. Единственный Cloudflare workflow запускается
на code push в dev. Изменения только **/*.md не собирают и не публикуют приложение; Markdown
содержит документацию, не исполняемую конфигурацию. Смешанный push проверяется полностью.

Последовательность: topology/format → Buf/types → Rust/WASM/Clippy/domain → сборка →
workerd/D1/Chromium → mandatory artifacts → deploy → live Access/browser/D1/Logs.
GitHub concurrency завершает текущий запуск и оставляет последнюю ожидающую версию.
Deployment не прерывается новым commit. CI блокирует публикацию, но не запись в dev.

Один pnpm workspace/lock, frozen installs и Cargo --locked; toolchain/codegen закреплены.
Cargo cache использует стандартную проверку freshness; build.rs отслеживает proto и импорты.
Изменения toolchain/lock/config/codegen начинают отдельное семейство кеша. При изменении этих
входов проверяется чистая сборка. Кеш не является доказательством приёмки текущего source.

Production native собирается один раз. Отдельный native-ci содержит изолированные тестовые
ключи и не публикуется. Release скачивает production artifact этого запуска и проверяет
SHA/checksum без пересборки. При новых миграциях вход закрывается до применения SQL;
без новых миграций нет остановки ради D1 apply. Затем выполняется live-приёмка текущего Worker.
При её провале применяется существующий fail-closed, без слепого отката на несовместимый Worker.

Browser preflight доказывает только запуск браузера, Node fetch — RPC integration. Сценарий
засчитывается после содержательных assertions. Missing mandatory evidence — FAIL.
Различать locked build, неизменность артефакта и bit-for-bit reproducibility: последнее требует
независимого сравнения сборок и не обещается автоматически.

## 6. Расширение и учёт результата

Новая функция: existing owner → proto/manifest → generation → handler/policy/D1 → shared client/UI
→ domain/runtime/browser tests → тот же CI. Новый crate нужен только при реальной границе.
Нельзя ослаблять архитектурное требование ради PASS; ошибочную проверку исправляют с сохранением
инварианта и регрессионным доказательством.

Issue хранит текущий SHA, CI/live result, Worker version, чекпойнты и остаток задач. Markdown
приложения меняется при изменении общей схемы и правил. CI подтверждает проверенные инварианты,
не все возможные бизнес-состояния и не промышленную готовность CRM.

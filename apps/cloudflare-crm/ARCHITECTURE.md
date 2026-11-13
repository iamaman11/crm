# Cloudflare CRM — нормативная архитектура

Архитектурная основа и evidence: [#338](https://github.com/iamaman11/crm/issues/338). Текущий план продуктового развития, приёмки и эксплуатации: [#352](https://github.com/iamaman11/crm/issues/352). [#350](https://github.com/iamaman11/crm/issues/350) завершён и не используется как текущий implementation roadmap.

Решение владельца от 12.11.2026 и [Issue #350](https://github.com/iamaman11/crm/issues/350): **жёсткий cutover тестового приложения**, без сохранения старых CRM API/UI/данных. PR #351 объединён в dev. Принятая исходная версия: `272aba214114f3db6247cad115f86d54312e3030`, [CI и live acceptance](https://github.com/iamaman11/crm/actions/runs/43302137364) PASS. Worker `08e73b33-da86-44e2-bb68-48855be214b1`, 100% traffic, прежняя единственная D1. Это базовая версия, а не автоматически обновляемая ссылка на HEAD.

CI сравнивает Protobuf и защищает SQL migrations штатным git diff относительно двух точных commit: этой неизменной версии и предыдущего dev (для PR — actual base SHA, для push — event.before). Поэтому защита распространяется и на контракты/миграции, добавленные позже. Следующие изменения D1 оформляются новыми миграциями; reset 0002 повторно не применяется. API/schema evolution требует явного решения и тех же проверок. Реальная приёмка включила 24 записи с audit и 117 запросов, сопоставленных с Cloudflare Logs. Две CI-сборки одного дерева (PR 43298969184 и dev 43302137364) дали одинаковые 13 файлов UI/Worker; это проверенная повторяемость этой пары, не гарантия для любых окружений.

## 1. Состав приложения

```text
proto/crm/<owner>/vN
  → Buf FileDescriptorSet
  → штатные generated Rust/Buffa и TypeScript/Protobuf-ES
React strict TSX + packages/ui
  → packages/client/cloudflare → один binary Connect-Web transport
Cloudflare Access → один workers-rs Rust/WASM Worker
  → проверенный request context → явная Rust policy
  → один embedded Axum/Connect router
  → typed wire/domain mapping → оригинальный domain owner
  → owner SQL/CAS/idempotency/audit → одна native D1
```

Axum — встроенный router, не второй сервер. Generated workers-rs JavaScript — сборочная оболочка, не JS backend. Нельзя вводить второй Worker/D1/Environment, business REST, JS gateway, ORM/generic CRUD framework или новый workflow.

Оригинальные crates владеют бизнес-инвариантами. Существующие owner-файлы Worker могут владеть D1 SQL, CAS, receipts и audit и вызывать эти crates. Не переносить SQL в domain crates ради формы. Типизированная конверсия wire/domain законна; копия бизнес-модели/правил, JSON API DTO и Buffa→bytes→Prost bridge — нет.

## 2. Авторитеты и автоматические проверки

| Сведения | Авторитет | Обязательная проверка |
|---|---|---|
| RPC, поля, presence, enums | versioned .proto | Buf build/lint, descriptors, generated code, wire tests |
| Владелец capability/RPC | существующий module.yaml | strict manifest schema, exact binding и Rust package |
| Зарегистрированные методы | compiled Connect Router | Router::methods()/has_method() |
| Доступ | одна typed Rust policy | выполнение того же кода и независимые role/tenant tests |
| Семантика | original Rust owner | domain tests, workerd/D1 assertions |
| Клиент | packages/client/cloudflare | strict TypeScript и AST/module graph |
| UI | packages/ui и feature TSX | browser actions через generated client |
| Хранилище | versioned D1 schema | empty-schema fixtures, tenant/CAS/replay/audit checks |
| Release | tested checkout и artifact | SHA/tree/features/checksums/live readback |

Конечный scoped API: согласованные RPC = registered RPC = policy RPC = RPC с выполненными обязательными scenarios. Проверка «у owner есть один RPC» недостаточна. Ожидаемый набор нельзя выводить только из уже реализованных handlers. Исторические proto других потребителей не являются автоматически Cloudflare API.

Количество модулей/RPC не зафиксировано. Новый RPC расширяет область обязательных проверок. Отсутствующий handler, policy, owner binding, scenario или обязательный evidence — FAIL.

Никаких semantic regex для Rust/Protobuf, .includes() доказательств тестов, нового hand-written coverage catalog или генератора метаданных. Buf descriptors, штатный router и скомпилированная policy уже содержат нужные факты. Временный CI-отчёт допустим только как результат проверок.

## 3. Identity, protocol и data semantics

Access проверяется до dispatch. Tenant/actor/role/server request_id не принимаются от клиента. Один immutable verified context передаётся сервисам. Unknown RPC, missing policy, invalid auth и недостаточные права закрыты. Право не определяется названием Get/List/Create.

Стандартные Connect error codes; stable domain details по контракту, request_id сквозь response/D1/Logs. Generated Connect-клиент обязан получить ожидаемый ConnectError.code. HTTP status alone недостаточен. Не логировать JWT, secrets или raw business payload.

Money — signed int64 minor units/bigint + currency/exponent; без floating point. Timestamp/date-only различаются. Patch явно сохраняет absent/set/clear. Unknown enums, limits, pagination/filter/sort и optional expected_version проверяются. Mutation semantics задаются явно, не префиксом имени.

Owner обеспечивает CAS, idempotent replay, atomic state/receipt/audit и авторитетную валидацию. Read-only запрос не получает mutation semantics. UI не копирует domain policy. После успешной записи ошибка последующего чтения означает сохранённое изменение и устаревшее отображение, а не failed write.

## 4. Frontend

React strict TSX/TS; allowJs=false, strict=true, noUncheckedIndexedAccess=true, exactOptionalPropertyTypes=true. Runtime-generated wire types, без any/as unknown as для обхода проверки и без suppressed diagnostics.

Один packages/client/cloudflare export и binary Connect transport. UI не использует legacy client barrel, прямой business fetch или второй transport. Health/session/doctor/version и обоснованный bounded CSV download — явно ограниченные non-RPC исключения.

Единственная общая UI-библиотека — существующий packages/ui. Общие controls и semantic tokens меняются централизованно; CSS Modules для feature layouts, system/light/dark, keyboard/focus/disabled/error/mobile states. Не создавать параллельный web/src/ui design system.

## 5. Hard cutover и данные

До принятия удалить legacy JSX/business REST, временный экран, Published/Candidate и migration-freeze/contract-only, frozen-source exceptions, obsolete clients/stubs/bridges. Несколько implementation commits допустимы в одном PR; единственный принимаемый результат — весь конечный вариант. Неполный кандидат должен давать FAIL/NO-GO.

Тестовые бизнес-данные не мигрируют. CI создаёт одну локальную D1 с нуля по канонической схеме и fixtures. Финальное переключение использует ту же опубликованную D1, явную проверку её binding и одноразовый контролируемый reset. Не удалять Access configuration/service principals под видом удаления business data. Reset не выполняется на каждом deploy.

Старый Worker не обязан работать на новой схеме; слепой rollback после несовместимого reset запрещён. Failure path финального release должен безопасно прекратить обслуживание/оставить NO-GO и позволить исправить проверяемый кандидат. Новую D1 для этого не создавать.

Старые Cloudflare контракты не ограничивают новый тестовый API. Общие contracts/crates других приложений изменять только после проверки потребителей. После полного принятия закрепить новый immutable API/schema baseline; будущие breaking changes требуют явного version/schema evolution, не автоматического baseline=HEAD.

## 6. Воспроизводимость и CI

Один pnpm workspace/pnpm-lock.yaml для приложения и shared packages; frozen install и локальные pinned tools. Закрепить Node/pnpm/Rust/Buf/codegen/Python dependency graph. Cargo locked распространяется на probes. Явные fmt/clippy/domain tests плюс native WASM/workerd/D1; install Clippy не заменяет его запуск.

Тот же cloudflare-crm-spike.yml на PR→dev и push dev. Проверки по стоимости: config/topology → manifests/Buf/types → compiler/unit → workerd/D1 → реальный Chromium→Connect → mandatory artifacts. Deploy из dev после общего PASS использует sealed artifact, без повторной сборки. Не добавлять workflows/Environment.

Browser preflight data: URL доказывает только запуск Chromium. Node fetch доказывает RPC integration, не browser E2E. Каждый обязательный сценарий засчитывается после assertions. Артефакты обязательны, отсутствие — FAIL. Negative checks обязаны обнаруживать пропуск policy/method/scenario.

Evidence различает PR head_sha, tested_sha, Git tree, features/tool versions, artifact hash и deployed_sha/version/traffic. В PR tested_sha может быть synthetic merge commit. Test-only native-ci keys/features не выдаются за production artifact.

Различать locked clean build, неизменность передаваемого artifact и bit-for-bit reproducibility. Последнее требует независимых сборок и сравнения; без доказательства NOT_TESTED.

## 7. Поддержка и завершение

Новая capability: owner → manifest/versioned proto → generation → handler/policy/D1 → shared client/UI → обязательные scenarios → тот же CI. Новые framework/crates только при реальной границе, не на каждый RPC. Dependency updates обозримы и проходят те же conformance/E2E checks.

Issue #350 содержит функциональный объём, execution checklist и актуальное evidence; #338 — верхний план. Docs обновляются вместе с кодом. CI доказывает заданные инварианты, не все возможные бизнес-состояния.

Branch protection/required GitHub checks не включать по решению владельца. CI блокирует штатный deploy при FAIL, но не запрещает direct push.

DONE: полный API+UI, удалённый legacy, чистая схема, один окончательный CI PASS с evidence, независимая проверка, единое переключение и реальные Access A/B/browser/D1/Logs/readback. Частичный CI PASS не завершает #350.

# Cloudflare CRM — нормативная архитектура

Здесь находятся устойчивые правила приложения. Продуктовый план и ссылка на текущий курсор с результатами приёмки — в [#484](https://github.com/iamaman11/crm/issues/484). Техническая надёжность — [#485](https://github.com/iamaman11/crm/issues/485), эффективность CI и измерения — [#486](https://github.com/iamaman11/crm/issues/486). Архитектурные evidence — [#471](https://github.com/iamaman11/crm/issues/471), [#338](https://github.com/iamaman11/crm/issues/338); история cutover — [#350](https://github.com/iamaman11/crm/issues/350).

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
| Клиент | packages/client/cloudflare | TypeScript strict/tsc --showConfig, scoped native Oxlint и пакетные import boundaries |
| UI | packages/ui + feature TSX | Chromium через generated client |
| Данные | канонические SQL migrations | создание с нуля, CAS/replay/atomicity/audit |
| Release | проверенный artifact | SHA/checksum и live readback |

В `module.yaml` вручную задаются владелец и точное имя RPC. Типы
`request`/`response` приходят исключительно из compiled Protobuf descriptors:
их повторное объявление в manifest запрещено. Уже существующая проверка
сопоставляет метод с реальным Rust router/policy и исполненными сценариями.

Активные RPC из descriptors совпадают с registered RPC, policy RPC и исполненными обязательными
сценариями. Новый метод расширяет эту область. Отсутствующие owner, handler, policy или evidence —
FAIL. Исторические proto других потребителей не становятся Cloudflare API автоматически.
Число модулей/RPC не фиксируется константой.

Не использовать semantic regex для Rust/Protobuf, поиск строк как доказательство теста,
ручной каталог покрытия или второй metadata generator. Использовать компиляторы, descriptors,
штатный router, TypeScript --showConfig и Oxlint. Старый собственный TS AST checker удалён;
не возвращать его как параллельную защиту. Отчёт о манифестах фиксирует объявленных владельцев
и подключения crate. Отчёт workerd фиксирует выполненные вызовы и скомпилированные правила.
CI сверяет оба отчёта с Buf-дескрипторами на одном SHA. Отчёты создаются в ходе проверки.

## 3. Типы, права и поведение

Access проверяется до dispatch. Tenant/actor/role/server request_id не принимаются на доверии
от клиента. Один verified context передаётся сервисам. Unknown RPC, missing policy, invalid auth
и недостаточные права закрыты. Право определяется policy, не названием метода.

**Доступ к бизнес-полям в CRM:** пользователь с ролью `viewer` читает **все
доступные по текущему типизированному контракту поля каждой записи своего tenant** —
наравне с `admin`, без полевого маскирования, скрытия персональных значений или
отдельной `field visibility` матрицы. Для Get/List/Customer360 нельзя вводить
разные проекции полей по ролям. Обычные Write и AdminRead доступны только `admin`.
Узкие SelfWrite RPC разрешают только подтверждённое собственное действие: для Task это
завершение и напоминание своей задачи активным человеческим участником, без создания,
редактирования, делегирования или чужих задач. Проверка подписанного actor, tenant,
membership и версии выполняется владельцем, активность/владелец повторно проверяются
атомарно внутри D1 CAS. Служебный viewer не получает личных прав. Отсутствующие в
Protobuf бизнес-поля не становятся автоматически новыми публичными API; служебные
секреты, Access JWT и технические данные не являются бизнес-полями CRM.
Исторические generic `QueryVisibilityAuthorizer`/field masks — **осознанное C
для текущего продукта**, их не переносить. Редактирование/скрытие секретов, PII и
содержимого ответов в диагностических логах, трассах и публичных ошибках **сохраняется**:
право читать свою карточку не является правом раскрывать её в логах.

Новый подтверждённый человеческий Access principal может получить изолированный тестовый tenant
по текущему onboarding. **Service principal никогда не создаёт себе tenant или роль автоматически**:
только заранее внесённый активный `service:<verified common_name>` в `crm_users` связывает
его с явно выбранным существующим tenant и ролью. CI сначала регистрирует две отдельные
служебные тестовые учётные записи в изолированных tenant, затем проверяет их реальные Access JWT.
Для браузерного MCP нужен отдельный служебный principal в tenant владельца, а не CI-аккаунт.
Cloudflare Access / Worker secret / D1 enrollment — три независимые контрольные точки;
без любой из них CRM должна отказать, не делать self-enrollment.

Ошибки используют стандартные Connect codes и typed domain details. Request_id связывает ответ,
D1 audit и логи. Generated client проверяет ConnectError.code; одного HTTP status мало.
Не логировать JWT, secrets или raw business payload.

Деньги — int64 minor units/bigint + currency/exponent, без floating point. Timestamp и date-only
различаются. Patch явно определяет absent/set/clear. Unknown enums, limits, pagination/filter/sort
и expected_version проверяются. Read-only RPC не получает mutation semantics.

Owner обеспечивает CAS, idempotent replay и атомарность state/receipt/audit. Ошибочная команда
не меняет агрегат частично. UI не копирует domain policy; после успешной записи ошибка refresh
означает сохранённый результат с устаревшим отображением, а не failed write.

Каждая команда записи объявляет стандартную семантику Protobuf `IDEMPOTENT`, одно поле
`precondition` типа `MutationPrecondition` и одно поле `receipt` типа `MutationReceipt`.
Для новых версий действуют те же проверки Buf-дескрипторов, включая вложенные типы.
При потере подтверждения D1 владелец сверяет намерение с квитанцией своего tenant и ключа.
Клиент снимает ключ после отказа только при получении `DomainErrorDetail` с исходом
`REJECTED` и идентификатором текущего запроса. При `UNCONFIRMED` он сохраняет ключ для повтора.
Штатный workerd-тест сверяет квитанции и ошибки вызванных команд с реальным идентификатором ответа.
Для нового пути записи нужен сценарий со сбоем внутри транзакции и проверкой результата в D1.

Для локальной разработки и CI действуют одна команда `pnpm web:check` и базовый
`tsconfig.base.json` для приложения, клиента и UI. Команда собирает Buf-дескрипторы,
генерирует клиент и сверяет полный набор путей его бинарных вызовов с активными RPC.
Штатный CI дополнительно сопоставляет команды записи с квитанциями, полученными от workerd.

React использует strict TSX, allowJs=false, noUncheckedIndexedAccess и exactOptionalPropertyTypes.
Не обходить типы через any/as unknown as или suppressed diagnostics. Общий client — единственный
business transport, включая CSV download через generated Connect. Только технические
health/session/doctor/version остаются отдельными HTTP-маршрутами.
Общие controls/tokens принадлежат packages/ui; feature layouts используют CSS Modules.

## 4. Эволюция тестового приложения

Принимается целостность текущей версии, а не неизменность всех прошлых API. Собственные CRM
контракты, handlers, generated client, UI и тесты меняются согласованно. Совместимость старого
открытого UI не обещана; после несовместимого обновления он может требовать перезагрузки.
Приёмку текущего development API не подменять historical/accepted SHA или last-green. Buf build/lint/format, генерация и проверки поведения обязательны. Для опубликованного внутреннего persisted v1 Sales–Activities wire link сохраняется штатный Buf breaking против неизменяемой принятой ревизии; это защита конкретного совместимого контракта, не фиксация всего развивающегося API.

Контракты других приложений сохраняют их требования совместимости и соответствующий CI.
Появление внешних клиентов или production-обязательств требует отдельного решения о версиях;
это решение не выводится из зелёного CI.

CI создаёт локальную D1 с нуля. Live D1 использует Wrangler migration ledger: обычная эволюция
добавляет миграцию, применённый файл не исполняется заново от редактирования. Тестовые данные
не являются обязательством сохранения. Намеренный reset выполняется явно, не удаляет Access
infrastructure и не нужен для каждого deploy.

### Типы тестов и диагностики

Новые и изменяемые штатные RPC-фикстуры связывают метод с типами запроса и ответа из
сгенерированного Protobuf. TypeScript-сценарии и их помощники входят в strict typecheck
активного CI. Единый Protobuf-ES выводит ESM и объявления типов для браузера и Node.
Новый RPC использует существующие фикстуры и автоматически входит в область проверок
по Buf-дескрипторам и скомпилированным router/owner/policy. Добавление метода требует
предметного сценария и явной политики доступа. Тестовый адаптер, ручной тестовый
реестр RPC или отдельный CI-скрипт для обычного метода не допускаются.
Низкоуровневые фикстуры нужны для намеренно некорректных входов, проверки границ
протокола и сверки выполненных RPC и receipts по Buf-дескрипторам. Проверки ролей,
tenant, CAS, повторов, откатов и сохранения в D1 выполняются на штатных уровнях
Cargo, workerd и Chromium.

События приложения формируются централизованно из типизированных структур. Закрытые
множества значений задаются перечислениями. Connect-коды сохраняют штатный тип до
сериализации, доменные коды следуют Protobuf-контракту. Cloudflare Logs и Traces используют
структурированный JSON, внешние данные на входе разбираются как unknown с проверкой формы.
Публикуемые диагностические события и CI-лог исключают тела запросов и ответов,
JWT, секреты и бизнес-данные. Для связи с D1 и аудитом используется request_id.

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

Production native собирается один раз. Отдельный native-ci не публикуется: локальные guards и fault/recovery probes отличаются, оба артефакта используют обычный JWKS path. Подмена внешнего JWKS в локальном harness не добавляет ключи в production. Release скачивает production artifact этого запуска и проверяет
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
не все возможные бизнес-состояния и не промышленную готовность CRM. Для функционального среза отдельный LIVE Gate B после CI выполняется по AGENTS.md; CI deploy smoke его не заменяет.

## 7. Расширение автоматизации и внешних эффектов

Продуктовые стадии и требования определяет #484; следующие границы — устойчивые правила, не обещание уже реализованного функционала. Новые trigger, rule, command, approval, run, result и recovery semantics задавать versioned Protobuf и типами оригинального Rust owner. Event/timer — штатная входная граница того же Worker; обработчик не становится владельцем чужого агрегата. Межмодульное действие вызывает целевой owner с проверенной identity/policy, закреплённым источником/версией, идемпотентным ключом и атомарными receipts/audit. Не создавать универсальный event bus, rule engine, generic CRUD или второй persistence ради будущих сценариев. Native Cron/Queues/Workflows — только при конкретном доказанном требовании и явном решении об их runtime semantics, не включать автоматически.

AI и provider adapters — внешние недоверенные источники предложений/результатов, не новые авторитеты бизнес-данных. Вход валидируется typed owner, права и актуальность проверяются перед эффектом; approval, ограничения автономности, durable intent, retry/reconciliation и provenance проектируются в применимом сценарии. Никаких произвольных SQL/HTTP действий от модели и никакой бизнес-policy в React. Уникальные проверки выполняются на соответствующем штатном уровне; оптимизация CI по #486 не меняет гарантии. Новый слой только при доказанной границе и реальном потребителе.

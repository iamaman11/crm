# CRM / Cloudflare — целевая архитектурная спецификация

**Решение #350, подзадача единственного верхнеуровневого плана #338.**
**Статус: целевая схема утверждена для реализации; текущая CRM ещё НЕ соответствует ей.**
Канонический источник проекта: https://github.com/iamaman11/crm .
Изменение этой спецификации — осознанное архитектурное решение в #350, а не обход тестов для старого кода.

## 1. Сначала правильная модель, затем миграция старого приложения

Старые REST DTO, JS-клиент, PostgreSQL/gateway-архитектура корня репозитория и исторические .proto **не задают целевую форму Cloudflare CRM**. Ревизия начинается от бизнес-сценариев, принадлежности состояния, типов и ошибок. Рабочий код повторно используем, только если он удовлетворяет целевой модели; несовместимое удаляем или меняем версионированно. Не поддерживаем «два окончательных API».

Область: **всё исполняемое CRM-приложение** (`apps/cloudflare-crm`, используемые `modules/crm-*`, его публичные контракты `proto/`, `packages/client`, React, Worker, D1, проверка и публикация). Не следует ради этой миграции переписывать исторические неиспользуемые сервисы монорепозитория.

Принятая цепочка:
```text
Browser / React 19 + TypeScript strict / shared UI + CSS Modules
  → generated Protobuf-ES service descriptors + Connect-Web client
  → same-origin Cloudflare Access
  → ОДИН workers-rs Rust/WASM Worker
       verified identity / authorization / request_id / Connect routing
       → generated Buffa + Connect-Rust request
       → typed Rust domain command/query
       → original modules/crm-* business owner
       → native D1 binding / one application database
       → typed Connect response + structured errors + audit
```

**Не создавать:** JS backend/route gateway, отдельный Axum-сервер, второй Worker, вторую D1, PostgreSQL, Docker, другой Environment, скрытый ручной API, дубли бизнес-правил и собственный кодек протокола. Axum разрешён только как штатный минимальный HTTP/Tower router внутри того же Rust Worker. Сгенерированный `worker-build/wasm-bindgen` JS — техническая часть официальной Rust-сборки, не JS backend.

## 2. Слой домена и слой транспорта

- Каждый доменный объект/переход имеет **одного** Rust-владельца: соответствующий оригинальный `modules/crm-*`; в Worker нет альтернативной проверки бизнес-правил. Межмодульные действия — явно вызываемые Rust-команды/запросы, не публичные фальшивые RPC.
- Доменные модели не должны зависеть от Connect, Axum, HTTP, React или генератора транспортных сообщений. Доменные команды принимают **типизированные доменные аргументы** и возвращают типизированные результаты/ошибки.
- На внешней границе: **generated Buffa request → проверка формата/identity → доменная команда → generated Buffa response**. Нужен только неизбежный mapping wire ↔ domain; **не** `Buffa.encode → prost.decode` и не промежуточный JSON или DTO только ради трансляции типов.
- `connectrpc 0.9.1` использует Buffa в сгенерированном Rust API; используем его штатно. Существующий `prost` допустим **только внутри независимых исторических/межмодульных контрактов**, где он действительно нужен. Не вводить второй транспортный Rust API; миграция protobuf-backend домена не является предварительным условием.
- SQL, D1 binding, сохранение aggregate, CAS, transaction batch и журнал — Rust persistence boundary при владельце операции; это техническая граница хранения, а не второй owner или общий бизнес-router. Вся рабочая БД — **одна D1**.

## 3. Нормативные правила внешнего Protobuf API

Источник публичного контракта: **только `proto/crm/<domain>/vN/*.proto`**. Используем стандартные `service / rpc / message / enum`, официальные Protobuf/Buf и версионированные пакеты; у каждого RPC один доменный owner и один живой обработчик. Один активный API для каждой бизнес-операции. Не публиковать автоматически все 73 исторических файла как готовые endpoint.

**Версии:** совместимый со смыслом v1 можно использовать; несовместимый по типам/поведению — новое `v2` в исходном доменном пакете, без gateway-пакета или скрытого alias. После cutover старый маршрут для этой операции в Cloudflare CRM удаляется. Другие потребители монорепозитория не ломаются произвольным удалением старого `.proto`. Неподключённые RPC обозначаются как DECLARED_NOT_IMPLEMENTED; их нельзя считать функционалом продукта.

Общие правила для проектирования всех RPC:

| Решение | Норма |
|---|---|
| Идентификаторы | Opaque string ID; tenant и actor только из проверенного Access, не доверять клиентскому `tenant_id` и `role` |
| Money | Новый авторитетный контракт **signed `int64 minor_units` + `currency_code`** с фиксированным доменным диапазоном и проверенной разрядностью/экспонентой валюты; TS представляет int64 как `bigint`, Connect JSON — строкой; в UI никогда Number для хранения денег |
| Момент времени | `google.protobuf.Timestamp` (секунды + наносекунды); календарная дата без времени — отдельный Date-тип; не смешивать их и не терять точность при преобразовании |
| Версии | `int64 version/expected_version`; каждая конфликтующая запись обновляется CAS, не last-write-wins |
| Частичное изменение | Явная присутствующая операция `set / clear` для изменяемых полей; отсутствие означает keep; `UNSPECIFIED` в enum не является валидным бизнес-значением |
| Ссылки | Сервер проверяет type, existence, owner tenant, разрешения и актуальность; не принимать клиентский tenant в ref как подтверждение доступа |
| Списки | Ограниченный `page_size`, непрозрачный `page_token`, устойчивый sort/filter. Нереализованные параметры должны быть отклонены, не игнорироваться |
| Мутации | Idempotency-key, логический owner, CAS и audit; повтор идентичной команды не создаёт вторую запись |
| Ошибки | Стандартный Connect code + стабильный типизированный domain error detail / field violations + серверный `request_id`; никаких вторых ручных JSON ошибок |
| Поля | Явная семантика required-by-validation, optional, oneof, defaults, negative/unknown enum; резервировать удалённые номера/имена при новой версии |

Это **целевые решения**, не утверждение, что текущий `crm.core.v1.ExactMoney` или `crm.sales.v1.DealService` уже им соответствуют. Изменять старые опубликованные поля in-place нельзя: при несовместимости создаём новую версию. Для времени, денег, patch и domain errors нужны обязательные JSON+binary conformance и boundary tests. Не генерировать параллельную OpenAPI/DTO-модель для тех же операций.

**Правила реализации RPC:** все реально предлагаемые методы и поля обслуживаются корректно; неподдерживаемый метод явно NOT_IMPLEMENTED/UNIMPLEMENTED, но не является частью активного UI. Не принимать клиентскую `allow_skip/allow_regression` как источник политики стадий: команда Rust решает допустимость перехода. Ни один входной параметр не отбрасывается молча.

## 4. Auth, D1 и наблюдаемость — одна сквозная модель

- Access JWT проверяется в одном Rust ingress; actor, tenant и role извлекаются только из удостоверенных claims и текущих серверных прав. Проверка доступа к каждой операции, изоляция tenants, отрицательные случаи A/B; клиентские заголовки `x-tenant-id`, `x-role`, `x-request-id` не считаются доверенными.
- Один серверный `request_id`: ответ + Rust log + D1 audit + Cloudflare Logs. UI показывает его в ошибках. Внутренний request context не может быть создан или перезаписан браузером.
- Мутации: atomic CAS+state+idempotency receipt+audit. Применять нативный параметризованный D1 SQL, предикаты версий и, где применимо, транзакционный `D1Database.batch`. `batch` сам по себе **не** доказывает, что конфликт CAS не оставил ложный audit: это надо проверять отдельными интеграционными тестами.
- Одна существующая D1, миграции версионированы и недеструктивны. Rollback Worker **не откатывает D1**, поэтому изменения БД сначала backward-compatible; проверить повторное выполнение migrations и прошлый Worker на новой схеме.
- Настоящая текущая версия = Cloudflare Worker version + Git SHA + 100% traffic + bindings. Doctor сообщает подтверждённое состояние, а не предполагаемый PASS.
- Connect unary — обычный API браузера. Browser использует `createConnectTransport` same-origin; JSON разрешён для диагностики, protobuf binary проверяется отдельно. Нет собственного сериализатора, второго gRPC-Web gateway или второго HTTP бизнес-API.
- Исключения из RPC — только действительно отличающиеся HTTP-потоки: скачивание файла/CSV, static assets и низкоуровневый health/doctor; список явно фиксируется и должен проходить сквозные тесты, без дублирования бизнес-команды. Не обещать browser client-streaming, пока нет отдельной нативной проверки браузерной поддержки.

## 5. Единственная frontend-система

`apps/cloudflare-crm/web/src/`: `*.tsx` + `*.ts`, `strict`, `allowJs:false`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`; без ручных запросов/ответов API, произвольного `any` и `as unknown as` как способа обходить проверку. UI строится на сгенерированном `packages/client` — один общий Connect transport, типизированные RPC и единый обработчик ошибок.

`ui/` содержит общие Button/Input/Select/Dialog/Table и другие действительно повторяемые элементы; внешнее оформление — семантические tokens, CSS Modules для локальных стилей. Встроенные `system/light/dark` темы, ручной выбор в localStorage (не D1). Новый экран использует эти примитивы по умолчанию. React отвечает за формы/отображение и локальный UI state; все списки допустимых действий, права и бизнес-справочники — Rust.

## 6. Один воспроизводимый build и обязательный CI

- **Один** pnpm workspace/lock для приложения и существующего `packages/client`. Один Buf source tree `proto/`, pinned codegen. Clean checkout должен собрать без ручной подготовки `Buf lint → breaking → TS generation → strict tsc → Vite → Rust Connect/WASM → workerd + D1`.
- Для Rust используем штатный `connectrpc-build`/Buffa codegen, для TypeScript `protoc-gen-es`; оба читают одни `.proto`. Проверять детерминизм. Ignored generated output проверять в чистых сборках/hashes, не притворяться, что `git diff` увидит ignored файлы.
- `Buf breaking FILE` против неизменяемой **физически принятой API baseline** и актуального `main` при разных версиях, не только против текущего `dev`. Неизвестный baseline => FAIL. Если меняется contract major version — старый опубликованный контракт не удалять до фактического cutover и проверки клиентов.
- Единственный `.github/workflows/cloudflare-crm-spike.yml` должен запускаться на изменение **любых исходников, влияющих на CRM**, в т.ч. `proto/`, `modules/`, `crates/`, `packages/client/`, корневых config. Вначале простой fail-closed запуск на каждый push dev/PR, без хрупких paths-фильтров; без дополнительного workflow/Environment.
- Проверять AST/import boundaries: новый `fetch('/api/...')` бизнес-эндпоинт, `ApplicationGatewayService` в Cloudflare bundle, новые JSX/JS-клиенты, прямой вызов D1 из React, обход `ui/` для общих контролов — FAIL. Условные исключения версионированы и ограничены.
- Проверять реальный код, не только presence файлов: source descriptor ↔ generated service ↔ handler ↔ original owner ↔ route ↔ UI ↔ tests; user-visible операции не могут обходить API. Run negative self-tests для breaking, ручного fetch, неправильного типа, недостающего handler, D1 CAS и неучтённого UI-контрола.
- Существующая штатная публикация: CI на одном SHA, защищённый crm-dev, real workerd/Chromium Access A/B, role/tenant, D1 audit, Cloudflare Logs, exact deployed version+SHA и rollback. Branch rules/required checks — отдельная репозиторная гарантия: CI без protection не мешает владельцу прямым push изменять dev.

## 7. Переход ВСЕГО действующего CRM

Это **один завершённый архитектурный релизный рубеж**, допускающий несколько промежуточных коммитов в `dev`, но не долгоживущий двойной backend.

1. **Входной gate.** Независимо проверить реальную active Worker version, accepted SHA, один D1, причину последнего неуспешного Logs audit и откат. Зафиксировать ожидаемые пользовательские сценарии: 12 разделов, операции, варианты, отрицательные действия и текущие UI-кнопки. Старый API — только материал инвентаризации, не целевой стандарт.
2. **Сначала API design.** Для каждого существующего пользовательского сценария определить владельца, допустимые действия, состояние, инварианты и схему нового RPC. Сопоставить `proto/`, но **не** подчиняться ему: оставить совместимые части, несовместимые выразить в `v2`, отсутствующее описать. Опубликовать contract acceptance fixtures до изменения React. Не переносить неработающие вымышленные функции из исторических `.proto`.
3. **Закрепить сборку и требования.** Один pnpm workspace, Buf, generated TS/Rust, strict typecheck, CI без пропусков. Новые возможности с этого момента только по конечной схеме.
4. **Rust API end-to-end.** Native Access/Connect/D1 для всех уже работающих возможностей: Parties, Accounts, ContactPoints, Sales, Activities, Relationships, Consents (достоверный deny/evaluate), Customer360, Identity Resolution (только реальные reads), Quality read, ограниченные CSV operations, Privacy intake, Enrichment config, Session/doctor. Internal sales-activities-link остаётся внутренней командой. Нереализованные внешние операции не маскировать.
5. **Frontend cutover.** Все существующие 10 React JSX компонентов и presentation files в strict TS/TSX; все видимые бизнес-запросы — generated Connect; общие UI-компоненты/темы. Поведение и пользовательские действия сохранены.
6. **Удалить старое в том же candidate.** В Cloudflare CRM нет заменённых `/api` business REST handlers, ручных JSON DTO, дублирующего gateway, пустых фальшивых RPC. Если другие приложения зависят от старого `packages/client`, менять его с учётом их потребителей, не ломать их вслепую. Исключения только явные.
7. **Одна финальная физическая приёмка ПОСЛЕ удаления.** Все действующие операции, кнопки, отмены, повтор, ошибка, различие tenants/roles, idempotency, версии, деньги, темы, D1 и Logs. Тот же кандидат из CI публикуется на `crm-dev`; ровно его SHA/Worker version и 100% traffic подтверждены; возможен rollback на прежнем совместимом D1.
8. **Закрыть #350 и checkpoint #338** только при доказанном PASS. После — новый функционал по `owner → .proto → generated RPC/client → TSX/shared UI → CI/E2E` без исключений по умолчанию.

**Критерий DONE:** новая схема *определяет* весь активный CRM, а не «соответствует старому». Любой новый бизнес-API не по ней автоматически блокируется CI. Неудачная публикация не считается принятием.

## 8. Основания технического выбора

- Cloudflare official Rust Worker: https://developers.cloudflare.com/workers/languages/rust/
- Connect-Rust 0.9.1 / Buffa code generation: https://docs.rs/crate/connectrpc/0.9.1
- Connect-Web generated client: https://connectrpc.com/docs/web/getting-started/
- Buf compatibility FILE: https://buf.build/docs/breaking/
- Native D1 SQL/batch: https://developers.cloudflare.com/d1/worker-api/d1-database/

**Важно:** документ задаёт целевое состояние; отмечать архитектуру реализованной только по реальному коду и доказанному CI/live, а не по наличию этого документа.

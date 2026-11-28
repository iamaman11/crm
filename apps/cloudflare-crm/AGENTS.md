# Cloudflare CRM — инструкции разработки

Общие правила — [корневой AGENTS.md](../../AGENTS.md); техническая схема — [ARCHITECTURE.md](ARCHITECTURE.md). Единственный продуктовый план [#484](https://github.com/iamaman11/crm/issues/484) указывает текущий исполнительный курсор ([#487](https://github.com/iamaman11/crm/issues/487) или его официальный преемник). [#485](https://github.com/iamaman11/crm/issues/485) — надёжность; [#486](https://github.com/iamaman11/crm/issues/486) — единственный документ оптимизации CI, экспериментов и измерений. [#489](https://github.com/iamaman11/crm/issues/489) определяет только подготовку, допуск и интеграцию параллельных batch; он не заменяет продуктовый курсор и не разрешает запуск до полного Definition of Ready.

## Подготовка функционального среза

Сначала постоянный скелет S0, затем развитие S1–S4 в порядке #484. Не ограничивать полезную функциональность временным MVP, не начинать с очереди старых crates. До изменения поведения проверить текущий код и принятые evidence, выявить конкретное действие пользователя, его данные, права и результат. Только затем найти соответствующие исторические Rust-операции и отрицательные тесты в [архивной матрице](../../docs/archive/cloudflare-snapshots/crm-functional-execution-matrix.md); изучить все относящиеся к действию источники и потребителей. Исторический OPEN — повод проверить, не приказ писать код.

A — нужная семантика уже доказана; B — полезный конкретный пробел, реализовать полный путь; C — обоснованное решение не переносить или отложить с предметной причиной и условием пересмотра, без потери нужной семантики; D — только устаревшая инфраструктура после проверки потребителей и инвариантов. Иначе OPEN. Записать ID, источники, причины и proof в курсоре до кода. Не переносить автоматически внутренние IDs, универсальные фильтры, технические настройки и старые runtime в UI. Продуктовая оценка — подготовка среза, не третий gate.

## Принятая модель видимости полей (обязательна при каждом новом модуле)

По решению владельца **все бизнес-поля доступной записи своего tenant видимы и
`viewer`, и `admin` в рамках существующих Get/List/Customer360 контрактов**.
Не создавать field-level ACL, field masks, отдельные роли чтения отдельных полей
или копию исторического `QueryVisibilityAuthorizer`: это **принятое C для текущего продукта**.
`viewer` остаётся без обычных Write/AdminRead; узкие SelfWrite только по действующей typed policy и проверке активного human membership владельцем. Signed Access, tenant isolation,
cross-reference checks и ограничения операций сохраняются. Не переносить
возможность видеть данные CRM на диагностические логи: JWT, secrets, PII и
raw business payload там по-прежнему нельзя раскрывать. При разработке проверять
одинаковые доступные бизнес-поля в read-ответах admin/viewer и отсутствие
межтенантной утечки; использовать существующий native workerd/Chromium, а не
новую систему тестирования. Ранее помеченная неопределённость о том, **какие
поля скрывать от viewer, снята решением C**, но это не закрывает остальные
инварианты и пользовательские карточки.

## Завершение задания исполнителя и production-приёмка координатора

Единица разработки — **крупный связанный пользовательский сценарий**, а не один RPC/экран/таблица. Точные границы нового среза задаются в текущем курсоре по #484. До полного CI согласованно подготовить `.proto`, owner/Rust, policy, D1, React и tests; локальные commits и unpublished Git objects допустимы.

**Обычный одиночный режим:** разработчик завершает один связанный срез и делает один атомарный code push в `dev`; существующий workflow запускается только на code push в `dev` (`cancel-in-progress: false`). Последовательность микрокоммитов в `dev` запрещена.

**Параллельный batch:** допускается только после полного Definition of Ready в #489. Все writers получают один frozen base SHA, отдельные checkout/worktree/CWD и временные `feat/parallel/*` ветки, самодостаточные ТЗ и непересекающиеся write scopes. Между агентами нет чата, ожидания или согласования; writer не push/merge в `dev`, не деплоит, не получает Cloudflare credentials и не использует remote D1. Каждый writer лично выполняет Writer Gate A и независимый Writer Gate B на одном exact SHA своей ветки. Writer Gate A — полный build/test/workerd/D1/Chromium в `cloudflare-crm-spike.yml`, deployment `crm-dev` на feature refs пропускается. Writer Gate B — Browser MCP на реально опубликованном branch-preview с отдельными Worker и D1; preview и D1 evidence выдаёт только защищённая automation/координатор. `crm-dev` и общая D1 для branch Gate B запрещены. Если любой writer gate или безопасный preview отсутствует, writer фиксирует BLOCKED; handoff запрещён. После обоих PASS writer получает `READY_FOR_HANDOFF`: его задание завершено, и он передаёт координатору SHA, scope, CI run, preview Worker/version, Browser MCP сценарии и D1 evidence. Writer не обязан проходить интеграционные или production gate и не отвечает за публикацию в `dev`.

Для каждого завершённого **параллельного продуктового среза** обязательны две приёмки исполнителя и две production-приёмки координатора на exact SHA:
1. **Writer Gate A = PASS:** тот же `cloudflare-crm-spike.yml` запускается на `feat/parallel/**` и проверяет Buf/generated Rust+TS, policy/owner, D1 workerd, negative cases, Chromium и runtime RPC; артефакт запечатывается для exact SHA. На feature ref deployment `crm-dev` пропускается.
2. **Writer Gate B = PASS:** writer лично через Universal Browser MCP проверяет действия пользователя на опубликованном preview именно своей ветки. Сверяются source SHA/Worker version; проверяются реальные сценарии, сохранение после reload, права/tenant isolation/ошибки, а trusted verifier выдаёт read-only D1/receipt/audit/trace evidence. У preview отдельные Worker и физическая D1; `crm-dev` не используется. Preview создаёт/удаляет защищённая automation/координатор; writer не получает deploy/D1 credentials. CI Chromium и встроенный smoke не заменяют этот Gate B.
3. **Candidate integration preflight — проверка координатора, не Production Gate A:** после приёма handoff координатор объединяет ветки на одной временной `integration/parallel/*` и запускает существующий build/test workflow на exact combined SHA; deployment обязан быть SKIPPED. Это проверяет совместимость набора изменений перед fast-forward в `dev`, но не продлевает задание writer и не заменяет production acceptance.
4. **Production Gate A = PASS:** после fast-forward в `dev` координатор получает PASS всего существующего `cloudflare-crm-spike.yml` на exact `dev` SHA: CI build/test job и `deploy-crm-dev` job оба PASS; deploy job скачивает и проверяет sealed artifact того же run/SHA без повторной сборки, публикует его в `crm-dev`, сверяет source SHA/Worker version и выполняет опубликованный Chromium smoke, Access/RPC, физическую D1 readback, receipts/audit/traces и fail-closed checks. Одного CI build PASS или одного deployment status недостаточно.
5. **Production Gate B = PASS:** только после Production Gate A координатор независимо через Universal Browser MCP проверяет реальный опубликованный `crm-dev` на том же exact SHA/Worker version: пользовательские сценарии, сохранение и связи после reload, права/tenant isolation, негативные случаи и физическую D1/readback. CI Chromium и live smoke внутри Production Gate A не заменяют независимый Gate B.

Использовать существующую авторизованную service-admin identity в owner tenant; не выдавать её за human. Реальный внешний human login/двухпользовательский сценарий принимается отдельно, когда он является предметом функции. `MCP_FAIL` инструмента → исправить Browser MCP и повторить только соответствующий Gate B на том же SHA; реальный `CRM_FAIL` → исправить дефект, получить новый SHA и повторить затронутые writer A+B и production A+B.

Статус writer branch `READY_FOR_HANDOFF` допустим **только при Writer Gate A PASS + Writer Gate B PASS** и всех применимых дополнительных security/human prerequisites. После передачи writer считается закончившим свою задачу; он не отвечает за integration preflight или production gates. Статус `INTEGRATED_PENDING_PRODUCTION` означает, что координатор включил exact branch result в `dev`, но production acceptance ещё не завершена. Статус продуктового среза `ACCEPTED` допустим только при Production Gate A PASS + Production Gate B PASS на одном exact опубликованном `dev` SHA. Все exact SHA, CI links, preview Worker/version, independent browser actions и D1 evidence фиксируются в #487/#489/#task issue. При любом FAIL/UNCERTAIN — `NO-GO`, не передавать и не закрывать срез.

**Если код для функции уже существует и работает (A):** не писать его повторно и не запускать новый CI только из-за `OPEN`. Использовать существующий PASS Gate A на exact опубликованном SHA; проверить, есть ли достаточный независимый Gate B для данного сценария, и выполнить только отсутствующий proof. При наличии обоих и сверки legacy семантики зафиксировать ACCEPTED в текущем курсоре без code push. C оформляется с причиной и условием пересмотра без реализации; только доказанный бизнес-gap B требует кодового среза и новой A→B приёмки.

**Нулевая потеря старой функциональности как обязательство проверки, НЕ как уже доказанный факт:** [полная 93/93 crosswalk-таблица](../../docs/crm-functional-execution-matrix.md) связывает все 532 исходных файла и их 6 633 найденные функции с функциональными ID либо с `X-01` (незакрытая legacy-infrastructure review). Сохранить source/tests. До изменения функции — изучить её nested исторические методы/негативные тесты и current owner; если выявлен неизвестный нужный кейс, добавить B в существующий ID или отдельный пакет. До **любого удаления crate** и заявления полной parity разрешение дают только рассмотренные **по операциям** A/B/C/D, проверенные consumers и соответствующие доказательства, не один ярлык по имени crate. `X-01` не является механической очередью S0, но блокирует массовое удаление и заявление 93/93 behavior DONE.

## Рабочий процесс

1. Прочитать #484, текущий курсор и архитектуру; **сначала** сверить принятые сценарии, действующие owner/RPC/UI и имеющееся evidence, затем проверить ещё не решённую пользовательскую задачу и соответствующие legacy Rust-операции/тесты. Выбрать конкретный B-gap **только после** этой сверки; A/C не требуют реализации.
2. Обычная одиночная работа изменяет канонический GitHub `dev`. Параллельный batch — только по #489: writers изменяют собственные временные ветки, каждый доказывает Writer Gate A + Writer Gate B и не публикует в `dev`; после `READY_FOR_HANDOFF` его работа завершена. Только координатор выполняет candidate integration preflight, fast-forward в `dev`, Production Gate A и Production Gate B. PR может использоваться для обсуждения/review, но не как обход этого порядка. Не менять branch protection, обязательные reviews или схему CI без решения владельца.
3. Окончательные сборки и проверки выполняет GitHub Actions; локальный preflight штатными инструментами допустим. Не просить пользователя запускать команды.
   Локальное чтение, редактирование и генерация документов допустимы.
4. В одиночном режиме один code push в `dev` запускает существующий workflow: контракты/типы → Rust/TS/тесты → сборка → workerd/D1/Chromium → публикация sealed artifact; полный dev workflow (CI job + `deploy-crm-dev` job) является Production Gate A, после его PASS отдельно выполняется Production Gate B. В параллельном batch каждый push в `feat/parallel/*` запускает Writer Gate A без deployment, writer затем выполняет Writer Gate B; только ветки с A+B допускаются к candidate integration preflight на `integration/parallel/*` (deploy SKIPPED). После fast-forward координатором в `dev` он обязан получить Production Gate A (оба dev jobs PASS) и отдельный Production Gate B на одном exact SHA. `deploy-crm-dev` остаётся разрешённым только для `push` в `refs/heads/dev`. Пока PRE-02 и PRE-07 не реализованы и не доказаны, parallel batch не запускать. Наличие commit в `dev` не означает приёмку.
5. Изменения только `**/*.md` не запускают сборку или deployment. Markdown не должен быть
   входом runtime/codegen. Смешанные изменения документации и кода проходят полный CI.
   Редактор проверяет смысл и ссылки; тесты документации выполняются при следующем code CI.
6. Выполняющийся запуск заканчивается; GitHub concurrency оставляет последнюю ожидающую версию.
   Не прерывать deployment каждым новым push. Промежуточные commits не обязаны публиковаться.
7. Результаты записывать в issue: source SHA, ссылка на CI, результат live-приёмки, версия Worker
   и остаток работы. Не коммитить эти чекпойнты, номера запусков и текущие статусы в Markdown
   приложения. Документация меняется при изменении правил, а не после каждого релиза.

## Обязательные границы

- Protobuf — единственный wire contract; generated Rust/TS, оригинальные Rust owners и typed mappings. **Обхода нет:** нельзя создавать альтернативные ручные DTO/типы или транспорт, исполнять RPC без canonical `module.yaml`/compiled policy/owner, писать бизнес-правила в React/JS handler или обращаться к D1 в обход оригинального Rust owner. Любое новое поведение проходит существующие descriptor/router/policy/TypeScript/Oxlint/native evidence checks; `OPEN` не разрешает архитектурное исключение.
- Собственный CRM API не обещает совместимость со старыми тестовыми клиентами. Менять контракт,
  обработчик, UI и тесты согласованно. Не подменять приёмку текущего SHA last-green или историческим результатом; сохранять принятые persisted-wire compatibility checks. Общие контракты других потребителей сохраняют
  свои обязательства. Legacy PostgreSQL process runtime (`services/crm-api`,
  `crates/crm-application-runtime`) выведен из активного workspace и CI; не восстанавливать
  и не расширять его как альтернативный backend без нового прямого решения владельца.
- Один Worker, Connect router, `packages/client/cloudflare`, `packages/ui` и одна D1. Никаких
  второго транспорта, business REST, JS backend, generic CRUD/ORM или копии domain rules.
- Identity, tenant, role и request_id проверяет Rust; одна явная fail-closed policy.
- Buf descriptors, compiled router/policy и TypeScript compiler и штатный Oxlint проверяют структуру.
  Не разбирать Rust/Protobuf semantic regex и не доказывать тесты строками в исходниках.
- Новый RPC расширяет проверяемую область. Обязательны owner, policy, handler и исполненные
  сценарии. Количество RPC/модулей не фиксируется константой.
- Сохранять auth, tenant, CAS, replay, audit, domain и browser tests. Ошибочная команда оставляет
  агрегат неизменным; audit отражает фактическую операцию. UI не копирует бизнес-валидацию.
- Нельзя получать PASS исключением неудобного метода или ослаблением инварианта. Исправление
  ошибочной проверки сохраняет её назначение и подтверждается регрессионным тестом.
- Обычное изменение D1 — следующая миграция. Редактирование применённого SQL не обновит live D1.
  Намеренное пересоздание тестовых данных оформляется явно; Access/учётные данные не удаляются.
- Никакого нового crate, реестра, генератора метаданных, workflow или doctor для обычной функции.
  Существующие модули и стандартные средства остаются основным путём расширения.
- Сначала используй штатные возможности текущего стека и действующие проверки. Если задача
  требует нового механизма, изменения архитектурной границы или ослабления проверки, до
  реализации покажи в issue конкретный пробел, проверенный штатный вариант и гарантию, которую
  нужно сохранить. Согласуй изменение с владельцем. Адресный тест в существующем наборе
  добавляется в обычном порядке.

## Приёмка

Code CI проверяет текущий контракт, владельцев, типы, реальные права и поведение.
Для изменения тестов и диагностики применяй [нормативные границы типов](ARCHITECTURE.md#типы-тестов-и-диагностики).
При ревью проверь включение сценария в strict typecheck, сохранение негативных проверок
и отсутствие тела ответа или секретов в диагностике сбоя.
Production `native` и test-only `native-ci` различаются; оба проверяют обычный JWKS path, локальные fault probes и CI-only endpoint/guards не попадают в release.
Cargo cache ускоряет компиляцию, но не заменяет проверки текущего source. Артефакт публикуется
без пересборки, с проверкой SHA/checksum и live Access/UI/D1/Logs. Отсутствие обязательного evidence
после выполненного сценария — FAIL; ранняя ошибка не создаёт вторичную ошибку загрузки скриншота.

DONE функции означает законченный пользовательский сценарий и применимые CI/live assertions,
зафиксированные в issue. Типизация не доказывает всю бизнес-семантику, а зелёный CI не означает
промышленную готовность. Продуктовые ограничения и будущие этапы зафиксированы в #484.

## CI и штатные инструменты

[#486](https://github.com/iamaman11/crm/issues/486) задаёт цели скорости, baseline, историю и методику измерений. Сначала Rust owner unit/integration, затем Wrangler/workerd/D1, затем Playwright и Buf/generated contracts. Свои адаптеры — только при доказанной уникальной гарантии и отсутствии штатного эквивалента. Не убирать уникальные postcommit/recovery tests и не добавлять test hooks в production. Технический CI-only эксперимент принимает полный CI и сравнимое измерение, не новый продуктовый Gate B. Не принимать ускорение по одному cache hit или размеру архива. Технические замечания по надёжности ведутся в #485. Историческая [редакция инструкций](https://github.com/iamaman11/crm/blob/c69f1e1beb621c22b907c877cfdb5cf6ec88c25f/apps/cloudflare-crm/AGENTS.md) сохранена в Git, не нормативна.

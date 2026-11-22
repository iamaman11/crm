# CRM #352 — функция → исторический Rust → действующий owner → приемка

**Назначение:** долговечная исполнимая очередь разработки тестовой CRM, а не просто индекс старых crate. Открывать при каждом следующем сеансе **до** выбора задач и изменения кода. Правило приоритетов: `M1 → M2 → NEXT (по доказанному спросу) → LATER → END`. Ни один пункт LATER/END не блокирует готовность MVP. **Первое действие: проверить, какую реальную пользовательскую задачу ещё НЕЛЬЗЯ выполнить, и сверить исходный owner, существующий UI и реальные тесты. OPEN в старой матрице НЕ означает отсутствующий функционал: нельзя выбирать ID механически, заново реализовывать или прогонять тяжёлые Gate без доказанного пробела. Для уже существующего A — переиспользовать прежний Gate A, быстро проверить независимый Gate B только при недостающем свидетельстве, зафиксировать ACCEPTED без кода; для C — не реализовывать; только доказанный пользовательский B требует кодового среза.** Нельзя превращать исходную бизнес-логику в отдельный старый runtime.

**Доказательная база:** [инвентаризация всех 93 crates](legacy-crate-semantic-matrix.md) · [все 532 Rust-файла и test/guard symbols](legacy-crate-operation-evidence.md) · [20 глубоких gap comparisons](legacy-crate-capability-audit.md). Исходный снимок [`a627e6a9f196`](https://github.com/iamaman11/crm/commit/a627e6a9f1966aaac5fcc0179c943811f879eb38); успешно проверенный основной CI+deploy [#44913591777](https://github.com/iamaman11/crm/actions/runs/44913591777). Поздние docs-only коммиты НЕ означают новый опубликованный Worker. Каждая из 39 строк содержит ссылки на реально существующие старые и нынешние файлы, проверенные по Git tree.

## Сначала полезный пользовательский сценарий, затем перенос бизнес-семантики

Нельзя автоматически превращать обнаруженную в старых crates операцию в новую кнопку, фильтр или RPC. Для каждого ID **до кодирования** ответить по существующим исходникам и реальному UI: кто пользователь, какое конкретное действие хочет совершить, каково ожидаемое изменение/чтение и как оно проверяется, хватает ли существующих данных и прав, что станет непонятным или лишним, можно ли решить задачу меньшим изменением. Проверить несколько связанных записей/ролей, пустой результат, отрицательные случаи, navigation/reload. Если нет действительного пользовательского смысла или отсутствует фундаментальная функция (например, назначения сотрудника), оформить в #352 **обоснованное C только для MVP** с условием пересмотра; не объявлять исчезнувшей доменную семантику и не удалять historical source/tests. Если есть смысл, **B = полный конкретный путь UI→типизированный owner→D1→обратные ссылки**, а не 1:1 перенос generic query engine. Если уже работает — доказать A; D только для технической инфраструктуры и после consumer review.

Продуктовый разбор записать в #352 **перед code push**, в виде короткого решения `пользователь → задача → самая простая модель → A/B/C/D → критерий`. Это не третий acceptance gate и не новый реестр; два обязательных Gate A и B остаются прежними. Не показывать пользователю внутренние ресурсные типы, ID, версии и не обещать несуществующее создание связей или назначение задач.

## Единица разработки: крупный логический срез, а не микропатч

**Обязательные логические срезы (S-M1-A/S-M1-B/S-M2 и N-01/N-02/N-03/N-04 приняты для dev/MVP; остальные NEXT — только кандидаты):**

| Срез | Связанный пользовательский результат | Реестр ID | Граница приёмки |
|---|---|---|---|
| **S-M1-A** | Найти/выбрать человека и организацию, создать Account, контакт, связь, перейти по связанным карточкам без ручных ID (включая фильтры Deal/Task) | `M1-01…M1-06` | **ОДИН завершённый кодовый кандидат**, встроенные сценарии и независимый LIVE Browser |
| **S-M1-B** | От клиентской карточки провести Deal и Task через stage/close/reopen с atomic Sales→Task и reload | `M1-07…M1-10`; `M1-11` как явное решение | Один сквозной пользовательский flow, не отдельно «RPC успешно зарегистрирован» |
| **S-M2** | Удобная воронка, этапы, контакт, views и точные метрики; lead решён как B/C | `M2-01…M2-05` | Полный Sales user journey, stage/currency/permission регрессии |
| **S-NEXT** | Один **только востребованный** ограниченный поток CSV или Customer360/quality | Один связанный subset `N-01…N-06` | Каждая самостоятельная история имеет полный user outcome; не объединять несвязанные ради размера |

**Порядок:** S-M1-A/S-M1-B ACCEPTED #7070842108, S-M2 ACCEPTED #7071938677/#7072122525/#7071303504/#7072127526, N-01 ACCEPTED #7079664966, N-02 A ACCEPTED #7079751195 без нового кода, N-03 A+B ACCEPTED #7079988713, N-04 A+B ACCEPTED #7080576593 (Gate A #45031786040 + independent Gate B на опубликованном `583cc43c`); продолжать с первого ДОКАЗАННОГО полезного B-пробела среди NEXT, а не со следующего номера; NEXT по требованию, LATER/END только по новому продуктовому решению. Связанные protobuf/owner/policy/D1/React/browser тесты готовить **одним рабочим срезом** до обращения к полному CI. Поскольку существующий `.github/workflows/cloudflare-crm-spike.yml` запускается **на каждом code push в `dev`** и concurrency стоит `cancel-in-progress: false`, **не делать промежуточных code push в `dev` по отдельным файлам**: подготовить полный связанный diff (рабочие commits/подготовленные blobs/tree без обновления `dev` допустимы), затем **ОДИН атомарный push готового кандидата в `dev`** → один обычный CI. После реального FAIL исправлять и повторять только необходимый полный code gate; docs-only не запускают CI. **Не начинать новый полный CI на каждый отдельный файл, RPC, форматирование или частичное изменение.** Если в срезе обнаружен дополнительный необходимый guard/UX/contract, включить его в тот же срез; отдельную независимую функцию не присоединять ради размера.

**ОБА обязательных gate на ОДНОМ code/source SHA для КАЖДОГО принятого среза:**

- **Gate A / CODE:** существующий `cloudflare-crm-spike.yml`: Buf/strict generated Rust/TS, Rust owners/policy, D1 migrations+atomicity/CAS/replay/audit, workerd и встроенный Chromium → deploy **того же** проверенного artifact без пересборки → exact published SHA/Worker version, live Access/traces/D1 checks. Статус `PASS/FAIL`, URL run и SHA. CI Chromium является частью **Gate A**, не Gate B.
- **Gate B / INDEPENDENT LIVE:** только **после** PASS Gate A, **отдельный Universal Browser MCP** открывает опубликованную CRM через реальную Cloudflare Access/Connect identity, проверяет тот же SHA/Worker version до/после, сам выполняет пользовательские действия и проверяет postconditions после reload, связанные сущности, применимые негативные случаи, readback/audit. Статус `PASS/FAIL/UNCERTAIN`, реальные шаги/доказательства, идентичные source SHA/Worker version. Service principal — не реальный human: positive-human-only операции требуют отдельного честного proof, impersonation запрещён. MCP инструмент при сбое → `MCP_FAIL` и повторить **только Gate B** после исправления инструмента; дефект CRM → исправить код и повторить Gate A → Gate B.
- **DONE/ACCEPTED:** **оба `Gate A=PASS` И `Gate B=PASS`** + применимые human-only доп. условия, source→active evidence и business readback. Нет Browser MCP или непроверенный SHA? **NO-GO/PARTIAL**. Не подменять Gate B CI/live-check внутри Gate A. За один срез один issue/checkpoint в #352; не создавать второй CI/регистр/approval platform.

**Встроенная защита от потери legacy:** все исходные 93 crates распределены по таблице покрытия ниже, но **ни одна не подтверждённая операция не считается C/D автоматически**. В срезе нужно проверить ВСЕ соответствующие старые `src`/tests, включая вложенные public методы/побочные эффекты и скрытые invariant helpers. Любая новая полезная функция, замеченная в source, получает отдельную явную строку текущей таблицы или чётко обоснованный B-LATER/B-END; её нельзя потерять потому, что её crate уже записан как инфраструктурный.

## Правило для следующего чата/исполнителя (обязательно, не рекомендация)

1. Прочитать [#352](https://github.com/iamaman11/crm/issues/352), `apps/cloudflare-crm/AGENTS.md`, **эту матрицу** и текущий `dev`. **Сначала** определить, какое конкретное полезное действие пользователь пока не может выполнить, и проверить имеющиеся owner/RPC/UI/native tests и ранее принятые Gate A/B. `OPEN` означает **проверить**, а не **реализовать**; номер строки и её непринятый статус не определяют следующую кодовую задачу. Уже принятые сценарии повторно не разрабатывать и не тестировать без регрессии. Выбирать B только после доказанного пробела; для существующего A — принять по достаточному актуальному evidence без нового кода, для C — записать обоснованное решение.
2. **До изменения кода обязательно открыть источники из поля «Legacy» выбранного ID**, связанные с ними негативные тесты, вложенные modules/re-exports и их consumers (по [evidence](legacy-crate-operation-evidence.md)). Записать инварианты/ветки: вход, успешный выход, actor/tenant, права, version/CAS, idempotency/replay, audit/receipt, side effects. Если старой реализации именно этой операции нет — так и записать, без домыслов.
3. **Сверить «Сейчас»**, один original Rust owner, действующий `.proto`, generated Rust/TS, RPC/manifest/compiled policy, D1/transaction, React и браузерные тесты. Поставить **по операции** вердикт `A` (эквивалентность доказана), `B` (бизнес-gap к реализации), `C` (не нужен сознательно), `D` (legacy infrastructure, когда доказаны инварианты и все consumers), иначе `OPEN`. Наличие legacy crate — это не команда перепортировать crate.
4. Делать только gap в существующей архитектуре: один Worker + D1 + Buf Protobuf + original owners + Connect/React + один штатный CI. Не новые генераторы, registry, worker, SQLx/PostgreSQL, generic framework и обратная совместимость тестовой БД.
5. После каждого завершённого логического среза — обязательные ДВА отдельных Gate A и Gate B на одном опубликованном SHA (см. раздел выше): штатный CI/workerd/D1/Chromium/exact-artifact deploy, затем **отдельный независимый LIVE Browser MCP**, без исключения для обычного UX. Отдельные human-only prerequisites отмечать честно. В #352 записать **ID, исходный метод/отрицательный тест, точный B-gap либо решение C, SHA, CI/live/Gate B, доказательство и остаток**. Только после **Gate A=PASS + Gate B=PASS** и исходного A/B/C/D evidence пометить ID как ACCEPTED. **Новый чат начинает с проверки реального незакрытого пользовательского сценария и уже имеющихся доказательств, не с первого `OPEN`.** Для уже реализованного A переиспользовать успешно выполненный Gate A на точном опубликованном SHA; независимый Gate B выполнять лишь при отсутствии достаточного прежнего proof. Если оба подтверждены, принять A без кода, новой сборки или CI. Только подтверждённый B начинает новый логический кодовый срез и требует оба gate на новом SHA.
6. Действующие auth, tenant, signed identity, policy, CAS/idempotency, audit, PII redaction и fail-closed действуют **сейчас**. Partial Privacy 2/9 `complete=false` остаётся закрыт для export/erasure. Полная 93-crate behavioral parity **ещё не доказана**, поэтому старый исходный код не удалять массово.

## Финальная приёмка M1 для dev/MVP (22.11.2026; принята, не перепроверять без регрессии)

**S-M1-A + S-M1-B / M1-01…M1-10 = ACCEPTED для текущей dev/MVP. M1-11 = DECIDED C, не реализовывать generic Delete/archive.** Канонический code/source SHA [`af84e93766f0238a7536669efa6135a2acc28904`](https://github.com/iamaman11/crm/commit/af84e93766f0238a7536669efa6135a2acc28904): изменён только существующий `native.mjs`, без нового runtime, D1 schema, proto или дублирования CI. [Штатный Gate A #44960331611](https://github.com/iamaman11/crm/actions/runs/44960331611) PASS (оба job, exact-artifact deploy). **Отдельный независимый Browser MCP Gate B PASS** на том же SHA, HAR и D1 readback; полный per-ID legacy A/B/C + сцепленные postconditions и границы в [финальном #352 / #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108). Архив HAR: 97 entries, 54 POST/RPC, 78 HTTP 200, 0 HTTP 4xx/5xx (19 aborted favicon без ответа). D1 audit 182→182, Task receipts 28→28 по сохранённому read-only proof. Ранее отдельно приняты B-slices для контактов/связей [#7069314351](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351), Customer360 [#7070322333](https://github.com/iamaman11/crm/issues/352#issuecomment-7070322333) и Task [#7068829386](https://github.com/iamaman11/crm/issues/352#issuecomment-7068829386).

**Решение по правам:** все бизнес-поля собственного tenant-authorized Get/List/Customer360 доступны `viewer` и `admin` без field-level masks. Signed native workerd+D1 проверил `deepEqual` семи полных Get-бизнес-объектов и шести List-ответов admin/viewer, другой tenant = 404, отсутствие read-аудита (`crm.native-m1-read-parity-linked-flow: PASS`). Старый `QueryVisibilityAuthorizer` и generic field masks = C для MVP. Это **не** отменяет verified Access identity, compiled policy, Write/AdminRead denial, tenant isolation, CAS/replay, receipts/audit и приватность диагностических логов.

**Граница приёмки:** admin service-browser разрешён для обычных UI-сценариев M1; **положительный интерактивный human Cloudflare Access login/verification остаётся отдельным pre-real-user release check** и не притворяется доказанным service session. Глобальная поведенческая эквивалентность **93 crates НЕ принята**: сохранённые ниже 93 legacy crosswalk строки / «все операции OPEN» относятся именно к индивидуальным historical operations за пределами доказанного M1 dev/MVP, не открывают M1 заново. Privacy scope 2/9 остаётся fail-closed.

**M2-02/M2-04 ACCEPTED #7072122525/#7072127526; N-01 ACCEPTED позднейшим #7079664966, следующая исполнимая очередь N-02 по реальному пользовательскому gap** (M2-05 = C). Не запускать новые M1 QA-срезы, новый CI или ACL engine ради формальной галочки. Проверять существующий owner и историческую операцию *до* выбора B; уже принятые M2 stage labels и Deal name search не дублировать. #352 остаётся открытой до остальной функциональной очереди.

## M2-01 FINAL dev/MVP ACCEPTED — Sales Pipeline, этапы, Kanban и адресуемая ручная доска (22.11.2026)

**M2-01 = A+B ACCEPTED для dev/MVP** по исходным методам, negative cases и отдельным Gate A/B: [финальная пооперационная #352 / #7071938677](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677). Реальная задача пользователя: выбрать конкретную воронку из общего списка, открыть полную сделку и вернуться на тот же Kanban/метрики; прежде React держал ручной выбор только в transient state и после Back/reload терял доску. **B закрыт** через один существующий native типизированный маршрут `#sales?pipelineId=...` (включая поиск и Customer360 scoped view), без второго router, storage, RPC, D1 таблиц или Rust owner. `viewId` сохранённого представления исключает `pipelineId`; duplicate/malformed/ambiguous refs дают Not Found, неизвестный pipeline — безопасное недоступное состояние. Прежние принятые имена стадий `c7eddfb5` и Deal name-search `d94248fd` не переписывались.

**A сохранено:** historical `a627e6a9` `crm-sales-capability-adapter/src/planner.rs::plan_advance`, `crm-sales-activities-capability-composition/src/lib.rs`; действующие `modules/crm-sales/src/pipeline.rs`, `domain.rs`, `sales_pipelines.rs`, `sales_deals.rs`, `connect_sales/sales_handlers.rs`. Один исходный typed owner: Protobuf/Connect → Rust domain → D1; каталоги, stage IDs/ordinals, forward-only MVP transitions, каскадные/атомарные rank projections, CAS/replay, signed tenant authorization/denial, запрет удаления/архивации занятых стадий, receipts/audit, immutable Deal events, Close/Reopen. Native `native.mjs` продолжает проверять stale CAS, invalid stage, wrong tenant, rollback, полный Kanban count/valuta/page и referenced-stage-denial.

**Gate A PASS** на точном code/test SHA [`f8f1fcea59588f84762044c46116fb5c39590276`](https://github.com/iamaman11/crm/commit/f8f1fcea59588f84762044c46116fb5c39590276), [штатный CI #44968322133](https://github.com/iamaman11/crm/actions/runs/44968322133): `cloudflare-crm-spike` и `deploy-crm-dev` = SUCCESS; exact-artifact deploy без пересборки, native signed workerd/Chromium, опубликованный Browser E2E и D1 traces. Ранние CI #44967474021/#44967503796 выявили тестовый отсутствующий helper и устаревшее требование сбрасывать Kanban в Customer360; тесты исправлены **с усилением** сохраняемого URL и фильтров, не удалялись и не ослаблялись.

**Независимый Gate B PASS** на опубликованном том же SHA: Browser MCP `27ee381c-15cb-4fd1-9c74-27410c0320ed`, точный `/api/version`; ручной выбор `pipeline.enterprise` пережил Deal→Back/reload, поиск/reload и reset; отказ для duplicate/conflicting/malformed URLs, unknown = unavailable. Полный отдельный QA user flow: 3 стадии, Deal 250.50 USD, advance, occupied reorder (StageId неизменен, Deal version 2), archive-open **ожидаемый 409**, won→reopen→won, archive after close. D1 QA Pipeline v3 archived=1, QA Deal v5 won, пять immutable events. Independent D1 before/after audit `188→200`, Pipeline rows `2→3`, Deal rows `14→15`, Pipeline receipts `5→8`, Deal receipts `56→61`; 12 audit actions разбиты точно на 3 каталога + 5 Deal + 4 original Sales–Activities/Task. Read-only navigation не создаёт write. Official sanitized HAR 1.2 = 199 entries / 151305 bytes / SHA256 `c3baf97e74cc5d54985e15d18097b8ee648094478d301e881539506cb6bbc7de`, Browser Run закрыт. Ошибка консоли одна, от намеренного HTTP 409, не необъяснённая регрессия.

**M2-02/M2-04 позднее ACCEPTED, следующая очередь N-01 по доказанному пользовательскому gap**, M2-03 ранее ACCEPTED, M2-05 C. Не возобновлять M2-01 QA без регрессии; независимый positive human Cloudflare Access login остаётся pre-real-user check, а глобальная behavioral parity 93 crates **не** принята. #352 остаётся открытой для всей очереди.

## M2-01 accepted scope (22.11.2026): пользовательские названия стадий в Deal

- **B→ACCEPTED только конкретный UX-срез**: общий `DealDetails/DealHistory` показывает текущие имена Pipeline и Stage вместо внутренних кодов, как исходная Kanban; immutable `DealEvent` продолжает хранить original IDs, отсутствующий stage отображается по ID, поясняется происхождение текущего имени. Вынесен каталог из уже загруженного `ListPipelines`, **без дополнительного RPC/таблиц/генератора**.
- **Gate A=PASS** [#44950874461](https://github.com/iamaman11/crm/actions/runs/44950874461), точный code source [`c7eddfb5`](https://github.com/iamaman11/crm/commit/c7eddfb577e7ed349ed9d8fad32227467c449ab3) (45 основного job + 14 deploy), **Gate B=PASS** через независимый LIVE Browser MCP на том же SHA: полная и встроенная Deal, `Согласование → Входящие`, reload/back, D1 original stage IDs/events v1/v2 без новых записей audit 182→182. [Фактическая приёмка с legacy source и HAR #7069532306](https://github.com/iamaman11/crm/issues/352#issuecomment-7069532306).
- **M2-01 впоследствии полностью ACCEPTED dev/MVP по #7071938677; M2-02/M2-04 позднее ACCEPTED #7072122525/#7072127526, M2-03 ACCEPTED, M2-05 C** до отдельной сверки существующих owners/артефактов; не записывать false MVP-completion. Следующая задача выбирается по реальному product gap, а не по названию старого crate. Human-only Cloudflare Access positive login остаётся pre-real-user check; viewer business-field parity M1 уже ACCEPTED по #7070842108.

## M2 continuation decision (22.11.2026): не дублировать работающий Sales runtime

- [Пооперационный product-first review #7069547465](https://github.com/iamaman11/crm/issues/352#issuecomment-7069547465): **M2-02 (Deal primary Contact), M2-04 (exact metrics/forecast/conversion) — A-кандидаты; M2-03 впоследствии ACCEPTED #7071303504**, существующие typed protobuf/owners/D1/UI/native tests есть, но целый ID не объявлять ACCEPTED без проверки соответствующей исторической семантики и independent Gate B по полному user flow. Дублирующие API/калькулятор/генератор запрещены.
- **M2-05 = DECIDED C для MVP**: отдельную Lead сущность **не** создавать; квалификация запроса сейчас выражается через original Deal, Party/Account, pipeline stages `lead→qualified`, immutable events, linked Task. Исторические просмотренные `crm-sales-capability-adapter::plan_create/update/advance` относятся к Deal и не доказывают обязательный `Lead` CRUD. Если когда-либо появится конкретный user-flow и потребность в независимом Lead lifecycle — решение пересмотреть как B с исходной source проверкой. C не удаляет legacy code и не означает автоматический Gate B PASS для несуществующей операции.
- Следующим новым кодовым срезом выбирать **только доказанный B-пробел**, не продолжать ради заполнения чисел очереди. Остальная per-operation legacy parity остаётся OPEN; M1 viewer visibility принята, а human login отдельно ожидает pre-real-user release.

## M2 Deal name-search checkpoint (22.11.2026): один принятый product-first B-срез

- **M2-01 / M2-03 — B→ACCEPTED только «найти сделку по названию/ID»**, не полный M2: исторический `crm-sales-activities-query-adapter/src/executor.rs::execute_list_deals/collect_deals` имел tenant+pipeline/status/owner и cursor, **не поиск по названию**. Единственный typed `crm.sales.v2.ListDeals` теперь принимает `search` (generated Protobuf → existing Rust `wire_search`/owner parameterized D1 `instr(name,?) OR instr(id,?)` before limit/cursor → existing React SearchField). Account/Status применяются вместе с поиском; results across pipelines отдельны от Kanban/forecast; `#sales?search=` выдерживает Back/reload. Нет второго query runtime, RPC, Worker, D1 migration, генератора, индекса или SQLx.
- **Точный опубликованный код** [`d94248fd145ea15c04d8ba6db5f15dc02a5c3450`](https://github.com/iamaman11/crm/commit/d94248fd145ea15c04d8ba6db5f15dc02a5c3450), [штатный Gate A #44954180291](https://github.com/iamaman11/crm/actions/runs/44954180291) **PASS** (45+14 steps, signed native workerd, real Chromium, exact artifact deploy/live E2E+traces). **Отдельный Gate B PASS**: native LIVE Browser MCP on same SHA + read-only D1: 2 matching deals for `SM1B`, 1 exact; status intersects, Back/reload and empty/reset, total Kanban unaffected. D1 14 Deal / 56 receipts / 182 audit rows unchanged. [Полный исходный A/B/C, отказавшие проверки и Gate B evidence #7069992607](https://github.com/iamaman11/crm/issues/352#issuecomment-7069992607). **Admin service session ≠ human-only proof**.
- **Что дальше:** не перечёркивать принятый B-срез и не дублировать серверный поиск; M2-02/04 позднее ACCEPTED по #7072122525/#7072127526; M2-03 полностью принят позднейшим #7071303504, M2-05 уже C для MVP. M1 viewer business-field visibility принята позднейшим #7070842108; human login остаётся отдельной проверкой перед реальными пользователями; 93-crate parity остаётся OPEN.

## M2-03 FINAL dev/MVP ACCEPTED — сохранённые Sales views, фильтры и native URL (22.11.2026)

**M2-03 = ACCEPTED для текущей dev/MVP** по полной пооперационной [#352 / #7071303504](https://github.com/iamaman11/crm/issues/352#issuecomment-7071303504). Исходные Rust owner/typed Protobuf/Connect/D1 и прежние native tenant/CAS/replay/keyset/audit проверки переиспользованы (A); закрыт доказанный B-пробел пользовательского состояния выбранного представления: `#sales?viewId=...` сохраняет точную selection после reload, Deal→Back и поиска по названию, без потери Kanban и метрик. Создание, изменение, архивирование, восстановление фильтров account/status/pipeline, невидимость archived и отказ для повторяющегося/невалидного URL проверены в существующем React + signed native Chromium.

**Gate A PASS:** финальный исходный code SHA [`0f81fe754a35ccc1fd604d1ec93959c550a5dd0e`](https://github.com/iamaman11/crm/commit/0f81fe754a35ccc1fd604d1ec93959c550a5dd0e), [штатный CI #44963707652](https://github.com/iamaman11/crm/actions/runs/44963707652), оба job success и exact-artifact deploy без пересборки. Первые CI-ошибки (не замаскированы): [#44962750204](https://github.com/iamaman11/crm/actions/runs/44962750204) выявил неочищенный URL при normal-success archive, [#44963236685](https://github.com/iamaman11/crm/actions/runs/44963236685) — потерю Kanban при поиске. Исправлено в существующем коде, тесты сохранены.

**Independent LIVE Browser MCP Gate B PASS** на опубликованном том же SHA: create selected view, reload автоприменение полных фильтров, Deal→Back, search + reload, update + reload, archive, stale/duplicate/malformed links; сервисный admin, не human-only. Independent D1 readback: audit 185→188, saved-view receipts 6→9, records 2→3 — ровно три намеренных записи; созданный QA view v3/archived, 3 audit для него, чтение без дополнительных writes. HAR 181 entries / 149 RPC / 154 HTTP 200 / 0 HTTP 4xx-5xx, SHA256 `f770d1f3d02ff7129da9a478d41c1028633fe115126094084c135615e5f4617a`, Browser Run закрыт. Прежние M2 Deal name-search и stage-name B-slices сохраняются.

**Дальнейшая очередь:** M2-01 ACCEPTED позднейшим #7071938677, M2-02/M2-04 ACCEPTED позднейшими #7072122525/#7072127526; M2-05 C (отдельная Lead не нужна). Не переписывать готовые Sales owners/views или добавлять новый backend ради переноса. 93/93 исторических crates остаются источником для индивидуальной сверки, не доказанной глобальной parity; pre-real-user human Cloudflare Access login остаётся отдельно, #352 OPEN.

## M2 FINAL dev/MVP ACCEPTED (22.11.2026) — следующая очередь N-01

**M2-01, M2-02, M2-03, M2-04 = ACCEPTED для текущей dev/MVP; M2-05 = DECIDED C (отдельная Lead не нужна).** Gate A — штатный подписанный workerd/Chromium/Rust/TS и exact artifact deploy; Gate B — независимый LIVE Browser MCP с D1 SELECT-only на том же опубликованном SHA. Исходная бизнес-семантика сверена по конкретным legacy crates до приёмки, работающие owners не переписывались. **Глобальная эквивалентность 93 исторических crates НЕ доказана; human Cloudflare Access login остаётся отдельным pre-real-user gate; #352 OPEN.**

- **M2-01**: исходный Sales pipeline/Deal owner, stages, stable IDs, advance/reorder/archive/close/reopen, Kanban и native `pipelineId` после reload/Back/search. [#7071938677](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677), Gate A #44968322133 + independent B PASS, source SHA `f8f1fcea`.
- **M2-02**: original typed ContactPoint/Account/Sales owner, проверка active/tenant/Party, выбор предпочтительного контакта и clear/CAS/receipt. Закрыт реальный B-пробел: из Deal ссылка на точный `contactHref(contactPointId,partyId)`, а не на список. [#7072122525](https://github.com/iamaman11/crm/issues/352#issuecomment-7072122525), Gate A #44969797203 + independent B PASS, source SHA `54e37933`.
- **M2-03**: saved Sales views, status/account/pipeline, URL `viewId`, reload/Back/search, cursor/CAS/replay/audit. [#7071303504](https://github.com/iamaman11/crm/issues/352#issuecomment-7071303504), Gate A #44963707652 + independent B PASS, source SHA `0f81fe75`.
- **M2-04**: **A ACCEPTED без нового кода**. Один original `GetPipelineMetrics` Rust/D1 owner, точные `i128` и целочисленное weighted money, независимые валюты, no-date, manual 0%, UTC cohorts, closed-only conversion, нулевой знаменатель=null. Native scale/concurrency/strict-persistence и independent Browser UI↔D1 точно совпали: 12 Deals/9 open/3 won, 100%, EUR/PLN/USD и exclusions. [#7072127526](https://github.com/iamaman11/crm/issues/352#issuecomment-7072127526), Gate A #44969797203 + independent B PASS, код не менялся в M2-04.
- **M2-05 = C:** квалификация через Deal+Stage+immutable events; отдельный Lead CRUD не подтверждён и не нужен MVP. [#7069547465](https://github.com/iamaman11/crm/issues/352#issuecomment-7069547465).

**N-01 принят #7079664966; N-02 принят #7079751195 БЕЗ НОВОГО КОДА; N-03 принят #7079988713; N-04 принят #7080576593.** Остальные NEXT (N-05…N-06) — только кандидаты на продуктовую проверку: сначала выяснять, что пользователь реально не может сделать, читать current owner/UI/native и исторические операции, A/B/C до кода. Никакого механического выполнения OPEN по порядку или повторного CI без кодовой необходимости. Не дублировать CI/Protobuf generator, Postgres runtime или old Phase6 projection.

## N-01 FINAL dev/MVP ACCEPTED — bounded CSV import с построчными ошибками (22.11.2026)

**N-01 A+B ACCEPTED dev/MVP.** Исходная проблема: пользователь не знал номер строки/поле по первому глобальному `IMPORT_*_INVALID`. Historical `crm-customer-data-operations-capability-adapter/src/planner.rs` и `crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs` дают нужную семантику `RowDiagnostic(field,code)` + 1-based `row_position`; долговременные jobs/partial skip/Postgres не переносим. [Продуктовое решение до кода #7079229443](https://github.com/iamaman11/crm/issues/352#issuecomment-7079229443).

**A:** оригинальные `crm-customer-data-operations` parser/ImportRow/PartyImportMapping/PreparedPartyRow/RowDiagnostic, единственный `PartyCsvService::PreviewPartyImport/ApplyPartyImport`, исходный `crm-parties` D1 owner, React DataScreen, 16 КиБ/20 записей, sha256/receipts, signed role/tenant, atomic write and rollback. **B:** одно typed поле `PreviewPartyImportResponse.row_issues=6`; Rust собирает все semantic row/field/reason diagnostics, Preview без actionable target IDs и без записей, Apply заново валидирует и отклоняет весь пакет, UI показывает ошибки по-русски и скрывает подтверждение до исправления; structural errors остаются fail-closed. Никаких новых RPC, второй схемы, клиента-парсера, таблиц, job runner или CI. Проверки legacy не ослаблены.

**Gate A PASS:** source SHA [`9e59833864c03d5fd92252d1aac06a6dc181cc7c`](https://github.com/iamaman11/crm/commit/9e59833864c03d5fd92252d1aac06a6dc181cc7c), штатный [CI #45024832811](https://github.com/iamaman11/crm/actions/runs/45024832811) оба job SUCCESS, native `crm.native-n01-csv-row-issues=PASS` (multiple row/field errors, digest, Apply denied, zero partial writes), весь original workerd/Rust/Chromium, exact artifact deploy, published E2E/D1/traces. Отдельно устранён `RUSTSEC-2026-0592`: `url 2.5.9` + `form_urlencoded 1.2.3`, проверенные crates.io hashes, оба Cargo.lock, `CRM_ROOT_AND_WORKER_DEPENDENCY_SECURITY=PASS`; без whitelist/skip.

**Independent Gate B PASS**: Browser MCP `fc3aaaa9-b09c-4cdd-9eff-0e0c006ba1df`, deployed `/api/version.source` совпал с code SHA. Невалидный четырёхстрочный CSV: четыре конкретные ошибки (row1 kind+source version, row2 duplicate source_id, row3 empty name), нет Confirm; D1 audit `200→200`, Party `12→12`, import receipts `2→2`. Исправленный CSV: 4 Party (3 Person/1 Organization), D1 Party `12→16`, audit `200→204`, import receipts `2→3`, Party receipts `16→20`. Повторный Preview `previouslyImported=4` и повторный Apply без создания Party, audit `204`, Party receipts `20`, только новый operation receipt `3→4`. 9 HTTP 200, 0 HTTP/console errors; sanitized HAR 1.2 11 entries, SHA256 `14b012e98c5a654473a03bafb07d2966f3c7fd0eebdc83c761885d3d5ea91bf5`, Browser Run закрыт. [Финальная приёмка #7079664966](https://github.com/iamaman11/crm/issues/352#issuecomment-7079664966).

**N-02 подтверждён A #7079751195 без кода, N-03 подтверждён A+B #7079988713, N-04 подтверждён A+B #7080576593; NEXT N-05…N-06 являются кандидатами, а не обязательным списком новых задач.** Исторический `export_execution_worker.rs` реализует долговременные Postgres/immutable artifact jobs и является C для ограниченного MVP; не воскрешать его без реального пользовательского запроса. **N-01 ≠ L-01 durable import**, глобальная parity 93 crates и human Cloudflare Access positive login отдельно, #352 остаётся OPEN.

## N-03 FINAL dev/MVP ACCEPTED — Customer360 named stages, open Task status (22.11.2026)

**N-03 A+B ACCEPTED.** Existing `GetAccountOverview` remains a single read-only original owner projection of Account→Party/Contacts/Relationships/Deals/Tasks with signed tenant, exact links, truncation and no read-audit; native `crm.native-account-overview-v2=PASS`. Historical frozen `crm-customer-360-composition/src/lib.rs` and `crm-party-relationships-projection/src/lib.rs` implement event contribution privacy and rebuildable Postgres hierarchy, deliberately **not** recreated. [Product-first review before code #7079832380](https://github.com/iamaman11/crm/issues/352#issuecomment-7079832380).

**Real B user defect:** Customer360 linked Deal in actual Account `5b5d91d1-dee1-4ecf-9e5d-dfcc2cf82a06` showed raw stage UUID `b43ec1cd-e28f-4663-a811-70fb71008429` rather than owner stage «Квалификация», while Open Tasks with no due date showed only «—». Existing stage name/status already correct in authoritative Sales/Task owners. **B minimal:** move existing paginated `SalesScreen::allPipelines` typed generated `ListPipelines` into one shared `salesPipelineCatalog.ts`; use same owner catalog in Sales/Deal/Customer360, including archived pipelines, one read per Deal list, fallback to immutable ID for absent stage; show «Открыта» on open Tasks, due date if any. **No new Rust/Protobuf/RPC/D1/CI/projection.**

**Gate A PASS:** [code SHA `f1260dfa9227ae67d32a4cdafefa09ac77e9eba5`](https://github.com/iamaman11/crm/commit/f1260dfa9227ae67d32a4cdafefa09ac77e9eba5), [one CI #45027741190](https://github.com/iamaman11/crm/actions/runs/45027741190) both `cloudflare-crm-spike` and `deploy-crm-dev` SUCCESS; original Rust/domain/test, generated contract, signed workerd + real Chromium, published D1/traces, exact artifact reused. Signed native `crm.native-account-overview-v2` still PASS.

**Independent LIVE Gate B PASS**, Browser MCP `6dd8a033-1836-4e48-8a38-46fadb35ca08`, deployed `/api/version.source=f1260dfa...`, exact Account with 3 Deals/5 Tasks. QA archived Sales pipeline stage `b43ec1cd...` now visibly «Квалификация», independently matching SELECT-only original D1 stage name with archived=1. Open Task «Открыта», completed «Выполнена»; reload, exact Deal→Back and Task→Back preserved original links/Account route. D1 tenant audit `204→204`, no reads writes. 21 HTTP 200, zero 4xx/5xx and console errors (8 navigational aborted/status-0), official sanitized HAR 1.2 35 entries, SHA256 `a2c465749cf7e8d3df262cdb1e4bcd8ff42e624e4cc2da528722da331642cc8b`, workspace closed. [Final acceptance #7079988713](https://github.com/iamaman11/crm/issues/352#issuecomment-7079988713).

**N-04 ПРИНЯТ #7080576593 — существующие `EvaluatePartyName` и `UpdateParty` + точная навигация и повторная проверка после reload; findings jobs не создавались. NEXT N-05…N-06 — только проверяемые кандидаты, не команда переписать модули.** N-05 уже имеет read-only кандидатов/сравнение, Merge C для MVP без продуктового кейса. N-06 уже имеет original Consent Grant/Withdraw/Evaluate, не создавать send pipeline без реальной отправки. Никакой глобальной parity 93 crates и human Cloudflare Access positive login пока не заявлено; #352 OPEN.

## Упорядоченная матрица бизнес-функций

| ID | Очередь | Функция | Уже имеется | Историческая реализация (вскрыть до работы) |
|---|---|---|---|---|
| [M1-01](#m1-01) | M1 | Выбор по имени вместо ID | A ACCEPTED — именованные Party/Account pickers, фильтры и переходы без ID | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs) |
| [M1-02](#m1-02) | M1 | Люди и организации — CRUD | A ACCEPTED — Person/Organization create/update, reload, CAS/replay/tenant | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) |
| [M1-03](#m1-03) | M1 | Клиент (Account) из Party | A ACCEPTED — связка Party→Account и primary association | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs) |
| [M1-04](#m1-04) | M1 | Контактные точки / verified email | A+B ACCEPTED — Contact CRUD, kind/status, точная навигация | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter/src/planner.rs) |
| [M1-05](#m1-05) | M1 | Связи Party ↔ Party | A+B ACCEPTED — directed Party Relationship, status, reload, ограничения | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter/src/planner.rs) |
| [M1-06](#m1-06) | M1 | Навигация Account→Contacts/Deals/Tasks | A+B ACCEPTED — Customer360 linked original records, Contact/Relationship/Deal/Task | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs) |
| [M1-07](#m1-07) | M1 | Создание и изменение Deal | A+B ACCEPTED — Deal create/edit/stage/close/reopen/history | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs) |
| [M1-08](#m1-08) | M1 | Задача — создать, изменить, выполнить | A+B ACCEPTED — Task create/edit/complete/reminders/linked Deal | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs) |
| [M1-09](#m1-09) | M1 | Sales→Task внутренняя связь | A ACCEPTED — atomic Sales→Task и replay/rollback | [src/link_event_processor.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/link_event_processor.rs) |
| [M1-10](#m1-10) | M1 | Единая приемка Party→Account→Deal→Task | A ACCEPTED — единый Party→Account→Deal→Task→Customer360 live-путь | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) |
| [M1-11](#m1-11) | M1 | Архивирование и чистка тестовых записей | C DECIDED — generic Delete/archive не нужен MVP | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) |
| [M2-01](#m2-01) | M2 | Sales pipeline, стадии, Kanban | A+B ACCEPTED dev/MVP · #7071938677 | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/lib.rs) |
| [M2-02](#m2-02) | M2 | Основной контакт Deal | A+B ACCEPTED dev/MVP · #7072122525 | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs) |
| [M2-03](#m2-03) | M2 | Сохраненные views и фильтры Sales | A+B ACCEPTED dev/MVP · #7071303504 | [src/query_catalog.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/query_catalog.rs) |
| [M2-04](#m2-04) | M2 | Sales: прогноз, точная математика и конверсия | A ACCEPTED dev/MVP · Gate A+B · #7072127526 | [src/phase6_projections.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/phase6_projections.rs) |
| [M2-05](#m2-05) | M2 | Lead qualification как отдельная сущность? | DECIDED C MVP · #7069547465 | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs) |
| [N-01](#n-01) | NEXT | Bounded CSV import и ошибки строк | A+B ACCEPTED dev/MVP · #7079664966 | [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs) |
| [N-02](#n-02) | NEXT | Bounded CSV export | A ACCEPTED dev/MVP · no code · #7079751195 | [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs) |
| [N-03](#n-03) | NEXT | Customer360 — недостающие связи карточки | A+B ACCEPTED dev/MVP · #7079988713 | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs) |
| [N-04](#n-04) | NEXT | Data Quality проверка и ручное исправление | A+B ACCEPTED dev/MVP · #7080576593 | [src/remediation_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs) |
| [N-05](#n-05) | NEXT | Дубликаты: поиск и сравнение | PARTIAL/READONLY | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/planner.rs) |
| [N-06](#n-06) | NEXT | Consent grant/withdraw/evaluate | ACTIVE/GUARD | [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/planner.rs) |
| [L-01](#l-01) | LATER | Durable CSV import/checkpoints/rows | SOURCE ONLY | [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs) |
| [L-02](#l-02) | LATER | Durable export/artifact/recovery | SOURCE ONLY | [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs) |
| [L-03](#l-03) | LATER | Identity merge/unmerge and topology | SOURCE ONLY | [src/merge_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/merge_planner.rs) |
| [L-04](#l-04) | LATER | Data Quality findings and remediation | SOURCE ONLY | [src/finding_stewardship_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs) |
| [L-05](#l-05) | LATER | Enrichment provider dispatch/secret/quota | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-registry/src/lib.rs) |
| [L-06](#l-06) | LATER | Enrichment suggestion human review | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/src/lib.rs) |
| [L-07](#l-07) | LATER | Enrichment materialization to Party owner | SOURCE ONLY | [src/process.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/process.rs) |
| [L-08](#l-08) | LATER | Global search / index / reindex | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-global-search-composition/src/lib.rs) |
| [L-09](#l-09) | LATER | Relationship hierarchy projection | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs) |
| [E-01](#e-01) | END | Privacy subject verification and preview 2/9 | PARTIAL/PAUSED | [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs) |
| [E-02](#e-02) | END | Privacy remaining 7 owner scopes and freeze | SOURCE ONLY | [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs) |
| [E-03](#e-03) | END | Privacy planning/approval/legal hold/retention | SOURCE ONLY | [src/planning.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs) |
| [E-04](#e-04) | END | Privacy governed subject access export | SOURCE ONLY | [src/access_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/access_export.rs) |
| [E-05](#e-05) | END | Privacy owner erasure/execution/restriction | SOURCE ONLY | [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/execution.rs) |
| [E-06](#e-06) | END | Metadata / admin / module registry | SOURCE ONLY | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/lib.rs) |
| [E-07](#e-07) | END | Enterprise CPQ/service/AI/marketplace | NOT REQUIRED MVP | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-workflow/src/lib.rs) |
| [E-08](#e-08) | END | Production operations / DR / реальная приватность | OUTSIDE TEST MVP | [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/lib.rs) |

## Карточки реализации (правило, прежняя реализация, текущий код, конкретная приёмка)

### M1-01

**Бизнес-функция:** Выбор по имени вместо ID. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A ACCEPTED — именованные Party/Account pickers, фильтры и переходы без ID**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ContactScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ContactScreen.tsx); [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx); [src/TaskScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/TaskScreen.tsx); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [src/Pickers.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/Pickers.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Использовать уже существующие PartyPicker/AccountPicker и текущие List RPC. Не создавать Search runtime.

**DONE только при:** На 4 экранах фильтры по имени вместо ввода ID; выбор, пустое состояние, cursor, refresh; tenant denied.

**Evidence для исполнения:** `A/B/C/D: A for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-02

**Бизнес-функция:** Люди и организации — CRUD. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A ACCEPTED — Person/Organization create/update, reload, CAS/replay/tenant**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-parties/src/domain.rs); [src/PartyScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/PartyScreen.tsx); [connect_sales/party_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не переносить уже существующий CRUD. Сверить правила типов, версии, auth/replay со старым planner.

**DONE только при:** Создать/найти/изменить Person и Organization; reload, stale version, tenant/actor denied, audit.

**Evidence для исполнения:** `A/B/C/D: A for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-03

**Бизнес-функция:** Клиент (Account) из Party. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A ACCEPTED — связка Party→Account и primary association**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-accounts/src/domain.rs); [src/AccountScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountScreen.tsx); [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [connect_sales/account_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить существующий AccountCreator/Editor, owner и Party reference; не вводить master модель.

**DONE только при:** Party→Account, роли и статус, изменение и карточка после reload; CAS, missing/cross-tenant ref denied.

**Evidence для исполнения:** `A/B/C/D: A for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-04

**Бизнес-функция:** Контактные точки / verified email. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A+B ACCEPTED — Contact CRUD, kind/status, точная навигация**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108). Настоящий human Cloudflare Access login отдельно проверить перед запуском для реальных пользователей; это не незавершённый M1 dev/MVP.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-contact-points/src/domain.rs); [src/ContactScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ContactScreen.tsx); [connect_sales/contact_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить нынешние Create/Update/Get/List/VerifyOwnEmail, сделать поиск Party по имени; не подменять signed human service principal.

**DONE только при:** Email/phone сохраняются, status/validity/verification правильны; чужой owner/tenant, stale, fake identity rejected.

**Evidence для исполнения:** `A/B/C/D: A+B for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-05

**Бизнес-функция:** Связи Party ↔ Party. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A+B ACCEPTED — directed Party Relationship, status, reload, ограничения**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-party-relationships/src/domain.rs); [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx); [connect_sales/relationship_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Показывать понятные имена вместо 2 ID; старые planner-инварианты ролей/направления сохранить.

**DONE только при:** Создать/редактировать связь по выбранным именам; роли, порядок, временные границы и tenant верны после reload.

**Evidence для исполнения:** `A/B/C/D: A+B for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-06

**Бизнес-функция:** Навигация Account→Contacts/Deals/Tasks. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A+B ACCEPTED — Customer360 linked original records, Contact/Relationship/Deal/Task**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [src/routes.ts](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/routes.ts); [src/navigation.ts](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/navigation.ts); [connect_sales/overview_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/overview_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Использовать GetAccountOverview и owner reads; не переносить projection runtime.

**DONE только при:** Переходы и обратные ссылки по названиям, Back/refresh, правильный Account/tenant.

**Evidence для исполнения:** `A/B/C/D: A+B for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-07

**Бизнес-функция:** Создание и изменение Deal. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A+B ACCEPTED — Deal create/edit/stage/close/reopen/history**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [src/DealRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DealRecord.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Переиспользовать existing Deal/Account/Contact pickers, exact money и оригинальный Sales owner.

**DONE только при:** Account→Deal, primaryContact, stage, amount; save/reload, currency precision, CAS/replay/audit.

**Evidence для исполнения:** `A/B/C/D: A+B for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-08

**Бизнес-функция:** Задача — создать, изменить, выполнить. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A+B ACCEPTED — Task create/edit/complete/reminders/linked Deal**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-activities/src/domain.rs); [src/TaskScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/TaskScreen.tsx); [connect_sales/task_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не переписывать Create/Update/CompleteTask; поправить Account filter, статусы, сроки и понятные ссылки.

**DONE только при:** Из Account создать Task, изменить, выполнить; readback, stale CAS, tenant и idempotency.

**Evidence для исполнения:** `A/B/C/D: A+B for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-09

**Бизнес-функция:** Sales→Task внутренняя связь. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A ACCEPTED — atomic Sales→Task и replay/rollback**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/link_event_processor.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/link_event_processor.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales-activities-link/src/lib.rs); [src/DealRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DealRecord.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить реальный internal typed v1 consumer и атомарную D1 границу; не заменять новым orchestration.

**DONE только при:** Предусмотренная linked Task создаётся не более одного раза; failed write не даёт partial side effect; receipt/audit PASS.

**Evidence для исполнения:** `A/B/C/D: A for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-10

**Бизнес-функция:** Единая приемка Party→Account→Deal→Task. **Очередь:** M1. **Что уже есть:** проверенный связанный dev/MVP flow. **Статус пользовательской приёмки:** **A ACCEPTED — единый Party→Account→Deal→Task→Customer360 live-путь**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [test/typed-browser.mjs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/test/typed-browser.mjs); [e2e/live.mjs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/e2e/live.mjs); [src/app.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/app.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не новый тестовый фреймворк: расширить штатный native workerd/Chromium и live/browser сценарий.

**DONE только при:** Один связанный пользовательский поток и negative tenant/CAS/replay; единственный CI+publish exact SHA, отдельный Gate B.

**Evidence для исполнения:** `A/B/C/D: A for dev/MVP · code SHA: af84e93766f0238a7536669efa6135a2acc28904 · Gate A: #44960331611 PASS · independent Gate B: PASS, same SHA · 93-crate parity: OPEN`; source/negative tests, ранее принятые B-части и полный linked readback: [#7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

### M1-11

**Бизнес-функция:** Архивирование и чистка тестовых записей. **Очередь:** M1. **Что уже есть:** осознанно исключён из MVP. **Статус пользовательской приёмки:** **C DECIDED — generic Delete/archive не нужен MVP**. [Финальная приёмка M1 #7070842108](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [src/ContactScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ContactScreen.tsx); [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сначала использовать уже существующий inactive/archive. Не считать Delete реализованным в старом CRUD; физическое удаление только при необходимости.

**DONE только при:** Документированное решение C или B; если B — owner-mediated archive/disable без нарушения ссылок/аудита.

**Evidence для исполнения:** `A/B/C/D: C (generic deletion not an MVP user operation) · legacy: concrete planners reviewed · code: no new implementation · Gate A/B: not applicable to C decision`; [#7069323040](https://github.com/iamaman11/crm/issues/352#issuecomment-7069323040), [final M1](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108). Решение C не разрешает удалять старые исходники.

### M2-01

**Бизнес-функция:** Sales pipeline, стадии, Kanban. **Очередь:** M2. **Что уже есть:** original typed owner/catalog/React Kanban, сохранённые immutable Stage IDs и история, нативный фильтр, пользовательский deep-link. **Статус пользовательской приёмки: M2-01 A+B ACCEPTED dev/MVP** включая этапы/CRUD/reorder/advance/close/reopen/archive, native forward-only/tenant/CAS/rollback tests, ранее принятые имена `c7eddfb5` и поиск `d94248fd`, новый ручной Kanban URL `f8f1fcea`; полная [#352 / #7071938677](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677).

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/lib.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/pipeline.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/pipeline.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Существующие Create/Update/ListPipeline, ListStageDeals и AdvanceStage. DnD должен вызывать единственный путь.

**DONE только при:** Создание/изменение этапов, stage move, archive, reopen, no invalid stage, stale CAS denied.

**Evidence для исполнения:** `M2-01 dev/MVP A+B ACCEPTED · historical planner/StageTransitionPolicy/replay reviewed · original Rust owner: crm.sales · exact code SHA f8f1fcea59588f84762044c46116fb5c39590276 · standard Gate A #44968322133 both jobs PASS · independent Live Browser MCP Gate B PASS same published SHA · D1 audit/receipts/stage IDs/history verified · global 93-crate parity OPEN`. [Финальная пооперационная #7071938677](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677).

### M2-02

**Бизнес-функция:** Основной контакт Deal. **Очередь:** M2. **Что уже есть:** original typed ContactPoint/Account/Sales owner, active/Party/tenant guard, one picker, CAS/replay/audit и адресуемая карточка. **Статус пользовательской приёмки: M2-02 A+B ACCEPTED dev/MVP** после точной Deal→ContactPoint ссылки, reload/Back, D1 no-read-audit; [#7072122525](https://github.com/iamaman11/crm/issues/352#issuecomment-7072122525).

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/DealRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DealRecord.tsx); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** DealContactPicker уже есть; проверка принадлежности активного контакта Account должна жить в Sales owner.

**DONE только при:** Чужой или inactive контакт не сохраняется; выбранный контакт и Account корректны после reload.

**Evidence для исполнения:** `M2-02 A+B ACCEPTED dev/MVP · historical original Deal ResourceRef and ContactPoint Party query reviewed · canonical Rust validates same tenant active contact Account association · native linked-contact-date PASS with missing/foreign denial/CAS/clear/rollback · code SHA 54e379331b968b7a213438405307d57a965fc79f · Gate A CI #44969797203 both jobs PASS · independent recorded LIVE Browser MCP Gate B PASS exact published SHA · D1 audit 200→200, receipts unchanged, no HTTP/console error · global 93-crate parity OPEN`. [Приёмка #7072122525](https://github.com/iamaman11/crm/issues/352#issuecomment-7072122525).

### M2-03

**Бизнес-функция:** Сохранённые views и фильтры Sales. **Очередь:** M2. **Что уже есть:** исходный owner/view queries, типизированные фильтры, native pagination/tenant/CAS/replay/audit и linked React UX. **Статус пользовательской приёмки:** **M2-03 A+B ACCEPTED dev/MVP**, включая ранее принятый Deal-name search `d94248fd`, сохранение выбранного view при URL/reload/Deal→Back/поиске, полные фильтры create/update/archive, negative links; [#7071303504](https://github.com/iamaman11/crm/issues/352#issuecomment-7071303504). Human Access login отдельно перед реальными пользователями.

**Legacy — прочитать перед разработкой:** [src/query_catalog.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/query_catalog.rs); [src/definitions.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/definitions.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/views.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/views.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Повторно использовать Create/Update/Get/ListSalesView, ListDeals, ListStageDeals, не generic filter engine.

**DONE только при:** Views/filters сохранены после reload; cursor, status, pipeline, account и tenant совпадают.

**Evidence для исполнения:** `M2-03 dev/MVP: A+B ACCEPTED · original source/query/cursor/tenant reviewed · current owner: crm.sales · code SHA: 0f81fe754a35ccc1fd604d1ec93959c550a5dd0e · Gate A #44963707652 PASS both jobs · independent live Browser MCP Gate B PASS exact same published SHA · D1 audit/receipt verified · global 93-crate parity OPEN`. [Полная пооперационная приёмка / HAR / негативные проверки #7071303504](https://github.com/iamaman11/crm/issues/352#issuecomment-7071303504).

### M2-04

**Бизнес-функция:** Sales: прогноз, точная математика и конверсия. **Очередь:** M2. **Что уже есть:** original typed `GetPipelineMetrics` Rust/D1, i128, weighted/currency/status/UTC cohorts и native scale/concurrency proofs. **Статус пользовательской приёмки: M2-04 A ACCEPTED dev/MVP, без нового кода** — независимый UI↔D1 совпал по итогам, датам, manual 0%, no-date и конверсии; [#7072127526](https://github.com/iamaman11/crm/issues/352#issuecomment-7072127526).

**Legacy — прочитать перед разработкой:** [src/phase6_projections.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/phase6_projections.rs); [src/executor.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/executor.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx); [connect_sales/sales_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** GetPipelineMetrics уже есть: проверять expected close/no date, cohort denominator, currency и probability.

**DONE только при:** Суммы и win rate совпадают с D1 owner, 0/100% и no-date не теряются; React не присваивает бизнес-решения.

**Evidence для исполнения:** `M2-04 A ACCEPTED dev/MVP, NO new code · frozen Phase6 Postgres timeline/status projection is not financial forecast and remains C · original Rust/D1 GetPipelineMetrics checked i128, UTC close-only conversion, currency/forecast/undated buckets · native-r73-forecast, metrics-scale, concurrent-metrics, strict-persistence, Chromium PASS · source SHA 54e379331b968b7a213438405307d57a965fc79f · Gate A CI #44969797203 both jobs PASS · independent LIVE Browser MCP Gate B PASS exact published SHA · D1 matched UI 12/9/3, 100%, multiple currency sums/weights, no read audit · global 93-crate parity OPEN`. [Приёмка #7072127526](https://github.com/iamaman11/crm/issues/352#issuecomment-7072127526).

### M2-05

**Бизнес-функция:** Lead qualification как отдельная сущность?. **Очередь:** M2. **Что уже есть:** NOT PROVEN/DECIDE. **Статус пользовательской приёмки:** **DECIDED C для MVP: отдельная Lead entity не нужна**, использовать исходную Deal и stage history; не путать с реальным реализованным Lead CRUD (см. M2 continuation decision #7069547465).

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-sales/src/domain.rs); [src/SalesScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SalesScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Историческая отдельная Lead сущность здесь не доказана. Проверить, достаточна ли начальная стадия Deal; не строить новую сущность заранее.

**DONE только при:** Явно C (stage-based достаточно) либо B с конкретным контрактом/сценарием и proof; не принимать по названию roadmap.

**Evidence для исполнения:** `M2-05 DECIDED C for dev/MVP · historical Deal create/update/advance source reviewed; no independent Lead CRUD business requirement proven · Stage-based qualification sufficient · no duplicate Lead proto/entity/owner/migration`. [Product decision #7069547465](https://github.com/iamaman11/crm/issues/352#issuecomment-7069547465).

### N-01

**Бизнес-функция:** Bounded CSV import и ошибки строк. **Очередь:** NEXT. **Что уже есть:** original typed Rust parser/ImportRow/RowDiagnostic + Party/D1 atomic owner, единственные Protobuf/Connect Preview/Apply и React, per-row errors. **Статус пользовательской приёмки: N-01 A+B ACCEPTED dev/MVP**, [#7079664966](https://github.com/iamaman11/crm/issues/352#issuecomment-7079664966).

**Legacy — прочитать перед разработкой:** [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs); [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx); [connect_sales/data_operations_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Уже работает preview/apply для ≤16KiB и ≤20 строк; улучшать только реальные UX ошибки, не вводить job framework.

**DONE только при:** Невалидные строки имеют причину, нет частичной записи, digest и idempotency корректны.

**Evidence для исполнения:** `N-01 A+B ACCEPTED dev/MVP · historic RowDiagnostic/row_position vs existing original Rust/Party owner verified · code SHA 9e59833864c03d5fd92252d1aac06a6dc181cc7c · Gate A #45024832811 both jobs PASS; native-n01-csv-row-issues PASS · Gate B independent Browser MCP exact deployed SHA PASS · D1 preview no writes, 4 Party create/4 audit, replay no duplicate Party writes · cargo-deny RUSTSEC-2026-0592 patched url 2.5.9 · global 93-crate parity OPEN`. [Финальная приёмка #7079664966](https://github.com/iamaman11/crm/issues/352#issuecomment-7079664966).

### N-02

**Бизнес-функция:** Bounded CSV export. **Очередь:** NEXT. **Что уже есть:** original `crm-customer-data-operations` PartyExportScope/Profile/Specification, canonical CSV, один Rust/D1 owner, typed PreviewPartyExport, защищённый Download и существующий DataScreen. **Статус пользовательской приёмки: N-02 A ACCEPTED dev/MVP без изменений кода**, [#7079751195](https://github.com/iamaman11/crm/issues/352#issuecomment-7079751195).

**Legacy — прочитать перед разработкой:** [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs); [src/export_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx); [connect_sales/data_operations_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Preview и download уже существуют. Не путать с privacy-экспортом, не добавлять durable artifact без нужды.

**DONE только при:** Фильтр Person/Organization, корректный CSV, tenant authorization, version semantics.

**Evidence для исполнения:** `N-02 A ACCEPTED dev/MVP, zero new code · historical long-lived Postgres ExportJob/immutable file worker excluded C · original bounded Party ExportScope/Profile/Specification and canonical CSV · existing code SHA 9e59833864c03d5fd92252d1aac06a6dc181cc7c · Gate A existing CI #45024832811 both jobs PASS, native-party-csv-v2 PASS; signed admin, anonymous 401/viewer 403, tenant isolation, no-store, max 100 tenant Party (101st denies), sha256 exact, version field · independent Browser MCP Gate B same source SHA, UI Preview all/person/organization 16/8/8 matches D1, real CSV GET 200, header party_id/kind/display_name/resource_version, byte SHA matches response header, audit 204→204 and receipts unchanged, zero console/HTTP errors · no new CI or database changes`. [Актуальная продуктовая приёмка #7079751195](https://github.com/iamaman11/crm/issues/352#issuecomment-7079751195).

### N-03

**Бизнес-функция:** Customer360 — исходный linked Account read, понятные этапы/статусы и прямые ссылки. **Очередь:** NEXT. **Что уже есть:** original typed read-only `GetAccountOverview`, все глубокие links, 100-item bounds, original Sales/Task owners, единый typed ListPipelines со stage names. **Статус пользовательской приёмки: N-03 A+B ACCEPTED dev/MVP**, [#7079988713](https://github.com/iamaman11/crm/issues/352#issuecomment-7079988713).

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/AccountRecord.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/AccountRecord.tsx); [connect_sales/overview_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/overview_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** GetAccountOverview уже есть; добавлять только недостающие read-данные. Projection — только при доказанной необходимости.

**DONE только при:** Текущий linked summary после reload и permission-aware reads, нет чужих персональных полей.

**Evidence для исполнения:** `N-03 A+B ACCEPTED dev/MVP · historical frozen Customer360 contribution + Relationship hierarchy read, no Postgres projection copying · original read-only typed AccountOverview A PASS signed native account-overview-v2 tenant/100-item/zero audit · B real QA Account raw stage UUID and open Task blank, fixed via shared existing typed ListPipelines includeArchived + Task status · code SHA f1260dfa9227ae67d32a4cdafefa09ac77e9eba5 · Gate A CI #45027741190 both jobs SUCCESS · independent LIVE Browser MCP Gate B same SHA exact archived stage name, Task open/completed, reload/Back, D1 audit 204→204, no HTTP/console errors · global 93-crate parity OPEN`. [Full acceptance #7079988713](https://github.com/iamaman11/crm/issues/352#issuecomment-7079988713).

### N-04

**Бизнес-функция:** Data Quality проверка и ручное исправление. **Очередь:** NEXT. **Статус пользовательской приёмки:** **A+B ACCEPTED dev/MVP #7080576593** — полный ограниченный пользовательский путь, а не принятие старых findings/jobs.

**Legacy — прочитать перед разработкой:** [src/remediation_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs); [src/finding_stewardship_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ReviewScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ReviewScreen.tsx); [connect_sales/quality_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/quality_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** EvaluatePartyName уже работает; сперва исправить Party через его owner, не делать findings/job engine автоматически.

**DONE только при:** Плохие данные обнаружены и исправлены существующей записью; повторная проверка и права корректны.

**N-04 acceptance evidence:** frozen `crm-data-quality-capability-adapter/src/remediation_planner.rs` и `finding_stewardship_planner.rs` оценены: materialized findings/assign/waive/remediation attempt = C для ограниченного MVP, сохранить исходники для L-04. A = original typed `EvaluatePartyName` + `GetParty/UpdateParty`/CAS/tenant/audit; B = точный переход `#review?partyId` → `#parties?partyId` → исправление → повторная проверка и reload. Первоначальный live FAIL #45030551466 был исправлен в существующем PartyPicker (`disabled` при preselected value) без ослабления тестов. **Code SHA [`583cc43c`](https://github.com/iamaman11/crm/commit/583cc43c94de2b81a8993a04915b7068f5244a37), Gate A PASS [#45031786040](https://github.com/iamaman11/crm/actions/runs/45031786040) оба jobs, independent LIVE Gate B PASS**: wrong `unknown` → failed 1 → exact edit → corrected name → failed 0 + reload, D1 Party v1→v2, audit 1→2 only for writes, zero read audit; Browser MCP official HAR 40 entries, zero HTTP errors; [final #352 #7080576593](https://github.com/iamaman11/crm/issues/352#issuecomment-7080576593). Нет новых RPC/Protobuf/owner/D1/CI. Остальная поведенческая parity 93 crates OPEN.

### N-05

**Бизнес-функция:** Дубликаты: поиск и сравнение. **Очередь:** NEXT. **Что уже есть:** PARTIAL/READONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/planner.rs); [src/owner_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/owner_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ReviewScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ReviewScreen.tsx); [connect_sales/identity_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** FindSharedEmailCandidates/CompareParties доступны; не внедрять Merge на MVP.

**DONE только при:** Совпадение email — лишь кандидат, не identity proof; read tenant/PII boundaries.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### N-06

**Бизнес-функция:** Consent grant/withdraw/evaluate. **Очередь:** NEXT. **Что уже есть:** ACTIVE/GUARD. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/planner.rs); [src/owner_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/owner_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/ConsentScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ConsentScreen.tsx); [connect_sales/consent_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить действующий consent guard; интеграцию с отправкой строить только при реальной отправке.

**DONE только при:** Отзыв разрешения блокирует применимый внешний эффект; signed actor, replay/tenant proofs.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-01

**Бизнес-функция:** Durable CSV import/checkpoints/rows. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/import_execution_coordinator.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-source-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-data-operations/src/execution.rs); [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Нужен только при объёмах за пределом bounded CSV; переносить инварианты, не PostgreSQL coordinator.

**DONE только при:** Resume после fault, row outcome/duplicates/replay, версионный progress в D1.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-02

**Бизнес-функция:** Durable export/artifact/recovery. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/export_execution_worker.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs); [src/privacy_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-data-operations/src/export.rs); [src/DataScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/DataScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только при реальном длительном или регулируемом экспорте; не путать с CSV.

**DONE только при:** Stable artifact digest, version binding, expiry, authorized download, replay proof.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-03

**Бизнес-функция:** Identity merge/unmerge and topology. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/merge_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/merge_planner.rs); [src/current_canonical.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition/src/current_canonical.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-merge-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/merge_lineage.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-identity-resolution/src/merge_lineage.rs); [connect_sales/identity_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сейчас read-only; сохранить reversible lineage и канонические ссылки, не переносить Postgres transaction.

**DONE только при:** Никаких потерянных links; stale merge denied, unmerge reversible, audit/replay.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-04

**Бизнес-функция:** Data Quality findings and remediation. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/finding_stewardship_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs); [src/remediation_planner.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-data-quality/src/domain.rs); [src/ReviewScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/ReviewScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только при конкретном процессе stewardship. Не generic job/rule framework.

**DONE только при:** Persist/assign/remediate/re-evaluate, stale evidence denied, owner-only mutation.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-05

**Бизнес-функция:** Enrichment provider dispatch/secret/quota. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-registry/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-worker-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-registry-http-transport/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/EnrichmentScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/EnrichmentScreen.tsx); [connect_sales/enrichment_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Активен лишь publish/get/list configuration; нужен выбранный реальный provider.

**DONE только при:** Endpoint/secret/consent/quota/timeout/circuit, sanitized result, D1 provenance.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-06

**Бизнес-функция:** Enrichment suggestion human review. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/lifecycle.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-enrichment/src/lifecycle.rs); [src/EnrichmentScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/EnrichmentScreen.tsx). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только после L-05; не путать сохранённый mapping с результатом обогащения.

**DONE только при:** Approve/reject с версионированной policy, expiry и stale guard, no unwanted owner write.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-07

**Бизнес-функция:** Enrichment materialization to Party owner. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/process.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/process.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/materialization.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-enrichment/src/materialization.rs); [connect_sales/enrichment_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** После L-05/L-06; использовать existing Party mutation, не generic materialization engine.

**DONE только при:** Один approved suggestion→один owner UpdateParty; stale/CAS/replay/provenance.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-08

**Бизнес-функция:** Global search / index / reindex. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-global-search-composition/src/lib.rs); [src/generation.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-runtime/src/generation.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-query-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/SearchField.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/SearchField.tsx); [connect_sales/party_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только когда current owner search недостаточен по измеримому сценарию.

**DONE только при:** Read-time ACL and hidden-field non-disclosure, tenant, rebuild/permission revoke.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### L-09

**Бизнес-функция:** Relationship hierarchy projection. **Очередь:** LATER. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/RelationshipScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/RelationshipScreen.tsx); [connect_sales/relationship_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Прямые owner reads сначала; projection only with clear user scenario.

**DONE только при:** Directed/reverse/effective edges, tenant and rebuild state correct.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-01

**Бизнес-функция:** Privacy subject verification and preview 2/9. **Очередь:** END. **Что уже есть:** PARTIAL/PAUSED. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/PrivacyScreen.tsx](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/web/src/PrivacyScreen.tsx); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Сохранить действующие VerifyOwnPrivacyCaseSubject + PreviewOwnPrivacyScope; не расширять сейчас.

**DONE только при:** Signed real human/tenant proof, service denied, 2/9 complete=false; export/erasure blocked.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-02

**Бизнес-функция:** Privacy remaining 7 owner scopes and freeze. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/discovery.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs); [src/planning.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/domain.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Когда будет требование реальных данных: all 9 real owner scans, no fake empty scopes.

**DONE только при:** 9/9 bounded scans, immutable snapshot, completeness & version digest, fail closed.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-03

**Бизнес-функция:** Privacy planning/approval/legal hold/retention. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/planning.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs); [src/retention.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/retention.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/domain.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/domain.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Только после E-02 и требований реальных данных.

**DONE только при:** Trusted case+scope, approved plan, correct hold/retention, D1 audited decisions.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-04

**Бизнес-функция:** Privacy governed subject access export. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/access_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/access_export.rs); [src/privacy_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/access_export.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/access_export.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не подменять bounded CSV; без E-02/E-03 экспорта нет.

**DONE только при:** Signed human+complete scope, immutable versioned artifact, allowed recipient and audit.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-05

**Бизнес-функция:** Privacy owner erasure/execution/restriction. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/execution.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [src/execution.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/modules/crm-customer-privacy/src/execution.rs); [connect_sales/privacy_handlers.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не делать partial erasure; нужен весь workflow и legal conditions.

**DONE только при:** Approved complete scope, legal hold checks, idempotent owner checkpoint, audit.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-06

**Бизнес-функция:** Metadata / admin / module registry. **Очередь:** END. **Что уже есть:** SOURCE ONLY. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-registry/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-composition/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [cloudflare-crm/ARCHITECTURE.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/ARCHITECTURE.md); [cloudflare-crm/AGENTS.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/AGENTS.md). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не подключать старый runtime без конкретного бизнес-требования.

**DONE только при:** Без второго источника схемы/registry/worker; explicit product decision.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-07

**Бизнес-функция:** Enterprise CPQ/service/AI/marketplace. **Очередь:** END. **Что уже есть:** NOT REQUIRED MVP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-workflow/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-events/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-integrations/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [cloudflare-crm/ARCHITECTURE.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/ARCHITECTURE.md). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Наличие планов/контрактов не доказывает старую работающую бизнес-реализацию.

**DONE только при:** Каждая новая область с отдельным владельцем и ценностью, не по инерции roadmap.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

### E-08

**Бизнес-функция:** Production operations / DR / реальная приватность. **Очередь:** END. **Что уже есть:** OUTSIDE TEST MVP. **Статус пользовательской приёмки:** OPEN; наличие работающего RPC не означает acceptance целого flow.

**Legacy — прочитать перед разработкой:** [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/lib.rs); [src/lib.rs](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/lib.rs). [Тесты, методы, инварианты исторических crates](legacy-crate-operation-evidence.md) — использовать для развёрнутой ветки/negative-case проверки; не считать заголовок crate доказательством готового продукта.

**Действующая реализация — переиспользовать:** [cloudflare-crm/AGENTS.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/AGENTS.md); [cloudflare-crm/README.md](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/apps/cloudflare-crm/README.md). Проверить текущие `.proto`, `module.yaml`, compiled policy и D1; не дублировать авторитет типов или доменные правила.

**Точное решение / что НЕ делать:** Не возвращать PostgreSQL/SQLx process для production-гарантий.

**DONE только при:** Отдельные требования реальных пользователей, нагрузка/DR/право, без ложного production-ready.

**Evidence для исполнения:** `Legacy operation / source test: OPEN · A/B/C/D: OPEN · actual gap / product decision: OPEN · commit: — · CI: — · live/Browser Gate B: —`. Обновлять эти поля только после подтверждённых результатов в #352; для `LATER/END` это сознательно отложено.

## Полное покрытие legacy исходников — 93/93 crates → функциональные ID или обязательная проверка X-01

**Проверка реального репозитория:** 93/93 crates, 532/532 файлов `src/**/*.rs`+`tests/**/*.rs`, **1443 публичных** + **5190 внутренних/trait/helper** = **6633** найденных объявлений функций, 691 распознанных тестовых функций. Это **100% регистрация источников для последующего решения, но НЕ 100% смысловой паритет**. Полные имена и строки исходников по КАЖДОМУ файлу — в [operation evidence](legacy-crate-operation-evidence.md) на pinned SHA; здесь каждой функции наследуется обязанность проверки соответствующего crate и связанных ID. В текущей матрице из 39 задач напрямую упоминаются не все crates: этот crosswalk намеренно устраняет 47 неприкреплённых ссылок без дополнительного runtime/генератора.

**X-01 = историческая инфраструктура и невыявленные бизнес-инварианты, НЕ разрешение удалить.** Это постоянная проверка source-only после продуктивных срезов и **обязательная до физического удаления/объявления полной parity**: проверить каждое nested API, негативный тест, зависимости, вызовы, side effects. Каждая функциональная операция внутри «инфраструктуры» должна быть перенесена в ближайший соответствующий бизнес-ID либо получить **отдельный B-LATER/B-END или аргументированный C**, а не исчезнуть под общей меткой D. Сохранять source-only и тесты, не включать old Postgres/SQLx runtime в workspace/CI.

| № | Исторический crate (реальные source/tests) | Файлов | Public / internal / tests | Точка назначения / OPEN obligation |
|---:|---|---:|---:|---|
| 1 | [`crm-activities-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter) · NOW | 3 | 1 / 40 / 6 | [`M1-08`](#m1-08), [`M1-09`](#m1-09); **все операции OPEN** до соответствующего proof |
| 2 | [`crm-application-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-composition) · RETIRE-REVIEW | 1 | 35 / 53 / 8 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 3 | [`crm-application-runtime`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime) · RETIRE-REVIEW | 62 | 91 / 495 / 69 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 4 | [`crm-capability-adapters`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters) · RETIRE-REVIEW | 9 | 23 / 116 / 24 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 5 | [`crm-capability-ingress`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress) · RETIRE-REVIEW | 14 | 26 / 165 / 18 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 6 | [`crm-capability-plan-support`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-plan-support) · RETIRE-REVIEW | 1 | 24 / 6 / 2 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 7 | [`crm-capability-runtime`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime) · RETIRE-REVIEW | 6 | 5 / 34 / 6 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 8 | [`crm-consents-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter) · ACTIVE-GUARD | 3 | 7 / 31 / 7 | [`N-06`](#n-06); **все операции OPEN** до соответствующего proof |
| 9 | [`crm-consents-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-composition) · ACTIVE-GUARD | 1 | 8 / 26 / 6 | [`N-06`](#n-06); **все операции OPEN** до соответствующего proof |
| 10 | [`crm-consents-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter) · END | 11 | 31 / 39 / 9 | [`E-02`](#e-02), [`N-06`](#n-06); **все операции OPEN** до соответствующего proof |
| 11 | [`crm-consents-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-query-adapter) · ACTIVE-GUARD | 2 | 4 / 59 / 5 | [`N-06`](#n-06); **все операции OPEN** до соответствующего proof |
| 12 | [`crm-contact-points-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter) · NOW | 2 | 7 / 19 / 3 | [`M1-04`](#m1-04), [`M2-02`](#m2-02); **все операции OPEN** до соответствующего proof |
| 13 | [`crm-contact-points-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-composition) · NOW | 3 | 6 / 14 / 2 | [`M1-04`](#m1-04), [`M2-02`](#m2-02); **все операции OPEN** до соответствующего proof |
| 14 | [`crm-contact-points-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter) · END | 11 | 36 / 35 / 8 | [`E-02`](#e-02), [`M1-04`](#m1-04); **все операции OPEN** до соответствующего proof |
| 15 | [`crm-contact-points-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter) · NOW | 1 | 3 / 28 / 3 | [`M1-04`](#m1-04), [`M2-02`](#m2-02); **все операции OPEN** до соответствующего proof |
| 16 | [`crm-core-data`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data) · RETIRE-REVIEW | 46 | 92 / 386 / 38 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 17 | [`crm-core-events`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-events) · LATER | 1 | 5 / 5 / 0 | [`L-01`](#l-01), [`L-05`](#l-05), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 18 | [`crm-core-files`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-files) · LATER | 1 | 4 / 8 / 1 | [`L-02`](#l-02), [`E-04`](#e-04), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 19 | [`crm-core-integrations`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-integrations) · END | 1 | 0 / 0 / 0 | [`L-05`](#l-05), [`E-07`](#e-07), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 20 | [`crm-core-permissions`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-permissions) · END | 1 | 0 / 0 / 0 | [`N-06`](#n-06), [`E-03`](#e-03), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 21 | [`crm-core-workflow`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-workflow) · END | 1 | 0 / 0 / 0 | [`E-06`](#e-06), [`E-07`](#e-07), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 22 | [`crm-customer-360-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition) · NOW/NEXT | 1 | 9 / 32 / 7 | [`M1-06`](#m1-06), [`N-03`](#n-03); **все операции OPEN** до соответствующего proof |
| 23 | [`crm-customer-accounts-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter) · NOW | 2 | 8 / 18 / 3 | [`M1-03`](#m1-03), [`M1-06`](#m1-06); **все операции OPEN** до соответствующего proof |
| 24 | [`crm-customer-accounts-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter) · END | 11 | 34 / 40 / 10 | [`E-02`](#e-02), [`M1-03`](#m1-03); **все операции OPEN** до соответствующего proof |
| 25 | [`crm-customer-accounts-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter) · NOW | 3 | 5 / 36 / 3 | [`M1-03`](#m1-03), [`M1-06`](#m1-06); **все операции OPEN** до соответствующего proof |
| 26 | [`crm-customer-data-operations-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter) · NEXT/LATER | 10 | 39 / 105 / 13 | [`N-01`](#n-01), [`N-02`](#n-02), [`L-01`](#l-01), [`L-02`](#l-02); **все операции OPEN** до соответствующего proof |
| 27 | [`crm-customer-data-operations-execution-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition) · NEXT/LATER | 20 | 55 / 153 / 24 | [`N-01`](#n-01), [`N-02`](#n-02), [`L-01`](#l-01), [`L-02`](#l-02), [`E-04`](#e-04); **все операции OPEN** до соответствующего proof |
| 28 | [`crm-customer-data-operations-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter) · END | 11 | 43 / 63 / 10 | [`E-02`](#e-02), [`E-04`](#e-04); **все операции OPEN** до соответствующего proof |
| 29 | [`crm-customer-data-operations-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter) · NEXT/LATER | 4 | 12 / 83 / 8 | [`N-02`](#n-02), [`L-02`](#l-02); **все операции OPEN** до соответствующего proof |
| 30 | [`crm-customer-data-operations-source-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-source-composition) · NEXT/LATER | 2 | 3 / 40 / 3 | [`N-01`](#n-01), [`L-01`](#l-01); **все операции OPEN** до соответствующего proof |
| 31 | [`crm-customer-enrichment-application-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-adapter) · LATER | 1 | 12 / 28 / 2 | [`L-07`](#l-07), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 32 | [`crm-customer-enrichment-application-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition) · LATER | 8 | 12 / 105 / 10 | [`L-07`](#l-07), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 33 | [`crm-customer-enrichment-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter) · LATER | 15 | 32 / 146 / 26 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 34 | [`crm-customer-enrichment-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-composition) · LATER | 3 | 4 / 36 / 0 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 35 | [`crm-customer-enrichment-materialization-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-adapter) · LATER | 2 | 2 / 29 / 4 | [`L-07`](#l-07), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 36 | [`crm-customer-enrichment-materialization-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition) · LATER | 11 | 8 / 105 / 10 | [`L-07`](#l-07), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 37 | [`crm-customer-enrichment-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter) · END | 10 | 32 / 89 / 6 | [`E-02`](#e-02), [`L-05`](#l-05); **все операции OPEN** до соответствующего proof |
| 38 | [`crm-customer-enrichment-provider-process-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition) · LATER | 9 | 18 / 122 / 15 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 39 | [`crm-customer-enrichment-provider-registry`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-registry) · LATER | 1 | 21 / 65 / 12 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 40 | [`crm-customer-enrichment-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-query-adapter) · LATER | 1 | 6 / 27 / 2 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 41 | [`crm-customer-enrichment-registry-http-transport`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-registry-http-transport) · LATER | 2 | 3 / 34 / 8 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 42 | [`crm-customer-enrichment-request-list-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-request-list-query-adapter) · LATER | 1 | 2 / 30 / 2 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 43 | [`crm-customer-enrichment-review-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-adapter) · LATER | 2 | 15 / 33 / 7 | [`L-06`](#l-06), [`L-07`](#l-07); **все операции OPEN** до соответствующего proof |
| 44 | [`crm-customer-enrichment-review-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition) · LATER | 4 | 9 / 28 / 4 | [`L-06`](#l-06), [`L-07`](#l-07); **все операции OPEN** до соответствующего proof |
| 45 | [`crm-customer-enrichment-suggestion-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-suggestion-query-adapter) · LATER | 4 | 31 / 19 / 4 | [`L-06`](#l-06), [`L-07`](#l-07); **все операции OPEN** до соответствующего proof |
| 46 | [`crm-customer-enrichment-visibility`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-visibility) · LATER | 1 | 1 / 6 / 5 | [`L-06`](#l-06), [`L-07`](#l-07); **все операции OPEN** до соответствующего proof |
| 47 | [`crm-customer-enrichment-worker-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-worker-composition) · LATER | 3 | 4 / 51 / 6 | [`L-05`](#l-05), [`L-06`](#l-06); **все операции OPEN** до соответствующего proof |
| 48 | [`crm-customer-privacy-application`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application) · END | 8 | 32 / 156 / 26 | [`E-01`](#e-01), [`E-02`](#e-02), [`E-03`](#e-03), [`E-04`](#e-04), [`E-05`](#e-05); **все операции OPEN** до соответствующего proof |
| 49 | [`crm-customer-privacy-cancel-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-cancel-capability-adapter) · END | 2 | 4 / 36 / 8 | [`E-01`](#e-01), [`E-03`](#e-03); **все операции OPEN** до соответствующего proof |
| 50 | [`crm-customer-privacy-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter) · END | 8 | 21 / 113 / 19 | [`E-01`](#e-01), [`E-02`](#e-02), [`E-03`](#e-03), [`E-04`](#e-04), [`E-05`](#e-05); **все операции OPEN** до соответствующего proof |
| 51 | [`crm-customer-privacy-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition) · END | 10 | 12 / 121 / 11 | [`E-01`](#e-01), [`E-02`](#e-02), [`E-03`](#e-03), [`E-04`](#e-04), [`E-05`](#e-05); **все операции OPEN** до соответствующего proof |
| 52 | [`crm-customer-privacy-owner-scope-support`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support) · END | 7 | 9 / 35 / 9 | [`E-02`](#e-02); **все операции OPEN** до соответствующего proof |
| 53 | [`crm-customer-privacy-persistence-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-persistence-adapter) · END | 2 | 16 / 15 / 4 | [`E-01`](#e-01), [`E-03`](#e-03), [`E-05`](#e-05), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 54 | [`crm-customer-privacy-postgres`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres) · END | 11 | 13 / 207 / 11 | [`E-03`](#e-03), [`E-05`](#e-05), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 55 | [`crm-customer-privacy-production`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production) · END | 7 | 12 / 42 / 6 | [`E-05`](#e-05), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 56 | [`crm-customer-privacy-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-query-adapter) · END | 3 | 21 / 80 / 5 | [`E-01`](#e-01), [`E-02`](#e-02), [`E-04`](#e-04); **все операции OPEN** до соответствующего proof |
| 57 | [`crm-customer-privacy-subject-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-subject-capability-adapter) · END | 2 | 2 / 27 / 5 | [`E-01`](#e-01), [`E-03`](#e-03); **все операции OPEN** до соответствующего proof |
| 58 | [`crm-customer-privacy-submit-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-submit-capability-adapter) · END | 1 | 2 / 21 / 3 | [`E-01`](#e-01), [`E-03`](#e-03); **все операции OPEN** до соответствующего proof |
| 59 | [`crm-data-quality-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter) · NEXT/LATER | 11 | 55 / 76 / 8 | [`N-04`](#n-04), [`L-04`](#l-04); **все операции OPEN** до соответствующего proof |
| 60 | [`crm-data-quality-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter) · END | 10 | 31 / 71 / 6 | [`E-02`](#e-02), [`N-04`](#n-04); **все операции OPEN** до соответствующего proof |
| 61 | [`crm-data-quality-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter) · NEXT/LATER | 17 | 12 / 68 / 2 | [`N-04`](#n-04), [`L-04`](#l-04); **все операции OPEN** до соответствующего proof |
| 62 | [`crm-data-quality-source-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition) · NEXT/LATER | 12 | 19 / 55 / 1 | [`N-04`](#n-04), [`L-04`](#l-04); **все операции OPEN** до соответствующего proof |
| 63 | [`crm-first-party-modules`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-first-party-modules) · RETIRE-REVIEW | 1 | 1 / 0 / 0 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 64 | [`crm-global-search-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-global-search-composition) · LATER | 1 | 8 / 19 / 4 | [`L-08`](#l-08); **все операции OPEN** до соответствующего proof |
| 65 | [`crm-identity-resolution-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter) · LATER | 4 | 12 / 54 / 6 | [`N-05`](#n-05), [`L-03`](#l-03); **все операции OPEN** до соответствующего proof |
| 66 | [`crm-identity-resolution-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-composition) · LATER | 2 | 8 / 25 / 3 | [`N-05`](#n-05), [`L-03`](#l-03); **все операции OPEN** до соответствующего proof |
| 67 | [`crm-identity-resolution-merge-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-merge-composition) · LATER | 1 | 3 / 30 / 1 | [`N-05`](#n-05), [`L-03`](#l-03); **все операции OPEN** до соответствующего proof |
| 68 | [`crm-identity-resolution-merge-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-merge-query-adapter) · LATER | 1 | 3 / 36 / 1 | [`N-05`](#n-05), [`L-03`](#l-03); **все операции OPEN** до соответствующего proof |
| 69 | [`crm-identity-resolution-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter) · END | 11 | 40 / 60 / 9 | [`E-02`](#e-02), [`N-05`](#n-05); **все операции OPEN** до соответствующего proof |
| 70 | [`crm-identity-resolution-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-query-adapter) · LATER | 1 | 3 / 30 / 2 | [`N-05`](#n-05), [`L-03`](#l-03); **все операции OPEN** до соответствующего proof |
| 71 | [`crm-identity-resolution-topology-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition) · LATER | 4 | 3 / 22 / 3 | [`N-05`](#n-05), [`L-03`](#l-03); **все операции OPEN** до соответствующего proof |
| 72 | [`crm-metadata-api-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-api-adapter) · END | 1 | 15 / 12 / 5 | [`E-06`](#e-06), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 73 | [`crm-metadata-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-query-adapter) · END | 1 | 1 / 18 / 3 | [`E-06`](#e-06), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 74 | [`crm-metadata-runtime`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime) · END | 3 | 36 / 28 / 9 | [`E-06`](#e-06), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 75 | [`crm-metadata-schema`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-schema) · END | 1 | 6 / 40 / 6 | [`E-06`](#e-06), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 76 | [`crm-module-manifest`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-manifest) · RETIRE-REVIEW | 2 | 10 / 34 / 6 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 77 | [`crm-module-registry`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-registry) · RETIRE-REVIEW | 3 | 29 / 40 / 9 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 78 | [`crm-parties-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter) · NOW | 3 | 6 / 24 / 3 | [`M1-02`](#m1-02), [`M1-01`](#m1-01); **все операции OPEN** до соответствующего proof |
| 79 | [`crm-parties-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter) · END | 10 | 23 / 45 / 10 | [`E-02`](#e-02), [`M1-02`](#m1-02); **все операции OPEN** до соответствующего proof |
| 80 | [`crm-parties-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter) · NOW | 3 | 7 / 33 / 5 | [`M1-02`](#m1-02), [`M1-01`](#m1-01); **все операции OPEN** до соответствующего proof |
| 81 | [`crm-party-reference-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-reference-composition) · NOW | 1 | 7 / 9 / 0 | [`M1-03`](#m1-03), [`M1-05`](#m1-05), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 82 | [`crm-party-relationships-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter) · NOW | 2 | 7 / 20 / 3 | [`M1-05`](#m1-05), [`M1-06`](#m1-06); **все операции OPEN** до соответствующего proof |
| 83 | [`crm-party-relationships-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-composition) · NOW | 1 | 4 / 6 / 0 | [`M1-05`](#m1-05), [`M1-06`](#m1-06); **все операции OPEN** до соответствующего proof |
| 84 | [`crm-party-relationships-privacy-scope-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter) · END | 11 | 35 / 41 / 11 | [`E-02`](#e-02), [`M1-05`](#m1-05); **все операции OPEN** до соответствующего proof |
| 85 | [`crm-party-relationships-projection`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection) · NOW/NEXT | 1 | 8 / 18 / 3 | [`M1-06`](#m1-06), [`N-03`](#n-03), [`L-09`](#l-09); **все операции OPEN** до соответствующего proof |
| 86 | [`crm-party-relationships-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-query-adapter) · NOW | 1 | 3 / 34 / 4 | [`M1-05`](#m1-05), [`M1-06`](#m1-06); **все операции OPEN** до соответствующего proof |
| 87 | [`crm-projection-runtime`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-projection-runtime) · RETIRE-REVIEW | 1 | 15 / 26 / 4 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 88 | [`crm-query-runtime`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-query-runtime) · RETIRE-REVIEW | 4 | 16 / 43 / 8 | **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 89 | [`crm-sales-activities-capability-composition`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition) · NOW | 10 | 16 / 101 / 16 | [`M1-09`](#m1-09), [`M2-01`](#m2-01), [`M2-04`](#m2-04); **все операции OPEN** до соответствующего proof |
| 90 | [`crm-sales-activities-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter) · NOW | 4 | 12 / 41 / 1 | [`M1-07`](#m1-07), [`M1-09`](#m1-09), [`M2-03`](#m2-03), [`M2-04`](#m2-04); **все операции OPEN** до соответствующего proof |
| 91 | [`crm-sales-capability-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter) · NOW | 3 | 1 / 36 / 3 | [`M1-07`](#m1-07), [`M2-01`](#m2-01), [`M2-02`](#m2-02), [`M2-04`](#m2-04); **все операции OPEN** до соответствующего proof |
| 92 | [`crm-search-query-adapter`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-query-adapter) · LATER | 3 | 2 / 15 / 5 | [`L-08`](#l-08), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |
| 93 | [`crm-search-runtime`](https://github.com/iamaman11/crm/tree/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-runtime) · LATER | 3 | 14 / 45 / 5 | [`L-08`](#l-08), **X-01** (review, preserve); **все операции OPEN** до соответствующего proof |

**X-01 — критерий закрытия:** сверены исходные все 532 файла и каждое содержательное business API/side effect/test с действующим owner, пунктами очереди или явно отложенным B; для **каждой** операции есть подтверждённое A, обоснованное C, B с дальнейшим ID и условием, либо доказанное D с отсутствием потребителей. Проверены не только прямые Cargo dependencies, но re-exports и transitively calling paths. Доказательство, когда требуется, проходит **оба Gate A+B**. Пока нет доказательства, **сохранить** crate/source/tests и запретить массовое удаление. **X-01 не блокирует M1/M2/MVP**: она блокирует лишь заявления «ничего не осталось» и физическую очистку старой реализации.

## Пределы этапов

**M1:** ID M1-01…M1-10 приняты по UI+owner+D1+signed CI/browser; M1-11 явно решён (B или C), не бесконечный спор об удалении. **M2:** M2-01…M2-04 доказаны; M2-05 имеет чёткое B/C решение, а не новый модуль по умолчанию. **MVP:** M1/M2 и только необходимые NEXT, без LATER/END. Полная **93/93** исходная карта существует в двух связанных документах, но не означает полную parity старых алгоритмов.

**Запрет на забывание:** новый чат начинается с чтения этого файла и #352. Если отсутствует сравнение с историческим кодом для выбранного ID, работа над ним **не может называться DONE**. Результат конкретной функции — строка `ID → source/tests → current mapping → A/B/C/D → exact evidence`, а не ещё одна глобальная матрица или промежуточный отчёт.

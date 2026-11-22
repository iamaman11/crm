# Продуктовый план Cloudflare CRM

## Результат SK-00 · 23 ноября 2026

Действующее приложение — dev/apps/cloudflare-crm. Срез исходников c1f6501. Последняя принятая сборка 77611bd. Между версиями изменились три файла документации. [CI и опубликованный артефакт](https://github.com/iamaman11/crm/actions/runs/45063068917) · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7084953850).

Предмет сверки — возможности самой CRM, включая исполнение без сессии человека. Основания — доменные методы и их потребители, манифесты, D1, React, исторические адаптеры, подписанный runtime-отчёт и журнал опубликованных сценариев. В сборке 14 связанных модулей, 72 выполненных метода, 72 анонимных отказа и 32 отказа viewer для Write/AdminRead. Для независимой проверки каждого продукта нужен соответствующий сценарий. Успешный вызов сам по себе устанавливает существование кода.

Идентификаторы C и P сохраняют строку соответствующего источника на проверенном SHA. В строках со сложным целевым требованием статус относится ко всему указанному объёму; более узкие доказанные действия выделены в каталоге методов и в строках F.

**Статус операции** — ACCEPTED — доказан обозначенный результат в dev; CODE-NOT-ACCEPTED — найдена реализация или часть пути, полной приёмки для указанной границы нет; NOT-IMPLEMENTED — исполняемая реализация указанной операции не найдена в проверенном дереве. Декларация сообщения v1, ADR или пакет с одной константой не выполняют операцию.

**Решение о переносе** — A — сохранить принятое; B — построить нужное поведение на уже существующей семантике либо добавить нового владельца при новом изменяемом домене; C — отложить до указанной зависимости; D — оставить историческую инфраструктуру. Эти решения относятся к конкретной границе действия. При появлении новых доказательств строка пересматривается.

**Следующий срез — SK-01 + SK-04.** Реальный сотрудник входит в свой tenant; уполномоченный инициатор назначает задачу действующему коллеге; коллега видит её в своей очереди и завершает. Actor/Team, Patch<TaskOwner> и фильтр владельца уже есть в [domain.rs#L101](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L101). В адаптере create владелец выводится из текущего actor, публичный UpdateTask не даёт выбрать коллегу. Переиспользовать apply_update с проверкой версии. До записи проверить членство, полномочия и tenant; в том же пути проверить повтор, потерю ответа и чтение подтверждения из #485. Положительный human Access проверяется человеком. Сервисный актор получает только разрешения своей операции.

Далее SK-05 — фиксировать звонок, встречу и заметку с источником, временем и ссылками; SK-06/07 — рабочая очередь из разрешённых чтений и переход к первичным записям; SK-09 — единый пилот. У SK-02, SK-03 и SK-08 уже есть принятые части. Отдельный Lead нужен при независимом жизненном цикле до сделки. Большой CSV нужен при подтверждённом выходе за пределы 20 строк/16 KiB импорта или 100 Party экспорта.

## Очередь по зависимостям

1. Ядро и закрытый пилот SK-01…09.
2. Один входящий сайт или мессенджер, затем разрешённая доставка и сверка.
3. Сервис и сопровождение поверх канала, назначений и времени.
4. Каталог → цена → предложение → заказ → договор → подписка.
5. Триггеры и cadences над уже работающими командами.
6. Сегменты, кампании, рассылки, доставка и атрибуция.
7. Проекты, документы, отчётность и интеграции данных.
8. Управляемое расширение, AI, marketplace и вертикали. Защита данных и диагностика действуют в каждой волне.

## 01. Клиенты и Customer 360

Party, Account и Relationship уже имеют владельцев. Принятая merge-линия обратима; при оценке требования о survivorship проверять ещё и выбор значения каждого поля с его источником. Статус verify собственного email не переносить на подтверждение номера телефона.

**Очередь:** 1. **Ответственность:** Party, Account, ContactPoint, Relationship, Consent, Identity, Customer360. **Критерий перехода:** Сверить имя и первичный контакт, исправить качество, повторно прочитать связанные записи; чужой tenant ничего не получает.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-parties/src/lib.rs). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F01-01 | Вычислить владельца и provenance отдельного значения при конфликте источников | CODE-NOT-ACCEPTED | В исходниках импорта и enrichment есть provenance. Правила survivorship с несколькими источниками в dev не приняты. |
| C32 | Person and Organization Party identities | ACCEPTED | M1-02/03: создание и изменение Party, Account со ссылкой на Party. |
| C33 | customer/commercial Accounts referencing parties | ACCEPTED | M1-02/03: создание и изменение Party, Account со ссылкой на Party. |
| C34 | multiple email, phone, postal, web, social and messaging Contact Points | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C35 | preferred and verified contact methods | CODE-NOT-ACCEPTED | У ContactPoint есть preferred; signed self-verification определена для email. Остальные способы верификации требуют политики и независимого доказательства. |
| C36 | Party Relationships: employment, household, parent/subsidiary, partner and configurable typed roles | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C37 | account hierarchies and group structures | CODE-NOT-ACCEPTED | Историческая projection строит иерархию Party. Сначала решить, какая связь нужна для Account; доказать обработку цикла, tenant и удалённой связи. [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-party-relationships-projection/src/lib.rs) |
| C38 | source-system identifiers and reconciliation | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C39 | field-level provenance and survivorship | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C40 | deterministic and explainable duplicate candidates | ACCEPTED | N-05: детерминированные кандидаты по общему email и сравнение. |
| C41 | governed merge and unmerge with immutable lineage | ACCEPTED | L-03: обратимая линия merge/unmerge, без перепривязки всех сторонних объектов. |
| C42 | consent, communication preferences and legal-basis evidence | CODE-NOT-ACCEPTED | Доступны GrantVerifiedEmailConsent, WithdrawConsentAuthorization, SuppressCommunication и EvaluateConsent. На границе реальной доставки потребуется повторить проверку согласия перед отправкой. |
| C43 | privacy export, deletion, restriction and legal-hold interaction | CODE-NOT-ACCEPTED | Существуют legacy owner action, hold/restriction и access export. В dev приняты intake и отмена; positive human verification, frozen scope и полный экспорт проверяются отдельно. [access_export.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/tests/access_export.rs) |
| C44 | data-quality rules, completeness indicators and stewardship queues | CODE-NOT-ACCEPTED | N-04 принимает оценку имени и повторную проверку после исправления. Legacy finding assignment/waiver и его тесты пригодны для очереди stewardship. [finding_stewardship.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-data-quality/tests/finding_stewardship.rs) |
| C45 | governed enrichment with provenance and source policy | CODE-NOT-ACCEPTED | В dev принята публикация конфигурации. Dispatch, review, materialization и provenance результата требуют отдельного исполнения с провайдером. [независимая приёмка](https://github.com/iamaman11/crm/issues/424#issuecomment-6905530278) |
| C46 | versioned import/export, mapping, dry run, resumability and reconciliation | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C47 | customer timeline and cross-domain Customer 360 projections | CODE-NOT-ACCEPTED | GetAccountOverview принят для связанных Deal/Task/Contact/Relationship. Потребуется включить Interaction с временем/источником и проверить хронологию. |

## 02. Продажи и партнёры

Account может быть связан со сделкой, primary Contact — с контактной точкой. Для независимого Lead необходимы собственное состояние до сделки, причина дисквалификации и однократная конверсия. Для PRM отдельно проверять конфликт двух регистраций одной возможности.

**Очередь:** 1 → 4. **Ответственность:** Sales; будущий PRM владеет регистрацией партнёрской сделки. **Критерий перехода:** Пройти допустимый переход сделки, проверить отказ при конфликте версии, историю и результат повтора. PRM добавить при независимом партнёрском процессе.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/src/lib.rs). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C56 | leads/prospects and qualification lifecycle | NOT-IMPLEMENTED | Отдельная конверсия Lead отсутствует. Стадии lead/qualified сделки доступны в Sales. |
| C57 | opportunity/deal pipelines and stages | ACCEPTED | M2-01: воронки и стадии Sales. |
| C58 | account/contact/opportunity relationship roles | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C59 | activities, tasks, calls, meetings, reminders and follow-ups | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C63 | products and line items through Catalog/CPQ references rather than Sales-owned catalog state | NOT-IMPLEMENTED | C до зависимостей волны. |
| C64 | configurable sales processes and stage-entry/exit rules | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C65 | routing, assignment, queues and workload distribution | CODE-NOT-ACCEPTED | TaskListQuery.owner и UpdateTask.owner доступны в Rust. Следующий срез добавляет проверяемые выборку и назначение в публичном API. [domain.rs#L101](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L101) |
| C66 | territory management | NOT-IMPLEMENTED | C до зависимостей волны. |
| C67 | teams, overlays and account/opportunity team roles | NOT-IMPLEMENTED | C до зависимостей волны. |
| C68 | quota and target management | NOT-IMPLEMENTED | C до зависимостей волны. |
| C69 | forecasting, forecast categories, rollups and scenario comparison | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C70 | win/loss reason governance and sales-cycle analytics | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C71 | renewal, expansion and cross-sell opportunity support | NOT-IMPLEMENTED | C до зависимостей волны. |
| C72 | partner-influenced and channel-sourced opportunity attribution | NOT-IMPLEMENTED | C до зависимостей волны. |
| C189 | partner organizations and partner contacts | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C190 | partner tiers, programs, certifications and eligibility | NOT-IMPLEMENTED | C до зависимостей волны. |
| C191 | deal registration and conflict rules | NOT-IMPLEMENTED | C до зависимостей волны. |
| C192 | lead/opportunity distribution to partners | NOT-IMPLEMENTED | C до зависимостей волны. |
| C193 | partner-sourced/influenced attribution | NOT-IMPLEMENTED | C до зависимостей волны. |
| C194 | channel incentives/rebate integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C195 | partner portal and delegated access boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C196 | partner performance scorecards | NOT-IMPLEMENTED | C до зависимостей волны. |
| P297 | multiple pipelines per supported owner/resource type | CODE-NOT-ACCEPTED | Несколько Sales pipelines приняты. Для другого типа ресурса потребуется собственный owner и тест смены воронки. |
| P298 | versioned stage definitions and ordering | ACCEPTED | Принята граница Sales в M2-01/M2-04; обобщение на другие домены выполняется отдельно. |
| P299 | stage entry/exit criteria | CODE-NOT-ACCEPTED | StageTransitionPolicy и фиксированная связь Sales→Task работают в ограниченной области. Пользовательские условия/действия требуют отдельной управляемой конфигурации. [Sales domain](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/src/domain.rs#L234) |
| P300 | required fields and checklists | NOT-IMPLEMENTED | C до зависимостей волны. |
| P301 | allowed transitions and role restrictions | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P302 | stage probability, target duration and SLA | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P303 | automatic transitions through workflow actions where policy allows | CODE-NOT-ACCEPTED | StageTransitionPolicy и фиксированная связь Sales→Task работают в ограниченной области. Пользовательские условия/действия требуют отдельной управляемой конфигурации. [Sales domain](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/src/domain.rs#L234) |
| P304 | manual transition with validation and conflict handling | ACCEPTED | Принята граница Sales в M2-01/M2-04; обобщение на другие домены выполняется отдельно. |
| P305 | reasons for skip, regression, loss, cancellation or reopening | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P306 | pipeline templates and controlled cloning | NOT-IMPLEMENTED | C до зависимостей волны. |
| P307 | migration of active records when a pipeline version changes | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P308 | cross-pipeline transfer rules | NOT-IMPLEMENTED | C до зависимостей волны. |
| P309 | stage history and duration analytics | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P310 | funnel conversion and leakage analytics | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P356 | leads/prospects, qualification and conversion lineage | NOT-IMPLEMENTED | Отдельный Lead с собственной конверсией отложен C для MVP. Сейчас квалификация выполняется стадиями Deal. |
| P357 | richer opportunities/deals and configurable sales processes | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P358 | account/contact/opportunity roles and buying groups | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P359 | routing, assignment, queues, workload balancing and territories | CODE-NOT-ACCEPTED | TaskListQuery.owner и UpdateTask.owner доступны в Rust. Следующий срез добавляет проверяемые выборку и назначение в публичном API. [domain.rs#L101](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L101) |
| P360 | teams, overlays and collaboration | NOT-IMPLEMENTED | C до зависимостей волны. |
| P361 | quotas, targets and period management | NOT-IMPLEMENTED | C до зависимостей волны. |
| P362 | forecasting, categories, rollups and scenarios | CODE-NOT-ACCEPTED | Часть поведения реализована в типах Sales/Activities, runtime доступа, ModuleRegistry либо штатной поставке. Принятая граница и недостающее развитие разделены в каталогах ниже. |
| P363 | pipeline inspection, risk signals and coaching | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P364 | win/loss taxonomies and sales-cycle analytics | CODE-NOT-ACCEPTED | Часть поведения реализована в типах Sales/Activities, runtime доступа, ModuleRegistry либо штатной поставке. Принятая граница и недостающее развитие разделены в каталогах ниже. |
| P365 | renewals, expansion and cross-sell coordination | NOT-IMPLEMENTED | C до зависимостей волны. |
| P366 | partner-sourced/influenced attribution | NOT-IMPLEMENTED | C до зависимостей волны. |
| P367 | product/quote context through Phase 8B references | NOT-IMPLEMENTED | C до зависимостей волны. |
| P368 | mobile and offline field-selling journeys where required | NOT-IMPLEMENTED | C до зависимостей волны. |
| P423 | partner organizations/contacts | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P424 | tiers, programs, certification and eligibility | NOT-IMPLEMENTED | C до зависимостей волны. |
| P425 | deal registration and conflict resolution | NOT-IMPLEMENTED | C до зависимостей волны. |
| P426 | lead/opportunity distribution | NOT-IMPLEMENTED | C до зависимостей волны. |
| P427 | incentives/rebate integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| P428 | delegated partner portal access | NOT-IMPLEMENTED | C до зависимостей волны. |
| P429 | partner performance analytics | NOT-IMPLEMENTED | C до зависимостей волны. |

## 03. Рабочие представления и поиск

В Task различаются сохранение даты напоминания и доставка уведомления в нужный момент. Второму пути нужны планировщик, идентификатор действия, обработка отмены и доказательство доставки без браузера. Заголовок клиента в списке Task сейчас может породить отдельное чтение на строку (#485).

**Очередь:** 1. **Ответственность:** Activities, Sales, читающие представления. **Критерий перехода:** Очередь строится по правам текущего субъекта; фильтр действует до пагинации и сохраняется в ссылке.

Проверенная смежная граница: [TaskScreen.tsx](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/web/src/TaskScreen.tsx). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7084072103)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F03-01 | Создать задачу, назначить срок, перенести напоминание, завершить | ACCEPTED | M1-08; отдельные операции и публичные методы перечислены ниже. |
| F03-02 | Отобрать открытые просроченные задачи до пагинации | ACCEPTED | R1; Gate A/B 77611bd. |
| F03-03 | Переназначить задачу сотруднику или команде | CODE-NOT-ACCEPTED | UpdateTask.owner и отрицательные тесты уже есть в Rust domain. Нужны контракт, проверка членства и адаптер D1. |
| F03-04 | Открыть персональную очередь по владельцу | CODE-NOT-ACCEPTED | TaskListQuery.owner есть в домене. Требуется публичный фильтр с ограничением видимости. |
| F03-05 | Показать сводку ближайших задач, сделок и последних взаимодействий | NOT-IMPLEMENTED | Отдельные списки и Customer360 доступны; совместная рабочая очередь требует агрегированного чтения. |
| C60 | calendar synchronization boundaries and scheduling | NOT-IMPLEMENTED | C до зависимостей волны. |
| C62 | pipeline views, Kanban, lists, saved views and bulk actions | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C272 | notifications and notification preferences | NOT-IMPLEMENTED | C до зависимостей волны. |
| C273 | mentions, comments and collaboration threads where appropriate | NOT-IMPLEMENTED | C до зависимостей волны. |
| C274 | shared/team queues and work lists | CODE-NOT-ACCEPTED | TaskListQuery.owner и UpdateTask.owner доступны в Rust. Следующий срез добавляет проверяемые выборку и назначение в публичном API. [domain.rs#L101](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L101) |
| C275 | personal task/calendar views | CODE-NOT-ACCEPTED | TaskListQuery.owner и UpdateTask.owner доступны в Rust. Следующий срез добавляет проверяемые выборку и назначение в публичном API. [domain.rs#L101](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L101) |
| C276 | activity feed and recent items | NOT-IMPLEMENTED | C до зависимостей волны. |
| C277 | favorites/pins | NOT-IMPLEMENTED | C до зависимостей волны. |
| C278 | command palette and global navigation | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C279 | saved views, filters and column configurations | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C280 | bulk actions with preview, authorization and partial-failure semantics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C282 | keyboard accessibility and productivity shortcuts | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C290 | global search across authorized CRM resources | ACCEPTED | L-08: поиск по Party/Account/Deal/Task, фильтры и переходы в доступные записи. |
| C291 | domain-specific search and filtering | ACCEPTED | L-08: поиск по Party/Account/Deal/Task, фильтры и переходы в доступные записи. |
| C292 | typo tolerance/relevance adapters where enabled | NOT-IMPLEMENTED | C до зависимостей волны. |
| C293 | permission changes enforced at disclosure time | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C294 | deterministic pagination and rebuild/switch generations | CODE-NOT-ACCEPTED | Keyset в списках есть. В legacy существует SearchReindexCoordinator; поколение восстановить и переключить без пропуска записи перед переходом на общий индекс. [generation.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-search-runtime/src/generation.rs) |
| C295 | recent/popular suggestions as non-authoritative projections | NOT-IMPLEMENTED | C до зависимостей волны. |
| C296 | knowledge and document discovery | NOT-IMPLEMENTED | C до зависимостей волны. |
| C297 | semantic/vector retrieval only through permission-filtered governed paths | NOT-IMPLEMENTED | C до зависимостей волны. |
| C298 | deletion/restriction/consent effects propagated with defined freshness guarantees | NOT-IMPLEMENTED | C до зависимостей волны. |
| P314 | drag-and-drop with server-authoritative transition validation | CODE-NOT-ACCEPTED | Реальные Chromium-тесты выполняют dragTo и проверяют предвыбор этапа. Для жеста отдельно зафиксировать независимое подтверждение; сам переход AdvanceStage принят. [Browser test](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/test/typed-browser.mjs#L1312) |
| P315 | optimistic UX only with explicit rollback on conflict | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P316 | customizable cards and fields | NOT-IMPLEMENTED | C до зависимостей волны. |
| P317 | grouping and swimlanes | NOT-IMPLEMENTED | C до зависимостей волны. |
| P318 | filters, saved views and sharing | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P319 | sorting, search and quick actions | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P320 | aggregate totals and weighted values | ACCEPTED | Принята граница Sales в M2-01/M2-04; обобщение на другие домены выполняется отдельно. |
| P321 | WIP/staleness/SLA indicators | NOT-IMPLEMENTED | C до зависимостей волны. |
| P322 | bulk move with preview and partial-failure results | NOT-IMPLEMENTED | C до зависимостей волны. |
| P323 | permissions and field masking on every card | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P324 | responsive and keyboard-accessible operation | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P325 | large-pipeline virtualization and stable pagination | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P326 | real-time or bounded-freshness updates | NOT-IMPLEMENTED | C до зависимостей волны. |

## 04. Автоматизация и правила

Событие смены этапа уже может породить Task через sales_activities_link. Для произвольного процесса workflow.proto задаёт сообщения, crm-module-sdk — start/signal/cancel, а crm-core-workflow содержит marker. Выделять runner только при принятии версии определения, durable таймера и воспроизводимого шага.

**Очередь:** 5. **Ответственность:** Workflow координирует опубликованные операции владельцев. **Критерий перехода:** На событии запустить правило без браузера; сохранить версию, остановить цикл, восстановить таймер, повторить доставку с одним эффектом.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-core-workflow/src/lib.rs). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F04-01 | При переходе сделки автоматически создать связанную задачу один раз | ACCEPTED | M1-09: внутренний Sales→Activities link и атомарное изменение в D1; общий редактор правил — следующая волна. |
| C257 | trigger, condition, branch, wait, timer and schedule semantics | CODE-NOT-ACCEPTED | Есть фиксированная Sales→Task связь и расчёт recurrence в Activities. Универсальная публикация триггера и durable запуск по расписанию требуют отдельного исполнения. [recurrence.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/recurrence.rs) |
| C258 | exact governed capability invocation as workflow actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| C259 | approvals and human tasks | NOT-IMPLEMENTED | C до зависимостей волны. |
| C260 | retries, idempotency and compensation/recovery strategies | CODE-NOT-ACCEPTED | Есть replay конкретных команд и публикация версий metadata; требуется исполняемая общая модель workflow. |
| C261 | versioned workflow definitions and immutable execution evidence | CODE-NOT-ACCEPTED | Опубликованные revision metadata доступны в прежнем runtime; цель включает версию запущенного workflow и неизменяемый журнал его шагов. |
| C262 | business calendars and SLA timers | CODE-NOT-ACCEPTED | В Activities есть календарный расчёт recurrence с отрицательными тестами. Durable пробуждение и SLA в Worker отдельно не приняты. [Recurrence](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/recurrence.rs#L72) |
| C263 | event-driven and scheduled automation | NOT-IMPLEMENTED | C до зависимостей волны. |
| C264 | integration actions through allowlisted governed connectors | NOT-IMPLEMENTED | C до зависимостей волны. |
| C265 | no arbitrary SQL, secret access, unrestricted HTTP or hidden mutation bypass | CODE-NOT-ACCEPTED | Архитектурное ограничение действует в текущем Worker. Для планируемого редактора или проекции самостоятельно проверить соблюдение после подключения. |
| C266 | simulation/test mode and execution observability | NOT-IMPLEMENTED | C до зависимостей волны. |
| P177 | record created, updated, deleted or restored | NOT-IMPLEMENTED | C до зависимостей волны. |
| P178 | selected field/value changes | NOT-IMPLEMENTED | C до зависимостей волны. |
| P179 | lifecycle or pipeline stage entry/exit | NOT-IMPLEMENTED | C до зависимостей волны. |
| P180 | governed domain events | NOT-IMPLEMENTED | C до зависимостей волны. |
| P181 | inbound verified webhook events | NOT-IMPLEMENTED | C до зависимостей волны. |
| P182 | scheduled date/time and recurring schedules | CODE-NOT-ACCEPTED | В Activities есть календарный расчёт recurrence с отрицательными тестами. Durable пробуждение и SLA в Worker отдельно не приняты. [Recurrence](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/recurrence.rs#L72) |
| P183 | relative timers, inactivity and SLA thresholds | NOT-IMPLEMENTED | C до зависимостей волны. |
| P184 | message delivery/open/reply/bounce events where providers expose them | NOT-IMPLEMENTED | C до зависимостей волны. |
| P185 | assignment, ownership or queue changes | NOT-IMPLEMENTED | C до зависимостей волны. |
| P186 | analytics/score/health threshold crossings through governed derived events | NOT-IMPLEMENTED | C до зависимостей волны. |
| P187 | manual user start | NOT-IMPLEMENTED | C до зависимостей волны. |
| P188 | API/integration start | NOT-IMPLEMENTED | C до зависимостей волны. |
| P189 | bulk/import completion and reconciliation outcomes | NOT-IMPLEMENTED | C до зависимостей волны. |
| P197 | typed comparisons and null/empty semantics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P198 | related-resource predicates through authorized queries | NOT-IMPLEMENTED | C до зависимостей волны. |
| P199 | AND/OR/NOT groups | NOT-IMPLEMENTED | C до зависимостей волны. |
| P200 | deterministic expression functions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P201 | business-calendar calculations | NOT-IMPLEMENTED | C до зависимостей волны. |
| P202 | changed-from/changed-to predicates | NOT-IMPLEMENTED | C до зависимостей волны. |
| P203 | actor/team/territory/permission conditions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P204 | consent, privacy and data-class conditions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P205 | feature/module activation conditions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P206 | bounded decision tables | NOT-IMPLEMENTED | C до зависимостей волны. |
| P207 | explainable rule outcomes | NOT-IMPLEMENTED | C до зависимостей волны. |
| P215 | create or update a domain resource through its owner capability | NOT-IMPLEMENTED | C до зависимостей волны. |
| P216 | move a record through an allowed lifecycle or pipeline transition | NOT-IMPLEMENTED | C до зависимостей волны. |
| P217 | assign/reassign to user, team, queue, territory or routing policy | NOT-IMPLEMENTED | C до зависимостей волны. |
| P218 | create task, appointment, reminder, checklist or human work item | NOT-IMPLEMENTED | C до зависимостей волны. |
| P219 | send notification | NOT-IMPLEMENTED | C до зависимостей волны. |
| P220 | send consent-authorized email/SMS/message through Communications | NOT-IMPLEMENTED | C до зависимостей волны. |
| P221 | start, pause, resume or cancel a sales cadence/journey where allowed | NOT-IMPLEMENTED | C до зависимостей волны. |
| P222 | request serial/parallel approval | NOT-IMPLEMENTED | C до зависимостей волны. |
| P223 | generate a document or e-signature envelope | NOT-IMPLEMENTED | C до зависимостей волны. |
| P224 | invoke an allowlisted connector action | NOT-IMPLEMENTED | C до зависимостей волны. |
| P225 | publish a governed integration event | NOT-IMPLEMENTED | C до зависимостей волны. |
| P226 | wait until time, event, condition or human completion | NOT-IMPLEMENTED | C до зависимостей волны. |
| P227 | branch, parallelize, join or call a reusable subflow | NOT-IMPLEMENTED | C до зависимостей волны. |
| P228 | invoke a governed AI tool with policy and approval controls | NOT-IMPLEMENTED | C до зависимостей волны. |
| P229 | compensate or reverse a prior action only when the owner exposes a supported reversal | NOT-IMPLEMENTED | C до зависимостей волны. |
| P237 | immutable definition versions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P238 | explicit draft, test, published, suspended and retired states | NOT-IMPLEMENTED | C до зависимостей волны. |
| P239 | exact binding to action/capability versions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P240 | durable execution instances and step history | NOT-IMPLEMENTED | C до зависимостей волны. |
| P241 | retries with bounded backoff | NOT-IMPLEMENTED | C до зависимостей волны. |
| P242 | idempotency and duplicate-trigger suppression | NOT-IMPLEMENTED | C до зависимостей волны. |
| P243 | leases, crash recovery and replay | NOT-IMPLEMENTED | C до зависимостей волны. |
| P244 | waits and timers that survive deployment/restart | NOT-IMPLEMENTED | C до зависимостей волны. |
| P245 | parallel branches and deterministic joins | NOT-IMPLEMENTED | C до зависимостей волны. |
| P246 | bounded loops/iteration with quota controls | NOT-IMPLEMENTED | C до зависимостей волны. |
| P247 | cancellation and timeout | NOT-IMPLEMENTED | C до зависимостей волны. |
| P248 | compensation strategy where supported | NOT-IMPLEMENTED | C до зависимостей волны. |
| P249 | dead-letter and operator recovery queues | NOT-IMPLEMENTED | C до зависимостей волны. |
| P250 | version-aware migration policy for running instances | NOT-IMPLEMENTED | C до зависимостей волны. |
| P251 | complete actor, reason, input digest, result and cost evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| P252 | tenant/workflow/action concurrency and rate limits | NOT-IMPLEMENTED | C до зависимостей волны. |
| P258 | trigger/action node palette | NOT-IMPLEMENTED | C до зависимостей волны. |
| P259 | drag-and-drop graph editor | NOT-IMPLEMENTED | C до зависимостей волны. |
| P260 | typed configuration forms | NOT-IMPLEMENTED | C до зависимостей волны. |
| P261 | branch and decision-table editors | NOT-IMPLEMENTED | C до зависимостей волны. |
| P262 | reusable subflows/templates | NOT-IMPLEMENTED | C до зависимостей волны. |
| P263 | validation before save and publish | CODE-NOT-ACCEPTED | Историческое metadata authoring валидирует пакет и его влияние. Для графа процесса проверять связи, права команд и циклы перед публикацией. |
| P264 | dependency and permission impact analysis | CODE-NOT-ACCEPTED | Историческое metadata authoring валидирует пакет и его влияние. Для графа процесса проверять связи, права команд и циклы перед публикацией. |
| P265 | simulation using representative/synthetic data | NOT-IMPLEMENTED | C до зависимостей волны. |
| P266 | dry-run mode with no side effects | NOT-IMPLEMENTED | C до зависимостей волны. |
| P267 | step-through testing and captured traces | NOT-IMPLEMENTED | C до зависимостей волны. |
| P268 | version diff, clone and rollback | CODE-NOT-ACCEPTED | Admin Studio умеет показать изменения версии metadata и откатить публикацию; редактор исполняемого workflow и управление запущенными экземплярами требуют отдельного среза. |
| P269 | sandbox-to-production promotion | NOT-IMPLEMENTED | C до зависимостей волны. |
| P270 | run explorer, failure diagnostics and replay controls | NOT-IMPLEMENTED | C до зависимостей волны. |
| P271 | usage, latency, error and cost dashboards | NOT-IMPLEMENTED | C до зависимостей волны. |
| P272 | localized descriptions and accessibility | NOT-IMPLEMENTED | C до зависимостей волны. |

## 05. Cadences и playbooks

Запланированный Task — полезный шаг последовательности. Enrollment требует собственного состояния цели, версии и позиции в последовательности. При двух событиях ответа проверить единственную остановку и отсутствие последующего сообщения.

**Очередь:** 5. **Ответственность:** Cadence — состояние участия, Communications — доставка. **Критерий перехода:** Остановить последовательность при ответе или отзыве согласия, включая уже запланированный шаг.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-consents/src/lib.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C74 | sales playbooks, sequences and guided next actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P336 | enroll/unenroll individuals or segments | NOT-IMPLEMENTED | C до зависимостей волны. |
| P337 | manual, rule-based and workflow-based enrollment | NOT-IMPLEMENTED | C до зависимостей волны. |
| P338 | email, call, task, meeting, social/manual and wait steps | NOT-IMPLEMENTED | C до зависимостей волны. |
| P339 | conditional branches based on activity, reply, field, score or stage | NOT-IMPLEMENTED | C до зависимостей волны. |
| P340 | automatic stop on reply, opt-out, conversion, disqualification or policy condition | NOT-IMPLEMENTED | C до зависимостей волны. |
| P341 | quiet hours, locale/time zone and frequency caps | NOT-IMPLEMENTED | C до зависимостей волны. |
| P342 | consent and suppression enforcement | NOT-IMPLEMENTED | C до зависимостей волны. |
| P343 | templates, snippets, scripts and personalization | NOT-IMPLEMENTED | C до зависимостей волны. |
| P344 | owner/team work queues and next-best task ordering | NOT-IMPLEMENTED | C до зависимостей волны. |
| P345 | overdue handling, reassignment and pause/resume | NOT-IMPLEMENTED | C до зависимостей волны. |
| P346 | A/B variants and performance analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P347 | cadence versioning and in-flight target policy | NOT-IMPLEMENTED | C до зависимостей волны. |
| P348 | reusable sales/service/success playbooks | NOT-IMPLEMENTED | C до зависимостей волны. |
| P349 | guided stage checklists and recommended next actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P350 | AI-assisted drafting/recommendation only through governed policy | NOT-IMPLEMENTED | C до зависимостей волны. |

## 06. Каналы и взаимодействия

ContactPoint с типом messaging хранит адрес. Для входа сайта/мессенджера проверить источник и tenant до записи, выделить внешний ID. Для исходящего сообщения сохранять ID провайдера, при таймауте сверять исход, учитывать запоздалые delivery/bounce callbacks.

**Очередь:** 1 → 2. **Ответственность:** Interaction/Conversation, Consent, адаптер канала. **Критерий перехода:** Сначала ручное взаимодействие, затем первый сайт или мессенджер. Подтвердить вход при закрытом UI, повтор, историю и реальное подтверждение исходящей доставки.

Проверенная смежная граница: [message.proto](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/communications/v1/message.proto). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F06-01 | Записать завершённый звонок, встречу или заметку в историю клиента и сделки | NOT-IMPLEMENTED | Задача и технический аудит не содержат такого общего журнала. Ввести Interaction и проверить ссылку, автора и видимость. |
| C61 | notes, attachments and timeline activity | NOT-IMPLEMENTED | C до зависимостей волны. |
| C127 | email send/receive and threading | NOT-IMPLEMENTED | C до зависимостей волны. |
| C128 | telephony/call events, recordings and transcription references | NOT-IMPLEMENTED | C до зависимостей волны. |
| C129 | SMS and messaging channels | NOT-IMPLEMENTED | C до зависимостей волны. |
| C130 | web chat and bot handoff | NOT-IMPLEMENTED | C до зависимостей волны. |
| C131 | social/direct-message channel adapters where enabled | NOT-IMPLEMENTED | C до зависимостей волны. |
| C132 | unified conversation/thread model | NOT-IMPLEMENTED | C до зависимостей волны. |
| C133 | inbound/outbound message identity and participant resolution | NOT-IMPLEMENTED | C до зависимостей волны. |
| C134 | templates, signatures and localization | NOT-IMPLEMENTED | C до зависимостей волны. |
| C135 | attachments and governed file references | NOT-IMPLEMENTED | C до зависимостей волны. |
| C136 | delivery, bounce, read and provider status events where available | NOT-IMPLEMENTED | C до зависимостей волны. |
| C137 | communication consent enforcement before send | CODE-NOT-ACCEPTED | Доступны GrantVerifiedEmailConsent, WithdrawConsentAuthorization, SuppressCommunication и EvaluateConsent. На границе реальной доставки потребуется повторить проверку согласия перед отправкой. |
| C138 | quiet hours, frequency limits and channel preferences | NOT-IMPLEMENTED | C до зависимостей волны. |
| C139 | agent inbox, queues and assignment | NOT-IMPLEMENTED | C до зависимостей волны. |
| C140 | searchable interaction history with live authorization | NOT-IMPLEMENTED | C до зависимостей волны. |
| C141 | provider abstraction, webhook verification, replay safety and delivery reconciliation | NOT-IMPLEMENTED | C до зависимостей волны. |

## 07. Сервис, knowledge, field service

Исторический ticket.proto содержит id, tenant, subject, status, priority и assignee. Для смены статуса тикета нужно реализовать правила перехода и очередь. Для SLA вход вне рабочих часов и смена часового пояса должны давать воспроизводимый срок.

**Очередь:** 3. **Ответственность:** Service, Knowledge, назначения выездов. **Критерий перехода:** Входящее обращение назначается, срок рассчитывается по календарю, просрочка создаёт одну эскалацию.

Проверенная смежная граница: [ticket.proto](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/support/v1/ticket.proto).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C104 | cases/tickets with configurable lifecycle and priority | NOT-IMPLEMENTED | C до зависимостей волны. |
| C105 | queues, assignment, skills and routing | NOT-IMPLEMENTED | C до зависимостей волны. |
| C106 | SLAs, milestones, business calendars and breach escalation | NOT-IMPLEMENTED | C до зависимостей волны. |
| C107 | entitlements, service contracts and warranty references | NOT-IMPLEMENTED | C до зависимостей волны. |
| C108 | omnichannel case creation and conversation linkage | NOT-IMPLEMENTED | C до зависимостей волны. |
| C109 | customer, asset/product and order/contract context | NOT-IMPLEMENTED | C до зависимостей волны. |
| C110 | internal notes versus customer-visible replies | NOT-IMPLEMENTED | C до зависимостей волны. |
| C111 | macros, templates, guided resolution and automation | NOT-IMPLEMENTED | C до зависимостей волны. |
| C112 | parent/child, duplicate and major-incident case relationships | NOT-IMPLEMENTED | C до зависимостей волны. |
| C113 | escalation, swarming and collaboration | NOT-IMPLEMENTED | C до зависимостей волны. |
| C114 | knowledge-base authoring, review, publication, localization and feedback | NOT-IMPLEMENTED | C до зависимостей волны. |
| C115 | knowledge suggestion and deflection analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C116 | CSAT/NPS/CES or configurable feedback collection boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C117 | service analytics, backlog, response/resolution time and SLA reporting | NOT-IMPLEMENTED | C до зависимостей волны. |
| C118 | self-service portal/API boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C119 | field service where enabled: work orders, dispatch, skills, territories, appointments, technician mobile/offline workflow and parts/inventory integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| P374 | cases/tickets with configurable lifecycle, priority and major-incident relationships | NOT-IMPLEMENTED | C до зависимостей волны. |
| P375 | skills, queues, assignment and capacity-aware routing | NOT-IMPLEMENTED | C до зависимостей волны. |
| P376 | SLAs, milestones, business calendars and escalations | NOT-IMPLEMENTED | C до зависимостей волны. |
| P377 | customer/order/contract/product/asset context | NOT-IMPLEMENTED | C до зависимостей волны. |
| P378 | email, messaging, SMS, telephony, web chat and bot handoff | NOT-IMPLEMENTED | C до зависимостей волны. |
| P379 | unified conversation and participant resolution | NOT-IMPLEMENTED | C до зависимостей волны. |
| P380 | internal notes versus customer-visible replies | NOT-IMPLEMENTED | C до зависимостей волны. |
| P381 | templates, macros and guided resolution | NOT-IMPLEMENTED | C до зависимостей волны. |
| P382 | knowledge authoring, review, localization, publication and feedback | NOT-IMPLEMENTED | C до зависимостей волны. |
| P383 | self-service portal/API and deflection analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P384 | CSAT/NPS/CES or configurable feedback boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| P385 | field-service work orders, dispatch, territories, appointments and technician mobile/offline workflows | NOT-IMPLEMENTED | C до зависимостей волны. |
| P386 | parts/inventory integration rather than hidden inventory ownership | NOT-IMPLEMENTED | C до зависимостей волны. |

## 08. Маркетинг

Consent suppression действует в узкой области. Для кампании определить состав аудитории на старте, повторно проверить разрешение у получателя перед каждой отправкой и согласовать наблюдение конверсии с первичным событием сайта.

**Очередь:** 6. **Ответственность:** Campaign, Audience, Journey; доставка через канал. **Критерий перехода:** Проверить изменение сегмента, отписку между планированием и отправкой, результат провайдера и один учтённый touchpoint.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-consents/src/lib.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C149 | campaigns and campaign hierarchy | NOT-IMPLEMENTED | C до зависимостей волны. |
| C150 | audience segmentation and dynamic segments | NOT-IMPLEMENTED | C до зависимостей волны. |
| C151 | lists and suppression lists | NOT-IMPLEMENTED | C до зависимостей волны. |
| C152 | acquisition source and campaign-member history | NOT-IMPLEMENTED | C до зависимостей волны. |
| C153 | forms, landing-page and event ingestion boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C154 | lead scoring and qualification models | NOT-IMPLEMENTED | C до зависимостей волны. |
| C155 | journeys, branching, waits, triggers and goals | NOT-IMPLEMENTED | C до зависимостей волны. |
| C156 | consent-aware channel activation | CODE-NOT-ACCEPTED | Доступны GrantVerifiedEmailConsent, WithdrawConsentAuthorization, SuppressCommunication и EvaluateConsent. На границе реальной доставки потребуется повторить проверку согласия перед отправкой. |
| C157 | A/B and multivariate experiment support where applicable | NOT-IMPLEMENTED | C до зависимостей волны. |
| C158 | marketing content/template references | NOT-IMPLEMENTED | C до зависимостей волны. |
| C159 | campaign costs and external ad-platform integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C160 | attribution models and touchpoint lineage | NOT-IMPLEMENTED | C до зависимостей волны. |
| C161 | funnel, cohort and conversion analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C162 | event/webinar registration and attendance integration | NOT-IMPLEMENTED | C до зависимостей волны. |
| C163 | account-based marketing and buying-group support | NOT-IMPLEMENTED | C до зависимостей волны. |
| C164 | loyalty/referral programs as optional governed modules rather than hidden campaign fields | NOT-IMPLEMENTED | C до зависимостей волны. |
| P392 | campaigns and hierarchy | NOT-IMPLEMENTED | C до зависимостей волны. |
| P393 | static and dynamic segments | NOT-IMPLEMENTED | C до зависимостей волны. |
| P394 | lists and suppression lists | NOT-IMPLEMENTED | C до зависимостей волны. |
| P395 | forms, landing-page and event ingestion boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| P396 | scoring and qualification models | NOT-IMPLEMENTED | C до зависимостей волны. |
| P397 | multi-step journeys with trigger, branch, wait, goal and exit semantics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P398 | email/SMS/messaging activation through consent-aware Communications | NOT-IMPLEMENTED | C до зависимостей волны. |
| P399 | experiments and holdout groups | NOT-IMPLEMENTED | C до зависимостей волны. |
| P400 | frequency caps, quiet hours and deliverability policy | NOT-IMPLEMENTED | C до зависимостей волны. |
| P401 | campaign-member and touchpoint lineage | NOT-IMPLEMENTED | C до зависимостей волны. |
| P402 | attribution, funnel, cohort and conversion analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P403 | webinar/event integration | NOT-IMPLEMENTED | C до зависимостей волны. |
| P404 | account-based marketing and buying groups | NOT-IMPLEMENTED | C до зависимостей волны. |
| P405 | loyalty/referral as optional governed modules | NOT-IMPLEMENTED | C до зависимостей волны. |

## 09. Customer success

Account и Deal поставляют идентичность клиента и историю продаж. Health должен хранить версию формулы и причины изменения. Поздний сигнал использования пересчитывает только зависимые показатели и не повторяет уже созданную задачу.

**Очередь:** 3. **Ответственность:** Success plan и health; ссылки на Account и использование. **Критерий перехода:** Повтор сигнала использования пересчитывает показатель с объяснимыми входами и создаёт одно поручение.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-accounts/src/lib.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C172 | customer lifecycle stage and onboarding plans | NOT-IMPLEMENTED | C до зависимостей волны. |
| C173 | success plans, objectives and milestones | NOT-IMPLEMENTED | C до зависимостей волны. |
| C174 | health scores with explainable component evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| C175 | adoption and usage-signal integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C176 | risks, alerts and playbooks | NOT-IMPLEMENTED | C до зависимостей волны. |
| C177 | customer touch plans and business reviews | NOT-IMPLEMENTED | C до зависимостей волны. |
| C178 | renewal and expansion coordination | NOT-IMPLEMENTED | C до зависимостей волны. |
| C179 | churn reason taxonomy and retention analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C180 | stakeholder maps and relationship coverage | NOT-IMPLEMENTED | C до зависимостей волны. |
| C181 | product entitlement/subscription context through stable references | NOT-IMPLEMENTED | C до зависимостей волны. |
| P413 | onboarding plans and lifecycle stages | NOT-IMPLEMENTED | C до зависимостей волны. |
| P414 | success plans, objectives and milestones | NOT-IMPLEMENTED | C до зависимостей волны. |
| P415 | explainable health scores | NOT-IMPLEMENTED | C до зависимостей волны. |
| P416 | product adoption/usage signals | NOT-IMPLEMENTED | C до зависимостей волны. |
| P417 | risks, alerts and playbooks | NOT-IMPLEMENTED | C до зависимостей волны. |
| P418 | business reviews and stakeholder maps | NOT-IMPLEMENTED | C до зависимостей волны. |
| P419 | renewal, expansion and churn coordination | NOT-IMPLEMENTED | C до зависимостей волны. |

## 10. Каталог и CPQ

Объект Quote v1 содержит сумму, валюту, ссылку Deal и статус. Для расчёта и согласования нужны строки с точной версией цены, срок, правило скидки и история ревизии. Принятая денежная арифметика Deal здесь переиспользуется отдельно.

**Очередь:** 4. **Ответственность:** Catalog, Pricing, Quote. **Критерий перехода:** Зафиксировать версию цены, дату действия и валюту. Повторить расчёт после смены прайса, провести согласование исключения.

Проверенная смежная граница: [quote.proto](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/sales/v1/quote.proto).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C73 | approval processes for discounts and other high-risk commercial actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| C83 | product and service catalog ownership | NOT-IMPLEMENTED | C до зависимостей волны. |
| C84 | product families, bundles, options, dependencies and compatibility rules | NOT-IMPLEMENTED | C до зависимостей волны. |
| C85 | catalog versioning and effective dating | NOT-IMPLEMENTED | C до зависимостей волны. |
| C86 | price books, currencies, price lists and customer/segment-specific pricing | NOT-IMPLEMENTED | C до зависимостей волны. |
| C87 | discounts, promotions, approval thresholds and exception evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| C88 | CPQ configuration and validation | NOT-IMPLEMENTED | C до зависимостей волны. |
| C89 | quote creation, revisions, comparison, expiry and approval | NOT-IMPLEMENTED | C до зависимостей волны. |
| C90 | quote line items, taxes/fees as governed inputs or integration results | NOT-IMPLEMENTED | C до зависимостей волны. |
| P116 | products and services | NOT-IMPLEMENTED | C до зависимостей волны. |
| P117 | families, categories, variants and units of measure | NOT-IMPLEMENTED | C до зависимостей волны. |
| P118 | bundles, options, dependencies and compatibility constraints | NOT-IMPLEMENTED | C до зависимостей волны. |
| P119 | catalog versions and effective dates | NOT-IMPLEMENTED | C до зависимостей волны. |
| P120 | price books, currencies and price-list versions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P121 | customer, segment and channel price eligibility | NOT-IMPLEMENTED | C до зависимостей волны. |
| P122 | base prices, tiered prices and governed promotion references | NOT-IMPLEMENTED | C до зависимостей волны. |
| P123 | active/inactive/retired lifecycle | NOT-IMPLEMENTED | C до зависимостей волны. |
| P124 | import/export and migration profile | NOT-IMPLEMENTED | C до зависимостей волны. |
| P125 | catalog search, list and record UX | NOT-IMPLEMENTED | C до зависимостей волны. |
| P126 | authorization-aware product selection for downstream domains | NOT-IMPLEMENTED | C до зависимостей волны. |
| P142 | quote creation and revision lineage | NOT-IMPLEMENTED | C до зависимостей волны. |
| P143 | quote lines referencing exact catalog/price versions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P144 | configurable product validation | NOT-IMPLEMENTED | C до зависимостей волны. |
| P145 | discount and exception rules | NOT-IMPLEMENTED | C до зависимостей волны. |
| P146 | serial and parallel approval chains | NOT-IMPLEMENTED | C до зависимостей волны. |
| P147 | human approval tasks, delegation, expiry and escalation | NOT-IMPLEMENTED | C до зависимостей волны. |
| P148 | wait/timer semantics and business calendars | NOT-IMPLEMENTED | C до зависимостей волны. |
| P149 | immutable orchestration execution evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| P150 | retry, idempotency, cancellation and recovery | NOT-IMPLEMENTED | C до зависимостей волны. |
| P151 | quote comparison, expiry and customer-ready document preview | NOT-IMPLEMENTED | C до зависимостей волны. |
| P152 | Kanban/list views for quote work queues and approval state | NOT-IMPLEMENTED | C до зависимостей волны. |
| P160 | 8B.3 — CPQ configuration depth: option groups, dependency rules, guided selling, validation explanations and simulation | NOT-IMPLEMENTED | C до зависимостей волны. |
| P161 | 8B.4 — Quotes: customer-facing revisions, acceptance, expiry, approval evidence and document generation | NOT-IMPLEMENTED | C до зависимостей волны. |

## 11. Заказ, договор и подписка

Invoice/payment v1 — входные формы данных. Полный путь потребует связи с order/contract, обработки возврата либо отмены и сверки внешнего платежа. Источником бухгалтерского факта остаётся финансовая система.

**Очередь:** 4. **Ответственность:** Order, Contract, Subscription; ERP и платёж — внешние системы. **Критерий перехода:** Связать заказ с принятой версией предложения, повтор уведомления не удваивает обязательство.

Проверенная смежная граница: [payment.proto](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/billing/v1/payment.proto).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C91 | order capture and order lifecycle handoff | NOT-IMPLEMENTED | C до зависимостей волны. |
| C92 | contracts, amendments, renewals and termination | NOT-IMPLEMENTED | C до зависимостей волны. |
| C93 | subscriptions, plans, entitlements, usage references and lifecycle changes | NOT-IMPLEMENTED | C до зависимостей волны. |
| C94 | billing/ERP/payment/tax integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C95 | commercial commitment lineage from opportunity through quote, order, contract and subscription | NOT-IMPLEMENTED | C до зависимостей волны. |
| P162 | 8B.5 — Orders: order capture, change/cancel lifecycle, fulfillment coordination and reconciliation boundaries | NOT-IMPLEMENTED | Требуется отдельный владелец заказа, ссылающийся на точную ревизию предложения. |
| P163 | 8B.6 — Contracts: terms, amendments, renewals, termination, obligations and e-signature references | NOT-IMPLEMENTED | C до зависимостей волны. |
| P164 | 8B.7 — Subscriptions/Entitlements/Usage: plans, lifecycle changes, entitlement checks and governed usage ingestion | NOT-IMPLEMENTED | C до зависимостей волны. |
| P165 | 8B.8 — Billing integration: invoicing/payment/tax/ERP boundaries, reconciliation and commercial lineage without moving accounting ownership into CRM | NOT-IMPLEMENTED | C до зависимостей волны. |

## 12. Проекты и рабочие дела

TaskOwner и срок можно использовать в work item. Project отвечает за зависимости и доступ к общему проекту. Замкнутая зависимость и попытка читать соседний проект должны завершаться без изменения состояния.

**Очередь:** 7. **Ответственность:** Project, зависимости и доступ к проекту; Tasks для исполнения. **Критерий перехода:** Создать работу по шаблону, проверить зависимость, срок и видимость для клиента.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/lib.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C204 | projects, workstreams, milestones and tasks | NOT-IMPLEMENTED | C до зависимостей волны. |
| C205 | configurable business cases/process instances distinct from support cases | NOT-IMPLEMENTED | C до зависимостей волны. |
| C206 | dependencies, assignees, teams and due dates | CODE-NOT-ACCEPTED | Есть типы субъекта/владельца, текущая политика или TaskOwner. Открыт полный путь сотрудника, назначения и чтения области доступа. |
| C207 | checklists and repeatable templates | NOT-IMPLEMENTED | C до зависимостей волны. |
| C208 | status, risk and issue tracking | NOT-IMPLEMENTED | C до зависимостей волны. |
| C209 | time/cost integration boundaries where enabled | NOT-IMPLEMENTED | C до зависимостей волны. |
| C210 | customer-facing project visibility boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C211 | automation and timeline integration | NOT-IMPLEMENTED | C до зависимостей волны. |
| C212 | portfolio and delivery analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P433 | projects, workstreams, milestones and tasks | NOT-IMPLEMENTED | C до зависимостей волны. |
| P434 | dependencies, checklists, templates and recurring work | NOT-IMPLEMENTED | C до зависимостей волны. |
| P435 | risks, issues and decisions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P436 | Kanban/list/calendar/timeline views | NOT-IMPLEMENTED | C до зависимостей волны. |
| P437 | customer-facing visibility policy | NOT-IMPLEMENTED | C до зависимостей волны. |
| P438 | portfolio and delivery analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P439 | configurable operational case types built on governed metadata/process primitives | NOT-IMPLEMENTED | C до зависимостей волны. |

## 13. Документы и подпись

Исторический immutable file store проверяет метаданные и поддерживает chunks/finalize. Для подписи сравнить хеш экземпляра, полномочия подписанта и итоговое состояние конверта. OCR и индекс должны перестраиваться из разрешённого экземпляра.

**Очередь:** 7. **Ответственность:** Document, File, адаптер подписи. **Критерий перехода:** Сохранить точную версию, запретить чтение чужого файла, связать подтверждение подписания с хешем и проверить повтор callback.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-core-files/src/lib.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C220 | secure file upload/download and malware scanning boundary | CODE-NOT-ACCEPTED | Legacy immutable file store поддерживает chunks/finalize. Для пользовательского вложения потребуются связь с записью, права и результат проверки файла. [postgres_file_artifact.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-core-data/tests/postgres_file_artifact.rs) |
| C221 | versioned documents and metadata | NOT-IMPLEMENTED | C до зависимостей волны. |
| C222 | folders/workspaces or typed document relationships | NOT-IMPLEMENTED | C до зависимостей волны. |
| C223 | document generation from governed templates | NOT-IMPLEMENTED | C до зависимостей волны. |
| C224 | merge fields with authorization-aware data access | NOT-IMPLEMENTED | C до зависимостей волны. |
| C225 | e-signature envelope, signer and status integration | NOT-IMPLEMENTED | C до зависимостей волны. |
| C226 | immutable signed-document references and evidence | NOT-IMPLEMENTED | Файловый артефакт доступен в прежнем store. Связать неизменяемый экземпляр с подписанным документом и подтверждением провайдера. |
| C227 | retention, legal hold and deletion policy interaction | NOT-IMPLEMENTED | C до зависимостей волны. |
| C228 | preview/search/OCR as rebuildable derived capabilities, not authoritative file mutation | NOT-IMPLEMENTED | C до зависимостей волны. |
| P443 | secure files and malware scanning | NOT-IMPLEMENTED | C до зависимостей волны. |
| P444 | versioned documents and relationships | NOT-IMPLEMENTED | C до зависимостей волны. |
| P445 | template-based generation and authorized merge fields | NOT-IMPLEMENTED | C до зависимостей волны. |
| P446 | e-signature envelope/signer/status integration | NOT-IMPLEMENTED | C до зависимостей волны. |
| P447 | immutable signed evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| P448 | privacy, retention and legal-hold interaction | NOT-IMPLEMENTED | C до зависимостей волны. |
| P449 | rebuildable preview, OCR and search | NOT-IMPLEMENTED | C до зависимостей волны. |

## 14. Аналитика

Аналитика сделки уже учитывает период, валюту и историю этапа в пределах M2-04. Для сквозной аналитики задания сайта, канала и продажи нужен стабильный ключ факта. Повтор, позднее событие и пересчёт должны дать ту же сумму и объяснимую атрибуцию.

**Очередь:** 1 → 7. **Ответственность:** Владелец определяет факт; отчёт — перестраиваемая проекция. **Критерий перехода:** Повторно построить показатель, сверить период, валюту и исходные записи. Позднее событие учитывается один раз.

Проверенная смежная граница: [sales_deals.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/sales_deals.rs). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7072127526)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C96 | revenue-related analytics without moving accounting ownership into CRM | NOT-IMPLEMENTED | C до зависимостей волны. |
| C236 | operational dashboards and KPIs | CODE-NOT-ACCEPTED | Есть Deal metrics / Customer360 и переходы к источникам. Для обобщённого отчёта нужны проекция, расчёт по периоду и сверка результата. |
| C237 | configurable reports with permission-aware semantic fields | NOT-IMPLEMENTED | C до зависимостей волны. |
| C238 | drill-down to live-authorized source resources | CODE-NOT-ACCEPTED | Есть Deal metrics / Customer360 и переходы к источникам. Для обобщённого отчёта нужны проекция, расчёт по периоду и сверка результата. |
| C239 | funnels, cohorts, retention and lifecycle analytics | CODE-NOT-ACCEPTED | Есть Deal metrics / Customer360 и переходы к источникам. Для обобщённого отчёта нужны проекция, расчёт по периоду и сверка результата. |
| C240 | sales pipeline and forecast analytics | ACCEPTED | M2-04: принятые показатели воронки и ограниченный прогноз. |
| C241 | service/SLA and workforce analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C242 | marketing attribution and campaign analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C243 | customer success and churn analytics | NOT-IMPLEMENTED | C до зависимостей волны. |
| C244 | territory, quota and performance scorecards | NOT-IMPLEMENTED | C до зависимостей волны. |
| C245 | scheduled report delivery with authorization re-checks | NOT-IMPLEMENTED | C до зависимостей волны. |
| C246 | export controls and data-class policy | NOT-IMPLEMENTED | C до зависимостей волны. |
| C247 | warehouse/lakehouse/BI integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C248 | metric definitions, lineage, freshness and reproducibility | CODE-NOT-ACCEPTED | Есть Deal metrics / Customer360 и переходы к источникам. Для обобщённого отчёта нужны проекция, расчёт по периоду и сверка результата. |
| C249 | no analytical projection becoming an undocumented system of record | CODE-NOT-ACCEPTED | Архитектурное ограничение действует в текущем Worker. Для планируемого редактора или проекции самостоятельно проверить соблюдение после подключения. |
| P455 | operational dashboards and KPIs | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P456 | permission-aware report builder | NOT-IMPLEMENTED | C до зависимостей волны. |
| P457 | semantic fields and reusable metrics | NOT-IMPLEMENTED | C до зависимостей волны. |
| P458 | drill-down to live-authorized records | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P459 | funnel, cohort, retention, forecast, SLA and performance analytics | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P460 | scheduled delivery with authorization re-check | NOT-IMPLEMENTED | C до зависимостей волны. |
| P461 | lineage, freshness and reproducibility | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P462 | governed export and warehouse/BI integration | NOT-IMPLEMENTED | C до зависимостей волны. |
| P463 | no analytical projection becoming a hidden system of record | NOT-IMPLEMENTED | C до зависимостей волны. |

## 15. Данные и интеграции

У малого импорта есть preview/apply и диагностика строки. В историческом источнике есть chunks, jobs и checkpoint. Проверить сначала действительные пределы малой операции, затем возобновление с последней подтверждённой строки на D1.

**Очередь:** 2. **Ответственность:** Customer Data Operations, адаптер источника и целевой owner. **Критерий перехода:** Проверить идемпотентный вход, привязку к tenant, checkpoint, повтор после таймаута, сверку с поставщиком.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-data-operations/src/lib.rs). [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7084460484)

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F15-01 | Импортировать простой CSV с двумя колонками или расширенный с четырьмя | ACCEPTED | N-01; preview, apply, построчные ошибки и оригинальный Party owner. |
| F15-02 | Выгрузить малый CSV Party | ACCEPTED | N-02; preview и существующая граница скачивания. |
| F15-03 | Выполнить двустороннюю синхронизацию по курсору с независимым провайдером | NOT-IMPLEMENTED | Согласовать приоритет источников, хранить курсор, повторить после сбоя без второго эффекта. |
| C304 | versioned bulk import/export | CODE-NOT-ACCEPTED | Малый CSV уже принят. Для больших файлов есть исторические import/export jobs, chunks, validation, checkpoint. Подключать при доказанном лимите; возобновление после сбоя не дублирует строку. |
| C305 | API and webhook platform | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C306 | inbound webhook verification and replay protection | NOT-IMPLEMENTED | C до зависимостей волны. |
| C307 | outbound delivery retry, reconciliation and dead-letter handling | NOT-IMPLEMENTED | C до зависимостей волны. |
| C308 | connector lifecycle, credentials and secret-handle boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C309 | mapping/transformation with versioned schemas | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C310 | sync cursors, conflict strategy and source-of-truth declarations | NOT-IMPLEMENTED | C до зависимостей волны. |
| C311 | data enrichment with provenance | CODE-NOT-ACCEPTED | В dev принята публикация конфигурации. Dispatch, review, materialization и provenance результата требуют отдельного исполнения с провайдером. [независимая приёмка](https://github.com/iamaman11/crm/issues/424#issuecomment-6905530278) |
| C312 | iPaaS/event-stream/warehouse integration boundaries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C313 | ERP, finance, payment, tax, identity, telephony, messaging and ad-platform adapters | NOT-IMPLEMENTED | C до зависимостей волны. |
| C314 | rate limits, quotas and tenant isolation for integration workloads | NOT-IMPLEMENTED | C до зависимостей волны. |
| P467 | versioned bulk import/export | CODE-NOT-ACCEPTED | Малый CSV уже принят. Для больших файлов есть исторические import/export jobs, chunks, validation, checkpoint. Подключать при доказанном лимите; возобновление после сбоя не дублирует строку. |
| P468 | public API and webhook platform | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P469 | verified inbound webhooks and replay protection | NOT-IMPLEMENTED | C до зависимостей волны. |
| P470 | reliable outbound delivery, retry, reconciliation and dead-letter handling | NOT-IMPLEMENTED | C до зависимостей волны. |
| P471 | connector credentials through secret handles | NOT-IMPLEMENTED | C до зависимостей волны. |
| P472 | mapping/transformation with versioned schemas | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P473 | sync cursors, conflict strategy and source-of-truth declarations | NOT-IMPLEMENTED | C до зависимостей волны. |
| P474 | ERP, finance, payment, tax, identity, telephony, messaging, ad and data-provider adapters | NOT-IMPLEMENTED | C до зависимостей волны. |
| P475 | tenant quotas and rate limits | NOT-IMPLEMENTED | C до зависимостей волны. |

## 16. Администрирование и metadata

Исторический Admin Studio имеет UI и тесты жизненного цикла metadata (apps/web/e2e/admin-studio.spec.ts). В dev первым нужен каталог участников tenant для назначения работы. Публикацию пользовательского поля переносить отдельно с проверкой старых данных и отката версии.

**Очередь:** 1 → 8. **Ответственность:** Членство и полномочия tenant; Metadata для версий настройки. **Критерий перехода:** Сотрудник видит только доступную команду, отзыв права применяется перед действием. Для настройки проверить publish и rollback версии.

Проверенная смежная граница: [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F16-01 | Прочитать список сотрудников своего tenant | NOT-IMPLEMENTED | Верификация актора существует, отдельной принятой операции каталога команды нет. |
| F16-02 | Предоставить сотруднику право выполнить ограниченную рабочую операцию | CODE-NOT-ACCEPTED | Access связывает сервис/актора с tenant и ролью. Для рабочего сценария расширить policy и независимо проверить право. |
| C48 | configurable customer attributes through governed metadata extension points | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C322 | custom objects and fields through governed typed schemas | CODE-NOT-ACCEPTED | MetadataRuntime хранит документы, проверяет зависимости и выпускает immutable revision. Нужна публикация и откат конкретной конфигурации для tenant в dev. [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs) |
| C323 | layouts, related lists, saved views and pipelines | CODE-NOT-ACCEPTED | Есть операции клиентского/сделочного ядра в каталоге ниже; для расширенного набора свойств и связей требуется собственная проверка. |
| C324 | validation rules and business process configuration | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C325 | permission templates and role/team administration | CODE-NOT-ACCEPTED | Есть типы субъекта/владельца, текущая политика или TaskOwner. Открыт полный путь сотрудника, назначения и чтения области доступа. |
| C326 | workflow authoring through governed actions | CODE-NOT-ACCEPTED | MetadataKind::Workflow существует в прежнем authoring runtime. Нет принятого редактора и исполнения графа в Cloudflare. [Metadata](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs#L23) |
| C327 | immutable publication, impact analysis, activation and rollback | CODE-NOT-ACCEPTED | MetadataRuntime хранит документы, проверяет зависимости и выпускает immutable revision. Нужна публикация и откат конкретной конфигурации для tenant в dev. [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs) |
| C328 | module install/upgrade/suspend/uninstall lifecycle | CODE-NOT-ACCEPTED | Сохранить доменную семантику из таблицы исторических операций и реализаций ниже; подтвердить полный результат на D1. |
| C329 | tenant settings, branding, localization and feature policy | NOT-IMPLEMENTED | C до зависимостей волны. |
| C330 | audit views and administrative diagnostics | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C331 | sandbox/test environment strategy and controlled promotion between environments | NOT-IMPLEMENTED | C до зависимостей волны. |
| C332 | no raw production JSON/SQL/script escape hatch for critical invariants | CODE-NOT-ACCEPTED | Архитектурное ограничение действует в текущем Worker. Для планируемого редактора или проекции самостоятельно проверить соблюдение после подключения. |
| P479 | custom objects and typed custom fields | CODE-NOT-ACCEPTED | MetadataRuntime хранит документы, проверяет зависимости и выпускает immutable revision. Нужна публикация и откат конкретной конфигурации для tenant в dev. [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs) |
| P480 | layouts, sections, related lists and conditional visibility | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P481 | pipelines, stages, forms and saved views | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P482 | validation and calculation rules | NOT-IMPLEMENTED | C до зависимостей волны. |
| P483 | permissions, roles, teams and templates | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P484 | workflow/process authoring | CODE-NOT-ACCEPTED | MetadataKind::Workflow существует в прежнем authoring runtime. Нет принятого редактора и исполнения графа в Cloudflare. [Metadata](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs#L23) |
| P485 | module activation and lifecycle | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P486 | tenant settings, branding and localization | NOT-IMPLEMENTED | C до зависимостей волны. |
| P487 | audit/diagnostics | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P488 | sandbox, package and controlled promotion | NOT-IMPLEMENTED | C до зависимостей волны. |
| P489 | impact analysis, publication and rollback | CODE-NOT-ACCEPTED | MetadataRuntime хранит документы, проверяет зависимости и выпускает immutable revision. Нужна публикация и откат конкретной конфигурации для tenant в dev. [lib.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-metadata-runtime/src/lib.rs) |

## 17. UX, мобильность, локализация

Даже типизированная навигация требует исчерпывающего выбора экрана. Для нового раздела проверять отсутствие fallback в другой экран. В offline-сценарии после возвращения сети требуется различать отклонённую команду, повтор и неподтверждённую запись.

**Очередь:** 1 → 8. **Ответственность:** React + generated client, серверные владельцы. **Критерий перехода:** Пройти Back/reload, клавиатуру, ошибку и восстановление; для offline проверить конфликт с новой серверной версией.

Проверенная смежная граница: [routes.ts](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/web/src/routes.ts).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F17-01 | Найти принятую запись по ссылке после Back/reload | ACCEPTED | M1-06, M2-02 и R1 в границе соответствующих экранов. |
| C75 | mobile and offline-capable field selling workflows where product tier requires them | NOT-IMPLEMENTED | C до зависимостей волны. |
| C281 | imports and guided onboarding | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C397 | responsive desktop and mobile web experience | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C398 | accessible navigation, forms, tables, dialogs and charts | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C399 | localization, locale-aware dates/numbers/currency and RTL readiness where required | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C400 | fast list, record, timeline and workspace experiences | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C401 | empty/loading/error/offline/retry states | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C402 | onboarding and contextual guidance | NOT-IMPLEMENTED | C до зависимостей волны. |
| C403 | mobile-native or installable/offline experience where target deployment requires it | NOT-IMPLEMENTED | C до зависимостей волны. |
| C404 | notification surfaces and deep links | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C405 | safe optimistic UX only where server authority remains explicit | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C406 | browser E2E against real governed application paths for critical workflows | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| P493 | responsive desktop/mobile web | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P494 | installable/offline-capable journeys where required | NOT-IMPLEMENTED | C до зависимостей волны. |
| P495 | accessibility across critical flows | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P496 | localization and locale-aware values | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P497 | fast records, lists, boards and search | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P498 | empty/loading/error/offline/retry states | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P499 | onboarding, contextual help and templates | NOT-IMPLEMENTED | C до зависимостей волны. |
| P500 | notifications and deep links | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P501 | command palette, keyboard shortcuts, recent items and favorites | NOT-IMPLEMENTED | C до зависимостей волны. |

## 18. AI

Схема Actor или ai.proto не обеспечивает вызов модели. Сначала дать AI доступ к одному разрешённому read, доказать отсутствие данных другого tenant и воспроизводимость источника. Команда с побочным эффектом проходит тот же owner и согласование.

**Очередь:** 8. **Ответственность:** AI actor, Retrieval, разрешённые инструменты. **Критерий перехода:** Проверить область чтения, отказ без разрешения, исполнение согласованной команды, стоимость и происхождение ответа.

Проверенная смежная граница: [ai.proto](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/ai.proto).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C362 | AI as an authenticated audited Actor | NOT-IMPLEMENTED | C до зависимостей волны. |
| C363 | tenant/purpose/data-class/residency/cost-aware model routing | NOT-IMPLEMENTED | C до зависимостей волны. |
| C364 | tool schemas generated from governed capability/query registries | NOT-IMPLEMENTED | C до зависимостей волны. |
| C365 | live authorization before retrieval and before every side effect | NOT-IMPLEMENTED | C до зависимостей волны. |
| C366 | permission-filtered retrieval and grounding | NOT-IMPLEMENTED | C до зависимостей волны. |
| C367 | summarization, drafting, classification, extraction and recommendation | NOT-IMPLEMENTED | C до зависимостей волны. |
| C368 | next-best-action and risk/opportunity suggestions with explanations | NOT-IMPLEMENTED | C до зависимостей волны. |
| C369 | duplicate/match suggestions without autonomous identity merge | NOT-IMPLEMENTED | C до зависимостей волны. |
| C370 | human approval for configured high-risk actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| C371 | prompt-injection, data-leakage, hallucination and tool-correctness evaluations | NOT-IMPLEMENTED | C до зависимостей волны. |
| C372 | budget, latency and provider-failure controls | NOT-IMPLEMENTED | C до зависимостей волны. |
| C373 | complete actor/tool/model/prompt-policy/cost audit evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| C374 | reversible actions where the underlying domain supports reversal | NOT-IMPLEMENTED | C до зависимостей волны. |
| P509 | tenant/purpose/data-class/residency/cost-aware model routing | NOT-IMPLEMENTED | C до зависимостей волны. |
| P510 | permission-filtered retrieval and grounding | NOT-IMPLEMENTED | C до зависимостей волны. |
| P511 | summarization, drafting, classification and extraction | NOT-IMPLEMENTED | C до зависимостей волны. |
| P512 | next-best-action, risk and opportunity recommendations with explanations | NOT-IMPLEMENTED | C до зависимостей волны. |
| P513 | conversational analytics and knowledge assistance | NOT-IMPLEMENTED | C до зависимостей волны. |
| P514 | governed tool schemas generated from capability/query registries | NOT-IMPLEMENTED | C до зависимостей волны. |
| P515 | human approval for configured high-risk actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P516 | prompt-injection/data-leakage/hallucination/tool-correctness evaluations | NOT-IMPLEMENTED | C до зависимостей волны. |
| P517 | budget, latency and provider-failure controls | NOT-IMPLEMENTED | C до зависимостей волны. |
| P518 | complete actor/model/prompt-policy/tool/cost audit evidence | NOT-IMPLEMENTED | C до зависимостей волны. |
| P519 | reversible action handling where the domain supports reversal | NOT-IMPLEMENTED | C до зависимостей волны. |

## 19. Marketplace и вертикали

ModuleRegistry умеет разрешать зависимости и изменять состояние установки; старый UI умеет изолировать упавший виджет. Для недоверенного пакета нужны проверка подписи до активации и отзыв грантов у исполняемого действия. Сам CRM Worker в WASM не является sandbox для таких пакетов.

**Очередь:** 8. **Ответственность:** Registry, издатель, sandbox и полномочия расширения. **Критерий перехода:** Проверить подпись, совместимость, лимит, отключение и отзыв права у запущенной операции.

Проверенная смежная граница: [registry.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-module-registry/src/registry.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| C382 | signed packages and publisher identity | NOT-IMPLEMENTED | C до зависимостей волны. |
| C383 | dependency and compatibility resolution | CODE-NOT-ACCEPTED | ModuleRegistry::resolve_dependencies учитывает версии и установленные модули. Переносить при первом подписанном расширении. [registry.rs#L190](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-module-registry/src/registry.rs#L190) |
| C384 | SBOM/provenance and vulnerability policy | NOT-IMPLEMENTED | CI проверяет зависимости самого приложения. Для устанавливаемого стороннего пакета требуется проверка происхождения и SBOM до активации. |
| C385 | explicit capability/data/network/secret grants | NOT-IMPLEMENTED | C до зависимостей волны. |
| C386 | sandboxed untrusted execution, planned as WASM | NOT-IMPLEMENTED | C до зависимостей волны. |
| C387 | quotas, timeouts and resource limits | NOT-IMPLEMENTED | Для указанной целевой операции промышленная реализация в активном и историческом доменном пути не выявлена. Согласовать целевой предел и проверить отдельным сценарием. |
| C388 | install/upgrade/rollback/suspend/uninstall evidence | CODE-NOT-ACCEPTED | Есть переходы установки, активации, остановки и отката, а также тесты registry. Проверить отключение запущенного расширения в реальном Worker. [registry.rs#L203](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-module-registry/src/registry.rs#L203) |
| C389 | kill switch and emergency revocation | NOT-IMPLEMENTED | C до зависимостей волны. |
| C390 | marketplace UI extensions through bounded host-owned context | CODE-NOT-ACCEPTED | Исторический record page поддерживает typed extension slots и изоляцию render/load failures. Для Cloudflare нужен запуск с ограниченными правами. [Playwright](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/web/e2e/ui-extensions.spec.ts) |
| C391 | extension failure isolation and no infrastructure bypass | CODE-NOT-ACCEPTED | Исторический record page поддерживает typed extension slots и изоляцию render/load failures. Для Cloudflare нужен запуск с ограниченными правами. [Playwright](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/web/e2e/ui-extensions.spec.ts) |
| P278 | typed custom functions compiled or sandboxed under resource limits | NOT-IMPLEMENTED | C до зависимостей волны. |
| P279 | no raw database connectivity | NOT-IMPLEMENTED | C до зависимостей волны. |
| P280 | no unrestricted network or secret access | NOT-IMPLEMENTED | C до зависимостей волны. |
| P281 | declared input/output schemas | NOT-IMPLEMENTED | C до зависимостей волны. |
| P282 | declared data classes and permissions | NOT-IMPLEMENTED | C до зависимостей волны. |
| P283 | deterministic timeout, memory and CPU quotas | NOT-IMPLEMENTED | C до зависимостей волны. |
| P284 | signed package/version identity | NOT-IMPLEMENTED | C до зависимостей волны. |
| P285 | test fixtures and contract compatibility | NOT-IMPLEMENTED | C до зависимостей волны. |
| P286 | complete execution audit | NOT-IMPLEMENTED | C до зависимостей волны. |
| P287 | kill switch and revocation | NOT-IMPLEMENTED | C до зависимостей волны. |
| P525 | signed packages and publisher identity | NOT-IMPLEMENTED | C до зависимостей волны. |
| P526 | dependency/compatibility resolution | CODE-NOT-ACCEPTED | ModuleRegistry::resolve_dependencies учитывает версии и установленные модули. Переносить при первом подписанном расширении. [registry.rs#L190](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-module-registry/src/registry.rs#L190) |
| P527 | explicit data, capability, UI, network and secret grants | NOT-IMPLEMENTED | C до зависимостей волны. |
| P528 | sandboxed untrusted execution | NOT-IMPLEMENTED | C до зависимостей волны. |
| P529 | resource quotas and timeouts | NOT-IMPLEMENTED | C до зависимостей волны. |
| P530 | install, upgrade, rollback, suspend and uninstall evidence | CODE-NOT-ACCEPTED | Есть переходы установки, активации, остановки и отката, а также тесты registry. Проверить отключение запущенного расширения в реальном Worker. [registry.rs#L203](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-module-registry/src/registry.rs#L203) |
| P531 | emergency kill switch and revocation | NOT-IMPLEMENTED | C до зависимостей волны. |
| P532 | bounded UI extensions using host-owned context | CODE-NOT-ACCEPTED | Исторический record page поддерживает typed extension slots и изоляцию render/load failures. Для Cloudflare нужен запуск с ограниченными правами. [Playwright](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/web/e2e/ui-extensions.spec.ts) |
| P533 | connector and workflow action packages | NOT-IMPLEMENTED | C до зависимостей волны. |
| P534 | vertical packages using the same owner/security/audit rules | NOT-IMPLEMENTED | C до зависимостей волны. |
| P535 | extension failure isolation and no infrastructure bypass | CODE-NOT-ACCEPTED | Исторический record page поддерживает typed extension slots и изоляцию render/load failures. Для Cloudflare нужен запуск с ограниченными правами. [Playwright](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/web/e2e/ui-extensions.spec.ts) |

## 20. Enterprise и эксплуатация

Исторический PostgreSQL restore и ограничения конфиденциальности дают сценарии отказа. В D1 нужны отдельное восстановление, проверка свернутого scope, сроки хранения и измерение фактической потери данных/времени. Документационный commit не меняет опубликованную версию.

**Очередь:** 1 → 8. **Ответственность:** Access, privacy owners, операторы релиза и восстановления. **Критерий перехода:** До реальных данных доказать privacy и восстановление D1 в изоляции, измерить RPO/RTO, сверить аудит, доступ и видимость.

Проверенная смежная граница: [access.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/access.rs).

| ID | Операция или требование к ней | Статус | Решение / граница |
|---|---|---|---|
| F20-01 | Положительный вход человека с реальной Access identity | CODE-NOT-ACCEPTED | Проверяются подпись и tenant. Принятый live R1 использовал service admin. Требуется отдельный вход человека. |
| F20-02 | Восстановить исход Task после неопределённой ошибки D1 | CODE-NOT-ACCEPTED | Текущий адаптер может вернуть конфликт при ошибке записи. Проверить receipt, повтор и audit согласно #485. |
| C340 | authentication with OIDC/SAML where required | CODE-NOT-ACCEPTED | Есть типы субъекта/владельца, текущая политика или TaskOwner. Открыт полный путь сотрудника, назначения и чтения области доступа. |
| C341 | SCIM provisioning and deprovisioning | NOT-IMPLEMENTED | C до зависимостей волны. |
| C342 | tenant, organization, team, role and resource authorization | CODE-NOT-ACCEPTED | Есть типы субъекта/владельца, текущая политика или TaskOwner. Открыт полный путь сотрудника, назначения и чтения области доступа. |
| C343 | field-level visibility and masking | NOT-IMPLEMENTED | C до зависимостей волны. |
| C344 | delegated administration and separation of duties | NOT-IMPLEMENTED | C до зависимостей волны. |
| C345 | approval policies for high-risk actions | NOT-IMPLEMENTED | C до зависимостей волны. |
| C346 | tenant key hierarchy and field/data-class encryption where required | NOT-IMPLEMENTED | C до зависимостей волны. |
| C347 | immutable audit evidence and WORM export | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C348 | consent and communication authorization enforcement | CODE-NOT-ACCEPTED | Доступны GrantVerifiedEmailConsent, WithdrawConsentAuthorization, SuppressCommunication и EvaluateConsent. На границе реальной доставки потребуется повторить проверку согласия перед отправкой. |
| C349 | privacy access/export/deletion/restriction/legal-hold workflows | CODE-NOT-ACCEPTED | Существуют legacy owner action, hold/restriction и access export. В dev приняты intake и отмена; positive human verification, frozen scope и полный экспорт проверяются отдельно. [access_export.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/tests/access_export.rs) |
| C350 | data residency and tenant mobility | NOT-IMPLEMENTED | Для указанной целевой операции промышленная реализация в активном и историческом доменном пути не выявлена. Согласовать целевой предел и проверить отдельным сценарием. |
| C351 | backup, PITR, tenant restore and disaster recovery | CODE-NOT-ACCEPTED | Исторические операции восстановления касаются PostgreSQL. Тест выпуска D1 не выполняет tenant restore. Принять на отдельном восстановленном экземпляре с измеренными RPO/RTO. [customer_privacy_operations.py](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/scripts/customer_privacy_operations.py) |
| C352 | retention and crypto-shredding strategies | NOT-IMPLEMENTED | C до зависимостей волны. |
| C353 | security scans, SBOM, provenance, penetration and abuse testing | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| C354 | SLOs, alerting, incident response and operational runbooks | CODE-NOT-ACCEPTED | Есть часть в действующем клиенте/CI либо историческом product-plane. Полный перечисленный охват и эксплуатационные пределы отдельно не приняты. |
| P541 | OIDC/SAML and SCIM where required | CODE-NOT-ACCEPTED | Часть поведения реализована в типах Sales/Activities, runtime доступа, ModuleRegistry либо штатной поставке. Принятая граница и недостающее развитие разделены в каталогах ниже. |
| P542 | resource/field-level authorization, masking and separation of duties | CODE-NOT-ACCEPTED | Часть поведения реализована в типах Sales/Activities, runtime доступа, ModuleRegistry либо штатной поставке. Принятая граница и недостающее развитие разделены в каталогах ниже. |
| P543 | tenant key hierarchy, encryption and residency controls | NOT-IMPLEMENTED | C до зависимостей волны. |
| P544 | immutable audit/WORM export | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P545 | backup, PITR, tenant restore and disaster recovery | CODE-NOT-ACCEPTED | Исторические операции восстановления касаются PostgreSQL. Тест выпуска D1 не выполняет tenant restore. Принять на отдельном восстановленном экземпляре с измеренными RPO/RTO. [customer_privacy_operations.py](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/scripts/customer_privacy_operations.py) |
| P546 | defined RPO/RTO and tested runbooks | CODE-NOT-ACCEPTED | Исторические операции восстановления касаются PostgreSQL. Тест выпуска D1 не выполняет tenant restore. Принять на отдельном восстановленном экземпляре с измеренными RPO/RTO. [customer_privacy_operations.py](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/scripts/customer_privacy_operations.py) |
| P547 | SLOs, alerting, capacity and incident response | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P548 | security scans, SBOM, provenance, penetration and abuse testing | CODE-NOT-ACCEPTED | Расширить действующую границу семейства; подтвердить полный указанный результат. |
| P549 | upgrade, rollback and data migration safety | CODE-NOT-ACCEPTED | Часть поведения реализована в типах Sales/Activities, runtime доступа, ModuleRegistry либо штатной поставке. Принятая граница и недостающее развитие разделены в каталогах ниже. |
| P550 | high-volume and large-tenant performance profiles | NOT-IMPLEMENTED | Для указанной целевой операции промышленная реализация в активном и историческом доменном пути не выявлена. Согласовать целевой предел и проверить отдельным сценарием. |
| P551 | deployment topology and operational support policy | CODE-NOT-ACCEPTED | У Worker есть штатная поставка и fail-closed при ошибке; эксплуатационная поддержка коммерческой среды требует отдельных критериев. |
| P552 | representative vertical packages proving governed extensibility | NOT-IMPLEMENTED | C до зависимостей волны. |

## Действующий Worker — проверка по каждому RPC

Таблица ниже сопоставляет exact binding из module.yaml с исполняемой реализацией и текущим runtime-отчётом. У каждой строки был успешный подписанный вызов и отказ анонимному запросу. ACCEPTED относится только к указанному сценарию; для конфигурации enrichment и intake privacy учтены независимые #424 и #423. Для остальных действий без точной привязки независимого результата сохранён CODE-NOT-ACCEPTED. Верификацию человеческой identity учитывать отдельно.

Исполненные проверки доменной границы и браузера: [native.mjs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/test/native.mjs) / [typed-browser.mjs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/test/typed-browser.mjs).

### crm.activities

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/module.yaml) · [task_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CompleteTask](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs#L194) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [CreateTask](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs#L7) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [GetTask](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs#L234) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [ListTasks](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs#L266) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [ScheduleReminder](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs#L148) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [UpdateTask](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/task_handlers.rs#L69) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |

### crm.consents

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-consents/module.yaml) · [consent_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [EvaluateConsent](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs#L7) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213) |
| [GetConsentAuthorization](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs#L152) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213) |
| [GrantVerifiedEmailConsent](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs#L85) | SelfWrite | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213) |
| [ListConsentAuthorizations](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs#L198) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213) |
| [SuppressCommunication](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs#L47) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213) |
| [WithdrawConsentAuthorization](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/consent_handlers.rs#L121) | SelfWrite | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081939213) |

### crm.contact-points

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-contact-points/module.yaml) · [contact_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CreateContactPoint](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs#L7) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [GetContactPoint](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs#L133) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [ListContactPoints](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs#L165) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [UpdateContactPoint](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs#L60) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [VerifyOwnEmailContactPoint](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/contact_handlers.rs#L107) | SelfWrite | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |

### crm.customer-accounts

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-accounts/module.yaml) · [account_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CreateAccount](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs#L7) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [GetAccount](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs#L75) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [ListAccounts](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs#L108) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [UpdateAccount](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/account_handlers.rs#L36) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |

### crm.customer-data-operations

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-data-operations/module.yaml) · [data_operations_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [ApplyPartyImport](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs#L114) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7084460484) |
| [PreviewPartyExport](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs#L20) | AdminRead | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7079751195) |
| [PreviewPartyImport](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/data_operations_handlers.rs#L56) | AdminRead | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7084460484) |

### crm.customer-enrichment

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/module.yaml) · [enrichment_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [GetMapping](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs#L242) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [GetProviderProfile](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs#L189) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [ListMappings](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs#L317) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [ListProviderProfiles](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs#L293) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [PublishMapping](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs#L163) | Write | ACCEPTED · [независимая приёмка](https://github.com/iamaman11/crm/issues/424#issuecomment-6905530278) |
| [PublishProviderProfile](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/enrichment_handlers.rs#L122) | Write | ACCEPTED · [независимая приёмка](https://github.com/iamaman11/crm/issues/424#issuecomment-6905530278) |

### crm.customer-privacy

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/module.yaml) · [privacy_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CancelPrivacyCase](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L236) | Write | ACCEPTED · [независимая приёмка](https://github.com/iamaman11/crm/issues/352#issuecomment-6905077570) |
| [CreatePrivacyCase](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L141) | Write | ACCEPTED · [независимая приёмка](https://github.com/iamaman11/crm/issues/352#issuecomment-6905077570) |
| [GetPrivacyCase](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L320) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [ListPrivacyCases](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L359) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [PreviewOwnPrivacyScope](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L256) | Read | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |
| [SubmitPrivacyCase](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L182) | Write | ACCEPTED · [независимая приёмка](https://github.com/iamaman11/crm/issues/352#issuecomment-6905077570) |
| [VerifyOwnPrivacyCaseSubject](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/privacy_handlers.rs#L202) | SelfWrite | CODE-NOT-ACCEPTED · workerd PASS; проверить независимое доказательство для этой границы |

### crm.customer360

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-360/module.yaml) · [overview_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/overview_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [GetAccountOverview](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/overview_handlers.rs#L12) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7079988713) |

### crm.data-quality

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-data-quality/module.yaml) · [quality_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/quality_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [EvaluatePartyName](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/quality_handlers.rs#L7) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7080576593) |

### crm.identity-resolution

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-identity-resolution/module.yaml) · [identity_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CompareParties](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L123) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081074390) |
| [FindSharedEmailCandidates](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L51) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7081074390) |
| [GetMergeOperation](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L242) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7083505965) |
| [ListMergeOperationsByParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L281) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7083505965) |
| [MergeParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L156) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7083505965) |
| [ResolveCanonicalParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L327) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7083505965) |
| [UnmergeParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/identity_handlers.rs#L199) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7083505965) |

### crm.parties

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-parties/module.yaml) · [party_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CreateParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs#L7) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [GetParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs#L83) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [ListParties](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs#L116) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [UpdateParty](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/party_handlers.rs#L47) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |

### crm.party-relationships

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-party-relationships/module.yaml) · [relationship_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [CreatePartyRelationship](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs#L7) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [GetPartyRelationship](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs#L107) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [ListPartyRelationships](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs#L143) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |
| [UpdatePartyRelationship](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/relationship_handlers.rs#L64) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7069314351) |

### crm.sales

[module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/module.yaml) · [sales_handlers.rs](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs)

| Типизированное действие | Область доступа | Статус и доказательство |
|---|---|---|
| [AdvanceStage](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L771) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [CloseDeal](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L825) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [CreateDeal](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L590) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [CreatePipeline](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L179) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677) |
| [CreateSalesView](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L8) | Write | ACCEPTED · [crm-functional-execution-matrix.md#L135](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#L135) |
| [GetDeal](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L1033) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [GetPipeline](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L241) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677) |
| [GetPipelineMetrics](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L426) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7072127526) |
| [GetSalesView](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L88) | Read | ACCEPTED · [crm-functional-execution-matrix.md#L135](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#L135) |
| [ListDealEvents](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L918) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [ListDeals](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L1077) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [ListPipelines](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L295) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677) |
| [ListSalesViews](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L142) | Read | ACCEPTED · [crm-functional-execution-matrix.md#L135](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#L135) |
| [ListStageDeals](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L344) | Read | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677) |
| [ReopenDeal](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L875) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [UpdateDeal](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L661) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7070842108) |
| [UpdatePipeline](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L209) | Write | ACCEPTED · [Gate A/B](https://github.com/iamaman11/crm/issues/352#issuecomment-7071938677) |
| [UpdateSalesView](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/apps/cloudflare-crm/rust-party/src/connect_sales/sales_handlers.rs#L47) | Write | ACCEPTED · [crm-functional-execution-matrix.md#L135](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#L135) |

## Исторические операции — сопоставление 120 прежних привязок с dev

192 объявления в манифестах включают 72 текущих. Ниже перечислены остальные 120. Одноимённый новый метод показывает кандидата на эквивалентность исключительно в границах принятой операции выше. Прежний формат транспорта не требуется публиковать заново. Для исторической семантики с более широкими правами, источниками или объёмом действует отдельный критерий принятия.

Проверена неизменность дерева crates относительно a627e6a, на котором составлены [legacy-crate-operation-evidence.md](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/legacy-crate-operation-evidence.md) и [crm-functional-execution-matrix.md#L777](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#L777). Учёт охватывает 93 исторических crate и 532 Rust-файла. X-01 остаётся обязательной проверкой потребителей перед удалением.


### crm.activities · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.activities.v1.TaskService/CompleteTask` | [domain.rs#L112](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L112) · [postgres_noop.rs#L70](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-activities-capability-adapter/tests/postgres_noop.rs#L70) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-08](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-08) |
| `/crm.activities.v1.TaskService/CreateTask` | [domain.rs#L88](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L88) · [postgres_noop.rs#L45](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-activities-capability-adapter/tests/postgres_noop.rs#L45) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-08](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-08) |
| `/crm.activities.v1.TaskService/GetTask` | [executor.rs#L89](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-query-adapter/src/executor.rs#L89) · [postgres_queries.rs#L408](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-capability-composition/tests/postgres_queries.rs#L408) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-08](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-08) |
| `/crm.activities.v1.TaskService/ListTasks` | [executor.rs#L94](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-query-adapter/src/executor.rs#L94) · [postgres_queries.rs#L426](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-capability-composition/tests/postgres_queries.rs#L426) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-08](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-08) |
| `/crm.activities.v1.TaskService/ScheduleReminder` | [domain.rs#L118](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L118) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-08](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-08) |
| `/crm.activities.v1.TaskService/UpdateTask` | [domain.rs#L101](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-activities/src/domain.rs#L101) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-08](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-08) |

### crm.consents · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-consents/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.consents.v1.ConsentAuthorizationService/AuthorizeCommunication` | [lib.rs#L193](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-consents-query-adapter/src/lib.rs#L193) · [consent_process_e2e.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L37) | Сверить с принятым сценарием N-06; использовать новую версию контракта в границах её возможностей. [N-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#n-06) |
| `/crm.consents.v1.ConsentAuthorizationService/CreateConsentAuthorization` | [domain.rs#L228](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-consents/src/domain.rs#L228) · [consent_process_e2e.rs#L33](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L33) | Сверить с принятым сценарием N-06; использовать новую версию контракта в границах её возможностей. [N-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#n-06) |
| `/crm.consents.v1.ConsentAuthorizationService/GetConsentAuthorization` | [consent_process_e2e.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L35) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [N-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#n-06) |
| `/crm.consents.v1.ConsentAuthorizationService/ListConsentAuthorizations` | [consent_process_e2e.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L36) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [N-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#n-06) |
| `/crm.consents.v1.ConsentAuthorizationService/WithdrawConsentAuthorization` | [domain.rs#L245](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-consents/src/domain.rs#L245) · [consent_process_e2e.rs#L34](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L34) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [N-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#n-06) |
| `/crm.customer_privacy.v1.ConsentsPrivacyScopeContributionService/ConsentsPrivacyScopeContribution` | [scope.rs#L244](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L244) · [postgres_scope.rs#L99](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-consents-privacy-scope-adapter/tests/postgres_scope.rs#L99) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |

### crm.contact-points · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-contact-points/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.contact_points.v1.ContactPointService/CreateContactPoint` | [domain.rs#L143](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-contact-points/src/domain.rs#L143) · [consent_process_e2e.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L31) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-04) |
| `/crm.contact_points.v1.ContactPointService/GetContactPoint` | [lib.rs#L25](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-contact-points-query-adapter/src/lib.rs#L25) · [contact_point_process_e2e.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/contact_point_process_e2e.rs#L35) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-04) |
| `/crm.contact_points.v1.ContactPointService/ListContactPoints` | [lib.rs#L26](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-contact-points-query-adapter/src/lib.rs#L26) · [contact_point_process_e2e.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/contact_point_process_e2e.rs#L36) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-04) |
| `/crm.contact_points.v1.ContactPointService/UpdateContactPoint` | [domain.rs#L155](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-contact-points/src/domain.rs#L155) · [customer_360_process_e2e.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_360_process_e2e.rs#L37) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-04) |
| `/crm.contact_points.v1.ContactPointService/VerifyContactPoint` | [domain.rs#L166](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-contact-points/src/domain.rs#L166) · [consent_process_e2e.rs#L32](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/consent_process_e2e.rs#L32) | Сверить с принятым сценарием M1-04; использовать новую версию контракта в границах её возможностей. [M1-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-04) |
| `/crm.customer_privacy.v1.ContactPointsPrivacyScopeContributionService/ContactPointsPrivacyScopeContribution` | [scope.rs#L238](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L238) · [postgres_scope.rs#L175](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-contact-points-privacy-scope-adapter/tests/postgres_scope.rs#L175) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |

### crm.customer-accounts · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-accounts/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.accounts.v1.AccountService/CreateAccount` | [domain.rs#L102](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-accounts/src/domain.rs#L102) · [account_process_e2e.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/account_process_e2e.rs#L31) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-03) |
| `/crm.accounts.v1.AccountService/GetAccount` | [account_process_e2e.rs#L33](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/account_process_e2e.rs#L33) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-03) |
| `/crm.accounts.v1.AccountService/ListAccounts` | [account_process_e2e.rs#L34](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/account_process_e2e.rs#L34) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-03) |
| `/crm.accounts.v1.AccountService/UpdateAccount` | [domain.rs#L110](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-accounts/src/domain.rs#L110) · [account_process_e2e.rs#L32](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/account_process_e2e.rs#L32) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-03) |
| `/crm.customer_privacy.v1.CustomerAccountsPrivacyScopeContributionService/CustomerAccountsPrivacyScopeContribution` | [scope.rs#L234](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L234) · [postgres_scope.rs#L133](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-accounts-privacy-scope-adapter/tests/postgres_scope.rs#L133) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |

### crm.customer-data-operations · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-data-operations/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_data_operations.v1.CustomerDataExportService/CancelPartyExportJob` | [export_planner.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/export_planner.rs#L29) · [export_process_e2e.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/export_process_e2e.rs#L36) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-02) |
| `/crm.customer_data_operations.v1.CustomerDataExportService/CreatePartyExportJob` | [export_planner.rs#L26](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/export_planner.rs#L26) · [export_process_e2e.rs#L34](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/export_process_e2e.rs#L34) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-02) |
| `/crm.customer_data_operations.v1.CustomerDataExportService/GetPartyExportJob` | [export_process_e2e.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/export_process_e2e.rs#L37) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-02) |
| `/crm.customer_data_operations.v1.CustomerDataExportService/ListPartyExportJobs` | [export_query.rs#L23](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-query-adapter/src/export_query.rs#L23) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-02) |
| `/crm.customer_data_operations.v1.CustomerDataExportService/StartPartyExportExecution` | [export_planner.rs#L28](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/export_planner.rs#L28) · [export_process_e2e.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/export_process_e2e.rs#L35) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-02) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/CancelPartyImportJob` | [planner.rs#L112](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/planner.rs#L112) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/CreatePartyImportJob` | [planner.rs#L60](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/planner.rs#L60) · [import_process_e2e.rs#L394](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L394) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/FinalizePartyImportValidation` | [planner.rs#L86](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/planner.rs#L86) · [import_process_e2e.rs#L32](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L32) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/GetPartyImportJob` | [import_process_e2e.rs#L34](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L34) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/ListPartyImportJobs` | [import_process_e2e.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L35) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/ListPartyImportRows` | [import_process_e2e.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L36) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/StartPartyImportExecution` | [planner.rs#L99](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/planner.rs#L99) · [import_process_e2e.rs#L33](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L33) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsService/ValidatePartyImportRows` | [planner.rs#L73](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-capability-adapter/src/planner.rs#L73) · [source_capability_surface.rs#L41](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-source-composition/tests/source_capability_surface.rs#L41) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsSourceService/AppendPartyImportSourceChunk` | [import_process_e2e.rs#L28](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L28) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsSourceService/CreatePartyImportJobFromSourceArtifact` | [import_process_e2e.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L30) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsSourceService/CreatePartyImportSourceArtifact` | [import_process_e2e.rs#L27](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L27) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsSourceService/FinalizePartyImportSourceArtifact` | [import_process_e2e.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L29) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_data_operations.v1.CustomerDataOperationsSourceService/ValidatePartyImportSourceBatch` | [import_process_e2e.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/import_process_e2e.rs#L31) | C · при превышении малых CSV-лимитов перенести эту стадию job; доказать возобновление со старого checkpoint. [L-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-01) |
| `/crm.customer_privacy.v1.CustomerDataPrivacyScopeContributionService/CustomerDataPrivacyScopeContribution` | [scope.rs#L251](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L251) · [postgres_scope.rs#L323](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-data-operations-privacy-scope-adapter/tests/postgres_scope.rs#L323) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |

### crm.customer-enrichment · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_enrichment.v1.CustomerEnrichmentDefinitionService/GetMappingVersion` | [acceptance.rs#L45](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L45) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentDefinitionService/GetProviderProfileVersion` | [acceptance.rs#L44](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L44) · [customer_enrichment_process_e2e.rs#L14](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_enrichment_process_e2e.rs#L14) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentDefinitionService/PublishMappingVersion` | [mapping_planner.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/src/mapping_planner.rs#L31) · [acceptance.rs#L34](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L34) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentDefinitionService/PublishProviderProfileVersion` | [provider_profile_planner.rs#L32](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/src/provider_profile_planner.rs#L32) · [acceptance.rs#L33](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L33) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentRequestService/CancelEnrichmentRequest` | [request_cancel_planner.rs#L19](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/src/request_cancel_planner.rs#L19) · [acceptance.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L36) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentRequestService/CreateEnrichmentRequest` | [request_planner.rs#L22](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/src/request_planner.rs#L22) · [request_create_planner.rs#L25](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/tests/request_create_planner.rs#L25) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentRequestService/GetEnrichmentRequest` | [acceptance.rs#L46](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L46) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentRequestService/ListEnrichmentRequests` | [acceptance.rs#L47](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L47) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService/AcceptSuggestion` | [acceptance.rs#L38](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L38) · [mod.rs#L213](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-review-composition/tests/support/mod.rs#L213) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-06) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService/ApplyPartyDisplayNameSuggestion` | [acceptance.rs#L58](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L58) · [postgres_application_process.rs#L82](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-application-composition/tests/postgres_application_process.rs#L82) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-07) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService/GetSuggestion` | [acceptance.rs#L48](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L48) · [postgres_review_process.rs#L180](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-review-composition/tests/postgres_review_process.rs#L180) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-06) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService/ListSuggestionsByParty` | [acceptance.rs#L49](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L49) · [postgres_review_process.rs#L212](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-review-composition/tests/postgres_review_process.rs#L212) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-06) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService/RejectSuggestion` | [acceptance.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L37) · [suggestion_review_atomic_plan.rs#L88](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-review-adapter/tests/suggestion_review_atomic_plan.rs#L88) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-06](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-06) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentWorkerService/DispatchEnrichmentRequest` | [request_dispatch_planner.rs#L96](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/src/request_dispatch_planner.rs#L96) · [acceptance.rs#L55](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L55) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentWorkerService/MaterializeSuggestions` | [acceptance.rs#L57](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L57) · [postgres_materialization_process.rs#L135](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_process.rs#L135) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-07) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentWorkerService/RecordApplicationOutcome` | [acceptance.rs#L59](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L59) · [postgres_application_process.rs#L136](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-application-composition/tests/postgres_application_process.rs#L136) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-07) |
| `/crm.customer_enrichment.v1.CustomerEnrichmentWorkerService/RecordProviderResponse` | [request_reference_planner.rs#L157](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-capability-adapter/src/request_reference_planner.rs#L157) · [acceptance.rs#L56](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L56) | C · подключать вместе с реальным источником; принять provenance и один эффект при повторе. [L-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-05) |
| `/crm.customer_privacy.v1.CustomerEnrichmentPrivacyScopeContributionService/CustomerEnrichmentPrivacyScopeContribution` | [acceptance.rs#L92](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-enrichment/tests/acceptance.rs#L92) · [postgres_scope.rs#L929](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-enrichment-privacy-scope-adapter/tests/postgres_scope.rs#L929) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |

### crm.customer-privacy · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/ApprovePrivacyCase` | [lib.rs#L133](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-application/src/lib.rs#L133) · [customer_privacy_case_approval_process_e2e.rs#L26](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_case_approval_process_e2e.rs#L26) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/CancelPrivacyCase` | [planner.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-cancel-capability-adapter/tests/planner.rs#L35) · [customer_privacy_case_list_process_e2e.rs#L527](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_case_list_process_e2e.rs#L527) | C · текущая узкая операция есть; сверить полноту и независимую приёмку. [E-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-01) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/CreatePrivacyCase` | [seed_e2e_fixture.rs#L41](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/seed_e2e_fixture.rs#L41) · [generic_conformance_process_e2e.rs#L41](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/generic_conformance_process_e2e.rs#L41) | C · текущая узкая операция есть; сверить полноту и независимую приёмку. [E-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-01) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/GetPrivacyActionPlan` | [scope_planning.rs#L6](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope_planning.rs#L6) · [customer_privacy_production_inventory.rs#L22](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-application-runtime/tests/customer_privacy_production_inventory.rs#L22) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/GetPrivacyCase` | [lib.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-query-adapter/src/lib.rs#L31) · [customer_privacy_case_get_process_e2e.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_case_get_process_e2e.rs#L29) | C · текущая узкая операция есть; сверить полноту и независимую приёмку. [E-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-01) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/ListPrivacyCases` | [seed_e2e_fixture.rs#L44](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/seed_e2e_fixture.rs#L44) · [generic_conformance_process_e2e.rs#L43](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/generic_conformance_process_e2e.rs#L43) | C · текущая узкая операция есть; сверить полноту и независимую приёмку. [E-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-01) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/ListPrivacyOwnerOutcomes` | [scope_planning.rs#L9](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope_planning.rs#L9) · [customer_privacy_contract.rs#L99](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-proto-contracts/tests/customer_privacy_contract.rs#L99) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-05) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/SubmitPrivacyCase` | [seed_e2e_fixture.rs#L42](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/seed_e2e_fixture.rs#L42) · [generic_conformance_process_e2e.rs#L42](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/generic_conformance_process_e2e.rs#L42) | C · текущая узкая операция есть; сверить полноту и независимую приёмку. [E-01](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-01) |
| `/crm.customer_privacy.v1.CustomerPrivacyCaseService/VerifyPrivacyCaseSubject` | [planner.rs#L240](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-subject-capability-adapter/tests/planner.rs#L240) · [seed_e2e_fixture.rs#L43](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/seed_e2e_fixture.rs#L43) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-05](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-05) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/GetCustomerDataLegalHold` | [controls.rs#L32](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-query-adapter/src/controls.rs#L32) · [customer_privacy_hold_retention_process_e2e.rs#L25](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_hold_retention_process_e2e.rs#L25) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/GetProcessingRestriction` | [controls.rs#L27](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-query-adapter/src/controls.rs#L27) · [customer_privacy_restriction_process_e2e.rs#L27](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_restriction_process_e2e.rs#L27) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/ListCustomerDataLegalHoldsBySubject` | [controls.rs#L38](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-query-adapter/src/controls.rs#L38) · [customer_privacy_hold_retention_process_e2e.rs#L26](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_hold_retention_process_e2e.rs#L26) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/PlaceCustomerDataLegalHold` | [lib.rs#L38](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-application/src/lib.rs#L38) · [customer_privacy_hold_retention_process_e2e.rs#L23](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_hold_retention_process_e2e.rs#L23) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/PlaceProcessingRestriction` | [lib.rs#L39](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-application/src/lib.rs#L39) · [customer_privacy_restriction_process_e2e.rs#L25](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_restriction_process_e2e.rs#L25) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/ReleaseCustomerDataLegalHold` | [lib.rs#L40](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-application/src/lib.rs#L40) · [customer_privacy_hold_retention_process_e2e.rs#L24](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_hold_retention_process_e2e.rs#L24) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |
| `/crm.customer_privacy.v1.CustomerPrivacyControlService/ReleaseProcessingRestriction` | [lib.rs#L41](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-privacy-application/src/lib.rs#L41) · [customer_privacy_restriction_process_e2e.rs#L26](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_privacy_restriction_process_e2e.rs#L26) | B до работы с реальными данными · перенести семантику владельца; проверить отказ и аудит. [E-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-03) |

### crm.customer360 · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-360/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_360.v1.Customer360Service/GetCustomer360` | [postgres.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-customer-360-query-adapter/src/postgres.rs#L36) · [customer_360_process_e2e.rs#L41](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_360_process_e2e.rs#L41) | Сверить с принятым сценарием N-03; использовать новую версию контракта в границах её возможностей. [N-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#n-03) |

### crm.data-quality · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-data-quality/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_privacy.v1.DataQualityPrivacyScopeContributionService/DataQualityPrivacyScopeContribution` | [scope.rs#L253](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L253) · [postgres_scope.rs#L812](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-privacy-scope-adapter/tests/postgres_scope.rs#L812) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |
| `/crm.data_quality.v1.DataQualityDefinitionService/GetPartyCompletenessProfileVersion` | [data_quality_query_process_e2e.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_query_process_e2e.rs#L31) · [data_quality_field_redaction_process_e2e.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_field_redaction_process_e2e.rs#L31) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityDefinitionService/GetPartyRuleSetVersion` | [data_quality_query_process_e2e.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_query_process_e2e.rs#L30) · [data_quality_field_redaction_process_e2e.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_field_redaction_process_e2e.rs#L30) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityDefinitionService/PublishPartyCompletenessProfileVersion` | [completeness_profile_planner.rs#L50](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/completeness_profile_planner.rs#L50) · [data_quality_query_process_e2e.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_query_process_e2e.rs#L29) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityDefinitionService/PublishPartyRuleSetVersion` | [rule_set_planner.rs#L33](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/rule_set_planner.rs#L33) · [data_quality_process_e2e.rs#L26](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_process_e2e.rs#L26) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityEvaluationService/GetPartyCompletenessResult` | [query_lists.rs#L31](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-query-adapter/src/query_lists.rs#L31) · [data_quality_completion_surface_process_e2e.rs#L54](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_completion_surface_process_e2e.rs#L54) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityEvaluationService/GetPartyEvaluationJob` | [query_support.rs#L49](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-query-adapter/src/query_support.rs#L49) · [data_quality_completion_surface_process_e2e.rs#L50](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_completion_surface_process_e2e.rs#L50) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityEvaluationService/RequestPartyEvaluation` | [evaluation_job_planner.rs#L102](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/evaluation_job_planner.rs#L102) · [data_quality_contract.rs#L53](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-proto-contracts/tests/data_quality_contract.rs#L53) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityFindingService/AcknowledgeDataQualityFinding` | [finding_stewardship_planner.rs#L88](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs#L88) · [data_quality_contract.rs#L102](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-proto-contracts/tests/data_quality_contract.rs#L102) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityFindingService/AssignDataQualityFinding` | [finding_stewardship_planner.rs#L178](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs#L178) · [data_quality_evaluation_registry.rs#L52](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/support/data_quality_evaluation_registry.rs#L52) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityFindingService/GetDataQualityFinding` | [query_support.rs#L55](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-query-adapter/src/query_support.rs#L55) · [data_quality_read_contract.rs#L60](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-proto-contracts/tests/data_quality_read_contract.rs#L60) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityFindingService/ListAssignedDataQualityFindings` | [query_lists.rs#L17](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-query-adapter/src/query_lists.rs#L17) · [data_quality_completion_surface_process_e2e.rs#L53](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/data_quality_completion_surface_process_e2e.rs#L53) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityFindingService/ListDataQualityFindingsByParty` | [query_lists.rs#L3](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-query-adapter/src/query_lists.rs#L3) · [data_quality_read_contract.rs#L7](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-proto-contracts/tests/data_quality_read_contract.rs#L7) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityFindingService/WaiveDataQualityFinding` | [finding_stewardship_planner.rs#L114](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs#L114) · [data_quality_evaluation_registry.rs#L72](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/support/data_quality_evaluation_registry.rs#L72) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |
| `/crm.data_quality.v1.DataQualityRemediationService/RemediatePartyDisplayName` | [remediation_planner.rs#L81](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs#L81) · [data_quality_evaluation_registry.rs#L82](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/support/data_quality_evaluation_registry.rs#L82) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-04](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-04) |

### crm.identity-resolution · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-identity-resolution/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_privacy.v1.IdentityResolutionPrivacyScopeContributionService/IdentityResolutionPrivacyScopeContribution` | [scope.rs#L247](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L247) · [postgres_scope.rs#L446](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-privacy-scope-adapter/tests/postgres_scope.rs#L446) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |
| `/crm.identity_resolution.v1.IdentityResolutionService/ConfirmDuplicateCandidate` | [owner_planner.rs#L94](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-capability-adapter/src/owner_planner.rs#L94) · [identity_resolution_process_e2e.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/identity_resolution_process_e2e.rs#L37) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/DismissDuplicateCandidate` | [owner_planner.rs#L81](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-capability-adapter/src/owner_planner.rs#L81) · [identity_resolution_process_e2e.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/identity_resolution_process_e2e.rs#L36) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/GetDuplicateCandidateCase` | [lib.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-query-adapter/src/lib.rs#L29) · [identity_resolution_process_e2e.rs#L38](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/identity_resolution_process_e2e.rs#L38) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/GetMergeOperation` | [merge_unmerge_process_e2e.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/merge_unmerge_process_e2e.rs#L37) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/ListDuplicateCandidateCasesByParty` | [lib.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-query-adapter/src/lib.rs#L30) · [identity_resolution_process_e2e.rs#L39](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/identity_resolution_process_e2e.rs#L39) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/ListMergeOperationsByParty` | [merge_unmerge_process_e2e.rs#L38](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/merge_unmerge_process_e2e.rs#L38) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/MergeParty` | [planner.rs#L100](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-capability-adapter/src/planner.rs#L100) · [merge_unmerge_process_e2e.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/merge_unmerge_process_e2e.rs#L35) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/RefreshDuplicateCandidateEvidence` | [domain.rs#L512](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-identity-resolution/src/domain.rs#L512) · [identity_resolution_process_e2e.rs#L35](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/identity_resolution_process_e2e.rs#L35) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/RegisterDuplicateCandidate` | [owner_planner.rs#L53](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-capability-adapter/src/owner_planner.rs#L53) · [identity_resolution_process_e2e.rs#L34](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/identity_resolution_process_e2e.rs#L34) | C · сохранить прежнее решение/состояние и проверить отрицательные случаи на D1 при вводе stewardship. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/ResolveCanonicalParty` | [merge_unmerge_process_e2e.rs#L39](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/merge_unmerge_process_e2e.rs#L39) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |
| `/crm.identity_resolution.v1.IdentityResolutionService/UnmergeParty` | [merge_planner.rs#L65](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-identity-resolution-capability-adapter/src/merge_planner.rs#L65) · [merge_unmerge_process_e2e.rs#L36](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/merge_unmerge_process_e2e.rs#L36) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-03](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-03) |

### crm.parties · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-parties/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_privacy.v1.PartiesPrivacyScopeContributionService/PartiesPrivacyScopeContribution` | [tests.rs#L37](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-parties-privacy-scope-adapter/src/tests.rs#L37) · [postgres_scope.rs#L88](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-parties-privacy-scope-adapter/tests/postgres_scope.rs#L88) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |
| `/crm.parties.v1.PartyService/CreateParty` | [domain.rs#L52](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-parties/src/domain.rs#L52) · [postgres_scope.rs#L204](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-parties-privacy-scope-adapter/tests/postgres_scope.rs#L204) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-02) |
| `/crm.parties.v1.PartyService/GetParty` | [lib.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-parties-query-adapter/src/lib.rs#L29) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-02) |
| `/crm.parties.v1.PartyService/ListParties` | [lib.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-parties-query-adapter/src/lib.rs#L30) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-02) |
| `/crm.parties.v1.PartyService/UpdateParty` | [domain.rs#L60](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-parties/src/domain.rs#L60) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-02) |
| `/crm.parties.v2.PartyService/GetParty` | [lib.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-parties-query-adapter/src/lib.rs#L29) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-02) |

### crm.party-relationships · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-party-relationships/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.customer_privacy.v1.PartyRelationshipsPrivacyScopeContributionService/PartyRelationshipsPrivacyScopeContribution` | [scope.rs#L242](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-customer-privacy/src/scope.rs#L242) · [postgres_scope.rs#L200](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-party-relationships-privacy-scope-adapter/tests/postgres_scope.rs#L200) | C · вклад владельца в полный privacy scope; доказать доступ и freeze на D1. [E-02](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#e-02) |
| `/crm.party_relationships.v1.PartyRelationshipService/CreatePartyRelationship` | [domain.rs#L282](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-party-relationships/src/domain.rs#L282) · [customer_360_process_e2e.rs#L39](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_360_process_e2e.rs#L39) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-09](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-09) |
| `/crm.party_relationships.v1.PartyRelationshipService/GetPartyRelationship` | [lib.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-party-relationships-query-adapter/src/lib.rs#L29) · [party_relationship_process_e2e.rs#L41](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/party_relationship_process_e2e.rs#L41) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-09](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-09) |
| `/crm.party_relationships.v1.PartyRelationshipService/ListPartyRelationships` | [lib.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-party-relationships-query-adapter/src/lib.rs#L30) · [party_relationship_process_e2e.rs#L42](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/party_relationship_process_e2e.rs#L42) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-09](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-09) |
| `/crm.party_relationships.v1.PartyRelationshipService/UpdatePartyRelationship` | [domain.rs#L293](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-party-relationships/src/domain.rs#L293) · [customer_360_process_e2e.rs#L40](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/customer_360_process_e2e.rs#L40) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [L-09](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#l-09) |

### crm.sales · прежние привязки

Манифест и контракт: [module.yaml](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/module.yaml).
| Прежний метод | Код / потребитель | Решение по семантике |
|---|---|---|
| `/crm.sales.v1.DealService/AdvanceStage` | [domain.rs#L571](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/src/domain.rs#L571) · [process_e2e.rs#L29](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/process_e2e.rs#L29) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-07) |
| `/crm.sales.v1.DealService/CreateDeal` | [domain.rs#L205](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/src/domain.rs#L205) · [process_e2e.rs#L28](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/process_e2e.rs#L28) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-07) |
| `/crm.sales.v1.DealService/GetDeal` | [executor.rs#L79](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-query-adapter/src/executor.rs#L79) · [process_e2e.rs#L30](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/services/crm-api/tests/process_e2e.rs#L30) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-07) |
| `/crm.sales.v1.DealService/ListDeals` | [executor.rs#L84](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-query-adapter/src/executor.rs#L84) · [postgres_queries.rs#L300](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-activities-capability-composition/tests/postgres_queries.rs#L300) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-07) |
| `/crm.sales.v1.DealService/UpdateDeal` | [domain.rs#L220](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/modules/crm-sales/src/domain.rs#L220) · [postgres_mutations.rs#L94](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/crates/crm-sales-capability-adapter/tests/postgres_mutations.rs#L94) | A для ограниченного результата текущего метода; прежняя wire-версия — D. [M1-07](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/crm-functional-execution-matrix.md#m1-07) |

## Остальные объявления Protobuf

Компиляция всех .proto на c1f6501 дала **242 методов**. Из них 72 исполняются текущим Worker, 120 дополнительных привязаны в module.yaml, ещё 50 объявлены вне этих манифестов. В таблице статус отражает наличие соответствующей бизнес-семантики в репозитории. Ни один из этих дополнительных путей не зарегистрирован в dev.

| Версионное объявление | Состояние семантики | Решение для dev |
|---|---|---|
| [/crm.billing.v1.InvoiceService/IssueInvoice](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/billing/v1/invoice.proto) | NOT-IMPLEMENTED | Декларация IssueInvoice; в dev потребуется коммерческая цепочка и сверка финансовой системы. |
| [/crm.communications.v1.EmailService/SendEmail](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/communications/v1/email.proto) | NOT-IMPLEMENTED | Декларация SendEmail; consent/evaluate уже есть, delivery и readback провайдера потребуют адаптера. |
| [/crm.core.v1.CapabilityRegistryService/GetToolDescriptor](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/capability.proto) | NOT-IMPLEMENTED | Capability adapter registry разрешает точные версии. Для dev применяются compiled policy и module.yaml; отдельная публикация реестра отложена. |
| [/crm.core.v1.CapabilityRegistryService/ListCapabilities](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/capability.proto) | CODE-NOT-ACCEPTED | Capability adapter registry разрешает точные версии. Для dev применяются compiled policy и module.yaml; отдельная публикация реестра отложена. |
| [/crm.core.v1.CapabilityRegistryService/RegisterCapability](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/capability.proto) | CODE-NOT-ACCEPTED | Capability adapter registry разрешает точные версии. Для dev применяются compiled policy и module.yaml; отдельная публикация реестра отложена. |
| [/crm.core.v1.CapabilityRegistryService/ResolveCapability](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/capability.proto) | CODE-NOT-ACCEPTED | Capability adapter registry разрешает точные версии. Для dev применяются compiled policy и module.yaml; отдельная публикация реестра отложена. |
| [/crm.core.v1.FileService/CompleteUpload](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/file.proto) | NOT-IMPLEMENTED | Исторические immutable chunks/finalize обслуживают артефакты; для конкретного публичного file RPC реализация не подтверждена. |
| [/crm.core.v1.FileService/DeleteFile](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/file.proto) | NOT-IMPLEMENTED | Исторические immutable chunks/finalize обслуживают артефакты; для конкретного публичного file RPC реализация не подтверждена. |
| [/crm.core.v1.FileService/GetDownloadAuthorization](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/file.proto) | NOT-IMPLEMENTED | Исторические immutable chunks/finalize обслуживают артефакты; для конкретного публичного file RPC реализация не подтверждена. |
| [/crm.core.v1.FileService/PlaceLegalHold](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/file.proto) | NOT-IMPLEMENTED | Исторические immutable chunks/finalize обслуживают артефакты; для конкретного публичного file RPC реализация не подтверждена. |
| [/crm.core.v1.FileService/RequestUpload](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/file.proto) | NOT-IMPLEMENTED | Исторические immutable chunks/finalize обслуживают артефакты; для конкретного публичного file RPC реализация не подтверждена. |
| [/crm.core.v1.IntegrationService/CreateConnection](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/integration.proto) | NOT-IMPLEMENTED | Исторический integration.proto; специализированный enrichment adapter существует, универсальное исполнение этой операции не найдено. |
| [/crm.core.v1.IntegrationService/ReceiveWebhook](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/integration.proto) | NOT-IMPLEMENTED | Исторический integration.proto; специализированный enrichment adapter существует, универсальное исполнение этой операции не найдено. |
| [/crm.core.v1.IntegrationService/ResolveConflict](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/integration.proto) | NOT-IMPLEMENTED | Исторический integration.proto; специализированный enrichment adapter существует, универсальное исполнение этой операции не найдено. |
| [/crm.core.v1.IntegrationService/RotateCredential](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/integration.proto) | NOT-IMPLEMENTED | Исторический integration.proto; специализированный enrichment adapter существует, универсальное исполнение этой операции не найдено. |
| [/crm.core.v1.IntegrationService/StartSync](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/integration.proto) | NOT-IMPLEMENTED | Исторический integration.proto; специализированный enrichment adapter существует, универсальное исполнение этой операции не найдено. |
| [/crm.core.v1.MetadataService/GetDescriptorSet](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/metadata.proto) | NOT-IMPLEMENTED | Admin Studio и MetadataRuntime поддерживают revision/impact/rollback. Проверять тот же результат для tenant на новом Worker. |
| [/crm.core.v1.MetadataService/GetImpactReport](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/metadata.proto) | CODE-NOT-ACCEPTED | Admin Studio и MetadataRuntime поддерживают revision/impact/rollback. Проверять тот же результат для tenant на новом Worker. |
| [/crm.core.v1.MetadataService/PublishPackage](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/metadata.proto) | CODE-NOT-ACCEPTED | Admin Studio и MetadataRuntime поддерживают revision/impact/rollback. Проверять тот же результат для tenant на новом Worker. |
| [/crm.core.v1.MetadataService/RollbackPackage](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/metadata.proto) | CODE-NOT-ACCEPTED | Admin Studio и MetadataRuntime поддерживают revision/impact/rollback. Проверять тот же результат для tenant на новом Worker. |
| [/crm.core.v1.MetadataService/ValidatePackage](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/metadata.proto) | CODE-NOT-ACCEPTED | Admin Studio и MetadataRuntime поддерживают revision/impact/rollback. Проверять тот же результат для tenant на новом Worker. |
| [/crm.core.v1.ModuleLifecycleService/ActivateModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/GetModuleImpactReport](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/InstallModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/PublishModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/ResolveModuleDependencies](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/RollbackModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/SuspendModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/UninstallModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/UpgradeModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.ModuleLifecycleService/ValidateModule](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/module_lifecycle.proto) | CODE-NOT-ACCEPTED | Механика регистрации и переходов есть в crm-module-registry. Подключать при первой управляемой установке модуля. |
| [/crm.core.v1.PolicyService/Evaluate](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/policy.proto) | NOT-IMPLEMENTED | Права конкретных Worker RPC проверяются действующей policy. Универсальный публичный сервис этой версии не требуется для скелета. |
| [/crm.core.v1.PolicyService/Explain](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/policy.proto) | NOT-IMPLEMENTED | Права конкретных Worker RPC проверяются действующей policy. Универсальный публичный сервис этой версии не требуется для скелета. |
| [/crm.core.v1.PolicyService/ValidatePolicy](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/policy.proto) | NOT-IMPLEMENTED | Права конкретных Worker RPC проверяются действующей policy. Универсальный публичный сервис этой версии не требуется для скелета. |
| [/crm.core.v1.RecordService/BatchGetRecords](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.RecordService/CreateRecord](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.RecordService/DeleteRecord](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.RecordService/ExecuteMutationSet](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.RecordService/GetRecord](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.RecordService/ListRecords](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.RecordService/UpdateRecord](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/record.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.core.v1.WorkflowService/ApproveAction](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/workflow.proto) | NOT-IMPLEMENTED | SDK задаёт порт start/signal/cancel; требуется самостоятельное исполнение workflow в Worker. |
| [/crm.core.v1.WorkflowService/CancelWorkflow](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/workflow.proto) | NOT-IMPLEMENTED | SDK задаёт порт start/signal/cancel; требуется самостоятельное исполнение workflow в Worker. |
| [/crm.core.v1.WorkflowService/PublishWorkflow](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/workflow.proto) | NOT-IMPLEMENTED | SDK задаёт порт start/signal/cancel; требуется самостоятельное исполнение workflow в Worker. |
| [/crm.core.v1.WorkflowService/RetryFailure](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/workflow.proto) | NOT-IMPLEMENTED | SDK задаёт порт start/signal/cancel; требуется самостоятельное исполнение workflow в Worker. |
| [/crm.core.v1.WorkflowService/SignalWorkflow](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/workflow.proto) | NOT-IMPLEMENTED | SDK задаёт порт start/signal/cancel; требуется самостоятельное исполнение workflow в Worker. |
| [/crm.core.v1.WorkflowService/StartWorkflow](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/core/v1/workflow.proto) | NOT-IMPLEMENTED | SDK задаёт порт start/signal/cancel; требуется самостоятельное исполнение workflow в Worker. |
| [/crm.gateway.v1.ApplicationGatewayService/Mutate](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/gateway/v1/gateway.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.gateway.v1.ApplicationGatewayService/Query](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/gateway/v1/gateway.proto) | CODE-NOT-ACCEPTED | D · исторические generic ingress/хранилище. Сохранять необходимые семантические проверки в typed owner; этот общий интерфейс в dev не публиковать. |
| [/crm.sales.v1.DealService/ChangeStage](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/proto/crm/sales/v1/deal.proto) | CODE-NOT-ACCEPTED | В Cloudflare используется AdvanceStage. Сохранить принятый переход M1-07 и проверку ожидаемой версии; старый wire ChangeStage не переносить. |

## SK-00…SK-09 · решение о разработке

| Шаг | Текущая граница | Следующий результат и проверка |
|---|---|---|
| 00 | Сверены операции текущих и прежних контрактов, целевые семейства, ограниченные сценарии и зависимости. | Пересматривать строку после изменения кода или появления доказательств. |
| 01 | CODE-NOT-ACCEPTED · подписанный Access и tenant действуют; последний Gate B использовал сервис. | B: получить positive human sign-in, проверить видимость состава своей команды, отказ для чужой и отзыв роли. |
| 02 | ACCEPTED · Party → Account, контакты, связи и карточка Customer360, M1-02…06/N-03. | A: использовать существующие идентификаторы, связи и права в следующем срезе. |
| 03 | ACCEPTED · ограниченный Deal → stage → close/reopen и история; отдельный Lead — NOT-IMPLEMENTED. | A для Deal. C для отдельного Lead до подтверждения собственного жизненного цикла и lineage конверсии. |
| 04 | ACCEPTED · Task, срок, напоминание и overdue. CODE-NOT-ACCEPTED · назначение и выборка по владельцу. | B вместе с SK-01. Исполнитель завершает назначенную задачу; попытка чужого tenant, просроченная версия и повтор оставляют корректный результат. |
| 05 | NOT-IMPLEMENTED · единый журнал фактических звонков, встреч и заметок по клиенту/сделке. | B после идентификации субъектов: сохранить Interaction, provenance и связь. Проверить хронологию при повторе или поступлении старого события. |
| 06 | NOT-IMPLEMENTED · общая домашняя очередь сегодняшней работы и взаимодействий. | B после SK-04/05: прочитать очередь с фильтром до пагинации, ссылки ведут на разрешённую запись; чтение не создаёт аудит записи. |
| 07 | ACCEPTED · настройки Sales в пределах M2. CODE-NOT-ACCEPTED · полное управление членством и общие показатели. | B для минимальной настройки команды и обзорного чтения; запрет изменения для обычного сотрудника. |
| 08 | ACCEPTED · малый CSV import/export. CODE-NOT-ACCEPTED · durable jobs и неопределённый исход Task. | A для малых CSV. Включить исправление неопределённой записи в работу над Task; большие объёмы — C до измеримой потребности. |
| 09 | CODE-NOT-ACCEPTED · принят связанный M1-10, полной приёмки операционного пилота нет. | В одном tenant человек проходит login, клиента, взаимодействие, продажу и назначение; коллега завершает задачу. Проверить фоновой эффект, Back/reload, аудит и независимый live Gate на опубликованном SHA. |

## Правила реализации и наблюдения

Операция начинается с версии входного контракта, подтверждённого инициатора и tenant. Внешний adapter связывает источник с сервисным субъектом. Доменный владелец применяет policy и инварианты непосредственно перед изменением. Команды работают через Protobuf → generated Rust/TS → compiled policy → original owner → D1. Экран читает через generated Connect. Таймер и входящее сообщение должны работать при закрытом браузере.

Перед записью фиксировать ключ и ожидаемую версию. Доменные отказы, отсутствие изменения, подтверждённая запись, replay и неопределённый результат различимы в интерфейсе и журнале. Принятое изменение и подтверждение связывает request_id. Отдельно учитывать локальный commit и внешний эффект. Для неподтверждённой доставки проверить внешнюю квитанцию или сверить состояние по стабильному ключу.

Пример будущего связанного пути: подписанная заявка с сайта → событие с tenant и стабильным ID → Party/Account → задача по правилу → разрешённое сообщение → подтверждение провайдера → отчёт по подтверждённым фактам. Проверить повтор входа, потерю ответа, поздний callback, отзыв согласия между постановкой в очередь и отправкой и повторный расчёт аналитики. Это критерий волн 2, 5 и 6.

Gate A собирает и исполняет штатный контракт, политику, домен, workerd/D1 и Chromium, публикует тот же артефакт. Отдельный Gate B проверяет реального инициатора на опубликованном SHA. Для интеграции требуется наблюдать вход или доставку тестового канала; для таймера — выполнение без UI; для человеческого входа — human identity. Доказательства записываются в #352.

Порог коммерческого запуска: подтверждённый human Access, применимая privacy/consent перед эффектом, обслуживаемая запись с восстановлением, выверенные RPO/RTO после изолированного restore, согласованные показатели задержки и нагрузки, наблюдаемость и процедура инцидента. Принятый dev/MVP имеет собственные границы.

## Критерии качества и доказательства по волнам

Сохраняются следующие проверяемые требования к продукту. Для численных SLO владелец релиза фиксирует объём данных, нагрузку и допустимую границу перед измерением.

### Расширение каталога и цены

- ordinary capability growth does not edit owner-specific application-runtime composition;
- no new unjustified permanent gate;
- stable references survive catalog and price version changes;
- deterministic effective-date evaluation and replay;
- frontend and browser journey for product/price administration and lookup.

### Приёмка доменного среза

1. authoritative owner and mutable aggregates;
2. public/internal capabilities, queries, events and workers;
3. exact tenant activation and authorization behavior;
4. persistence, migrations, rollback and reapply;
5. idempotency, audit, outbox and replay where applicable;
6. privacy, consent, retention and deletion interaction;
7. product UX for list/record/workspace/configuration surfaces;
8. keyboard/accessibility and browser acceptance;
9. import/migration and demo/seed path;
10. observability, SLO, recovery and failure-mode evidence;
11. documentation, onboarding and administrator diagnostics;
12. bounded extension cost without unrelated runtime edits or CI fan-out.

### Автоматизация

- event-to-start latency and timer accuracy;
- durable wait/restart behavior;
- at-least-once delivery with idempotent effects or a stronger documented guarantee;
- duplicate-trigger suppression;
- retry/backoff and terminal failure handling;
- dead-letter recovery time;
- definition publication and rollback safety;
- in-flight instance version policy;
- maximum graph size, branch/loop limits and tenant quotas;
- simulation fidelity and side-effect isolation;
- run trace completeness and searchable diagnostics;
- connector timeout/rate-limit/failure behavior;
- authorization and consent re-check immediately before side effects;
- no workflow path capable of bypassing owner invariants;
- load tests for scheduled bursts and event storms.

### Доски и переходы

- transition validation and conflict behavior;
- stage-history correctness;
- large-board performance and stable pagination;
- keyboard and screen-reader operation;
- drag/drop rollback on rejected transition;
- permission and masking correctness on cards;
- bulk-move preview and partial-failure reporting;
- bounded-freshness updates across concurrent users;
- pipeline-definition version migration;
- funnel/stage-duration metric reproducibility.

При изменении контрактов сверять доменные тесты, wire boundary, права клиента и сервиса, хранение и диагностику. Показатели восстановления/доступности PostgreSQL и исторический статус Phase 8A относятся к прежнему runtime. Исходные формулировки P и C пронумерованы по строкам product plan и coverage baseline на c1f6501. Совпадающие требования из двух источников образуют разные ссылки на одну возможность, их суммы не используются как процент готовности.

[PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md) · [CRM_CAPABILITY_COVERAGE.md](https://github.com/iamaman11/crm/blob/c1f65019359641ae20c0426653c08aac3ef3529f/docs/CRM_CAPABILITY_COVERAGE.md) · [Архитектура #471](https://github.com/iamaman11/crm/issues/471) · [Ревизия #485](https://github.com/iamaman11/crm/issues/485)

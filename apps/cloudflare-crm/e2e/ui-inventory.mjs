// Explicit inventory of the visible CRM controls and mandatory real-browser
// evidence. Owned by the existing protected Chromium suite, not a second test
// framework. Each action is a user-observable click/flow/negative case.
// Button-tag totals ensure new controls cannot quietly bypass this inventory.
const actions=(pairs)=>pairs.map(([id,source])=>({id,source}));
export const uiInventory=[
  {file:"main.jsx",buttons:20,forms:1,inputs:4,selects:1,actions:actions([
    ["navigation.accounts","Клиенты"],
    ["navigation.contacts","Контакты"],
    ["navigation.deals","Сделки"],
    ["navigation.tasks","Задачи"],
    ["navigation.parties","Лица и организации"],
    ["navigation.relationships","Связи"],
    ["navigation.consents","Разрешения"],
    ["navigation.identity","Сравнение записей"],
    ["navigation.quality","Качество данных"],
    ["navigation.export","Экспорт данных"],
    ["navigation.privacy","Заявки на приватность"],
    ["navigation.enrichment","Обогащение"],
    ["global.refresh","Обновлено"],
    ["accounts.search","Поиск"],
    ["accounts.create","Добавить {kind.one}"],
    ["accounts.update","Изменить"],
    ["accounts.create.new-party.cancel","Новый клиент с новой организацией"],
    ["accounts.create.new-party-flow","Новый клиент с новой организацией"],
    ["accounts.create.new-party-linked","Лицо / организация"],
    ["sales-deals.create.cancel","Добавить {kind.one}"],
    ["sales-deals.create","Сумма"],
    ["sales-deals.invalid-amount","Сумма"],
    ["sales-deals.invalid-probability","Вероятность, %"],
    ["sales-deals.update","Изменить"],
    ["sales-deals.edit-exact-conversion","Вероятность, %"],
    ["sales-deals.display-major","formatDealMoney"],
    ["sales-deals.advance.cancel","Этап сделки"],
    ["sales-deals.advance","Этап сделки"],
    ["deals.board","Воронка"],
    ["deals.table","Таблица"],
    ["sales-deals.board-major-percent","percentFromBasisPoints"],
    ["contact-points.create.cancel","Добавить {kind.one}"],
    ["contact-points.create","Добавить {kind.one}"],
    ["contact-points.update","Изменить"],
    ["activity-tasks.create.cancel","Добавить {kind.one}"],
    ["activity-tasks.create","Добавить {kind.one}"],
    ["activity-tasks.complete","Выполнить"]
  ])},
  {file:"Parties.jsx",buttons:6,forms:1,inputs:2,selects:1,actions:actions([
    ["parties.create.cancel","Отмена"],
    ["parties.create","Сохранить"],
    ["parties.update","Изменить"],
    ["parties.search","Поиск лиц и организаций"],
    ["parties.refresh","Обновить"],
    ["parties.link-existing-account.cancel","Добавить клиента"]
  ])},
  {file:"Relationships.jsx",buttons:6,forms:1,inputs:2,selects:1,actions:actions([
    ["party-relationships.create.cancel","Отмена"],
    ["party-relationships.create.self-denied","К кому"],
    ["party-relationships.create","Добавить связь"],
    ["party-relationships.update","Изменить"],
    ["party-relationships.search","Поиск связей"],
    ["party-relationships.refresh","Обновить"],
    ["party-relationships.update.cancel","Отмена"]
  ])},
  {file:"Consents.jsx",buttons:5,forms:2,inputs:3,selects:1,actions:actions([
    ["consents.evaluate.fail-closed","Проверить разрешение"],
    ["consents.suppress.cancel","Отмена"],
    ["consents.suppress","Подтвердить запрет"],
    ["consents.evaluate.active-deny","Проверить разрешение"],
    ["consents.search","Поиск запретов"],
    ["consents.refresh","Обновить"]
  ])},
  {file:"IdentityResolution.jsx",buttons:3,forms:1,inputs:0,selects:2,actions:actions([
    ["identity-resolution.compare","Сравнить"],
    ["identity-resolution.self-pair-denied","Сравнить"],
    ["identity-resolution.reset","Сбросить"],
    ["identity-resolution.candidates","Найти совпадения email"],
    ["identity-resolution.candidates.failed","Найти совпадения email"],
    ["identity-resolution.candidates.retry","Найти совпадения email"]
  ])},
  {file:"DataQuality.jsx",buttons:2,forms:1,inputs:0,selects:1,actions:actions([
    ["data-quality.evaluate.pass","Проверить качество"],
    ["data-quality.evaluate.placeholder","Проверить качество"],
    ["data-quality.reset","Сбросить"]
  ])},
  {file:"DataExport.jsx",buttons:4,forms:0,inputs:1,selects:1,actions:actions([
    ["customer-data-operations.preview","Проверить экспорт"],
    ["customer-data-operations.preview.failed","Проверить экспорт"],
    ["customer-data-operations.preview.retry","Проверить экспорт"],
    ["customer-data-operations.export.download","Скачать CSV"],
    ["customer-data-operations.import.invalid","Проверить CSV"],
    ["customer-data-operations.import-preview","Проверить CSV"],
    ["customer-data-operations.import-apply","Импортировать новые записи"]
  ])},
  {file:"PrivacyCases.jsx",buttons:4,forms:0,inputs:0,selects:1,actions:actions([
    ["customer-privacy.create","Создать заявку"],
    ["customer-privacy.submit","Отправить заявку"],
    ["customer-privacy.cancel","Отменить заявку"],
    ["customer-privacy.refresh.failed","Обновить заявки"],
    ["customer-privacy.refresh.retry","Обновить заявки"]
  ])},
  {file:"Enrichment.jsx",buttons:5,forms:1,inputs:6,selects:1,actions:actions([
    ["customer-enrichment.profile.cancel","Добавить профиль провайдера"],
    ["customer-enrichment.profile_publish","Сохранить профиль"],
    ["customer-enrichment.mapping.cancel","Добавить сопоставление"],
    ["customer-enrichment.mapping.invalid","Сохранить сопоставление"],
    ["customer-enrichment.mapping_publish","Сохранить сопоставление"],
    ["customer-enrichment.refresh.failed","Обновить настройки"],
    ["customer-enrichment.refresh.retry","Обновить настройки"]
  ])},
  {file:"Customer360.jsx",buttons:5,forms:0,inputs:0,selects:0,actions:actions([
    ["customer360.retry","Повторить"],
    ["customer360.refresh","Обновить карточку"],
    ["customer360.back","К списку клиентов"],
    ["customer360.navigate.contacts","Все {title.toLowerCase()}"],
    ["customer360.navigate.deals","Все {title.toLowerCase()}"],
    ["customer360.navigate.tasks","Все {title.toLowerCase()}"],
    ["customer360.navigate.relationships","Все {title.toLowerCase()}"],
    ["customer360.edit.contact.cancel","Изменить"],
    ["customer360.edit.deal.cancel","Изменить"]
  ])}
];
export const requiredUiActions=uiInventory.flatMap(screen=>screen.actions.map(action=>action.id));

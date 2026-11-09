// Developer locator for the SINGLE native CRM. This registry maps SOURCE
// ownership and UI to original Rust modules; it is not business validation.
export const modules={
  accounts:{title:"Клиенты",route:"accounts",owner:"crm.customer-accounts"},
  contacts:{title:"Контакты",route:"contact-points",owner:"crm.contact-points"},
  deals:{title:"Сделки",route:"sales-deals",owner:"crm.sales"},
  tasks:{title:"Задачи",route:"activity-tasks",owner:"crm.activities"}
};
export const nativeModels={
  parties:{title:"Лица и организации",kind:"owner",route:"parties"}
};
export const readModels={
  customer360:{title:"Карточка клиента",kind:"read",route:"accounts/:id/overview"}
};
export const backendModels={
  "party-relationships":{title:"Связи лиц и организаций",kind:"owner",ui:"web/src/Relationships.jsx"},
  consents:{title:"Разрешения коммуникации",kind:"owner",
    ui:"web/src/Consents.jsx",grant_evidence:"NOT_CONNECTED",
    commands:["evaluate","operator_suppress"],grant_ingress:"NOT_CONNECTED"},
  "sales-activities-link":{title:"Связь сделки с задачей",kind:"internal",
    ui:"INTERNAL_LINK_NO_SCREEN",delivery:"SYNCHRONOUS_ATOMIC_D1"},
  "identity-resolution":{title:"Разрешение идентичности",kind:"backend",ui:"NOT_CONNECTED"},
  "customer-data-operations":{title:"Импорт и экспорт",kind:"backend",ui:"NOT_CONNECTED"},
  "data-quality":{title:"Качество данных",kind:"backend",ui:"NOT_CONNECTED"},
  "customer-enrichment":{title:"Обогащение",kind:"backend",ui:"NOT_CONNECTED"},
  "customer-privacy":{title:"Приватность",kind:"backend",ui:"NOT_CONNECTED"}
};
export const implementationOwners={
  parties:"rust-party",accounts:"original-accounts",
  contacts:"original-contact-points",deals:"original-sales",
  tasks:"original-activities",customer360:"account-overview",
  "party-relationships":"original-relationships",consents:"original-consents",
  "sales-activities-link":"original-link",
  "identity-resolution":"crm-identity-resolution",
  "customer-data-operations":"crm-customer-data-operations",
  "data-quality":"crm-data-quality",
  "customer-enrichment":"crm-customer-enrichment",
  "customer-privacy":"crm-customer-privacy"
};
const nativeTests="rust-party/test/native.mjs",live="e2e/live.mjs",ui="web/src/main.jsx";
export const ownerFiles={
  "rust-party":{ui:"web/src/Parties.jsx",api:"rust-party/src/parties.rs",
    validation:"../../modules/crm-parties/src/lib.rs",
    migration:"migrations/0002_rust_party.sql",tests:nativeTests,browser:live},
  "original-accounts":{ui,api:"rust-party/src/accounts.rs",
    validation:"../../modules/crm-customer-accounts/src/domain.rs",
    migration:"migrations/0004_original_customer_accounts.sql",
    tests:nativeTests,browser:live},
  "original-contact-points":{ui,api:"rust-party/src/contact_points.rs",
    validation:"../../modules/crm-contact-points/src/domain.rs",
    migration:"migrations/0005_original_contact_points.sql",
    tests:nativeTests,browser:live},
  "original-sales":{ui,api:"rust-party/src/sales_deals.rs",
    validation:"../../modules/crm-sales/src/domain.rs",
    migration:"migrations/0008_original_sales_deals.sql",
    tests:nativeTests,browser:live},
  "original-activities":{ui,api:"rust-party/src/activity_tasks.rs",
    validation:"../../modules/crm-activities/src/domain.rs",
    migration:"migrations/0007_original_activity_tasks.sql",
    tests:nativeTests,browser:live},
  "account-overview":{ui:"web/src/Customer360.jsx",
    api:"rust-party/src/account_overview.rs",validation:"rust-party/src/account_overview.rs",
    migration:"migrations/0004_original_customer_accounts.sql",
    tests:nativeTests,browser:live},
  "original-relationships":{ui:"web/src/Relationships.jsx",api:"rust-party/src/party_relationships.rs",
    validation:"../../modules/crm-party-relationships/src/domain.rs",
    migration:"migrations/0006_original_party_relationships.sql",
    tests:nativeTests,browser:live},
  "original-consents":{ui:"web/src/Consents.jsx",api:"rust-party/src/consents.rs",
    validation:"../../modules/crm-consents/src/domain.rs",
    migration:"migrations/0009_original_consents.sql",tests:nativeTests,browser:live},
  "original-link":{ui:null,api:"rust-party/src/sales_activities_link.rs",
    validation:"../../modules/crm-sales-activities-link/src/lib.rs",
    migration:"migrations/0010_original_sales_activities_link.sql",
    tests:nativeTests,browser:null},
  ...Object.fromEntries(["crm-identity-resolution","crm-customer-data-operations",
    "crm-data-quality","crm-customer-enrichment","crm-customer-privacy"]
    .map(name=>[name,{ui:null,api:null,
      validation:"../../modules/"+name+"/Cargo.toml",
      migration:null,tests:null,browser:null}]))
};
export const errors={
  INVALID_INPUT:"Проверьте поля формы",
  NOT_FOUND:"Запись не найдена",
  IDEMPOTENCY_CONFLICT:"Конфликт повтора операции",
  FORBIDDEN:"Недостаточно прав",
  AUTH_REQUIRED:"Требуется авторизация",
  INTERNAL_ERROR:"Ошибка приложения",
  SALES_DEAL_VERSION_CONFLICT:"Сделка изменена другим запросом"
};

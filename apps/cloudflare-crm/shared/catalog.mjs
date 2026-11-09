// Public React fields and native Rust implementation ownership for developer navigation.
// New functionality: registry + D1 migration + validation + a browser scenario.
export const modules = {
 customers:{title:"Клиенты",one:"клиента",primary:"name",table:"crm_customers",fields:[
  ["name","Имя / название","text"],["kind","Тип","select",["person","organization"]]]},
 contacts:{title:"Контакты",one:"контакт",primary:"name",table:"crm_contacts",fields:[
  ["customer_id","Клиент","customer"],["name","Имя","text"],["email","Электронная почта","email"],["phone","Телефон","tel"]]},
 deals:{title:"Сделки",one:"сделку",primary:"title",table:"crm_deals",fields:[
  ["customer_id","Клиент","customer"],["title","Название сделки","text"],
  ["stage","Этап","select",["new","qualified","proposal","won","lost"]],["amount","Сумма","number"]]},
 tasks:{title:"Задачи",one:"задачу",primary:"title",table:"crm_tasks",fields:[
  ["customer_id","Клиент","customer"],["title","Что сделать","text"],["due_date","Срок","date"],
  ["status","Статус","select",["open","done"]]]},
};

// Source ownership for fast, truthful developer navigation.
// The original four test screens use the native pilot adapter; each NEW expert module must
// declare its own source owner and files before it can pass the doctor gate.
// Read-only compositions do not own a database table; still require independent owners.
export const nativeModels = {
 parties: {title:"Лица и организации",kind:"owner",route:"parties"},
};
// Backend-only integrations are NOT React features until UI cutover.
// This prevents the original Account from masquerading as the legacy pilot customer.
export const backendModels = {
 accounts: {title:"Оригинальные клиентские аккаунты",kind:"backend",route:"accounts",
   ui:"NOT_CONNECTED",transport:"native-workers-rs",persistence:"D1"},
 "contact-points": {title:"Оригинальные контактные каналы",kind:"backend",route:"contact-points",
   ui:"NOT_CONNECTED",verification:"TRUSTED_EVIDENCE_NOT_CONNECTED",
   consent:"SEPARATE_MODULE_NOT_CONNECTED",transport:"native-workers-rs",persistence:"D1"},
 "party-relationships": {title:"Оригинальные отношения лиц и организаций",kind:"backend",
   route:"party-relationships",ui:"NOT_CONNECTED",
   transport:"native-workers-rs",persistence:"D1"},
 "activity-tasks":{title:"Оригинальные задачи CRM",kind:"backend",route:"activity-tasks",
   ui:"NOT_CONNECTED",transport:"native-workers-rs",persistence:"D1"},
 "sales-deals":{title:"Оригинальные сделки CRM",kind:"backend",route:"sales-deals",
   ui:"NOT_CONNECTED",primary_contact:"NOT_CONNECTED",
   transport:"native-workers-rs",persistence:"D1"},
 consents:{title:"Оригинальные разрешения коммуникации",kind:"backend",route:"consents",
   ui:"NOT_CONNECTED",grant_evidence:"NOT_CONNECTED",
   operator_suppressions:"ENABLED",transport:"native-workers-rs",persistence:"D1"},
 "sales-activities-link":{title:"Связь Sales и задач",kind:"backend",
   route:"sales-deals",ui:"NOT_CONNECTED",
   delivery:"SYNCHRONOUS_ATOMIC_D1",transport:"native-workers-rs",persistence:"D1"},
};
export const readModels = { customer360: { title:"Карточка клиента", kind:"read", route:"customer360" } };
export const implementationOwners = {
 parties:"rust-party", accounts:"original-accounts",
 "contact-points":"original-contact-points",
 "party-relationships":"original-party-relationships",
 "activity-tasks":"original-activities",
 "sales-deals":"original-sales", consents:"original-consents",
 "sales-activities-link":"original-sales-activities-link", customer360:"customer360",
 customers:"pilot-core", contacts:"pilot-core", deals:"pilot-core", tasks:"pilot-core",
};
export const ownerFiles = {
 "rust-party":{
  ui:"web/src/Parties.jsx",api:"rust-party/src/lib.rs",
  validation:"rust-party/src/lib.rs",migration:"migrations/0002_rust_party.sql",
  tests:"rust-party/test/native.mjs",browser:"e2e/live.mjs",
 },
 "original-sales-activities-link":{
  ui:null,api:"rust-party/src/sales_activities_link.rs",
  validation:"../../modules/crm-sales-activities-link/src/lib.rs",
  codec:"../../crates/crm-sales-activities-link-contract-adapter/src/lib.rs",
  migration:"migrations/0010_original_sales_activities_link.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "original-consents":{
  ui:null,api:"rust-party/src/consents.rs",
  validation:"../../modules/crm-consents/src/domain.rs",
  codec:"../../modules/crm-consents/src/persistence.rs",
  migration:"migrations/0009_original_consents.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "original-sales":{
  ui:null,api:"rust-party/src/sales_deals.rs",
  validation:"../../modules/crm-sales/src/domain.rs",
  codec:"../../modules/crm-sales/src/persistence.rs",
  migration:"migrations/0008_original_sales_deals.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "original-activities":{
  ui:null,api:"rust-party/src/activity_tasks.rs",
  validation:"../../modules/crm-activities/src/domain.rs",
  codec:"../../modules/crm-activities/src/persistence.rs",
  migration:"migrations/0007_original_activity_tasks.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "original-party-relationships":{
  ui:null,api:"rust-party/src/party_relationships.rs",
  validation:"../../modules/crm-party-relationships/src/domain.rs",
  codec:"../../modules/crm-party-relationships/src/persistence.rs",
  migration:"migrations/0006_original_party_relationships.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "original-contact-points":{
  ui:null,api:"rust-party/src/contact_points.rs",
  validation:"../../modules/crm-contact-points/src/domain.rs",
  codec:"../../modules/crm-contact-points/src/persistence.rs",
  migration:"migrations/0005_original_contact_points.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "original-accounts":{
  ui:null,api:"rust-party/src/accounts.rs",
  validation:"../../modules/crm-customer-accounts/src/domain.rs",
  codec:"../../modules/crm-customer-accounts/src/persistence.rs",
  migration:"migrations/0004_original_customer_accounts.sql",
  tests:"rust-party/test/native.mjs",browser:null,
 },
 "pilot-core": {
  ui:"web/src/main.jsx", api:"rust-party/src/pilot.rs",
  validation:"rust-party/src/pilot.rs:validate", migration:"migrations",
  tests:"rust-party/test/native.mjs", browser:"e2e/live.mjs",
 },
 customer360:{
  ui:"web/src/Customer360.jsx", api:"rust-party/src/pilot.rs",
  validation:"rust-party/src/pilot.rs:customer360", migration:"migrations",
  tests:"rust-party/test/native.mjs", browser:"e2e/live.mjs",
 },
};
export const labels={person:"Частное лицо",organization:"Компания",new:"Новая",qualified:"В работе",
 proposal:"Предложение",won:"Выиграна",lost:"Закрыта",open:"В работе",done:"Готово"};
export const errors = {
 INVALID_INPUT:"Проверьте поля формы",
 CUSTOMER_NOT_FOUND:"Клиент не найден",
 NOT_FOUND:"Not found",
 AUTH_REQUIRED:"Authentication required",
 DB_UNAVAILABLE:"Database unavailable",
 INTERNAL_ERROR:"Ошибка приложения",
};

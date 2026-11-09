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
export const readModels = { customer360: { title:"Карточка клиента", kind:"read", route:"customer360" } };
export const implementationOwners = {
 parties:"rust-party", customer360:"customer360",
 customers:"pilot-core", contacts:"pilot-core", deals:"pilot-core", tasks:"pilot-core",
};
export const ownerFiles = {
 "rust-party":{
  ui:"web/src/Parties.jsx",api:"rust-party/src/lib.rs",
  validation:"rust-party/src/lib.rs",migration:"migrations/0002_rust_party.sql",
  tests:"rust-party/test/native.mjs",browser:"e2e/live.mjs",
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

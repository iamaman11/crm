// Single registry shared by UI, Worker API and fast local developer navigation.
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
// The original four small modules share pilot-core; each NEW expert module must
// declare its own source owner and files before it can pass the doctor gate.
export const implementationOwners = {
 customers:"pilot-core", contacts:"pilot-core", deals:"pilot-core", tasks:"pilot-core",
};
export const ownerFiles = {
 "pilot-core": {
  ui:"web/src/main.jsx", api:"worker/index.mjs",
  validation:"worker/index.mjs:parseRecord", migration:"migrations",
  tests:"test/worker.test.mjs", browser:"e2e/live.mjs",
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

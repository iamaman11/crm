import test from "node:test";
import assert from "node:assert/strict";
import { crmFetch } from "../worker/index.mjs";
import { modules, readModels, nativeModels, implementationOwners, ownerFiles } from "../shared/catalog.mjs";
const url = (path, init = {}) => new Request("https://crm.test" + path, init);
const access = { access: { getIdentity: async () => ({ email: "user@example.test" }) } };
function db() {
  const queries = [];
  return {
    queries,
    prepare(sql) {
      return { bind(...values) {
        queries.push({ sql, values });
        return {
          async first() {
            if (sql.includes("FROM crm_users")) return { tenant_id: "tenant-a", role: "admin" };
            if (sql.includes("sqlite_master")) return { present: 4 };
            if (sql.includes("FROM crm_customers")) return { id: values[0] };
            if (sql.includes("SELECT id FROM")) return { id: values[0] };
            return null;
          },
          async all() { return { results: [] }; },
        };
      }};
    },
    async batch(items) {
      assert.equal(items.length, 2);
      return items.map(() => ({ success: true }));
    },
  };
}
test("Access required, HTTP request hints do not authenticate", async () => {
  const database = db();
  const result = await crmFetch(url("/api/customers", { headers: { "x-tenant-id": "tenant-a" } }), { DB: database }, {});
  assert.equal(result.status, 401);
  assert.equal(database.queries.length, 0);
});
test("all four lists query the signed-in tenant", async () => {
  const database = db();
  for (const name of ["customers", "contacts", "deals", "tasks"]) {
    const result = await crmFetch(url("/api/" + name), { DB: database }, access);
    assert.equal(result.status, 200);
    assert.deepEqual(await result.json(), { items: [] });
  }
  const lists = database.queries.filter(q => q.sql.includes("ORDER BY updated_at DESC"));
  assert.equal(lists.length, 4);
  for (const query of lists) assert.deepEqual(query.values, ["tenant-a"]);
});
test("customers can be created, changed and deleted", async () => {
  const database = db();
  const create = await crmFetch(url("/api/customers", {
    method: "POST", body: JSON.stringify({ name: "Компания", kind: "organization", tenant_id: "forged" })
  }), { DB: database }, access);
  assert.equal(create.status, 201);
  const customer = (await create.json()).item;
  assert.equal(customer.tenant_id, "tenant-a");
  assert.ok(uuid(customer.id));
  for (const method of ["PATCH", "DELETE"]) {
    const result = await crmFetch(url("/api/customers/" + customer.id, {
      method, body: method === "PATCH" ? JSON.stringify({ name: "Новая", kind: "organization" }) : undefined
    }), { DB: database }, access);
    assert.equal(result.status, method === "PATCH" ? 200 : 204);
  }
});
function uuid(value) { return /^[0-9a-f-]{36}$/.test(value); }
test("contacts, deals and tasks accept simple form records", async () => {
  const database = db(), customer_id = crypto.randomUUID();
  const cases = [
    ["contacts", { customer_id, name: "Иван", email: "ivan@example.test", phone: "+375000" }],
    ["deals", { customer_id, title: "Новая сделка", stage: "new", amount: 100 }],
    ["tasks", { customer_id, title: "Позвонить", status: "open", due_date: "2026-12-01" }],
  ];
  for (const [name, data] of cases) {
    const result = await crmFetch(url("/api/" + name, { method: "POST", body: JSON.stringify(data) }),
      { DB: database }, access);
    assert.equal(result.status, 201);
  }
  assert.equal(database.queries.filter(q => q.sql.includes("INSERT INTO crm_audit")).length, 3);
});
test("invalid form rejected without writes", async () => {
  const database = db();
  const response = await crmFetch(url("/api/deals", { method: "POST", body: JSON.stringify({ title: "Bad" }) }),
    { DB: database }, access);
  assert.equal(response.status, 422);
  assert.equal(database.queries.filter(q => q.sql.includes("INSERT INTO")).length, 0);
});

test("authenticated non-email service identity receives an isolated pilot workspace", async () => {
  const database = db();
  const service = { access: { getIdentity: async () => ({ common_name: "service-token.access" }) } };
  const result = await crmFetch(url("/api/session"), { DB: database }, service);
  assert.equal(result.status, 200);
  const membership = database.queries.find(q => q.sql.includes("FROM crm_users"));
  assert.deepEqual(membership.values, ["service:service-token.access"]);
});

test("missing or malformed Access JWT is rejected with Static Assets routing", async () => {
  const database = db();
  const missing = await crmFetch(url("/api/session"), { DB: database }, {});
  assert.equal(missing.status, 401);
  const malformed = await crmFetch(url("/api/session", { headers: { "Cf-Access-Jwt-Assertion": "not.a.jwt" } }), { DB: database }, {});
  assert.equal(malformed.status, 401);
  assert.equal(database.queries.length, 0);
});

test("two Access service credentials map to distinct workspace keys", async () => {
  const first = db(), second = db();
  const service = id => ({ access: { getIdentity: async () => ({ common_name: id + ".access" }) } });
  assert.equal((await crmFetch(url("/api/session"), { DB: first }, service("service-test-a"))).status, 200);
  assert.equal((await crmFetch(url("/api/session"), { DB: second }, service("service-test-b"))).status, 200);
  const getKey = database => database.queries.find(q => q.sql.includes("FROM crm_users")).values[0];
  assert.notEqual(getKey(first), getKey(second));
});
test("Access without a user or service-token identity is not a CRM workspace", async () => {
  const database = db();
  const response = await crmFetch(url("/api/session"), { DB: database }, {
    access: { getIdentity: async () => ({}) },
  });
  assert.equal(response.status, 403);
  assert.equal(database.queries.length, 0);
});

test("read-only diagnostics verify schema and emit a correlation id", async () => {
  const database = db();
  const response = await crmFetch(url("/api/doctor"), { DB: database, GIT_SHA: "abcdef" }, access);
  assert.equal(response.status, 200);
  assert.ok(response.headers.get("x-request-id"));
  const info = await response.json();
  assert.equal(info.source, "abcdef");
  assert.equal(info.database, "ok");
  assert.deepEqual(info.modules, ["customers", "contacts", "deals", "tasks"]);
  assert.deepEqual(info.tables, { present: 4, expected: 4 });
  assert.equal(database.queries.filter(q => q.sql.includes("sqlite_master")).length, 1);
});
test("expected API failure has a stable code and request id", async () => {
  const response = await crmFetch(url("/api/deals", {
    method: "POST", body: JSON.stringify({ title: "No customer" }),
  }), { DB: db() }, access);
  assert.equal(response.status, 422);
  assert.equal(response.headers.get("x-crm-error-code"), "INVALID_INPUT");
  assert.ok(response.headers.get("x-request-id"));
  assert.equal((await response.json()).code, "INVALID_INPUT");
});

test("all modules declare a discoverable owner; new features cannot silently reuse pilot-core", () => {
  assert.deepEqual(Object.keys(implementationOwners).sort(), [...Object.keys(modules),...Object.keys(readModels),...Object.keys(nativeModels)].sort());
  for (const [name, owner] of Object.entries(implementationOwners)) {
    assert.ok(ownerFiles[owner], "missing source owner for "+name);
    if (!["customers","contacts","deals","tasks"].includes(name)) assert.notEqual(owner, "pilot-core");
    for (const part of ["ui","api","validation","migration","tests","browser"]) {
      assert.ok(ownerFiles[owner][part], "missing "+part+" source for "+name);
    }
  }
});

import test from "node:test";
import assert from "node:assert/strict";
import { crmFetch } from "../worker/index.mjs";

function testDb(tenant = "tenant-a") {
  const queries = [];
  let existing = false;
  const db = {
    queries,
    prepare(sql) {
      return {
        bind(...values) {
          queries.push({ sql, values });
          return {
            async first() {
              if (sql.includes("FROM crm_users")) return { tenant_id: tenant, role: "operator" };
              if (sql.includes("FROM crm_customers")) return existing ? { id: values[0] } : null;
              return null;
            },
            async all() { return { results: [{ id: "one", name: "Visible", kind: "person" }] }; },
          };
        },
      };
    },
    async batch(statements) {
      assert.equal(statements.length, 2, "business write and audit must be one atomic batch");
      existing = true;
      return [{ success: true }, { success: true }];
    },
  };
  return db;
}
const access = { access: { getIdentity: async () => ({ email: "test@example.test" }) } };
const request = (path, init) => new Request("https://crm.example.test" + path, init);

test("a request without verified Cloudflare Access cannot read CRM", async () => {
  const db = testDb();
  const res = await crmFetch(request("/api/customers"), { DB: db }, {});
  assert.equal(res.status, 401);
  assert.equal(db.queries.length, 0, "no database lookup for an unauthenticated visitor");
});
test("a verified identity must map to an active member of a tenant", async () => {
  const db = { prepare() { return { bind() { return { async first() { return null; } }; } }; } };
  assert.equal((await crmFetch(request("/api/customers"), { DB: db }, access)).status, 403);
});
test("customer reads always bind the server-side tenant", async () => {
  const db = testDb("tenant-b");
  const res = await crmFetch(request("/api/customers"), { DB: db }, access);
  assert.equal(res.status, 200);
  const query = db.queries.find(x => x.sql.includes("FROM crm_customers"));
  assert.deepEqual(query.values, ["tenant-b"]);
  assert.match(query.sql, /WHERE tenant_id = \?/);
});
test("create is tenant-scoped, validated and atomically audited", async () => {
  const db = testDb();
  const res = await crmFetch(request("/api/customers", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: "Acme", kind: "organization", tenant_id: "tenant-evil" }),
  }), { DB: db }, access);
  assert.equal(res.status, 201);
  const body = await res.json();
  assert.equal(body.customer.name, "Acme");
  const insert = db.queries.find(q => q.sql.includes("INSERT INTO crm_customers"));
  assert.equal(insert.values[1], "tenant-a");
  assert.equal(db.queries.filter(q => q.sql.includes("INSERT INTO crm_audit")).length, 1);
  const invalid = await crmFetch(request("/api/customers", { method: "POST", body: "{}" }), { DB: db }, access);
  assert.equal(invalid.status, 422);
});
test("no arbitrary paths, tenant hints or authorization shortcuts", async () => {
  const db = testDb();
  const response = await crmFetch(request("/api/admin?tenant_id=somebody-else",
    { headers: { "x-tenant-id": "tenant-b" } }), { DB: db }, access);
  assert.equal(response.status, 404);
  assert.ok(response.headers.get("x-request-id"));
});

// Small Cloudflare CRM pilot. The only data store is D1.
import { createRemoteJWKSet, jwtVerify } from "jose";
import { modules, errors } from "../shared/catalog.mjs";

// Static Assets route through an internal Worker, so ctx.access is unavailable there.
// Read the signed Access JWT instead; no second login system is needed.
const ACCESS_ISSUER = "https://royal-limit-ea43.cloudflareaccess.com";
const ACCESS_AUDIENCE = "e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";
const ACCESS_KEYS = createRemoteJWKSet(new URL(ACCESS_ISSUER + "/cdn-cgi/access/certs"));
export const BUILD_SHA = "development";

const models = Object.fromEntries(Object.entries(modules).map(([name, spec]) => [
  name, { table: spec.table, fields: spec.fields.map(([key]) => key) },
]));
const moduleNames = Object.keys(models);
const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i;

function reply(value, status = 200) {
  const headers = { "cache-control": "no-store" };
  if (value?.code) headers["x-crm-error-code"] = value.code;
  return Response.json(value, { status, headers });
}
function problem(code, status) {
  return reply({ error: errors[code], code }, status);
}

function parseRecord(type, body) {
  if (!body || typeof body !== "object" || Array.isArray(body)) return null;
  const str = (key, max = 120) => typeof body[key] === "string" ? body[key].trim().slice(0, max) : "";
  const customer_id = str("customer_id", 36);
  if (type === "customers") {
    const name = str("name"), kind = str("kind");
    return name && ["person", "organization"].includes(kind) ? { name, kind } : null;
  }
  if (!uuid.test(customer_id)) return null;
  if (type === "contacts") {
    const name = str("name"), email = str("email", 180), phone = str("phone", 60);
    return name && (!email || /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) ?
      { customer_id, name, email, phone } : null;
  }
  if (type === "deals") {
    const title = str("title"), stage = str("stage");
    const amount = Number(body.amount);
    return title && ["new", "qualified", "proposal", "won", "lost"].includes(stage) &&
      Number.isFinite(amount) && amount >= 0 && amount <= 1000000000 ?
      { customer_id, title, stage, amount } : null;
  }
  if (type === "tasks") {
    const title = str("title"), status = str("status"), due_date = str("due_date", 10);
    return title && ["open", "done"].includes(status) && (!due_date || /^\d{4}-\d{2}-\d{2}$/.test(due_date)) ?
      { customer_id, title, status, due_date } : null;
  }
  return null;
}

async function requestBody(request) {
  if (Number(request.headers.get("content-length") || 0) > 8192) return null;
  const text = await request.text();
  if (text.length > 8192) return null;
  try { return JSON.parse(text); } catch { return null; }
}

function identityKey(identity) {
  const email = typeof identity?.email === "string" ? identity.email.trim().toLowerCase() : "";
  if (email) return email;
  // Each verified Cloudflare Access service token has its own common_name.
  const name = identity?.common_name;
  return typeof name === "string" && /^[a-z0-9.-]{8,120}\.access$/i.test(name)
    ? "service:" + name.toLowerCase() : null;
}

async function actorFor(ctx, db, request) {
  let identity;
  if (ctx.access) {
    identity = await ctx.access.getIdentity();
  } else {
    // Static Assets do not pass ctx.access through the internal router.
    const token = request.headers.get("cf-access-jwt-assertion");
    if (!token) return null;
    try {
      const verified = await jwtVerify(token, ACCESS_KEYS, {
        issuer: ACCESS_ISSUER, audience: ACCESS_AUDIENCE,
      });
      identity = verified.payload;
    } catch {
      return null;
    }
  }
  const key = identityKey(identity);
  if (!key) return null;
  const user = await db.prepare(
    "SELECT tenant_id, role FROM crm_users WHERE email = ? AND active = 1"
  ).bind(key).first();
  if (user) return { tenant: user.tenant_id, role: user.role };
  // A newly authenticated user or service token gets its own simple test workspace.
  const tenant = crypto.randomUUID();
  await db.batch([
    db.prepare("INSERT OR IGNORE INTO crm_tenants (id, name) VALUES (?, ?)").bind(tenant, "Test CRM"),
    db.prepare("INSERT OR IGNORE INTO crm_users (email, tenant_id, role) VALUES (?, ?, ?)").bind(key, tenant, "admin"),
  ]);
  const enrolled = await db.prepare(
    "SELECT tenant_id, role FROM crm_users WHERE email = ? AND active = 1"
  ).bind(key).first();
  return enrolled ? { tenant: enrolled.tenant_id, role: enrolled.role } : null;
}

async function belongsToTenant(db, customer, tenant) {
  return !!(await db.prepare(
    "SELECT id FROM crm_customers WHERE id = ? AND tenant_id = ?"
  ).bind(customer, tenant).first());
}

async function handleRecords(request, env, actor, url) {
  const match = /^\/api\/([a-z]+)(?:\/([a-f0-9-]{36}))?$/.exec(url.pathname);
  if (!match || !models[match[1]] || (match[2] && !uuid.test(match[2]))) return problem("NOT_FOUND", 404);
  const [, name, id] = match, model = models[name], db = env.DB, tenant = actor.tenant;
  if (request.method === "GET") {
    if (id) {
      const row = await db.prepare(
        "SELECT * FROM " + model.table + " WHERE id = ? AND tenant_id = ?"
      ).bind(id, tenant).first();
      return row ? reply({ item: row }) : problem("NOT_FOUND", 404);
    }
    const rows = await db.prepare(
      "SELECT * FROM " + model.table + " WHERE tenant_id = ? ORDER BY updated_at DESC LIMIT 300"
    ).bind(tenant).all();
    return reply({ items: rows.results || [] });
  }
  if (request.method === "POST" && !id || request.method === "PATCH" && id) {
    const data = parseRecord(name, await requestBody(request));
    if (!data) return problem("INVALID_INPUT", 422);
    if (data.customer_id && !(await belongsToTenant(db, data.customer_id, tenant)))
      return problem("CUSTOMER_NOT_FOUND", 422);
    if (id && !(await db.prepare(
      "SELECT id FROM " + model.table + " WHERE id = ? AND tenant_id = ?"
    ).bind(id, tenant).first())) return problem("NOT_FOUND", 404);
    const recordId = id || crypto.randomUUID(), stamp = new Date().toISOString();
    const values = model.fields.map(key => data[key]);
    const stmt = id
      ? db.prepare("UPDATE " + model.table + " SET " + model.fields.map(key => key + " = ?").join(", ") +
        ", updated_at = ? WHERE id = ? AND tenant_id = ?").bind(...values, stamp, id, tenant)
      : db.prepare("INSERT INTO " + model.table + " (id, tenant_id, " + model.fields.join(", ") +
        ", updated_at) VALUES (" + Array(model.fields.length + 3).fill("?").join(", ") + ")")
        .bind(recordId, tenant, ...values, stamp);
    await db.batch([
      stmt,
      db.prepare("INSERT INTO crm_audit (id, tenant_id, action, subject_id, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(crypto.randomUUID(), tenant, name + (id ? ".update" : ".create"), recordId, stamp),
    ]);
    return reply({ item: { id: recordId, tenant_id: tenant, ...data, updated_at: stamp } }, id ? 200 : 201);
  }
  if (request.method === "DELETE" && id) {
    const found = await db.prepare("SELECT id FROM " + model.table + " WHERE id = ? AND tenant_id = ?")
      .bind(id, tenant).first();
    if (!found) return problem("NOT_FOUND", 404);
    await db.batch([
      db.prepare("DELETE FROM " + model.table + " WHERE id = ? AND tenant_id = ?").bind(id, tenant),
      db.prepare("INSERT INTO crm_audit (id, tenant_id, action, subject_id, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(crypto.randomUUID(), tenant, name + ".delete", id, new Date().toISOString()),
    ]);
    return new Response(null, { status: 204 });
  }
  return problem("NOT_FOUND", 404);
}

export async function crmFetch(request, env, ctx) {
  const started = performance.now(), requestId = crypto.randomUUID(), url = new URL(request.url);
  let response;
  try {
    if (url.pathname === "/api/health" && request.method === "GET") {
      response = reply({ service: "crm-dev", status: "ok" });
    } else if (!url.pathname.startsWith("/api/")) {
      response = problem("NOT_FOUND", 404);
    } else if (!env.DB) {
      response = problem("DB_UNAVAILABLE", 503);
    } else {
      const actor = await actorFor(ctx, env.DB, request);
      if (!actor) response = problem("AUTH_REQUIRED", ctx.access ? 403 : 401);
      else if (url.pathname === "/api/session" && request.method === "GET")
        response = reply({ authenticated: true, role: actor.role });
      else if (url.pathname === "/api/version" && request.method === "GET")
        response = reply({ source: env.GIT_SHA || BUILD_SHA, database: "D1", api: "crm.dev/v1" });
      else if (url.pathname === "/api/doctor" && request.method === "GET") {
        // One non-mutating query verifies deployed D1 tables, under existing Access.
        const tables = await env.DB.prepare(
          "SELECT COUNT(*) AS present FROM sqlite_master WHERE type = 'table' AND name IN (" +
          moduleNames.map(() => "?").join(", ") + ")"
        ).bind(...moduleNames.map(name => models[name].table)).first();
        const present = Number(tables?.present ?? 0);
        response = reply({ source: env.GIT_SHA || BUILD_SHA,
          database: present === moduleNames.length ? "ok" : "missing_tables",
          modules: moduleNames, tables: { present, expected: moduleNames.length } },
          present === moduleNames.length ? 200 : 503);
      }
      else response = await handleRecords(request, env, actor, url);
    }
  } catch (error) {
    console.error(JSON.stringify({ event: "crm.error", request_id: requestId,
      code: "INTERNAL_ERROR", message: String(error?.message || error) }));
    response = reply({ error: errors.INTERNAL_ERROR, code: "INTERNAL_ERROR", request_id: requestId }, 500);
  }
  const headers = new Headers(response.headers);
  headers.set("x-request-id", requestId);
  console.log(JSON.stringify({
    event: "crm.request", request_id: requestId, method: request.method,
    path: url.pathname.replace(/[a-f0-9]{8}-[a-f0-9-]{27}/gi, ":id"),
    status: response.status, error_code: headers.get("x-crm-error-code") || undefined,
    duration_ms: Math.round((performance.now() - started) * 100) / 100,
  }));
  return new Response(response.body, { status: response.status, headers });
}

export default { fetch: crmFetch };

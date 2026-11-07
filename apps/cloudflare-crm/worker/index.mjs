// Minimal Cloudflare-native CRM API. One owner per feature; no bypass of Access.
export const BUILD_SHA = "development";

function json(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
      "x-content-type-options": "nosniff",
    },
  });
}

async function signedInActor(ctx, db) {
  // Identity only comes from Cloudflare Access, never an HTTP header.
  if (!ctx.access) return null;
  const identity = await ctx.access.getIdentity();
  const email = identity?.email?.toLowerCase();
  if (!email) return null;
  const row = await db.prepare(
    "SELECT tenant_id, role FROM crm_users WHERE email = ? AND active = 1",
  ).bind(email).first();
  return row ? { tenant: row.tenant_id, role: row.role } : null;
}

function parseCustomer(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const name = typeof value.name === "string" ? value.name.trim() : "";
  const kind = value.kind;
  if (!name || name.length > 120 || !["person", "organization"].includes(kind)) return null;
  return { name, kind };
}

async function readJson(request) {
  const size = Number(request.headers.get("content-length") ?? 0);
  if (!Number.isFinite(size) || size > 8192) return null;
  const body = await request.text();
  if (body.length > 8192) return null;
  try { return JSON.parse(body); } catch { return null; }
}

export async function crmFetch(request, env, ctx) {
  const started = Date.now();
  const requestId = crypto.randomUUID();
  const url = new URL(request.url);
  let outcome = 500;
  try {
    // Do not allow a public debug/development identity in the deployed app.
    if (url.pathname === "/api/health" && request.method === "GET") {
      const result = json({ service: "crm-dev", status: "ok" });
      outcome = result.status;
      return resultWithId(result, requestId);
    }
    if (!url.pathname.startsWith("/api/")) {
      outcome = 404;
      return resultWithId(json({ error: "Not found" }, 404), requestId);
    }
    if (!env.DB) {
      outcome = 503;
      return resultWithId(json({ error: "Database unavailable" }, 503), requestId);
    }
    const actor = await signedInActor(ctx, env.DB);
    if (!actor) {
      outcome = ctx.access ? 403 : 401;
      return resultWithId(json({ error: "Authentication required" }, outcome), requestId);
    }
    if (url.pathname === "/api/session" && request.method === "GET") {
      outcome = 200;
      return resultWithId(json({ authenticated: true, role: actor.role }), requestId);
    }
    if (url.pathname === "/api/version" && request.method === "GET") {
      outcome = 200;
      return resultWithId(json({ source: env.GIT_SHA ?? BUILD_SHA, database: "D1", api: "crm.dev/v1" }), requestId);
    }
    if (url.pathname === "/api/customers" && request.method === "GET") {
      const result = await env.DB.prepare(
        "SELECT id, name, kind, updated_at FROM crm_customers WHERE tenant_id = ? ORDER BY updated_at DESC LIMIT 100",
      ).bind(actor.tenant).all();
      outcome = 200;
      return resultWithId(json({ customers: result.results ?? [] }), requestId);
    }
    if (url.pathname === "/api/customers" && request.method === "POST") {
      const input = parseCustomer(await readJson(request));
      if (!input) {
        outcome = 422;
        return resultWithId(json({ error: "Customer name and type are required" }, 422), requestId);
      }
      const id = crypto.randomUUID();
      const stamp = new Date().toISOString();
      await env.DB.batch([
        env.DB.prepare(
          "INSERT INTO crm_customers (id, tenant_id, name, kind, updated_at) VALUES (?, ?, ?, ?, ?)",
        ).bind(id, actor.tenant, input.name, input.kind, stamp),
        env.DB.prepare(
          "INSERT INTO crm_audit (id, tenant_id, action, subject_id, created_at) VALUES (?, ?, ?, ?, ?)",
        ).bind(crypto.randomUUID(), actor.tenant, "customer.create", id, stamp),
      ]);
      outcome = 201;
      return resultWithId(json({ customer: { id, ...input, updated_at: stamp } }, 201), requestId);
    }
    const match = /^\/api\/customers\/([a-f0-9-]{36})$/.exec(url.pathname);
    if (match && request.method === "PATCH") {
      const input = parseCustomer(await readJson(request));
      if (!input) {
        outcome = 422;
        return resultWithId(json({ error: "Customer name and type are required" }, 422), requestId);
      }
      const existing = await env.DB.prepare(
        "SELECT id FROM crm_customers WHERE id = ? AND tenant_id = ?",
      ).bind(match[1], actor.tenant).first();
      if (!existing) {
        outcome = 404;
        return resultWithId(json({ error: "Customer not found" }, 404), requestId);
      }
      const stamp = new Date().toISOString();
      await env.DB.batch([
        env.DB.prepare(
          "UPDATE crm_customers SET name = ?, kind = ?, updated_at = ? WHERE id = ? AND tenant_id = ?",
        ).bind(input.name, input.kind, stamp, match[1], actor.tenant),
        env.DB.prepare(
          "INSERT INTO crm_audit (id, tenant_id, action, subject_id, created_at) VALUES (?, ?, ?, ?, ?)",
        ).bind(crypto.randomUUID(), actor.tenant, "customer.update", match[1], stamp),
      ]);
      outcome = 200;
      return resultWithId(json({ customer: { id: match[1], ...input, updated_at: stamp } }), requestId);
    }
    outcome = 404;
    return resultWithId(json({ error: "Not found" }, 404), requestId);
  } catch {
    // No stack, user email or SQL text in HTTP. Correlate through request_id.
    outcome = 500;
    return resultWithId(json({ error: "Unexpected error", request_id: requestId }, 500), requestId);
  } finally {
    console.log(JSON.stringify({
      event: "crm.request",
      request_id: requestId,
      method: request.method,
      path: url.pathname.replace(/[a-f0-9]{8}-[a-f0-9-]{27}/gi, ":id"),
      status: outcome,
      duration_ms: Date.now() - started,
    }));
  }
}

function resultWithId(response, requestId) {
  const headers = new Headers(response.headers);
  headers.set("x-request-id", requestId);
  return new Response(response.body, { status: response.status, headers });
}

export default {
  fetch(request, env, ctx) {
    return crmFetch(request, env, ctx);
  },
};

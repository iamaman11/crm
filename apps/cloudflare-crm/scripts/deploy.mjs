import { readFile } from "node:fs/promises";
import { join } from "node:path";

// Exact-SHA Cloudflare CRM technical pilot deployment. No UI assets yet.
// The only deployment credential is an environment-scoped GitHub Actions secret.
const account = "5bd5e1e1e813f061afc259e91334262d";
const database = "35fc7ee9-9710-485f-8fb3-f5a398b229b4";
const branch = "refs/heads/feature/cloudflare-native-crm-20261107";
const requireValue = (key) => {
  const value = process.env[key];
  if (!value) throw new Error("Required environment variable missing: " + key);
  return value;
};
const sha = requireValue("GITHUB_SHA");
if (!/^[0-9a-f]{40}$/.test(sha)) throw new Error("Invalid source SHA");
if (process.env.GITHUB_REF !== branch) throw new Error("Unsafe deployment ref");
if (process.env.GITHUB_EVENT_NAME !== "push") throw new Error("Deploy only on trusted push");
if (requireValue("CLOUDFLARE_ACCOUNT_ID") !== account) throw new Error("Wrong Cloudflare account");
if (requireValue("CRM_D1_DATABASE_ID") !== database) throw new Error("Wrong D1 target");

const token = requireValue("CLOUDFLARE_API_TOKEN");
const source = await readFile(join("apps", "cloudflare-crm", "worker", "index.mjs"), "utf8");
const metadata = {
  main_module: "index.mjs",
  bindings: [
    { type: "d1", name: "DB", id: database },
    { type: "plain_text", name: "GIT_SHA", text: sha },
  ],
  observability: { enabled: true, logs: { enabled: true }, traces: { enabled: true } },
};
const data = new FormData();
data.append("metadata", new Blob([JSON.stringify(metadata)], { type: "application/json" }));
data.append("index.mjs", new Blob([source], { type: "application/javascript+module" }), "index.mjs");
const api = "https://api.cloudflare.com/client/v4/accounts/" + account;
const response = await fetch(api + "/workers/scripts/crm-dev", {
  method: "PUT", headers: { Authorization: "Bearer " + token }, body: data,
});
const payload = await response.json();
if (!response.ok || !payload.success) {
  // Never print request headers, token, or unredacted response body.
  throw new Error("Cloudflare Worker upload failed (HTTP " + response.status +
    ", codes: " + (payload.errors || []).map(x => x.code).join(",") + ")");
}
const check = await fetch(api + "/workers/scripts", {
  headers: { Authorization: "Bearer " + token },
});
const scripts = await check.json();
if (!check.ok || !scripts.success ||
    !(scripts.result || []).some(x => x.id === "crm-dev")) {
  throw new Error("Deployed Worker failed Cloudflare API readback");
}
// Check real deployed bindings, not only the upload response.
const readback = await fetch(api + "/workers/scripts/crm-dev/settings", {
  headers: { Authorization: "Bearer " + token },
});
const settings = await readback.json();
const binding = (name) => (settings.result?.bindings || []).find(x => x.name === name);
if (!readback.ok || !settings.success ||
    binding("GIT_SHA")?.text !== sha || binding("DB")?.id !== database) {
  throw new Error("Cloudflare binding readback does not match exact source SHA/D1 target");
}
// A new script is not automatically reachable on workers.dev.
const subdomainEndpoint = api + "/workers/scripts/crm-dev/subdomain";
const currentRoute = await fetch(subdomainEndpoint, {
  headers: { Authorization: "Bearer " + token },
});
const route = await currentRoute.json();
if (!currentRoute.ok || !route.success) throw new Error("Worker route readback failed");
if (!route.result.enabled || route.result.previews_enabled) {
  const update = await fetch(subdomainEndpoint, {
    method: "POST",
    headers: { Authorization: "Bearer " + token, "Content-Type": "application/json" },
    body: JSON.stringify({ enabled: true, previews_enabled: false }),
  });
  const result = await update.json();
  if (!update.ok || !result.success) throw new Error("Worker route activation failed");
}
// Public visitors must encounter Access, never an unprotected API/SPA.
const url = "https://crm-dev.crm-5bd5e1e1.workers.dev/api/health";
const anonymous = await fetch(url, { redirect: "manual" });
if (anonymous.status === 200 || anonymous.status >= 500) {
  throw new Error("Unsafe/unavailable CRM Access boundary: HTTP " + anonymous.status);
}
const userId = requireValue("CRM_E2E_A_CLIENT_ID");
const userSecret = requireValue("CRM_E2E_A_CLIENT_SECRET");
const authorized = await fetch(url, {
  headers: { "CF-Access-Client-Id": userId, "CF-Access-Client-Secret": userSecret },
  redirect: "manual",
});
if (authorized.status !== 200) {
  throw new Error("Cloudflare Access service authentication failed: HTTP " + authorized.status);
}
const content = await authorized.json();
if (content.service !== "crm-dev" || content.status !== "ok") {
  throw new Error("Cloudflare authenticated health response mismatch");
}
console.log(JSON.stringify({
  event: "crm.deploy",
  service: "crm-dev",
  sha,
  account,
  database,
  state: "sha-d1-route-and-Access-smoke-verified",
  browserE2E: "NOT_TESTED",
}));

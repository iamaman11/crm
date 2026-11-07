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
console.log(JSON.stringify({
  event: "crm.deploy",
  service: "crm-dev",
  sha,
  account,
  database,
  state: "api-upload-verified",
  browserE2E: "NOT_TESTED",
}));

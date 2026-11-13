import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MCP_HOST } from "../src/security.ts";

type WranglerConfig = {
  name: string;
  account_id: string;
  workers_dev: boolean;
  preview_urls: boolean;
  vars: Record<string, string>;
};

test("MCP delivery route matches the verified Cloudflare Access application", () => {
  const config = JSON.parse(
    readFileSync(new URL("../wrangler.jsonc", import.meta.url), "utf8"),
  ) as WranglerConfig;
  assert.equal(config.name, "crm-browser-mcp");
  assert.equal(config.workers_dev, true,
    "protected workers.dev route is required for Managed OAuth /mcp");
  assert.equal(config.preview_urls, false, "no bypass via preview URLs");
  assert.equal(MCP_HOST, `${config.name}.crm-${config.account_id.slice(0, 8)}.workers.dev`,
    "MCP hostname must match exact published Access-protected origin");
  assert.match(config.vars.CRM_MCP_ACCESS_AUD ?? "", /^[0-9a-f]{64}$/,
    "Access application audience must be explicitly pinned");
  for (const secret of ["CRM_MCP_OWNER_EMAIL", "CRM_BROWSER_CLIENT_ID", "CRM_BROWSER_CLIENT_SECRET"]) {
    assert.equal(secret in config.vars, false,
      `${secret} must never appear in committed plain-text vars`);
  }
});

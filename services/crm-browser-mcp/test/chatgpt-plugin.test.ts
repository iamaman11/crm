import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MCP_HOST } from "../src/security.ts";

const root = new URL("../chatgpt-plugin/universal-browser-mcp/", import.meta.url);

test("private ChatGPT plugin points only to the canonical OAuth-protected Browser MCP", () => {
  const manifest = JSON.parse(readFileSync(new URL("plugin.json", root), "utf8"));
  const mcp = JSON.parse(readFileSync(new URL("mcp.json", root), "utf8"));
  assert.equal(manifest.$schema, "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json");
  assert.equal(mcp.$schema, "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json");
  assert.equal(manifest.name, "universal-browser-mcp");
  assert.equal(manifest.repository, "https://github.com/iamaman11/crm");
  const presentation = manifest.extensions["com.openai"].interface;
  assert.equal(presentation.displayName, "Universal Browser MCP");
  assert.ok(presentation.shortDescription.length <= 30);
  assert.equal(presentation.category, "Developer Tools");
  assert.deepEqual(Object.keys(mcp.mcpServers), ["universal-browser"]);
  const server = mcp.mcpServers["universal-browser"];
  assert.equal(server.type, "streamable-http");
  assert.equal(server.url, `https://${MCP_HOST}/mcp`);
  assert.deepEqual(server.extensions["com.openai"].auth,
    { type: "oauth", client: { mode: "dcr" } });
  const raw = readFileSync(new URL("mcp.json", root), "utf8");
  for (const forbidden of ["CF-Access-Client-Secret", "CLOUDFLARE_API_TOKEN",
    "CRM_BROWSER_CLIENT_SECRET", "Bearer "]) {
    assert.equal(raw.includes(forbidden), false, `no credentials in plugin MCP manifest: ${forbidden}`);
  }
});

test("portable skill explains authentic UI operations and CRM tenant isolation", () => {
  const text = readFileSync(new URL("skills/browser-operations/SKILL.md", root), "utf8");
  assert.match(text, /^---\nname: browser-operations\ndescription: /);
  assert.match(text, /real Playwright-backed MCP tools/);
  assert.match(text, /existing.*tenant/);
});

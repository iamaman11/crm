import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const source = (path: string) => readFileSync(new URL(path, import.meta.url), "utf8");

test("shared contracts are compile-time only and contain the common command types", () => {
  const contracts = source("../src/contracts.ts");
  const imports = contracts.split("\n").filter(line => line.startsWith("import "));
  assert.ok(imports.length > 0);
  assert.ok(imports.every(line => line.startsWith("import type ")),
    "contracts must not introduce runtime dependencies");
  for (const name of ["Env", "Target", "Command"]) {
    assert.match(contracts, new RegExp(`export (?:interface|type) ${name}\\b`));
  }
});

test("MCP controller identity and target-site credentials have separate owners", () => {
  const index = source("../src/index.ts");
  const session = source("../src/session.ts");
  const controller = source("../src/controller-auth.ts");
  const siteAccess = source("../src/site-access.ts");
  assert.match(index, /from "\.\/controller-auth"/);
  assert.match(session, /from "\.\/site-access"/);
  assert.doesNotMatch(controller, /SITE_ACCESS_|siteAccessTargets|siteRequestDisposition/);
  assert.doesNotMatch(siteAccess, /CRM_MCP_ALLOWED_EMAILS|CRM_MCP_ACCESS_AUD|verifiedMcpControllerEmail/);
  for (const code of [index, session]) assert.doesNotMatch(code, /from "\.\/security"/);
});

test("the Durable Object remains the only browser-state owner", () => {
  const session = source("../src/session.ts");
  const index = source("../src/index.ts");
  assert.match(session, /export class CrmBrowserSession extends DurableObject<Env>/);
  assert.match(index, /export \{ CrmBrowserSession \} from "\.\/session"/);
  assert.equal((session.match(/extends DurableObject</g) ?? []).length, 1);
});

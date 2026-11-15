import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as ts from "typescript";

// Check the exact SDK ownership contract in the actual production AST. Both
// acquire and recovery must opt in to persistent CDP, while retaining the
// existing browser contexts with blocked Service Workers.
test("all production Cloudflare Browser Run connects use typed persistent session options", () => {
  const code = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  const source = ts.createSourceFile("session.ts", code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  let connections = 0;
  let helperFound = false;
  function visit(node: ts.Node): void {
    if (ts.isFunctionDeclaration(node) && node.name?.text === "persistentConnectOptions") {
      helperFound = true;
      assert.equal(node.parameters.length, 1);
      assert.ok(node.body && node.body.statements.some(stmt =>
        ts.isReturnStatement(stmt) && stmt.expression &&
        ts.isObjectLiteralExpression(stmt.expression) &&
        stmt.expression.properties.some(p => ts.isPropertyAssignment(p) &&
          p.name.getText(source) === "persistent" && p.initializer.getText(source) === "true")
      ), "helper must expose persistent:true as a real SDK option");
    }
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) &&
        node.expression.text === "connect") {
      connections++;
      const arg = node.arguments[1];
      assert.ok(arg && ts.isCallExpression(arg) &&
        ts.isIdentifier(arg.expression) && arg.expression.text === "persistentConnectOptions",
        "every browser connection must pass the same typed persistent options");
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  assert.ok(helperFound, "persistent connection contract helper missing");
  assert.ok(connections >= 2, "both initial and recovery connection paths must be present");
  assert.match(code, /serviceWorkers:\s*"block"/, "SSRF interception relies on blocked Service Workers");
});

test("test CRM uses native Playwright continuation so Chromium owns the response", () => {
  const code = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  assert.ok(code.includes("await route.continue({\n          headers:"),
    "native Playwright continues the request with the configured test identity");
  assert.ok(code.includes('"CF-Access-Client-Id": this.env.SITE_ACCESS_CLIENT_ID!'));
  assert.ok(code.includes('"CF-Access-Client-Secret": this.env.SITE_ACCESS_CLIENT_SECRET!'));
  assert.ok(code.includes("siteRequestDisposition("),
    "site-origin request disposition is unchanged");
  assert.ok(!code.includes("route.fetch("),
    "MCP no longer downloads HTML, JS or CSS on Chromium's behalf");
  assert.ok(!code.includes("route.fulfill("),
    "MCP no longer replays responses without Content-Type");
});

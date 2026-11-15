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

test("test CRM uses native Playwright continuation so Chromium retains real MIME and response body", () => {
  const code = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  assert.match(code, /await route\\.continue\\(\\{\\s*headers:/,
    "the test service principal is injected by native Playwright request continuation");
  assert.match(code, /"CF-Access-Client-Id": this\\.env\\.SITE_ACCESS_CLIENT_ID!/);
  assert.match(code, /"CF-Access-Client-Secret": this\\.env\\.SITE_ACCESS_CLIENT_SECRET!/);
  assert.match(code, /siteRequestDisposition\\(/,
    "existing site-origin request disposition remains in place");
  assert.doesNotMatch(code, /route\\.fetch\\(/,
    "MCP must not download HTML, JS or CSS responses on Chromium's behalf");
  assert.doesNotMatch(code, /route\\.fulfill\\(/,
    "MCP must not replay HTTP bytes with missing Content-Type headers");
});

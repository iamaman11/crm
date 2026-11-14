import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as ts from "typescript";

// The exact Cloudflare Playwright connection contract: both initial acquire
// and DO restart must opt in to persistent CDP ownership. Extract from AST,
// not another manually maintained inventory of browser operations.
test("every production Browser Run connect uses persistent CDP transport", () => {
  const src = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  const ast = ts.createSourceFile("session.ts", src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  let connections = 0;
  const visit = (node: ts.Node): void => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) &&
        node.expression.text === "connect") {
      connections++;
      assert.equal(node.arguments.length, 2, "Cloudflare Browser Run requires endpoint and options");
      const arg = node.arguments[1];
      assert.ok(arg && ts.isObjectLiteralExpression(arg), "connect must use explicit session options");
      if (arg && ts.isObjectLiteralExpression(arg)) {
        const fields = new Map(arg.properties.filter(ts.isPropertyAssignment).map(p =>
          [p.name.getText(ast), p.initializer.getText(ast)]));
        // A shorthand sessionId is accepted for initial acquisition.
        const shorthand = arg.properties.some(p => ts.isShorthandPropertyAssignment(p) &&
          p.name.text === "sessionId");
        assert.equal(fields.get("persistent"), "true",
          "nonpersistent CDP context disappears after DO eviction");
        assert.ok(fields.has("sessionId") || shorthand,
          "reconnect must reattach existing Browser Run session ID");
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(ast);
  assert.ok(connections >= 2, "test must discover initial and recovery connection sites");
  assert.match(src, /serviceWorkers:\s*"block"/,
    "public-web SSRF routing requires blocked Service Workers in browser contexts");
});

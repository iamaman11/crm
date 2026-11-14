import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as ts from "typescript";

// Structural guard: discover tools from the actual source AST rather than
// keeping yet another manually maintained tool catalogue in CI.
test("every published MCP tool has the correct workspace contract", () => {
  const code = readFileSync(new URL("../src/index.ts", import.meta.url), "utf8");
  const source = ts.createSourceFile("index.ts", code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const tools = new Set<string>();
  function visit(node: ts.Node): void {
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) &&
        node.expression.name.text === "registerTool" &&
        ts.isStringLiteral(node.arguments[0]!)) {
      const name = (node.arguments[0] as ts.StringLiteral).text;
      assert.equal(tools.has(name), false, `duplicate MCP tool ${name}`);
      tools.add(name);
      const config = node.arguments[1];
      assert.ok(config && ts.isObjectLiteralExpression(config), `tool ${name} config`);
      if (!config || !ts.isObjectLiteralExpression(config)) return;
      const schema = config.properties.find(property => ts.isPropertyAssignment(property) &&
        property.name.getText(source) === "inputSchema");
      assert.ok(schema && ts.isPropertyAssignment(schema), `tool ${name} inputSchema`);
      if (!schema || !ts.isPropertyAssignment(schema)) return;
      assert.ok(ts.isObjectLiteralExpression(schema.initializer), `tool ${name} object schema`);
      if (!ts.isObjectLiteralExpression(schema.initializer)) return;
      const field = schema.initializer.properties.find(property =>
        ts.isPropertyAssignment(property) && property.name.getText(source) === "workspaceId");
      assert.ok(field && ts.isPropertyAssignment(field), `tool ${name} must select workspace`);
      if (field && ts.isPropertyAssignment(field)) {
        const value = field.initializer.getText(source);
        assert.equal(value, name === "browser_open"
          ? "workspaceIdSchema.optional()" : "workspaceIdSchema",
        `tool ${name} must ${name === "browser_open" ? "optionally create" : "require"} workspace`);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  assert.ok(tools.size > 0, "MCP tools were discovered from the source");
  assert.ok(tools.has("browser_open"));
  assert.match(code, /selectWorkspaceId\(payload.op, payload.workspaceId\)/);
  assert.doesNotMatch(code, /idFromName\("owner:/);
});

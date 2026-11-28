import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";
import * as ts from "typescript";

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

test("lifecycle has no independent browser owner and both tab and open share one context initializer", () => {
  const session = source("../src/session.ts");
  const lifecycle = source("../src/browser/lifecycle.ts");
  assert.match(session, /private readonly lifecycle: BrowserLifecycleState =/);
  assert.match(session, /state: this\.lifecycle,/);
  assert.match(session, /openBrowser\(this\.lifecycleDependencies\(\)/);
  assert.match(session, /restoreBrowser\(this\.lifecycleDependencies\(\)/);
  assert.match(session, /closeBrowser\(this\.lifecycleDependencies\(\)/);
  assert.match(session, /tabsBrowser\(this\.lifecycleDependencies\(\)/);
  assert.match(lifecycle, /state\.context \?\? await ensureBrowserContext\(/);
  assert.match(lifecycle, /await ensureBrowserContext\(deps, device\)/);
  assert.equal((lifecycle.match(/\.newContext\(/g) ?? []).length, 1);
  assert.doesNotMatch(session, /\.newContext\(/);
  assert.doesNotMatch(lifecycle, /extends DurableObject|from ["'].*session|from ["'].*index/);
});

test("entrypoint, tool adapters and browser capabilities have a single typed route", () => {
  const index = source("../src/index.ts");
  const tools = source("../src/tools.ts");
  const router = source("../src/command-router.ts");
  const session = source("../src/session.ts");
  assert.doesNotMatch(index, /registerTool\(/);
  assert.match(index, /createServer\(env, controllerEmail\)/);
  assert.match(tools, /executeWorkspaceCommand\(env, controllerEmail, payload\)/);
  assert.match(router, /stub\.fetch\(/);
  assert.match(session, /switch \(command\.op\)/);
  for (const [file, command] of [
    ["navigation", "navigationCommand"], ["inspection", "inspectionCommand"],
    ["interactions", "interactionCommand"], ["artifacts", "artifactCommand"], ["handoff", "structuredHandoff"],
  ]) {
    const code = source("../src/browser/" + file + ".ts");
    assert.match(session, new RegExp(command + "\\("));
    assert.match(code, new RegExp("export async function " + command + "\\("));
    assert.doesNotMatch(code, /extends DurableObject|\b(?:acquire|connect)\s*\(|from ["'].*(?:index|session|command-router|tools)["']/);
  }
  assert.doesNotMatch(session, /\.dragTo\(|\.ariaSnapshot\(|\.setInputFiles\(|\.route\(/);
});

test("runtime import graph is acyclic and only lifecycle acquires/connects browsers", () => {
  const root = fileURLToPath(new URL("../src/", import.meta.url));
  const graph = new Map<string, string[]>();
  const files = (dir: string): string[] => readdirSync(dir, { withFileTypes: true })
    .flatMap(entry => entry.isDirectory() ? files(join(dir, entry.name)) :
      entry.name.endsWith(".ts") ? [join(dir, entry.name)] : []);
  let doClasses = 0;
  let acquisitions = 0;
  let connections = 0;
  for (const file of files(root)) {
    const sourceFile = ts.createSourceFile(file, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true);
    const imports: string[] = [];
    for (const statement of sourceFile.statements) {
      let specifier: ts.Expression | undefined;
      if (ts.isImportDeclaration(statement)) {
        const clause = statement.importClause;
        if (clause?.isTypeOnly) continue;
        if (clause && !clause.name && clause.namedBindings &&
            ts.isNamedImports(clause.namedBindings) &&
            clause.namedBindings.elements.length > 0 &&
            clause.namedBindings.elements.every(el => el.isTypeOnly)) continue;
        specifier = statement.moduleSpecifier;
      } else if (ts.isExportDeclaration(statement) && !statement.isTypeOnly) {
        specifier = statement.moduleSpecifier;
      }
      if (!specifier || !ts.isStringLiteral(specifier) || !specifier.text.startsWith(".")) continue;
      const path = resolve(dirname(file), specifier.text);
      const target = [path, path + ".ts", join(path, "index.ts")].find(p => p.endsWith(".ts") && existsSync(p));
      assert.ok(target, "unresolved internal runtime import: " + specifier.text);
      imports.push(target);
    }
    graph.set(file, imports);
    const visit = (node: ts.Node): void => {
      if (ts.isClassDeclaration(node) && node.heritageClauses?.some(clause =>
        clause.token === ts.SyntaxKind.ExtendsKeyword && clause.types.some(t =>
          t.expression.getText(sourceFile) === "DurableObject"))) doClasses++;
      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) &&
          ["acquire", "connect"].includes(node.expression.text)) {
        assert.equal(relative(root, file).replaceAll("\\", "/"), "browser/lifecycle.ts");
        if (node.expression.text === "acquire") acquisitions++; else connections++;
      }
      ts.forEachChild(node, visit);
    };
    visit(sourceFile);
  }
  assert.equal(doClasses, 1, "one Durable Object class");
  assert.equal(acquisitions, 1, "one provider acquisition path");
  assert.equal(connections, 2, "initial and recovery connections only");
  const done = new Set<string>();
  const active = new Set<string>();
  function visit(file: string) {
    assert.ok(!active.has(file), "runtime import cycle: " + [...active, file].map(p => relative(root, p)).join(" -> "));
    if (done.has(file)) return;
    active.add(file);
    for (const dep of graph.get(file) ?? []) visit(dep);
    active.delete(file);
    done.add(file);
  }
  for (const file of graph.keys()) visit(file);
});

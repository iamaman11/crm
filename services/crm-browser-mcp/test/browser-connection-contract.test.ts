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

test("both download actions share one Chromium-side, same-origin byte path", () => {
  const code = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  assert.ok(code.includes('command.fileAction === "download_link" || command.fileAction === "download"'),
    "legacy and chunked download paths must share one browser mechanism");
  assert.ok(code.includes('const downloaded = await fetch(url.href,'),
    "the real page fetches its own same-origin download with cookies");
  assert.ok(code.includes('url.origin !== location.origin'),
    "cross-origin links must not be represented as downloadable bytes");
  assert.ok(code.includes('...transferDownload(response.name, [bytes])'),
    "legacy inline bytes use the existing size and nonempty checks");
  assert.ok(code.includes("this.transfers.storeDownload(response.name, response.mimeType, response.base64)"),
    "large verified file receipts still support chunk and discard");
  assert.ok(!code.includes('page.waitForEvent("download"'),
    "CDP event streams are not a supported file transport on Browser Run");
  assert.ok(!code.includes('await download.createReadStream('),
    "known empty CDP download streams must not be used again");
});

test("native keyboard press targets the requested locator, never a generic page when scoped", () => {
  const code = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  assert.match(code, /if \(command\.target\) \{\s*await this\.target\(command\.target\)\.press\(/,
    "targeted press must use the official Playwright locator.press API");
  assert.match(code, /else \{\s*await page\.keyboard\.press\(/,
    "non-targeted press retains the native page keyboard");
});

test("failed browser runs keep typed receipts compact and screenshot separately", () => {
  const session = readFileSync(new URL("../src/session.ts", import.meta.url), "utf8");
  const index = readFileSync(new URL("../src/index.ts", import.meta.url), "utf8");
  assert.ok(session.includes('key !== "latest_requests"'),
    "request diagnostics must not be duplicated in every run step");
  assert.ok(session.includes("this.lastFailureScreenshot = Buffer.from(png).toString"),
    "capture exactly the original failure image in workspace-local memory");
  assert.ok(session.includes('command.screenshotSource === "last_failure"'),
    "the existing screenshot tool retrieves a recorded failure separately");
  assert.ok(session.includes("this.lastFailureScreenshot = null"),
    "stale failure screenshots must be cleared");
  const failed = index.slice(index.indexOf("if (isFailedVerdict(data))"),
    index.indexOf("if (result.image)", index.indexOf("if (isFailedVerdict(data))")));
  assert.ok(failed.includes("isError: true"), "a failed scenario is an MCP error");
  assert.ok(!failed.includes('type: "image"'), "failed MCP tools must not inline PNGs in errors");
  assert.ok(index.includes('source: z.enum(["current","last_failure"])'),
    "no second screenshot MCP tool is introduced");
});

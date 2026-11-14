import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Miniflare } from "miniflare";

// Run the exact production CrmBrowserSession class compiled by the existing
// Wrangler dry run. Physically restart workerd against one persisted SQLite
// Durable Object storage, without test-only production API or Browser Run.
test("CrmBrowserSession preserves its provider identity across workerd restart", {
  timeout: 45_000,
}, async () => {
  const buildDir = process.env.MCP_TEST_BUNDLE_DIR;
  assert.ok(buildDir, "MCP_TEST_BUNDLE_DIR is required");
  const scripts = readdirSync(buildDir).filter(file => /\.(?:m?js)$/.test(file));
  assert.equal(scripts.length, 1,
    "expected exactly one Wrangler-built Worker entrypoint: " + scripts.join(", "));
  // Import the production compiled CrmBrowserSession; add only local RPC
  // helpers for seeding/inspecting its state, never changing its real restore.
  const fixturePath = join(buildDir, "__lifecycle_fixture.js");
  writeFileSync(fixturePath, [
    'import { CrmBrowserSession } from "./' + scripts[0] + '";',
    'export class LifecycleTestSession extends CrmBrowserSession {',
    '  async seedSynthetic(id) {',
    '    await this.ctx.storage.put("browser_session_id", id);',
    '    await this.ctx.storage.put("browser_recording", true);',
    '    await this.ctx.storage.put("browser_page_identity", {targetId:"target-before-restart",browserContextId:"incognito-before-restart"});',
    '    this.syntheticMemoryMarker = "alive-before-restart";',
    '  }',
    '  async storedId() { return await this.ctx.storage.get("browser_session_id"); }',
    '  async recorded() { return await this.ctx.storage.get("browser_recording"); }',
    '  async pageIdentity() { return await this.ctx.storage.get("browser_page_identity"); }',
    '  async memoryMarker() { return this.syntheticMemoryMarker ?? null; }',
    '}',
    'export default { fetch() { return new Response("fixture-only", {status: 404}); } };',
  ].join("\n") + "\n");
  const storageDir = mkdtempSync(join(tmpdir(), "crm-browser-mcp-do-"));
  const workspaceName = "owner:synthetic@example.invalid:workspace:df03890b-62b8-4d1c-a5c9-a8ff2b381035";
  const sessionId = "synthetic-provider-session-do-not-connect";
  const internal = "https://crm-browser-internal.invalid/internal";
  let mf;

  // Miniflare 5 accepts Cloudflare worker config and an explicit module
  // manifest. The compiled production bundle is loaded unchanged, together
  // with a local-only test subclass that exposes seeding/inspection RPCs.
  const start = () => new Miniflare({
    resourcePersistencePath: storageDir,
    workers: [{
      config: {
        name: "crm-browser-mcp-lifecycle-local",
        compatibilityDate: "2026-11-13",
        compatibilityFlags: ["nodejs_compat"],
        manifest: {
          mainModule: "__lifecycle_fixture.js",
          modulesRoot: buildDir,
          modules: {
            "__lifecycle_fixture.js": {
              type: "esm", contents: readFileSync(fixturePath, "utf8"),
            },
            [scripts[0]]: {
              type: "esm", contents: readFileSync(join(buildDir, scripts[0]), "utf8"),
            },
          },
        },
        exports: {
          LifecycleTestSession: { type: "durable-object", storage: "sqlite" },
        },
        env: {
          SESSIONS: {
            type: "durable-object",
            worker: "crm-browser-mcp-lifecycle-local",
            exportName: "LifecycleTestSession",
          },
        },
      },
    }],
  });

  const command = async (stub, cmd) => {
    const response = await stub.fetch(internal, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(cmd),
    });
    assert.equal(response.status, 200);
    return response.json();
  };

  try {
    mf = start();
    const ns = await mf.getDurableObjectNamespace("SESSIONS");
    const originalId = ns.idFromName(workspaceName);
    const originalIdString = originalId.toString();
    const stub = ns.get(originalId);
    const probe = await command(stub, { op: "runtime" });
    assert.equal(probe.ok, true);
    assert.match(probe.data.epoch, /^browser-mcp-\d{8}-fence-\d+$/);

    await stub.seedSynthetic(sessionId);
    assert.equal(await stub.storedId(), sessionId);
    assert.equal(await stub.recorded(), true);
    assert.deepEqual(await stub.pageIdentity(), {targetId:"target-before-restart",browserContextId:"incognito-before-restart"});
    assert.equal(await stub.memoryMarker(), "alive-before-restart");

    // Disposing the host discards DO and Playwright in-memory handles.
    // A separate workerd host must reconstruct the object from its storage.
    await mf.dispose();
    mf = undefined;

    mf = start();
    const recreatedNs = await mf.getDurableObjectNamespace("SESSIONS");
    const recreatedId = recreatedNs.idFromName(workspaceName);
    assert.equal(recreatedId.toString(), originalIdString);
    const recreated = recreatedNs.get(recreatedId);
    const reprobe = await command(recreated, { op: "runtime" });
    assert.equal(reprobe.ok, true);
    assert.equal(reprobe.data.epoch, probe.data.epoch);

    assert.equal(await recreated.storedId(), sessionId);
    assert.equal(await recreated.recorded(), true);
    assert.deepEqual(await recreated.pageIdentity(), {targetId:"target-before-restart",browserContextId:"incognito-before-restart"});
    assert.equal(await recreated.memoryMarker(), null,
      "a new DO process must not preserve its previous in-memory marker");

    // This isolated test intentionally has no Browser Run binding. Restoring
    // an inaccessible provider must fail closed WITHOUT discarding its saved
    // session ID, allowing a later real provider reconnect to retry safely.
    const result = await command(recreated, {
      op: "status", expectedRuntimeEpoch: reprobe.data.epoch,
    });
    assert.equal(result.ok, false, "missing Browser Run cannot be reported open");
    assert.equal(await recreated.storedId(), sessionId);
    assert.equal(await recreated.recorded(), true);
    assert.deepEqual(await recreated.pageIdentity(), {targetId:"target-before-restart",browserContextId:"incognito-before-restart"},
      "provider outage must not erase the original incognito target identity");
  } finally {
    if (mf) await mf.dispose();
    rmSync(storageDir, { recursive: true, force: true });
    rmSync(fixturePath, { force: true });
  }
});

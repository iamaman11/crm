import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
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
  const scriptPath = join(buildDir, scripts[0]);
  const storageDir = mkdtempSync(join(tmpdir(), "crm-browser-mcp-do-"));
  const workspaceName = "owner:synthetic@example.invalid:workspace:df03890b-62b8-4d1c-a5c9-a8ff2b381035";
  const sessionId = "synthetic-provider-session-do-not-connect";
  const internal = "https://crm-browser-internal.invalid/internal";
  let mf;

  const start = () => new Miniflare({
    modules: true,
    scriptPath,
    compatibilityDate: "2026-11-13",
    compatibilityFlags: ["nodejs_compat"],
    durableObjects: {
      SESSIONS: { className: "CrmBrowserSession", useSQLite: true },
    },
    durableObjectsPersist: storageDir,
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
    const stub = ns.get(originalId);
    const probe = await command(stub, { op: "runtime" });
    assert.equal(probe.ok, true);
    assert.match(probe.data.epoch, /^browser-mcp-\d{8}-fence-\d+$/);

    const storage = await mf.getDurableObjectStorage(originalId);
    await storage.put("browser_session_id", sessionId);
    await storage.put("browser_recording", true);
    assert.equal(await storage.get("browser_session_id"), sessionId);

    // Disposing the host discards DO and Playwright in-memory handles.
    // A separate workerd host must reconstruct the object from its storage.
    await mf.dispose();
    mf = undefined;

    mf = start();
    const recreatedNs = await mf.getDurableObjectNamespace("SESSIONS");
    const recreatedId = recreatedNs.idFromName(workspaceName);
    assert.equal(recreatedId.toString(), originalId.toString());
    const recreated = recreatedNs.get(recreatedId);
    const reprobe = await command(recreated, { op: "runtime" });
    assert.equal(reprobe.ok, true);
    assert.equal(reprobe.data.epoch, probe.data.epoch);

    const stored = await mf.getDurableObjectStorage(recreatedId);
    assert.equal(await stored.get("browser_session_id"), sessionId);
    assert.equal(await stored.get("browser_recording"), true);

    // This isolated test intentionally has no Browser Run binding. Restoring
    // an inaccessible provider must fail closed WITHOUT discarding its saved
    // session ID, allowing a later real provider reconnect to retry safely.
    const result = await command(recreated, {
      op: "status", expectedRuntimeEpoch: reprobe.data.epoch,
    });
    assert.equal(result.ok, false, "missing Browser Run cannot be reported open");
    assert.equal(await stored.get("browser_session_id"), sessionId);
    assert.equal(await stored.get("browser_recording"), true);
  } finally {
    if (mf) await mf.dispose();
    rmSync(storageDir, { recursive: true, force: true });
  }
});

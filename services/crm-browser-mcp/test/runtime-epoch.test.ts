import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import {
  BROWSER_RUNTIME_EPOCH, requireRuntimeEpoch, isVerifiedRuntimeProbe, cleanupRuntimeEpoch,
} from "../src/runtime-epoch.ts";

test("a compatible DO responds to a read-only runtime probe", () => {
  assert.equal(isVerifiedRuntimeProbe({ok:true,data:{epoch:BROWSER_RUNTIME_EPOCH}}),true);
  assert.doesNotThrow(()=>requireRuntimeEpoch(BROWSER_RUNTIME_EPOCH));
});

test("old, missing, malformed and unavailable DO epochs fail before any browser mutation", () => {
  for (const input of [
    null, undefined, {}, {ok:false,error:"CRM_BROWSER_UNKNOWN_OPERATION"},
    {ok:true}, {ok:true,data:{}},
    {ok:true,data:{epoch:"old-runtime"}}, {ok:true,data:{epoch:null}},
  ]) assert.equal(isVerifiedRuntimeProbe(input),false);
  for (const v of [undefined,"", "old-runtime"]) {
    assert.throws(()=>requireRuntimeEpoch(v), /BROWSER_RUNTIME_EPOCH_MISMATCH/);
  }
});


test("PRs changing browser runtime source must advance its compatibility epoch", () => {
  // Same canonical npm test job; no extra workflow or hand-maintained RPC list.
  if (process.env.GITHUB_EVENT_NAME !== "pull_request" || !process.env.GITHUB_EVENT_PATH) return;
  const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, "utf8")) as {
    pull_request?: { base?: { sha?: string } };
  };
  const base = event.pull_request?.base?.sha ?? "";
  assert.match(base, /^[0-9a-f]{40}$/);
  const changed = execFileSync("git", ["diff", "--name-only", base, "HEAD"], {
    cwd: new URL("../../../", import.meta.url),
    encoding: "utf8",
  }).trim().split("\n").filter(Boolean);
  if (changed.some(path => path.startsWith("services/crm-browser-mcp/src/") && path.endsWith(".ts"))) {
    assert.ok(changed.includes("services/crm-browser-mcp/src/runtime-epoch.ts"),
      "MCP runtime source changed but BROWSER_RUNTIME_EPOCH was not advanced");
  }
});

test("explicit stale cleanup can use a verified older DO epoch without allowing old actions", () => {
  const oldEpoch = "browser-mcp-20261114-fence-6";
  const reply = {ok:true,data:{epoch:oldEpoch}};
  assert.equal(cleanupRuntimeEpoch(reply),oldEpoch);
  assert.equal(isVerifiedRuntimeProbe(reply),false,
    "the same stale DO must never be authorized for any non-close action");
  assert.throws(()=>requireRuntimeEpoch(oldEpoch),/BROWSER_RUNTIME_EPOCH_MISMATCH/);
  assert.equal(cleanupRuntimeEpoch({ok:true,data:{epoch:BROWSER_RUNTIME_EPOCH}}),
    BROWSER_RUNTIME_EPOCH);
});

test("unsupported or forged cleanup probe versions are never trusted", () => {
  for (const input of [
    null,undefined,{}, {ok:false,data:{epoch:"browser-mcp-20261114-fence-6"}},
    {ok:true}, {ok:true,data:{epoch:""}}, {ok:true,data:{epoch:"../../admin"}},
    {ok:true,data:{epoch:"browser-mcp-20261114-fence-0"}},
    {ok:true,data:{epoch:"browser-mcp-20261114-fence-3\nextra"}},
    {ok:true,data:{epoch:42}},
  ]) assert.equal(cleanupRuntimeEpoch(input),BROWSER_RUNTIME_EPOCH);
});

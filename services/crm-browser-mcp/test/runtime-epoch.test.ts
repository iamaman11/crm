import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import {
  BROWSER_RUNTIME_EPOCH, requireRuntimeEpoch, isVerifiedRuntimeProbe,
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

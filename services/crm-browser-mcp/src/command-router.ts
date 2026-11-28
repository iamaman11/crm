// The single controller/workspace command route and bounded MCP response adapter.
import type { Command, Env } from "./contracts";
import { isFailedVerdict } from "./browser-checks";
import { selectWorkspaceId, workspaceObjectName } from "./browser-workspaces";
import { BROWSER_RUNTIME_EPOCH, isVerifiedRuntimeProbe, cleanupRuntimeEpoch } from "./runtime-epoch";

export async function executeWorkspaceCommand(env: Env, controllerEmail: string, payload: Command) {
  const workspaceId = selectWorkspaceId(payload.op, payload.workspaceId);
  const sessionId = env.SESSIONS.idFromName(workspaceObjectName(controllerEmail, workspaceId));
  const stub = env.SESSIONS.get(sessionId);
  // Workspace identity is solely a routing selector, never page-supplied.
  const { workspaceId: _transportWorkspaceId, ...browserCommand } = payload;
  const send = (command: Command) => stub.fetch(new Request(
    "https://crm-browser-internal.invalid/internal", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(command),
    },
  ));
  // A browser can outlive the deploying Worker. Normal operations require
  // THIS code epoch. Cleanup is the sole exception: interrogate the exact
  // old DO and use its read-only epoch only to close its original session.
  let expectedEpoch: string = BROWSER_RUNTIME_EPOCH;
  if (payload.op === "close") {
    try {
      const probe = await send({ op: "runtime" });
      if (probe.ok) expectedEpoch = cleanupRuntimeEpoch(await probe.json());
    } catch { /* Fallback to current epoch for normal closes. */ }
  } else {
    let verified = false;
    try {
      const probe = await send({ op: "runtime" });
      if (probe.ok) verified = isVerifiedRuntimeProbe(await probe.json());
    } catch { /* Provider outage/old DO: fail closed. */ }
    if (!verified) return {
      isError: true,
      content: [{ type: "text" as const, text: "BROWSER_RUNTIME_STALE_OR_UNVERIFIED" }],
    };
  }
  // Check again in the DO: protects races where the isolate restarts between
  // the probe and this action. Never trust caller-supplied runtime versions.
  const reply = await send({
    ...browserCommand, expectedRuntimeEpoch: expectedEpoch,
  });
  if (!reply.ok) throw new Error("CRM_BROWSER_SESSION_UNAVAILABLE");
  const result = await reply.json() as {
    ok: boolean; data?: unknown; image?: string; error?: string;
  };
  if (!result.ok) return {
    isError: true,
    content: [{ type: "text" as const, text: result.error ?? "CRM_BROWSER_OPERATION_FAILED" }],
  };
  // Only a successful browser_open reveals a minted workspace handle.
  const data = payload.op === "open" && result.data && typeof result.data === "object"
    ? { ...result.data, workspaceId } : result.data;
  // A failed assertion/scenario remains an MCP error with a small typed
  // receipt. Never attach a multi-megabyte PNG to a failed tool call: some
  // MCP clients stringify the entire image inside their error message.
  // The exact failure PNG, if requested, is retrieved separately through
  // browser_screenshot(source="last_failure") in this workspace.
  if (isFailedVerdict(data)) return {
    isError: true,
    content: [{ type: "text" as const, text: JSON.stringify(data) }],
  };
  if (result.image) return {
    content: [
      { type: "text" as const, text: JSON.stringify(data) },
      { type: "image" as const, data: result.image, mimeType: "image/png" },
    ],
  };
  return { content: [{ type: "text" as const, text: JSON.stringify(data) }] };
}


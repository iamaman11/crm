// Read-only control plane: every business write goes through the real CRM UI.
import { McpServer } from "@modelcontextprotocol/server";
import { createMcpHandler } from "agents/mcp/server";
import { z } from "zod";
import type { Env, Command, Target } from "./session";
import { MCP_HOST, verifiedMcpOwnerEmail } from "./security";
import { SCENARIO_OPERATIONS, isFailedVerdict } from "./browser-checks";
import { selectWorkspaceId, workspaceObjectName } from "./browser-workspaces";
import { BROWSER_RUNTIME_EPOCH, isVerifiedRuntimeProbe, cleanupRuntimeEpoch } from "./runtime-epoch";

export { CrmBrowserSession } from "./session";

const frameIndex = z.number().int().min(0).max(49).optional();
const target = z.object({
  kind: z.enum(["css", "role", "text", "label"]),
  value: z.string().min(1).max(512),
  name: z.string().min(1).max(512).optional(),
  frameIndex,
});
const action = z.enum([
  "click", "double_click", "fill", "type", "select", "check",
  "uncheck", "hover", "press", "scroll", "navigate", "reload", "back", "forward",
]);
const assertionKind = z.enum(["url", "title", "visible", "hidden", "text", "count"]);
const waitState = z.enum(["visible", "hidden", "attached", "detached"]);
const timeoutMs = z.number().int().min(100).max(30_000).optional();
const workspaceIdSchema = z.string().uuid();
const expected = z.union([z.string().max(512), z.number().finite(), z.boolean()]);
const scenarioStep = z.object({
  op: z.enum(SCENARIO_OPERATIONS),
  target: target.optional(),
  text: z.string().max(16_000).optional(),
  key: z.string().max(96).optional(),
  value: z.string().max(1_000).optional(),
  url: z.string().max(1_024).optional(),
  x: z.number().finite().min(-5000).max(5000).optional(),
  y: z.number().finite().min(-5000).max(5000).optional(),
  acceptDialog: z.boolean().optional(),
  kind: assertionKind.optional(),
  expected: expected.optional(),
  match: z.enum(["exact", "contains"]).optional(),
  state: waitState.optional(),
  timeoutMs,
});


function createServer(env: Env, ownerEmail: string) {
  const server = new McpServer({ name: "crm-browser-mcp", version: "0.1.0" });
  async function command(payload: Command) {
    const workspaceId = selectWorkspaceId(payload.op, payload.workspaceId);
    const sessionId = env.SESSIONS.idFromName(workspaceObjectName(ownerEmail, workspaceId));
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
    // A failed assertion/scenario must not appear as an MCP success.
    if (isFailedVerdict(data)) return {
      isError: true,
      content: [
        { type: "text" as const, text: JSON.stringify(data) },
        ...(result.image ? [{ type: "image" as const, data: result.image, mimeType: "image/png" as const }] : []),
      ],
    };
    if (result.image) return {
      content: [
        { type: "text" as const, text: JSON.stringify(data) },
        { type: "image" as const, data: result.image, mimeType: "image/png" },
      ],
    };
    return { content: [{ type: "text" as const, text: JSON.stringify(data) }] };
  }

  server.registerTool("browser_open", {
    description: "Open an authorized public HTTP(S) website in a reusable Cloudflare Playwright session. Recording is opt-in at session creation only and is accessed later through Cloudflare Browser Run.",
    inputSchema: {
      workspaceId: workspaceIdSchema.optional(),
      path: z.string().max(1024).default("/"),
      recording: z.boolean().default(false),
    },
  }, async ({ path, recording, workspaceId }) => command({ op: "open", url: path, recording, workspaceId }));

  server.registerTool("browser_status", {
    description: "Inspect whether the CRM browser is open without launching a new browser.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "status", workspaceId }));

  server.registerTool("browser_session", {
    description: "Read the real CRM /api/session response as the diagnostic Access principal, with no impersonation.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "session", workspaceId }));

  server.registerTool("browser_snapshot", {
    description: "Return the current page or selected iframe accessibility tree and title. Use browser_frames to discover frameIndex.",
    inputSchema: { workspaceId: workspaceIdSchema, frameIndex },
  }, async params => command({ op: "snapshot", ...params }));

  server.registerTool("browser_tabs", {
    description: "List, create, select or close tabs and popup windows in the one existing browser context; tab IDs are ephemeral after reconnection.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      tabAction: z.enum(["list", "new", "select", "close"]).default("list"),
      tabId: z.string().uuid().optional(),
      url: z.string().max(1024).optional(),
    },
  }, async params => command({ op: "tabs", ...params }));

  server.registerTool("browser_frames", {
    description: "List main document and iframe indices, names and sanitized URLs. Use frameIndex for snapshot, action and assertions.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "frames", workspaceId }));

  server.registerTool("browser_file", {
    description: "Upload explicit in-memory bytes to a file input, or click to download at most 1 MB into this authenticated MCP response. No Worker filesystem path or arbitrary URL fetch.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      fileAction: z.enum(["upload", "download"]),
      target,
      fileName: z.string().max(128).optional(),
      mimeType: z.string().max(100).optional(),
      base64: z.string().max(1_333_336).optional(),
    },
  }, async params => command({ op: "file", ...params }));

  server.registerTool("browser_inspect", {
    description: "Read a sanitized network timeline, browser navigation/paint performance timings, basic DOM accessibility heuristics or a capped PDF. No arbitrary code execution or credential-bearing headers.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      inspection: z.enum(["network", "performance", "accessibility", "pdf"]),
    },
  }, async params => command({ op: "inspect", ...params }));

  server.registerTool("browser_viewport", {
    description: "Resize the active tab for responsive layout testing (320-2560 pixels); does not impersonate a physical mobile device.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      width: z.number().int().min(320).max(2560),
      height: z.number().int().min(320).max(2560),
    },
  }, async params => command({ op: "viewport", ...params }));

  server.registerTool("browser_screenshot", {
    description: "Capture an actual full-page PNG screenshot of the currently open website.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "screenshot", workspaceId }));

  server.registerTool("browser_action", {
    description: "Perform one real Playwright UI action in the persistent browser. Browse ordinary public HTTP(S) sites. CRM admin actions use the real CRM UI, Connect and Rust.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      action,
      target: target.optional(),
      text: z.string().max(16000).optional(),
      key: z.string().max(96).optional(),
      value: z.string().max(1000).optional(),
      url: z.string().max(1024).optional(),
      x: z.number().finite().min(-5000).max(5000).optional(),
      y: z.number().finite().min(-5000).max(5000).optional(),
      acceptDialog: z.boolean().optional(),
    },
  }, async (params) => command({
    op: params.action, workspaceId: params.workspaceId, target: params.target as Target | undefined,
    text: params.text, key: params.key, value: params.value,
    url: params.url, x: params.x, y: params.y, acceptDialog: params.acceptDialog,
  }));

  server.registerTool("browser_wait", {
    description: "Deterministically wait for an element state, a URL, or DOM content load. Always bounded to 30 seconds; no arbitrary sleeps.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      target: target.optional(),
      state: waitState.optional(),
      url: z.string().max(1024).optional(),
      timeoutMs,
    },
  }, async params => command({ op: "wait", ...params }));

  server.registerTool("browser_assert", {
    description: "Read and compare actual DOM, URL, or title values; return a typed pass/fail verdict (no false success on mismatch).",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      kind: assertionKind,
      target: target.optional(),
      expected,
      match: z.enum(["exact", "contains"]).optional(),
    },
  }, async params => command({ op: "assert", ...params }));

  server.registerTool("browser_run", {
    description: "Prevalidate 12 or fewer typed Playwright steps, bound waits/actions to a run budget and stop on first failure. Return an explicit pass/fail report.",
    inputSchema: {
      workspaceId: workspaceIdSchema,
      steps: z.array(scenarioStep).min(1).max(12),
      timeoutMs: z.number().int().min(1_000).max(120_000).default(90_000),
      screenshotOnFailure: z.boolean().default(false),
    },
  }, async ({ steps, screenshotOnFailure, timeoutMs, workspaceId }) =>
    command({ op: "run", steps, screenshotOnFailure, timeoutMs, workspaceId }));

  server.registerTool("browser_diagnostics", {
    description: "Read sanitized cross-site browser console and HTTP statuses. Credential values and URL queries are never exposed.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "console", workspaceId }));

  server.registerTool("browser_live_view", {
    description: "Mint a 60-second read-only Cloudflare Live View link for the current tab. The signed URL is sensitive: share only with the authorized operator.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "live_view", workspaceId }));

  server.registerTool("browser_close", {
    description: "Close the cloud browser session and release Cloudflare Browser Run resources.",
    inputSchema: { workspaceId: workspaceIdSchema },
  }, async ({ workspaceId }) => command({ op: "close", workspaceId }));

  return server;
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    if (new URL(request.url).hostname !== MCP_HOST) return new Response("Not found", { status: 404 });
    if (new URL(request.url).pathname !== "/mcp") return new Response("Not found", { status: 404 });
    // Cloudflare Access must authenticate the *human controller*. Its signed
    // ctx.access is not forwarded from another Worker and cannot be model-supplied.
    const email = await verifiedMcpOwnerEmail(
      ctx.access, env.CRM_MCP_ACCESS_AUD, env.CRM_MCP_OWNER_EMAIL,
    );
    if (!email) return new Response("Access forbidden", { status: 403 });
    return createMcpHandler(
      () => createServer(env, email),
      {
        route: "/mcp",
        corsOptions: false,
        allowedHostnames: [MCP_HOST],
        allowedOriginHostnames: [MCP_HOST],
        responseMode: "json",
      },
    )(request, env, ctx);
  },
} satisfies ExportedHandler<Env>;

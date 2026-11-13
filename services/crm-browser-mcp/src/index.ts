// Read-only control plane: every business write goes through the real CRM UI.
import { McpServer } from "@modelcontextprotocol/server";
import { createMcpHandler } from "agents/mcp/server";
import { z } from "zod";
import type { Env, Command, Target } from "./session";
import { MCP_HOST, verifiedMcpOwnerEmail } from "./security";

export { CrmBrowserSession } from "./session";

const target = z.object({
  kind: z.enum(["css", "role", "text", "label"]),
  value: z.string().min(1).max(512),
  name: z.string().min(1).max(512).optional(),
});
const action = z.enum([
  "click", "double_click", "fill", "type", "select", "check",
  "uncheck", "hover", "press", "scroll", "navigate", "reload", "back", "forward",
]);
const assertionKind = z.enum(["url", "title", "visible", "hidden", "text", "count"]);
const waitState = z.enum(["visible", "hidden", "attached", "detached"]);
const timeoutMs = z.number().int().min(100).max(30_000).optional();
const expected = z.union([z.string().max(512), z.number().finite(), z.boolean()]);
const scenarioStep = z.object({
  op: z.enum([
    "click", "double_click", "fill", "type", "select", "check",
    "uncheck", "hover", "press", "scroll", "navigate", "reload",
    "back", "forward", "wait", "assert",
  ]),
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
  const sessionId = env.SESSIONS.idFromName("owner:" + ownerEmail.toLowerCase());
  const stub = env.SESSIONS.get(sessionId);

  async function command(payload: Command) {
    const reply = await stub.fetch(new Request("https://crm-browser-internal.invalid/internal", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    }));
    if (!reply.ok) throw new Error("CRM_BROWSER_SESSION_UNAVAILABLE");
    const result = await reply.json() as {
      ok: boolean; data?: unknown; image?: string; error?: string;
    };
    if (!result.ok) return {
      isError: true,
      content: [{ type: "text" as const, text: result.error ?? "CRM_BROWSER_OPERATION_FAILED" }],
    };
    if (result.image) return {
      content: [
        { type: "text" as const, text: JSON.stringify(result.data) },
        { type: "image" as const, data: result.image, mimeType: "image/png" },
      ],
    };
    return { content: [{ type: "text" as const, text: JSON.stringify(result.data) }] };
  }

  server.registerTool("browser_open", {
    description: "Open the real CRM by default, or any public HTTP(S) site. Preserve cloud browser state across calls; CRM service credentials are restricted to CRM.",
    inputSchema: {
      path: z.string().max(1024).default("/"),
    },
  }, async ({ path }) => command({ op: "open", url: path }));

  server.registerTool("browser_status", {
    description: "Inspect whether the CRM browser is open without launching a new browser.",
    inputSchema: {},
  }, async () => command({ op: "status" }));

  server.registerTool("browser_session", {
    description: "Read the real CRM /api/session response as the diagnostic Access principal, with no impersonation.",
    inputSchema: {},
  }, async () => command({ op: "session" }));

  server.registerTool("browser_snapshot", {
    description: "Return the current website accessibility tree and page title for reliable element targeting.",
    inputSchema: {},
  }, async () => command({ op: "snapshot" }));

  server.registerTool("browser_screenshot", {
    description: "Capture an actual full-page PNG screenshot of the currently open website.",
    inputSchema: {},
  }, async () => command({ op: "screenshot" }));

  server.registerTool("browser_action", {
    description: "Perform one real Playwright UI action in the persistent browser. Browse ordinary public HTTP(S) sites. CRM admin actions use the real CRM UI, Connect and Rust.",
    inputSchema: {
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
    op: params.action, target: params.target as Target | undefined,
    text: params.text, key: params.key, value: params.value,
    url: params.url, x: params.x, y: params.y, acceptDialog: params.acceptDialog,
  }));

  server.registerTool("browser_wait", {
    description: "Deterministically wait for an element state, a URL, or DOM content load. Always bounded to 30 seconds; no arbitrary sleeps.",
    inputSchema: {
      target: target.optional(),
      state: waitState.optional(),
      url: z.string().max(1024).optional(),
      timeoutMs,
    },
  }, async params => command({ op: "wait", ...params }));

  server.registerTool("browser_assert", {
    description: "Read and compare actual DOM, URL, or title values; return a typed pass/fail verdict (no false success on mismatch).",
    inputSchema: {
      kind: assertionKind,
      target: target.optional(),
      expected,
      match: z.enum(["exact", "contains"]).optional(),
    },
  }, async params => command({ op: "assert", ...params }));

  server.registerTool("browser_run", {
    description: "Execute up to 12 ordered Playwright actions/waits/assertions and stop on first failure. Return machine-readable results for every executed step.",
    inputSchema: {
      steps: z.array(scenarioStep).min(1).max(12),
    },
  }, async ({ steps }) => command({ op: "run", steps }));

  server.registerTool("browser_diagnostics", {
    description: "Read sanitized cross-site browser console and HTTP statuses. Credential values and URL queries are never exposed.",
    inputSchema: {},
  }, async () => command({ op: "console" }));

  server.registerTool("browser_live_view", {
    description: "Mint a 60-second read-only Cloudflare Live View link for the current tab. The signed URL is sensitive: share only with the authorized operator.",
    inputSchema: {},
  }, async () => command({ op: "live_view" }));

  server.registerTool("browser_close", {
    description: "Close the cloud browser session and release Cloudflare Browser Run resources.",
    inputSchema: {},
  }, async () => command({ op: "close" }));

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

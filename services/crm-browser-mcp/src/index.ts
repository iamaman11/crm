// Read-only control plane: every business write goes through the real CRM UI.
import { createMcpHandler } from "agents/mcp/server";
import type { Env } from "./contracts";
import { verifiedMcpControllerEmail } from "./controller-auth";
import { MCP_HOST, MCP_PATH } from "./endpoint";
import { createServer } from "./tools";

export { CrmBrowserSession } from "./session";

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    if (new URL(request.url).hostname !== MCP_HOST) return new Response("Not found", { status: 404 });
    if (new URL(request.url).pathname !== MCP_PATH) return new Response("Not found", { status: 404 });
    // Cloudflare Access must authenticate the *human controller*. Its signed
    // ctx.access is not forwarded from another Worker and cannot be model-supplied.
    const controllerEmail = await verifiedMcpControllerEmail(
      ctx.access, env.CRM_MCP_ACCESS_AUD, env.CRM_MCP_ALLOWED_EMAILS,
    );
    if (!controllerEmail) return new Response("Access forbidden", { status: 403 });
    return createMcpHandler(
      () => createServer(env, controllerEmail),
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

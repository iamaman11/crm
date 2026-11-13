/**
 * One-time inert first-version bootstrap for the pre-created crm-browser-mcp Worker.
 * This is NOT the MCP runtime and must never be used as a release artifact.
 * Public routes and preview URLs remain disabled until owner acceptance.
 */
export default {
  async fetch() {
    return new Response("Browser MCP not released", {
      status: 503,
      headers: {
        "Content-Type": "text/plain; charset=utf-8",
        "Cache-Control": "no-store",
      },
    });
  },
};

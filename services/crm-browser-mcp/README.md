# CRM Browser MCP — candidate, NOT DEPLOYED

Canonical plan: https://github.com/iamaman11/crm/issues/357

This is not a CRM business backend. The real CRM remains the single Rust
Connect Worker at https://crm-dev.crm-5bd5e1e1.workers.dev/ and existing D1.
Browser writes must pass React -> Connect -> Rust -> D1, like a human user.

## Architecture
- Separate protected crm-browser-mcp Cloudflare Worker with MCP SDK v2 stateless
  transport, Access Managed OAuth, one Durable Object per owner, and native
  Cloudflare Browser Run / @cloudflare/playwright.
- Controller: real owner authenticated by Cloudflare Access, checked through
  ctx.access plus exact CRM_MCP_OWNER_EMAIL secret; no owner JWT forging.
- Browser actor: distinct signed Access service principal, to be enrolled as
  admin of the existing owner's tenant. This is NOT a CI service token.
- Browser intercepts every request; foreign origins are blocked, CRM Access
  service headers are attached only to the exact CRM origin.
- No direct D1 writes, Cloudflare API mutations, arbitrary JS execution,
  shared owner cookie, or duplicate CRM backend.
- Browser errors, URLs and HTTP logs must not disclose secrets or query strings.

## Available candidate tools
browser_open, browser_status, browser_session, browser_snapshot,
browser_screenshot, browser_action, browser_diagnostics, browser_close.

Actions: click, double_click, fill, type, select, check, uncheck, hover, press,
scroll, navigate, reload, back, forward, with Playwright role/text/label/CSS
selectors. Browser state lives in one Durable Object. Idle/browser expiry
must be reported and recovered by an explicit reopen.

## Mandatory gates BEFORE ANY deploy or agent admin token provisioning
1. Generate and commit independent exact package-lock.json for this isolated
   package. Run npm ci, npm run check, npm test with Node 24. No unpinned
   dependencies or unverified transitive changes in release.
2. Validate latest Cloudflare Playwright/context routing/ariaSnapshot/screenshot,
   Agents MCP SDK v2 registration and DO reconnect/expiry against real Browser
   Run; run fail-closed tests for redirects and credential isolation.
3. Verify existing MCP Access application is Managed OAuth, and only the human
   CRM owner can control it; test anonymous and unrelated identity denial.
4. Create ONE distinct Cloudflare Access service token for the real CRM app.
   Bind verified service:<id> to admin in precisely the existing human owner's
   tenant in one existing D1. Never move/impersonate the owner or reuse CI A/B.
   Store credentials only as Cloudflare Worker secrets, not code or logs.
5. Deploy separate crm-browser-mcp Worker; leave the Rust CRM Worker, its
   routes, D1 schema, existing release and one CI unchanged.
6. Prove two-way test-record visibility between the agent's UI and the human
   owner's ordinary CRM URL, plus audit and Connect request correlation.
7. Extend advanced tools (tabs, upload, download, Live View, trace, timings)
   after first accepted same-tenant end-to-end. These features are NOT yet
   implemented in candidate code.
8. Connect MCP explicitly to ChatGPT as a private plugin and demonstrate tools
   callable in this chat. Deployment is not the same as tool installation.

Until these gates pass, this branch is DRAFT / NO-GO and is not a deployed app.

## Official references
- https://developers.cloudflare.com/browser-run/playwright/
- https://developers.cloudflare.com/agents/model-context-protocol/apis/handler-api/
- https://developers.cloudflare.com/workers/configuration/cloudflare-access/
- https://developers.cloudflare.com/browser-run/features/reuse-sessions/

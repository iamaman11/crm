# Universal Browser MCP — candidate, NOT RELEASED

Canonical plan: https://github.com/iamaman11/crm/issues/357

This is an independent universal browser automation tool (public HTTP/HTTPS
websites supported), not a CRM business backend. The real CRM remains the single Rust
Connect Worker at https://crm-dev.crm-5bd5e1e1.workers.dev/ and existing D1.
Browser writes must pass React -> Connect -> Rust -> D1, like a human user.

## 13 November 2026 — first-version Cloudflare bootstrap, not release

The previously empty Cloudflare Worker `crm-browser-mcp` received one deliberately inert first version from this canonical repository: [`bootstrap/deny.mjs`](bootstrap/deny.mjs). It always returns HTTP 503 and implements **no MCP functionality**. The Worker immutable ID is unchanged (`47a154994f254ca99e5d6d9b964412ee`).

Cloudflare confirms `subdomain.enabled=false` and `previews_enabled=false`, and a public request to the workers.dev address returns HTTP 404. The actual MCP remains **Draft / NO-GO**, uninstalled, without deployment credentials or owner/CRM tenant onboarding; release automation stays disabled. The new `workers_dev:false` in `wrangler.jsonc` is intentional until secured MCP acceptance. A bootstrap upload is **not** approval or deployment of real MCP code.

The Vault first-token request failed at Cloudflare API with HTTP 500/1004; its request ID is `crm-browser-mcp-github-deploy-20261113-01`. Vault status remains `outcome_unknown` with no receipt; Cloudflare full token inventory shows no matching new token. Do not blindly repeat POST or widen Worker privileges. See [Issue #357](https://github.com/iamaman11/crm/issues/357) for the readbacks and remaining acceptance gates.

## Architecture
- Separate protected crm-browser-mcp Cloudflare Worker with MCP SDK v2 stateless
  transport, Access Managed OAuth, one Durable Object per owner, and native
  Cloudflare Browser Run / @cloudflare/playwright.
- Controller: real owner authenticated by Cloudflare Access, checked through
  ctx.access plus exact CRM_MCP_OWNER_EMAIL secret; no owner JWT forging.
- Browser actor: distinct signed Access service principal, to be enrolled as
  admin of the existing owner's tenant. This is NOT a CI service token.
- The browser can navigate and interact with external public websites.
  Only non-web schemes, direct local/private network addresses, and embedded
  URL credentials are rejected. CRM Access service headers are attached only
  to the exact CRM origin, and only when requests originate from CRM or are
  explicitly triggered by a trusted controller navigation. Cross-site
  redirects are followed without privileged headers (no route.continue
  header overrides). External browser sessions can use ordinary site logins.
- No direct D1 writes, Cloudflare API mutations, arbitrary JS execution,
  shared owner cookie, or duplicate CRM backend. The universal browsing
  surface is separate from the CRM-only service identity.
- Browser errors, URLs and HTTP logs must not disclose secrets or query strings.

## Available candidate tools
browser_open, browser_status, browser_session, browser_snapshot,
browser_screenshot, browser_action, browser_diagnostics, browser_live_view,
browser_close.

Actions: click, double_click, fill, type, select, check, uncheck, hover, press,
scroll, navigate, reload, back, forward, with Playwright role/text/label/CSS
selectors, including third-party websites. Browser state lives in one Durable Object. Idle/browser expiry
must be reported and recovered by an explicit reopen.

## Mandatory gates BEFORE ANY deploy or agent admin token provisioning
1. Generate and commit independent exact package-lock.json for this isolated
   package. Run npm ci, npm run check, npm test with Node 24. No unpinned
   dependencies or unverified transitive changes in release.
2. Validate latest Cloudflare Playwright/context routing/ariaSnapshot/screenshot,
   Agents MCP SDK v2 registration and DO reconnect/expiry against real Browser
   Run; run fail-closed tests for redirects, redirects initiated by CRM, and
   credential isolation. Run full browsing scenario on public external sites
   and login flows, including multi-origin CSS/assets and normal links.
3. Verify existing MCP Access application is Managed OAuth, and only the human
   CRM owner can control it; test anonymous and unrelated identity denial.
4. Create ONE distinct Cloudflare Access service token for the real CRM app.
   Bind verified service:<id> to admin in precisely the existing human owner's
   tenant in one existing D1. Never move/impersonate the owner or reuse CI A/B.
   Store credentials only as Cloudflare Worker secrets, not code or logs.
5. Deploy the separate crm-browser-mcp Worker from its own `mcp` branch
   and workflow; leave the Rust CRM Worker, routes, D1 and dev CRM CI unchanged.
6. Prove two-way test-record visibility between the agent's UI and the human
   owner's ordinary CRM URL, plus audit and Connect request correlation.
7. Extend advanced tools (tabs, upload, download, Live View, trace, timings,
   page-level JS/permissions where explicitly authorized, multiple contexts)
   after first accepted same-tenant end-to-end. These features are NOT yet
   implemented in candidate code.
8. Connect MCP explicitly to ChatGPT as a private plugin and demonstrate tools
   callable in this chat. Deployment is not the same as tool installation.

Until these gates pass, this branch is DRAFT / NO-GO and is not a deployed app.


### Physical proof and secure preview boundary (13 Nov 2026)
- The real Cloudflare Browser Run account opened public `https://example.org/`
  through Cloudflare's Browser Run REST action and returned "Example Domain"
  from the rendered HTML. This is a **browser service smoke**, not a deployed
  MCP/authorized CRM action. Several rapid additional Quick Actions produced
  HTTP 429; do not spam retries or assume a specific plan/quota without evidence.
- The Worker validates both the Cloudflare-signed Access **audience** and the
  unique human owner's email. `CRM_MCP_ACCESS_AUD` is the real Access app
  audience, stored as public Wrangler configuration; the owner email is a
  Worker secret. Wrong audience, missing identity, and other users are denied.
- Native TypeScript and 8 security unit tests **PASS** on Windows: public
  websites, private/loopback URL rejection (including page-initiated links and
  redirects), precise CRM credential scope and Access audience/owner checks.
- The `browser_live_view` command asks the **native BrowserRun binding** for
  a 60-second **read-only** signed link. This URL must not be recorded in logs.
  The real command has **not** been exercised on a deployed MCP yet.
- GitHub Environment `browser-mcp` exists and permits deployment from the
  `mcp` branch **only**. No Cloudflare deployment token has been provisioned
  in that Environment; the Windows environment token tested as unauthorized/
  forbidden and must not be reused. The release job now exists but is disabled
  by the repository variable `BROWSER_MCP_RELEASE_ENABLED=false`.
- This candidate is **Draft / NO-GO**, pending live MCP OAuth authentication,
  Browser Run Playwright session operation, redirect safety, service principal
  enrollment into exactly the existing human owner's CRM tenant, and explicit
  installation in ChatGPT. The CRM Rust Worker and D1 are unchanged.

## Official references
- https://developers.cloudflare.com/browser-run/playwright/
- https://developers.cloudflare.com/agents/model-context-protocol/apis/handler-api/
- https://developers.cloudflare.com/workers/configuration/cloudflare-access/
- https://developers.cloudflare.com/browser-run/features/reuse-sessions/

## Build and release ownership — separate permanent branch and workflow
The **only** canonical source of project code is `iamaman11/crm`, but it has
two separate app release lanes:
- CRM: permanent `dev` branch; existing `.github/workflows/cloudflare-crm-spike.yml`;
  Rust/WASM + React, current D1 and ordinary CRM deployment.
- Universal Browser MCP: permanent `mcp` branch; independent
  `.github/workflows/browser-mcp.yml`; separate TypeScript bundle and Browser Run
  Cloudflare Worker. PR #358 targets `mcp`, never `dev`.

The Browser MCP package has a dedicated pinned `package-lock.json`, generated
and committed from the independent Browser MCP GitHub Actions run. Its CI
checks the exact dependency install, strict types, security tests and isolated
Wrangler dry-run. No CRM Rust build, D1 migration, or crm-dev deployment occurs
on `mcp`. Actual MCP publication remains disabled until the identity/tenant,
OAuth, real browser E2E and privacy controls are independently accepted.

The candidate is still Draft / NO-GO: the CI build is proven, but deployed MCP,
owner-tenant service principal, real end-to-end authorization, and ChatGPT
connector remain to prove.

### Operator release checklist (do not bypass)

- Required browser-mcp check must pass on final PR HEAD, merged only to mcp.
- The BROWSER_MCP_RELEASE_ENABLED repository switch stays false while any
  identity, authorization, network safety, or service-token gate is unproven.
- The dedicated browser-mcp Environment needs its own valid Cloudflare API
  token. Never reuse the unauthorized Windows token or the CRM release token.
- Publishing changes only the crm-browser-mcp Worker. The workflow contains no
  D1 migration or CRM deployment command.
- Disable the release switch again to stop future automatic publication.
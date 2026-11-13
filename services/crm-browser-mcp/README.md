# Universal Browser MCP — protected runtime deployed; OAuth/E2E pending

Canonical project: https://github.com/iamaman11/crm  
Specification and live acceptance ledger: https://github.com/iamaman11/crm/issues/357  
Implementation: [PR #358](https://github.com/iamaman11/crm/pull/358) merged to protected `mcp`; [PR #359](https://github.com/iamaman11/crm/pull/359) merged for the ChatGPT plugin package.

## Purpose — general-purpose browser automation

This is a **universal, remotely hosted Browser MCP** for ChatGPT and other authorized MCP clients. It is **not** limited to the CRM hostname and is **not** another CRM business service. It is designed to operate permitted public HTTP(S) websites by reading the rendered interface, clicking, typing, navigating, inspecting browser output, testing workflows, and returning verifiable results.

The **first acceptance target** is the user's real CRM at
`https://crm-dev.crm-5bd5e1e1.workers.dev/`.
The browser acts through that CRM's real React UI → Connect → Rust Worker → D1, with the **same existing owner tenant** and appropriate administrative permissions. No parallel CRM API, direct D1 writes, or surrogate tenant are allowed.

## One runtime and one owner of browser state

```text
ChatGPT / authorized MCP client
          |
Cloudflare Access / Managed OAuth (human controller)
          |
crm-browser-mcp Worker (MCP tools; no CRM business logic)
          |
CrmBrowserSession Durable Object (one session owner)
          |
Cloudflare Browser Run + official @cloudflare/playwright
          |
Permitted public HTTP(S) sites, including existing CRM UI
          |
On CRM: existing Connect API → Rust business logic → existing D1
```

- The main `dev/main` branches, CRM deployment pipeline, React/Rust business code and D1 migrations must remain untouched by the isolated MCP release.
- Browser Run + Playwright own the browser. The DO only coordinates session state, isolation, commands and cleanup. It never implements browser engine operations itself.
- Service tokens for CRM are **origin-specific**: only explicit trusted controller-initiated CRM navigation or requests already originating within CRM may use them. External pages, redirects, frames and assets must not receive these credentials.
- Other websites may use their ordinary authorized login processes; do not share CRM cookies or service tokens with them.
- Network policy must be checked against DNS resolution and redirects, not only initial URL parsing; Cloudflare Browser Run limitations and anti-bot restrictions must be described truthfully. Never promise access to every site regardless of its policies.
- Logs, screenshots, browser traces and Live View links may contain sensitive data: avoid persisting raw credentials and carefully control release, access and retention.

## Candidate tools and present implementation

| Tool | Purpose | Current state |
|---|---|---|
| `browser_open`, `browser_close`, `browser_status` | Browser session lifecycle | Candidate code; live proof pending |
| `browser_action` | Navigate/click/double-click/fill/type/select/check/hover/press/scroll/back/forward/reload | Candidate code; live proof pending |
| `browser_snapshot` | Accessibility snapshot and page title | Candidate code; live proof pending |
| `browser_screenshot` | Full-page screenshot | Candidate code; live proof pending |
| `browser_diagnostics` | Sanitized console errors and HTTP status history | Candidate code; richer trace required |
| `browser_live_view` | Short-lived, read-only, signed Live View URL | Candidate code; live proof pending |
| `browser_session` | Current CRM `/api/session` result using the real browser | CRM-specific diagnostic, not a generic-browser restriction |

Next capabilities must be built **inside this same MCP**, not as separate systems: tab/popup/context selection; uploads and downloads; navigation across multiple public domains; traces, timings and network debugging; deterministic assertions and test reporting; reconnect/expiry of Browser Run sessions; optional constrained page evaluation (explicitly authorized and safety-reviewed). See the capability matrix and acceptance checklist in Issue #357.

## Current verified deployment and authentication (14 Nov 2026)

- **Real Universal Browser MCP deployed.** [CI/deploy #43510554028](https://github.com/iamaman11/crm/actions/runs/43510554028) passed from protected `mcp` commit `70119e852c5107cd8337abdac70c2325b6bb2218`. Cloudflare version `cec55b95-58f4-4f5a-ade7-e2e7ce14e4fe` is live at 100%, with the official Browser Run binding `BROWSER` and `SESSIONS` Durable Object binding. The sole published origin is `https://crm-browser-mcp.crm-5bd5e1e1.workers.dev/mcp`; preview URLs are disabled.
- Cloudflare Access `crm-browser-mcp-access` protects the origin with a single owner-email policy. `CRM_MCP_OWNER_EMAIL` is a protected Worker secret. Anonymous GET and MCP initialize POST both receive HTTP 401 and OAuth `WWW-Authenticate`, never a tool response.
- Access Managed OAuth exposes protected-resource discovery, the authorization server, PKCE S256 and DCR. **DCR was initially misconfigured:** only localhost/loopback callbacks were allowed, causing `HTTP 400 invalid_client_metadata: redirect_uri is not allowed by the account configuration`. The exact allowed redirect URI list and live positive/negative tests are documented below.
- [PR #359](https://github.com/iamaman11/crm/pull/359) added the canonical private ChatGPT plugin package in `chatgpt-plugin/universal-browser-mcp/`. Private plugin exists, but human OAuth consent and authenticated `browser_status` / `browser_open` tool invocations from ChatGPT are **not yet proven**.
- **CRM administrative browser access remains intentionally disabled** until a distinct service principal is explicitly bound to the existing human-owner tenant. Neither `CRM_BROWSER_CLIENT_ID` nor `CRM_BROWSER_CLIENT_SECRET` is present. Do not reuse CI credentials or create a parallel CRM tenant.
- GitHub Environment `browser-mcp` holds an independently scoped `CLOUDFLARE_API_TOKEN` (Vault receipt [Issue #357](https://github.com/iamaman11/crm/issues/357#issuecomment-6864793396)); access to primary `crm-dev` was denied with HTTP 403. **`BROWSER_MCP_RELEASE_ENABLED=false`**, no automatic release, CRM and D1 unchanged.

## ChatGPT OAuth DCR redirect allowlist — physical acceptance

Cloudflare Access Managed OAuth must accept the actual ChatGPT callback URLs; enabling DCR alone is **insufficient**. The Access application `crm-browser-mcp-access` must keep `oauth_configuration.enabled=true` and `dynamic_client_registration.enabled=true`, with **only** these production callback patterns in `dynamic_client_registration.allowed_uris`:

```json
{
  "allowed_uris": [
    "https://chatgpt.com/connector/oauth/*",
    "https://chatgpt.com/connector_platform_oauth_redirect"
  ]
}
```

Cloudflare supports a trailing path `/*` for a fixed HTTPS host. OpenAI uses `/connector/oauth/{callback_id}` where OAuth metadata lacks RFC 9207 issuer-in-response (as in our Cloudflare Access metadata), and may use the stable `/connector_platform_oauth_redirect` callback in compatible cases. **Do not** allow arbitrary redirect hosts, external wildcards, or bypass the Cloudflare Access owner policy.

Provider change and independent verification on 14 Nov 2026:
- **Before:** DCR POST with real MCP resource + ChatGPT stable redirect failed HTTP 400 `invalid_client_metadata`, explanation `redirect_uri is not allowed by the account configuration`.
- **After:** DCR POST for each ChatGPT callback form succeeded HTTP 201 with a client ID issued. Non-ChatGPT `https://example.org/oauth/callback` still failed HTTP 400; anonymous MCP GET still HTTP 401 with OAuth challenge. Policy ID, audience and Worker version were unchanged.
- **Remaining:** retry creating the custom MCP app inside ChatGPT, select DCR, leave unadvertised scopes empty and disable optional OIDC discovery when unavailable, then complete the owner's real OAuth login and tools smoke. Never claim a successful ChatGPT connection before the UI and tools confirm it.

Cloudflare Access app configuration is account infrastructure managed via its API, **not** Wrangler Worker source. This README records the expected configuration alongside the canonical deployment evidence; changing it requires readback of the exact Access application, not any changes to `crm-dev`.

Official references: [OpenAI OAuth callback requirements](https://developers.openai.com/plugins/build/auth#redirect-url) and [Cloudflare Access Managed OAuth](https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/managed-oauth/).

## Confirmed R0 navigation fix (14 Nov 2026)

The former `browser_open(url)` implementation returned `already_open` whenever any page was open, even if `url` was a completely different public site. It now navigates the existing Playwright Page when the requested URL differs; same-URL reuse retains the no-op. `browser_open` and `browser_action(navigate)` share `navigateTo` and the testable `navigatePage` lifecycle. New regression tests verify actual `Page.goto` invocation, trusted CRM navigation being temporary, public-domain navigation not elevating CRM trust, and cleanup on redirects/failure. Live browser proof is **still pending**.

## Release acceptance gates

1. **Functional code and CI:** independent `.github/workflows/browser-mcp.yml`, pinned dependencies, strict TypeScript, unit/security tests and Wrangler dry-run. No CRM build or D1 migration. Verify the actual PR HEAD, not an older successful run.
2. **Safe reachable endpoint:** choose an Access-protected `workers.dev` hostname or controlled route; bind it to real worker code and verify `MCP_HOST`, Cloudflare Access audience, policy and Wrangler routing match. The deployed hostname is Access-protected `workers.dev`; no preview route is enabled.
3. **Human controller auth:** only the owner is authorized. Prove correct signed identity/audience and rejection of anonymous and unrelated users. Never trust a user-supplied email or header.
4. **Safe universal navigation:** visit at least two independent public websites; verify ordinary links, redirects, assets, forms, navigation and failures. Test private/loopback/metadata destinations, DNS and redirect bypasses, and protected CRM credentials staying on their exact origin.
5. **CRM tenant identity:** create a distinct Access service token and allow it in the existing CRM Access app; explicitly map its verified `service:<common_name>` identity as admin of the **existing** owner's tenant before navigation. Do not reuse CI tokens or just check `/api/session` role. Independently correlate tenant and records.
6. **Physical E2E:** ChatGPT → MCP → Browser Run → real CRM UI create/read/edit/delete → owner's ordinary CRM UI sees the same records → existing Connect/Rust/D1/audit confirm the same mutations.
7. **Session and diagnostics:** prove close, timeout/expiry, reconnect, Live View and logs without exposing privileged secrets. Add advanced tools without creating another browser owner.
8. **Explicit installation:** connect the published private MCP to ChatGPT and successfully invoke the tools. A Cloudflare release alone is not acceptance.

Keep the release flag OFF except during a reviewed independent MCP release. **Do not treat CI, a published Worker or anonymous OAuth metadata as proof that ChatGPT has authenticated and run a real tool.**

## Workflow ownership

- Canonical repository: `iamaman11/crm`, single Issue #357 for specification and evidence.
- MCP branch: permanent protected `mcp`; feature PRs #358 and #359 merged there.
- MCP workflow: `.github/workflows/browser-mcp.yml`, GitHub Environment `browser-mcp`, dedicated Worker only.
- Main CRM: `dev/main`, own existing CI and Worker; no dependency on the MCP release.

## Reference documentation

- [Cloudflare Browser Run Playwright](https://developers.cloudflare.com/browser-run/playwright/)
- [Browser session reuse](https://developers.cloudflare.com/browser-run/features/reuse-sessions/)
- [Cloudflare Access protection for Workers](https://developers.cloudflare.com/workers/configuration/cloudflare-access/)
- [Agents MCP handler](https://developers.cloudflare.com/agents/model-context-protocol/apis/handler-api/)

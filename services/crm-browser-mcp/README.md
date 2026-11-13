# Universal Browser MCP — Draft / NO-GO

Canonical project: https://github.com/iamaman11/crm  
Specification and live acceptance ledger: https://github.com/iamaman11/crm/issues/357  
Implementation review: https://github.com/iamaman11/crm/pull/358 (base: protected `mcp` branch).

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

## Current proven deployment/auth state (14 Nov 2026)

- PR #358 remains **Draft / NO-GO**. Current CI [#43508066499](https://github.com/iamaman11/crm/actions/runs/43508066499) succeeded on HEAD `4ab634412b617ac90dabfa6e84d94e97b216dffd`: TypeScript, security/navigation regression tests, source boundaries and Wrangler dry-run PASS; deployment job SKIPPED. This does **not** establish a live browser or MCP connection.
- Cloudflare `crm-browser-mcp` has a bootstrap-only `bootstrap/deny.mjs` first version `8d103dbe-3c8b-4811-9b41-5b7ec4c9fd91`; it is not the code in `src/`. Public `workers.dev` and preview routes are disabled, no working MCP is exposed.
- Cloudflare Access app `crm-browser-mcp-access` exists and its Managed OAuth is enabled according to read-only API verification; the actual authorized ChatGPT OAuth/MCP flow has **not yet been accepted**.
- **Deployment credential provisioning is COMPLETE**, as confirmed by the latest [Issue #357 Vault receipt comment](https://github.com/iamaman11/crm/issues/357#issuecomment-6864793396): exactly one `Individual Workers Editor` token for `crm-browser-mcp`, a tested HTTP 403 for `crm-dev`, encrypted in Windows Vault, and securely installed in GitHub Environment `browser-mcp` under `CLOUDFLARE_API_TOKEN`. Do **not** issue it again, broaden it or transfer it into the main CRM environment.
- `BROWSER_MCP_RELEASE_ENABLED=false` remains mandatory. Token provisioning did not authorize a code release.
- A distinct Cloudflare Access service identity for browsing the owner's existing CRM tenant is **not** onboarded yet. In the current CRM, an unknown verified service principal can be auto-enrolled into a different tenant. Explicit owner-tenant mapping must occur before the first CRM request.

Earlier 13 November status notes about failed Cloudflare POST 500/1004 and missing GitHub secret have been superseded; full evidence is retained in Issue #357 comments/history.

## Confirmed R0 navigation fix (14 Nov 2026)

The former `browser_open(url)` implementation returned `already_open` whenever any page was open, even if `url` was a completely different public site. It now navigates the existing Playwright Page when the requested URL differs; same-URL reuse retains the no-op. `browser_open` and `browser_action(navigate)` share `navigateTo` and the testable `navigatePage` lifecycle. New regression tests verify actual `Page.goto` invocation, trusted CRM navigation being temporary, public-domain navigation not elevating CRM trust, and cleanup on redirects/failure. Live browser proof is **still pending**.

## Release acceptance gates

1. **Functional code and CI:** independent `.github/workflows/browser-mcp.yml`, pinned dependencies, strict TypeScript, unit/security tests and Wrangler dry-run. No CRM build or D1 migration. Verify the actual PR HEAD, not an older successful run.
2. **Safe reachable endpoint:** choose an Access-protected `workers.dev` hostname or controlled route; bind it to real worker code and verify `MCP_HOST`, Cloudflare Access audience, policy and Wrangler routing match. Current `workers_dev:false` / no alternate route is a **deliberate bootstrap-only deny**, not a live endpoint.
3. **Human controller auth:** only the owner is authorized. Prove correct signed identity/audience and rejection of anonymous and unrelated users. Never trust a user-supplied email or header.
4. **Safe universal navigation:** visit at least two independent public websites; verify ordinary links, redirects, assets, forms, navigation and failures. Test private/loopback/metadata destinations, DNS and redirect bypasses, and protected CRM credentials staying on their exact origin.
5. **CRM tenant identity:** create a distinct Access service token and allow it in the existing CRM Access app; explicitly map its verified `service:<common_name>` identity as admin of the **existing** owner's tenant before navigation. Do not reuse CI tokens or just check `/api/session` role. Independently correlate tenant and records.
6. **Physical E2E:** ChatGPT → MCP → Browser Run → real CRM UI create/read/edit/delete → owner's ordinary CRM UI sees the same records → existing Connect/Rust/D1/audit confirm the same mutations.
7. **Session and diagnostics:** prove close, timeout/expiry, reconnect, Live View and logs without exposing privileged secrets. Add advanced tools without creating another browser owner.
8. **Explicit installation:** connect the published private MCP to ChatGPT and successfully invoke the tools. A Cloudflare release alone is not acceptance.

Keep the PR Draft, release flag OFF, and the bootstrap isolated until the gates are met. **Never treat a successful compiler, unit test or dry-run as proof of a working remote browser.**

## Workflow ownership

- Canonical repository: `iamaman11/crm`, single Issue #357 for specification and evidence.
- MCP branch: permanent protected `mcp`, feature PR #358 based exclusively on it.
- MCP workflow: `.github/workflows/browser-mcp.yml`, GitHub Environment `browser-mcp`, dedicated Worker only.
- Main CRM: `dev/main`, own existing CI and Worker; no dependency on the MCP release.

## Reference documentation

- [Cloudflare Browser Run Playwright](https://developers.cloudflare.com/browser-run/playwright/)
- [Browser session reuse](https://developers.cloudflare.com/browser-run/features/reuse-sessions/)
- [Cloudflare Access protection for Workers](https://developers.cloudflare.com/workers/configuration/cloudflare-access/)
- [Agents MCP handler](https://developers.cloudflare.com/agents/model-context-protocol/apis/handler-api/)

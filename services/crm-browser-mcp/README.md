# Universal Browser MCP — extended release deployed; physical acceptance in progress

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

## MCP capabilities and acceptance (P0/P1/P2)

[PR #361](https://github.com/iamaman11/crm/pull/361) **merged**, and
[release CI #43525548488](https://github.com/iamaman11/crm/actions/runs/43525548488)
built and deployed the expanded MCP bundle from `mcp` commit `20c11c2e7a20628511cf1ddab3b53f50ce6580c7`.
Cloudflare confirmed version `c17a8db8-76f9-4b74-96da-443095a9e83c` at **100%**.
The deployed server defines **17 tools**, and all **17/17** were actually
discoverable and callable in a new owner-authenticated ChatGPT conversation
on 14 November 2026. The previous 9-tool snapshot was historical, not a
current server blocker. The Cloudflare release remains the previous R1 build;
Draft PR #363 is **not** released. Below, physical acceptance differentiates
the previous published build's verified behavior from untested capabilities.
After future MCP tool schema updates, use ChatGPT Plugins → Refresh → new chat,
as documented at https://developers.openai.com/plugins/deploy/connect-chatgpt .

| Tool | Deployed functionality | Physical proof |
|---|---|---|
| `browser_open`, `browser_status`, `browser_close` | Official Browser Run acquire/connect, DO-persisted session ID, reattach path, opt-in native recording, explicit termination | **PASS** new version open/close and `recording=true`; durable reattach after actual eviction **not verified** |
| `browser_action` | Navigation, click/double-click, fill, select, type, keyboard, scrolling, history, iframe selector | **PASS** new version: Selenium form text fill, select, checkbox, Submit → actual `Form submitted / Received!`; other variants pending |
| `browser_snapshot`, `browser_screenshot` | Accessibility snapshot (main frame or selected iframe); full-page PNG | R1 screenshot/snapshot PASS; iframe unverified |
| `browser_tabs`, `browser_frames` | List/new/select/close tab; discover popup, iframe name/index; per-tab actions | **PASS** real list/new/close (selenium.dev → example.com) and main frame enumeration; popup/cross-origin iframe pending |
| `browser_wait`, `browser_assert`, `browser_run` | Bounded Playwright waits, typed deterministic comparisons, ≤12-step E2E report, optional screenshot on failure | **PASS** live wait, correct positive/negative assertions, stop-on-first-fail and Selenium fill/select/check/submit; PR #363 changes not deployed |
| `browser_file` | Explicit inline base64 upload, click-to-download; 1 MB maximum, no server filesystem | **PASS** live 6-byte text upload; download, larger files and readback still pending |
| `browser_diagnostics`, `browser_inspect(network)` | Sanitized console, recent HTTP response/failure events, resource type and approximate timings | **PASS** real HTTP resource/timing data; this is not HAR |
| `browser_inspect(performance)` | Browser navigation timings and paint entries; not complete Core Web Vitals | **PASS** API returned navigation/paint entries; Core Web Vitals and budgets pending |
| `browser_inspect(accessibility)` | Heuristic missing-alt/label/button-name/title findings; not WCAG certification | **PASS** live heuristic probe; WCAG audit pending |
| `browser_inspect(pdf)`, `browser_viewport` | Bounded PDF export, viewport resizing for responsive tests | **PASS** PDF payload (26,138 bytes) and 390×844 viewport; independent PDF parse and device emulation pending |
| `browser_live_view` | 60-second signed read-only Cloudflare Live View URL | **PASS** actual owner-authorized call returned HTTPS signed URL with `readonly=true`, expiry 60 seconds; URL intentionally never reproduced |
| `browser_session` | CRM authentication diagnostics (does not grant admin rights) | CRM owner-tenant delegation still absent |

Cloudflare Browser Run **session recording** is supported natively and may be
enabled only on a fresh `browser_open(recording=true)`; it can later be located
by the opaque session ID reported by `browser_status`. Native recording/HAR
inspection is in the Cloudflare Dashboard; **the MCP does not yet export HAR
artifacts directly**. Raw cookies, service credentials and unredacted headers
are never returned by the diagnostics endpoint.

**Still missing / gated:** DNS rebinding or private-address resolution checks
at the actual network destination (URL-host syntax alone cannot prove this),
full native HAR/trace artifact export through MCP, rich context/cookie storage
management, stable tab IDs across DO eviction, binary visual diffs, cross-browser
comparison, full mobile emulation, automated WCAG validation, downloads above
1 MB, and a physical CRUD/audit cycle on the existing CRM owner tenant.
These are not to be misrepresented as complete. Work in the *same browser
owner* and add evidence to Issue #357. Do not release CRM admin credentials
before the sensitive-origin negative tests and tenant mapping pass.

## 14 November 2026 extended release: physical test receipt

Source: [Issue #357](https://github.com/iamaman11/crm/issues/357).
Released Worker remains **only** `crm-browser-mcp`, protected by the original
single-owner Cloudflare Access app. GitHub `BROWSER_MCP_RELEASE_ENABLED=false`
was independently verified after the release. Worker secret readback contains
only `CRM_MCP_OWNER_EMAIL`; **no CRM access service credentials**, no primary
CRM/D1 changes.

**PASS (real ChatGPT → Access → Cloudflare DO → Browser Run → Playwright):**

1. `browser_status` after deployment returned `closed` plus the new
   `recording=false` field, proving the new version was invoked.
2. `browser_open(https://www.selenium.dev/selenium/web/web-form.html)` returned
   HTTP 200 / title `Web form`. Snapshot showed actual role-tagged text input,
   select dropdown, checkbox and Submit button.
3. Real `browser_action` calls filled a non-sensitive test marker, selected
   `2`, checked `Default checkbox`, clicked Submit, and navigated to
   `/selenium/web/submitted-form.html`. Snapshot showed `Form submitted` /
   `Received!`.
4. `browser_diagnostics` returned new resource-type and measured timing fields.
   `browser_live_view` produced a 60-second read-only signed HTTPS URL.
5. Attempts to open main CRM without an explicitly provisioned service principal
   were denied; metadata `169.254.169.254`, reserved `198.18.0.1` and local
   single-label `metadata` were denied without changing the active page.
6. After closing, `browser_open(recording=true)` started an isolated public
   example.org session, with `status.recording=true`. `browser_close()` stored
   `lastRecordedSessionId`; Cloudflare recording API returned four DOM/session
   events, proving actual native recording persisted.

**NOT PASS / pending:**
- The earlier 9-tool-only ChatGPT discovery restriction was **resolved** in a
  fresh conversation: 17/17 callable. See Issue #357 acceptance comment
  (14 November 2026) for receipts. This line is retained as a historical correction.
- Recording's network/HAR API for the observed session returned
  `404 Network recording not found`; native recording is proven, **HAR export
  is not**.
- DO isolate eviction/reconnect, safe handling of redirects/DNS rebinding,
  actual iframe/popup interactions, real file upload/download, PDF, screen
  resize, assertions/reporting and visual-performance/a11y probes need separate
  physical readbacks. Do not grant the CRM service principal until P0 security
  and correct existing owner-tenant mapping are proven.

## Current verified deployment and authentication (14 Nov 2026)

- **Real Universal Browser MCP deployed.** [CI/deploy #43510554028](https://github.com/iamaman11/crm/actions/runs/43510554028) passed from protected `mcp` commit `70119e852c5107cd8337abdac70c2325b6bb2218`. Cloudflare version `cec55b95-58f4-4f5a-ade7-e2e7ce14e4fe` is live at 100%, with the official Browser Run binding `BROWSER` and `SESSIONS` Durable Object binding. The sole published origin is `https://crm-browser-mcp.crm-5bd5e1e1.workers.dev/mcp`; preview URLs are disabled.
- Cloudflare Access `crm-browser-mcp-access` protects the origin with a single owner-email policy. `CRM_MCP_OWNER_EMAIL` is a protected Worker secret. Anonymous GET and MCP initialize POST both receive HTTP 401 and OAuth `WWW-Authenticate`, never a tool response.
- Access Managed OAuth exposes protected-resource discovery, the authorization server, PKCE S256 and DCR. **DCR was initially misconfigured:** only localhost/loopback callbacks were allowed, causing `HTTP 400 invalid_client_metadata: redirect_uri is not allowed by the account configuration`. The exact allowed redirect URI list and live positive/negative tests are documented below.
- [PR #359](https://github.com/iamaman11/crm/pull/359) added the canonical private ChatGPT plugin package in `chatgpt-plugin/universal-browser-mcp/`. Private plugin exists, and owner OAuth plus `browser_status` / `browser_open`, real rendered DOM, cross-domain click, screenshot and close were later physically confirmed in the actual ChatGPT conversation (Issue #357 R1 receipt).
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
- **Verified since that fix:** ChatGPT discovered nine live tools and completed status → public-page open → DOM → cross-domain click → screenshot → close. That is R1 owner OAuth and basic browser E2E accepted; the richer P0/P1/P2 candidate requires its own live tests.

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

## P0 scenario integrity candidate (Issue #357)

The MCP schema and DO now share one typed list of permitted scenario operations.
Every `browser_run` batch is prevalidated **before** any UI actions; an invalid
late step cannot trigger a preceding click or submit. The run has a 1–120 s
budget (90 s default). Supported Playwright action/navigation/wait timeouts are
clamped to the remaining budget; a late ACK cannot produce PASS. A timed-out
mutation is explicitly marked `outcome: uncertain` and MUST NOT be replayed
without independent readback. Failed assertions and runs produce MCP
`isError: true` without discarding JSON or optional failure PNG.

This is **not** a hard OS kill or complete cancellation guarantee. In-flight
operations without a cancellable Playwright API can still outlive a budget;
no `Promise.race` frees the DO queue while remote actions may be running.
Per-conversation session leases, network-level DNS/IP guardrails, genuine DO
eviction recovery, live CI and CRM R2 remain **NO-GO**.

## P1 download fail-closed gate — 14 November 2026

Live `browser_file(download)` on the previously released R1 Worker emitted a
download event and returned `completed:true` with zero bytes and empty base64
for **four distinct** publicly available files: `some-file.txt`, `sample.csv`,
`textfile.txt` and `1Mb.dat` (the latter filename is not independent proof
of the true byte size). This is **not** a valid download receipt. A similar
open upstream Cloudflare Playwright issue reports empty `createReadStream()`:
https://github.com/cloudflare/playwright/issues/93 .

This narrow fix relies on the official Playwright `createReadStream()`
failure contract and rejects unverified empty streams; it does not add a
second potentially unbounded `download.failure()` wait. No proxy fetch,
secondary browser, Worker-local file workaround or accidental credential
forwarding is added. Genuine zero-byte files are conservatively unsupported
until the provider can distinguish them from an empty broken stream.

**Download remains P1 NO-GO:** the fix was deployed on 14 November 2026;
live `browser_file(download)` now correctly returned
`BROWSER_DOWNLOAD_CONTENT_UNVERIFIED`, not false `completed:true`.
A successful nonempty file transfer with independent byte/count readback is
still required. CRM R2 remains OFF.

## Explicit isolated workspaces (P0, Issue #357)

The MCP HTTP request has **no trustworthy ChatGPT conversation identifier**.
The former `owner:<email>` Durable Object key shared one mutable browser
among parallel chats. FIFO only serialized commands; it did not isolate
tabs, cookies, forms, recordings or close.

`browser_open` with no `workspaceId` now creates a random UUID workspace,
opens a new Cloudflare Browser Run, and returns the handle with the successful
open response. Every later method **requires** `workspaceId` (including
`browser_status` and `browser_close`). Explicit `browser_open(workspaceId)`
reuses the selected workspace. Missing handles fail with
`BROWSER_WORKSPACE_REQUIRED`, without the old owner-wide fallback.
The same existing Durable Object class is indexed by owner+workspace UUID,
and alone owns that workspace's browser context, tabs, cookies and lifecycle.

A workspace handle is **not an authentication credential**: Cloudflare Access
still authenticates the owner for *every* request. It does not claim to
automatically identify a ChatGPT conversation. Two chats explicitly providing
the same handle intentionally share one workspace; two independently created
handles have different Durable Objects. A lost handle is not silently
rediscovered. The provider's idle expiry limits abandoned Browser Run sessions.
No new Worker, browser engine, D1 database, registry or hidden adapter.

**Published / physically checked (14 November 2026):** protected release
[CI #43534116743](https://github.com/iamaman11/crm/actions/runs/43534116743)
deployed commit `1f52141efd7fab9adba76b7b0c8c7072442bf77a` as Cloudflare
Worker version `6554e01a-3287-465e-9cc1-b418047558dd` at 100%.
`BROWSER_MCP_RELEASE_ENABLED=false` after release.

Two independent real workspaces A (Selenium form) and B (Example Domain)
opened simultaneously. A was filled while B's DOM and title stayed unchanged.
Closing A left B open; closing B succeeded. Calls without `workspaceId`
were rejected, and provider session list showed **zero** open browsers after
cleanup. An initial 429 when creating B was a new-browser creation rate
limit (Workers Free: one new instance every 20 seconds), not a concurrency
isolation failure. A same-workspace browser_open successfully navigated
without allocating a new workspace.

The deployed `browser_run` physically returned an MCP error for a negative
assertion and stopped before the second step; a five-step Selenium E2E passed.
A later forbidden URL in a batch was rejected **before** an earlier click.
These are specific PASS cases, not full P0 acceptance.

**Still NO-GO:** actual DO isolate eviction/reconnect and crash/expiry,
DNS rebinding/resolved-IP and hostile redirect enforcement, real nonempty
downloads/HAR/trace, full live CI and P2, and CRM R2 owner-tenant identity.
Refresh ChatGPT MCP tool metadata after schema changes. The installed private
Plugin Creator release must be synchronized separately from canonical source.

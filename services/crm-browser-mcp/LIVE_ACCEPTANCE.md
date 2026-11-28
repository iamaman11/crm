# Universal Browser MCP — single live acceptance procedure

This is the **one** repeatable smoke procedure for the 21 existing tools and the three-controller isolation contract.
Canonical branch: `mcp`, Issue [#357](https://github.com/iamaman11/crm/issues/357).
No new Worker, browser runtime, CI, credentials, or HTTP-proxy is required.
Run against **the released Worker**, not the PR preview. Existing `browser-mcp.yml`
still owns CI/release, including workerd Durable Object restart tests.

## Preconditions

- Universal Browser MCP is connected in ChatGPT with three separately authenticated, Access-allowlisted controller accounts (the current Access policy has exactly three intended controllers).
- Each account has completed its own Managed OAuth authorization; do not share cookies or workspace handles.
- Browser Run has capacity for three short concurrent sessions.
- CRM is a **test** tenant, and an existing service principal has `admin`.
- **Never paste Access tokens or Live View signed URLs into reports.**
- Use a separate fresh UUID `workspaceId` for each controller; keep tests read-only in CRM.
- Public file/popup/iframe fixture: `https://the-internet.herokuapp.com`.
  This fixture may change; report unavailable instead of claiming a product regression.

## Ordered live smoke (one controller's real Browser Run)

The table names the actual public MCP tool/operation, actual input and required
**observable** receipt. An API being registered, TypeScript passing, or a 200
without a rendered UI is not an acceptance.

| Tool | Live action | Required proof |
|---|---|---|
| `browser_open` | `path=https://crm-dev.crm-5bd5e1e1.workers.dev/, recording=true` | Actual `state=open`, `http_status=200` |
| `browser_status` | Read current session | `state=open`, provider active session count >=1 |
| `browser_session` | Read real `/api/session` | `authenticated=true`, `role=admin`, HTTP 200, no Google |
| `browser_snapshot` | Main document | Nonempty React navigation, client heading and form |
| `browser_tabs` | list; new `https://the-internet.herokuapp.com/windows`; select | Native tab IDs, two live tabs, external site loads |
| `browser_frames` | Navigate to `https://the-internet.herokuapp.com/iframe`; list | Main + actual iframe, `frameIndex=1` |
| `browser_file` | On `/upload` upload `hello` text to `#file-upload` and click Upload; on `/download` test both `download` and `download_link` with a real link; chunk/discard the second receipt | Real uploaded filename visible; exact nonempty bytes for both downloads, no CDP empty stream; verified SHA/chunk |
| `browser_inspect` | network; performance; accessibility; axe; pdf | Native HTTP events/timings, axe report, nonempty PDF bytes |
| `browser_visual` | capture; compare unchanged page; discard | `differenceRatio=0` and valid baseline lifecycle |
| `browser_viewport` | 900x650 | Exact dimensions returned |
| `browser_screenshot` | Screenshot current page | Actual image/png **image content**, not only a MIME receipt |
| `browser_action` | Navigate, click, fill or select actual form | Browser DOM change visible after action |
| `browser_wait` | Wait for existing visible heading | Bounded completed result |
| `browser_assert` | Title `CRM` true; deliberate wrong title false; iframe text | Pass/fail typed honestly, no false PASS |
| `browser_run` | At least 2 ordered assertions, e.g. current title and heading | `passed=true`, two step results |
| `browser_execute` | CDP `Runtime.evaluate` read `document.title` | Correct actual title, no second browser |
| `browser_diagnostics` | Read console/network | Real requests incl Connect; no MIME errors on CRM |
| `browser_recording` | After recording-enabled Close: targets → har → chunk → discard | Real targets, sanitized HAR 1.2, nonempty entries, SHA receipt |
| `browser_live_view` | Mint readonly view while Open | Signed URL exists and is short-lived; do not log URL |
| `browser_handoff` | Start with 30s timeout and no human; poll `status` | `waiting_human` then real provider `timed_out`, no false `completed`; provider Done event is a **separate human-assisted** check |
| `browser_close` | Close (and verify status) | `closed`, `activeSessionCount=0` |

**Sequence:** exercise main CRM, external fixture tab, iframe, popup (click `Click Here` on `/windows` with `followPopup=true`),
files, snapshots, diagnostics, visuals and PDF while Open. For handoff timeout,
use a separate short test session and Close it. For HAR, use the recording-enabled
session, Close and fetch recording only then. To test native `mobile` device,
Close first, then `browser_open(device=mobile)` and assert React/admin again.

### Three-controller parallel isolation (required for multi-account release)

1. In ChatGPT account A (the owner controller), open `https://example.org/`
   and retain workspace A.
2. In ChatGPT account B, independently authorize the same MCP endpoint and open
   `https://www.iana.org/`, retaining workspace B.
3. In ChatGPT account C, independently authorize the same MCP endpoint and open
   `https://www.cloudflare.com/`, retaining workspace C.
4. Confirm all three returned workspace UUIDs differ. In each account, run
   `browser_status` and `browser_snapshot`; each must show only its own URL,
   title and tabs.
5. Run cross-controller negative checks: from B, call `browser_status` with
   A's UUID; from C, call it with A's and B's UUIDs. Each foreign namespace
   must report `closed` with no foreign path/title/session. Re-read A and B
   from their own accounts and confirm both remain open. Never pass a foreign
   UUID to `browser_open` during this check.
6. Close A from account A. Verify B and C remain open and usable. Close B from
   account B. Verify C remains open and usable. Close C from account C and
   verify provider `activeSessionCount=0`.
7. In each account, confirm the same complete 21-tool MCP surface is available.
   A stale cached schema is a client refresh issue; do not change the Worker
   contract to accommodate one stale connection.
8. For CRM, read `/api/session` in each browser and confirm the configured
   service identity's real role. Record explicitly that this is a shared
   service-principal actor; it is not per-human CRM audit attribution.

The current Access policy readback has exactly three allowlisted controllers.
The Worker allowlist secret binding is present, but Cloudflare does not expose
its value for readback. The independent login tests below are still required
to prove each controller is actually accepted by the Worker.

Do not mark multi-controller LIVE PASS until all three actual ChatGPT accounts
have completed the concurrent test. Unit tests, a single account with three
workspaces, or a policy readback cannot prove three independent OAuth
identities.


### Cloudflare-hosted Durable Object restart (required)

1. Open a desktop test workspace on `https://the-internet.herokuapp.com/drag_and_drop`
   (or another public fixture), and retain its `workspaceId`. Do not use a
   service-credential CRM workspace for this restart test.
2. Record the exact URL/title and original target identity using the existing
   read-only browser tools. Keep this workspace open; do not call Close/Open.
3. From the protected GitHub Actions workflow, dispatch **Universal Browser MCP CI**
   on branch `mcp` with `release_browser_mcp=true`, `expected_sha` equal to the
   exact current `mcp` HEAD, and `restart_do_immediately=true`. This is an
   explicit exact-SHA redeploy of the same Worker, not a feature-branch deploy.
4. After the deploy job reports success and Cloudflare shows its new Worker
   Version ID, use the same controller and same `workspaceId` for
   `browser_status`, `browser_snapshot` and a real DOM assertion.
5. Require `state=open`, the exact same original URL/title/target identity,
   and a successful post-restart page action. Provider active-session count
   must not increase: recovery must reconnect the original Browser Run session,
   never acquire a replacement or create a new tab.
6. If the original target is missing, report FAIL/needs_reopen. Do not call
   `browser_open` to hide the failure. Close the original workspace only after
   recording the result; verify provider `activeSessionCount=0`.

The local workerd test proves persisted DO storage across a host restart; it
is not this Cloudflare-hosted test. A deploy receipt alone is not recovery proof.

### Human Handoff — real Done (required separately from timeout)

1. Open a clean public `https://example.org/` workspace and start
   `browser_handoff` with a clear instruction to inspect the page and press Done.
2. The authorized human opens the short-lived Live View, confirms the page, and
   presses **Done** before expiry.
3. Read the provider's actual handoff status. Require the real terminal
   `completed` state, then verify the same workspace is usable again and the
   original page remains open. Do not infer Done from a closed Live View tab or
   from the agent's own action.
4. Close the test workspace and verify provider cleanup.

---

### Provider limitations and truthfulness

- `@cloudflare/playwright` on Browser Run exposes a `download` event but
  `download.createReadStream()` can be empty even for nonempty files
  ([upstream issue](https://github.com/cloudflare/playwright/issues/93)).
  **Both** public download modes therefore use the **same browser-side
  same-origin anchor fetch**. This is native DOM Fetch with page credentials,
  **not** Worker-side HTTP proxy or new browser owner.
- `download` returns inline nonempty bytes up to **1MB**; `download_link`
  returns verified SHA-256/chunks up to **8MB**. Cross-origin and
  `blob:`-only event downloads are **not** represented as successful; report
  an explicit unsupported state rather than a false "downloaded" receipt.
- Real human `Done` requires a human participant. Unit tests cover Done,
  Failed, timeout and uncertain provider events; the unattended smoke proves
  the **real provider timeout** and owner action suspension.
- CI executes the production-compiled Worker in `workerd`, kills and restores
  persisted DO state; this does **not** substitute for a Cloudflare-hosted DO
  reattachment test. When testing live DO restart, keep an original test page
  open and perform a controlled exact-SHA redeploy with immediate DO update.
  Desktop now uses Chromium's native **persistent default context**, so require
  the exact original tab/target to survive. Mobile/tablet retain Playwright's
  ephemeral device contexts and are tested for fresh Open, not claimed durable.
  Report FAIL if an original tab disappears; never disguise fresh Open as restore.
- The service identity and Playwright request header override are **test-only**.
  Before production, review redirect-scoped Access credentials. Do not claim
  they are safe for production or secretly alter Access policy.

### Acceptance receipt

Record date, branch HEAD SHA, accepted PR, CI run, Worker Version ID, each
tool's LIVE PASS / PARTIAL / FAIL, links to the real CI and anonymized outcome.
Never report sensitive tokens, full querystrings or Live View URLs.


## Native cross-site input regression (15 November 2026)

- **Cross-site PASS:** Playwright TodoMVC (17/17 ordered UI steps), Selenium form,
  The Internet dynamic controls/iframe/popup/dialogs/files, Demoblaze full
  add-to-cart/remove, cross-site redirects, desktop and mobile smoke.
- **SauceDemo remains a provider compatibility issue, not an accepted PASS:**
  in a fresh browser and after a successful public `standard_user` login,
  `locator.click()` on `[data-test=add-to-cart-sauce-labs-backpack]`
  reports completion without any DOM `pointerdown`/`click` capture events
  or React change. Native `Input.dispatchMouseEvent` CDP also sends an ACK
  without DOM events. Explicit DOM `element.click()` produces an **untrusted**
  click and triggers React, proving different semantics. **Never silently
  substitute a JS synthetic click** for a native Playwright action.
- **Keyboard:** if `browser_action(action=press, target, key=Enter)` is
  requested, native Playwright `locator.press` focuses and presses that
  exact element. Without `target`, use `page.keyboard.press`. This is
  not a claim that SauceDemo accepts it: verify with a live assertion.
- **Scenario failure UX:** `browser_run(..., screenshotOnFailure=true)`
  returns one compact typed error receipt with
  `failureScreenshot=available_via_browser_screenshot` when it captured a
  PNG. Retrieve the exact failed-state image using
  `browser_screenshot(workspaceId, source=last_failure)` **before another
  browser_run or Close**. The separate `browser_diagnostics` provides HTTP
  events; do not embed repeated request logs or inline PNG data in an error.
- The screenshot receipt retains the page path captured **at the failed step**, even after a later navigate. The image must not be mislabeled as the currently open page.
- The screenshot is **workspace-local ephemeral state**; it is not persisted
  or claimed recoverable after Durable Object eviction.
- Distinguish the browser command returning `completed=true` from an
  application postcondition. Use `browser_wait` and `browser_assert`,
  and treat a failed postcondition as FAIL, not a successful click.

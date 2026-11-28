# Universal Browser MCP — operations

## 1. Canonical source and deployment

- Repository: `iamaman11/crm`.
- Protected source branch: `mcp`.
- Worker: `crm-browser-mcp`.
- MCP endpoint path: `/mcp`.
- One release workflow: `.github/workflows/browser-mcp.yml`.
- GitHub Environment: `browser-mcp`.
- Release instructions: [PROTECTED_RELEASE.md](./PROTECTED_RELEASE.md).
- Live evidence and test checklist: [LIVE_ACCEPTANCE.md](./LIVE_ACCEPTANCE.md).
- Current architecture specification: [Issue #500](https://github.com/iamaman11/crm/issues/500).

Do not deploy from a feature branch. Do not change CRM Worker, D1, CRM Access roles/policies or CRM CI as part of Browser MCP maintenance.

## 2. MCP controller authorization

Cloudflare Access authenticates the human ChatGPT/MCP client at the MCP endpoint. The Worker independently validates the signed `ctx.access` evidence, the exact `CRM_MCP_ACCESS_AUD` audience and the Worker secret `CRM_MCP_ALLOWED_EMAILS`.

- `CRM_MCP_ACCESS_AUD` is a non-secret Worker variable.
- `CRM_MCP_ALLOWED_EMAILS` is a protected Worker secret containing comma/newline-separated controller emails.
- The same intended controller set must be present in the existing Cloudflare Access allow policy.
- Missing, malformed, expired, wrong-audience or unlisted identities fail closed.
- Each ChatGPT account authorizes its own OAuth connection. Never share OAuth tokens, cookies, Live View URLs or workspace handles between accounts.
- Removing a controller requires removing the identity from both the Access policy and Worker allowlist, then reconnecting affected clients.

MCP controller authorization grants permission to operate the MCP endpoint only. It does not grant Cloudflare dashboard roles or CRM application permissions.

## 3. Target-site authorization and secrets

Target-site Access is independent from MCP controller authorization. It is configured per exact HTTPS origin and is never selected by a browser tool argument.

Existing bindings:
- `SITE_ACCESS_ORIGIN` — public exact origin variable for the primary CRM target.
- `SITE_ACCESS_CLIENT_ID` / `SITE_ACCESS_CLIENT_SECRET` — optional paired Worker secrets for that exact origin.
- `SITE_ACCESS_PREVIEW_1_ORIGIN` with `SITE_ACCESS_PREVIEW_1_CLIENT_ID` / `SITE_ACCESS_PREVIEW_1_CLIENT_SECRET`.
- `SITE_ACCESS_PREVIEW_2_ORIGIN` with `SITE_ACCESS_PREVIEW_2_CLIENT_ID` / `SITE_ACCESS_PREVIEW_2_CLIENT_SECRET`.
- `CF_BROWSER_RECORDING_READ_TOKEN` — optional read-only Browser Run recording API secret.

Preview credentials are isolated from the primary CRM pair and from each other. Incomplete required preview credentials fail closed for that preview without disabling public-site browsing.

Never put secrets in source, GitHub variables, MCP tool arguments, URLs, page DOM, logs, screenshots, Live View links or results. Do not use global browser-context headers for privileged credentials. Existing request routing and redirect/origin restrictions must be preserved. Native route.continue({headers}) may forward overridden headers across redirects; this documented limitation is accepted only for isolated test CRM data (see #357), never for customer data.

## 4. CRM business identity

When a CRM service principal is configured for a target origin, it is still a service identity, not the human ChatGPT controller. CRM/Rust performs its own authentication, tenant resolution and authorization. MCP does not write D1 and does not claim per-human CRM audit attribution for a shared service principal.

All CRM business actions must use the real browser UI → Connect → Rust → D1 path, followed by independent readback when acceptance requires it.

## 5. Workspace lifecycle

- A successful `browser_open` without a workspace handle creates a UUID and returns it.
- Reopening an existing workspace requires the same UUID.
- Every later tool call requires `workspaceId`.
- Workspace IDs are not credentials; Access identity is checked on every request.
- A different controller with the same UUID maps to a different DO.
- Close each workspace explicitly after acceptance. Verify `browser_status` is `closed` and provider `activeSessionCount` is zero.
- If a normal command returns `BROWSER_RUNTIME_STALE_OR_UNVERIFIED`, do not replay a mutation. Close the stale workspace, then explicitly open a new one if needed.

## 6. Local checks

Use the pinned Node version from the workflow and the lockfile. From this directory:

```bash
npm ci
npm run check
npm test
npx wrangler deploy --config wrangler.jsonc --dry-run --outdir /tmp/crm-browser-mcp-build
MCP_TEST_BUNDLE_DIR=/tmp/crm-browser-mcp-build npm run test:do-lifecycle
```

`npm ci` runs the pinned axe-core source generator. The generated source is build output and must not be edited manually. Do not change the lockfile unless dependency work is separately approved.

## 7. Branch and release procedure

1. Read Issue #500 and the current protected `mcp` SHA.
2. Create a temporary feature branch from that exact SHA.
3. Keep the diff limited to `services/crm-browser-mcp/**` unless a specific workflow change is separately justified.
4. Open a PR targeting `mcp`. The existing `browser-mcp.yml` runs strict TypeScript, tests, Worker dry-run and workerd DO lifecycle tests.
5. Fix failures and rerun checks on the final exact PR head.
6. After review and merge, publish only through the existing protected manual exact-SHA release procedure in `PROTECTED_RELEASE.md`.
7. Verify the workflow checked out the requested SHA and deployed only `crm-browser-mcp`.
8. Run independent live Gate B through an authorized MCP client. Test the changed behavior plus the relevant existing regression surface.
9. Close all test workspaces and verify provider cleanup.

A green PR check is Branch Gate A, not Production Gate B. A deployment receipt is not a live browser acceptance receipt.

## 8. Privacy and diagnostics

Diagnostics must remain bounded and sanitized. URL queries/fragments and credential values are not exposed. HAR/recording content must be sanitized and read only with the dedicated read-only token. Live View URLs are sensitive, short-lived capabilities and may only be shared with the authorized operator.

Do not log raw Cloudflare/Browser Run provider errors if they may contain credentials or target URLs. Preserve explicit error codes and fail-closed behavior.

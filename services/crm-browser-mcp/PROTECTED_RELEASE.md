# Universal Browser MCP — protected release

The Browser MCP Worker is released independently of CRM, Rust, React and D1.
Never deploy it from a local shell or bypass the protected `mcp` branch.

## Release prerequisites

- The reviewed code is merged into `mcp`.
- Repository variable `BROWSER_MCP_RELEASE_ENABLED` is exactly `true`.
- GitHub Environment `browser-mcp` contains the narrowly scoped
  `CLOUDFLARE_API_TOKEN` required by Wrangler.
- The release workflow's `browser-mcp` CI job has passed.

## Preferred manual release

Open **Actions → Universal Browser MCP CI → Run workflow** and select:

| Input | Value |
|---|---|
| Branch | `mcp` |
| `release_browser_mcp` | `true` |
| `expected_sha` | Exact reviewed 40-character `mcp` HEAD |
| `restart_do_immediately` | `false` unless an explicitly approved DO reset is required |

The workflow verifies the exact SHA before deployment. The deploy job is
additionally gated by the repository variable and the protected
`browser-mcp` Environment.

## Push-triggered release

A merge into `mcp` also starts the same workflow. Its deployment job runs
only when `BROWSER_MCP_RELEASE_ENABLED=true`. This is the supported fallback
when a workflow-dispatch client is unavailable; do not write directly to the
protected branch and do not bypass required checks.

## Release verification

A release is complete only when the exact run reports:

1. `browser-mcp` — success.
2. `deploy-browser-mcp` — success (not skipped).
3. Cloudflare shows a new `crm-browser-mcp` Worker version for that run.

This workflow deploys only `crm-browser-mcp`; it does not deploy CRM or
migrate, read, or write remote D1. A successful CI-only run is not a release.

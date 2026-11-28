# Universal Browser MCP

Universal Browser MCP is a general-purpose, remotely hosted browser control plane for ChatGPT and other authorized MCP clients. It operates permitted public HTTP(S) sites through one real Cloudflare Browser Run Chromium session and the official Playwright API.

It is not a CRM business service. When operating CRM, it uses the real browser UI and the CRM's existing Connect → Rust → D1 path. MCP does not write CRM data directly or implement CRM business authorization.

## Runtime ownership

```text
ChatGPT / authorized MCP client
  → Cloudflare Access (human controller)
  → crm-browser-mcp Worker (21 typed MCP tools)
  → controller + workspace Durable Object
  → Cloudflare Browser Run + official Playwright
  → permitted public HTTP(S) website
       CRM UI → Connect → Rust → D1 (CRM only)
```

One controller/workspace pair maps to one Durable Object. That Durable Object is the only owner of its Playwright browser, context, pages and session lifecycle. Different controllers and workspace UUIDs remain isolated.

## Public tool surface

The existing 21 tools cover browser lifecycle, navigation and actions, snapshots and tabs/frames, waits/assertions/scenarios, diagnostics and inspection, screenshots/visual comparison, file transfers, recordings, and human handoff. Their names and schemas are compatibility contracts.

## Architecture and operations

- [ARCHITECTURE.md](./ARCHITECTURE.md) — module boundaries, identity model, ownership and dependency rules.
- [OPERATIONS.md](./OPERATIONS.md) — local checks, Access configuration, secrets, release and recovery.
- [PROTECTED_RELEASE.md](./PROTECTED_RELEASE.md) — protected exact-SHA release procedure.
- [LIVE_ACCEPTANCE.md](./LIVE_ACCEPTANCE.md) — live verification evidence and procedure.
- [Issue #500](https://github.com/iamaman11/crm/issues/500) — canonical modular architecture specification and refactor acceptance.
- [Issue #357](https://github.com/iamaman11/crm/issues/357) — historical Browser MCP engineering evidence; it is not the module architecture specification.

## Local development

```bash
cd services/crm-browser-mcp
npm ci
npm run check
npm test
npm run test:do-lifecycle
npx wrangler deploy --config wrangler.jsonc --dry-run --outdir /tmp/crm-browser-mcp-build
```

`npm ci` generates the pinned axe-core source used by the Worker. Do not edit the generated file manually.

## Change and release rules

Develop in a temporary feature branch from the current protected `mcp` branch. Run the existing independent Browser MCP CI on the exact PR head. Release only through the existing protected `browser-mcp.yml` workflow after review and merge to `mcp`; never deploy from a feature branch or change CRM Worker/D1 as part of MCP work.

A green build is not live acceptance. Verify the published endpoint through an authorized MCP client, then close test workspaces and confirm the provider has no active sessions.

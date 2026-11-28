# Universal Browser MCP — architecture

**Canonical specification:** [Issue #500](https://github.com/iamaman11/crm/issues/500).  
**Historical engineering evidence:** [Issue #357](https://github.com/iamaman11/crm/issues/357).  
**Application:** `services/crm-browser-mcp`; protected source branch: `mcp`.

## 1. Product boundary

Universal Browser MCP is a general-purpose browser control plane. It accepts typed MCP commands, selects the caller's isolated workspace, executes them through the official Cloudflare Browser Run + Playwright runtime, and returns bounded results.

It does not own CRM business logic, tenant permissions, records, persistence, or a second HTTP path. A CRM business action must happen through the real CRM React UI → Connect → Rust Worker → D1 flow.

## 2. One request path and one browser owner

```text
MCP client
  → Cloudflare Access authenticates the human controller
  → Worker verifies signed controller identity + exact audience + allowlist
  → tool adapter validates the public MCP input
  → workspace router selects controller-email + workspace-UUID DO
  → Worker probes the DO runtime epoch and sends a typed command
  → one Durable Object serializes commands and owns browser state
  → official Browser Run + Playwright acts on the selected page
  → typed, bounded result returns to the MCP client
```

The Durable Object stores only the state needed to recover its own provider session and workspace. Browser processes, contexts, pages, cookies and network execution belong to Cloudflare Browser Run/Playwright. No feature module may create a second browser owner.

## 3. Three identity boundaries

These identities are different and must stay different.

### 3.1 MCP controller identity

The human ChatGPT account authorized to call the MCP endpoint. Cloudflare Access supplies server-verified `ctx.access`; the Worker checks the exact MCP audience and the `CRM_MCP_ALLOWED_EMAILS` allowlist. The verified email determines the controller namespace. Tool arguments, caller headers and user-supplied emails are not proof of identity.

### 3.2 Target-site identity

The identity used by Chromium when visiting a target website. It is normally the browser's human login/cookies, or an explicitly configured service principal whose credentials are bound to one exact HTTPS origin. Target-site credentials are never the MCP controller identity and must not escape to foreign origins, redirects, popups, iframes or subresources. The existing native route.continue({headers}) transport has a documented cross-origin redirect residual risk; it is accepted only for isolated test CRM data, not for production/customer data. See #357 and LIVE_ACCEPTANCE.md. Refactoring does not resolve or prove this risk away.

### 3.3 CRM application identity

The identity independently authenticated and authorized by the CRM. CRM/Rust resolves the tenant and role. MCP must not implement CRM RBAC, grant CRM privileges, impersonate a human, or claim that a shared service principal is the initiating human actor in CRM audit.

## 4. Module map

The target structure is intentionally small. It describes responsibilities, not a requirement to create a file for every helper.

| Module | Responsibility | Migration status |
|---|---|---|
| `index.ts` | Worker entrypoint and MCP transport boundary | Current entrypoint; tool registrations move in a later slice |
| `endpoint.ts` | Canonical public MCP hostname and route | Slice A |
| `contracts.ts` | Shared `Env`, `Command`, `Target` and compile-time types | Slice A |
| `tools.ts` | Existing 21 public tool schemas and thin adapters | Planned |
| `controller-auth.ts` | Signed human controller identity, audience and allowlist | Slice A |
| `site-access.ts` | Target-site exact-origin Access configuration and request disposition | Slice A |
| `workspaces.ts` | Controller + workspace UUID → DO identity | Existing `browser-workspaces.ts`; rename only if it improves clarity |
| `browser-session.ts` | The single Durable Object and command orchestration | Existing `session.ts`; split in later slices |
| `browser/lifecycle.ts` | Open, close, reconnect, page identity and context setup | Slice B; acts on DO-owned state |
| `browser/navigation.ts` | Navigation and browser request policy | Planned extraction; existing `page-navigation.ts` is a small helper |
| `browser/interactions.ts` | Locators, actions, waits, assertions and scenarios | Planned extraction |
| `browser/inspection.ts` | Snapshot, frames, screenshots, diagnostics, performance and accessibility | Planned extraction |
| `browser/artifacts.ts` | Files, recording/HAR and visual baselines | Planned extraction |
| `browser/security.ts` | Public-web URL guard and safe diagnostic formatting | Slice A |

Small existing pure helpers such as recovery, runtime epoch, browser checks, device configuration and binary transfer remain separate while their contracts and focused tests add value. Do not consolidate or split them mechanically.

## 5. Dependency rules

- Shared contracts contain types only; no environment reads, browser calls or runtime side effects.
- Controller auth does not import target-site Access configuration.
- Target-site Access does not authorize MCP controllers.
- `session.ts` owns one `BrowserLifecycleState` per DO. `browser/lifecycle.ts` performs the only provider acquire/connect/close operations against that state and the supplied storage/binding; open and new-tab share one context initializer.
- Other browser feature modules receive the page/context/operation dependencies they need from the one session owner; they never acquire or reconnect a browser independently.
- Feature modules do not import the Worker entrypoint or MCP server.
- No circular imports, generic plugin registry, dependency-injection framework or parallel execution path.
- Public tool schemas remain at the MCP boundary; internal modules consume typed commands and return typed/bounded results.
- CRM authorization and business behavior remain in CRM/Rust, not in MCP.

## 6. Session and workspace invariants

- Every controller/workspace pair maps to a separate Durable Object name.
- `browser_open` may mint a workspace UUID. Every subsequent command requires that exact UUID.
- A workspace UUID is a routing handle, not an authentication credential.
- Every MCP request is independently authenticated; controller identity is not inferred from the UUID.
- The existing DO queue serializes operations within one workspace.
- Runtime epoch is checked before normal commands and again by the DO. Explicit close remains available for stale workspaces.
- An uncertain mutation is never automatically replayed. Verify the actual UI/business state before retrying.
- Explicit close releases the exact Browser Run session; live acceptance verifies provider active-session count returns to zero.

## 7. Adding a browser capability

1. Decide whether it belongs to lifecycle, navigation, interaction, inspection or artifacts.
2. Define the smallest typed internal command/result; preserve existing public tool schemas unless a separate API change is approved.
3. Implement it using the existing page/context supplied by the session owner and official Playwright APIs.
4. Add focused unit tests, negative cases, bounds and secret-redaction assertions.
5. Add or update the public MCP adapter only when a new user-visible tool capability is explicitly required.
6. Run strict TypeScript, all MCP tests, DO lifecycle test and Wrangler dry-run.
7. Update the runtime epoch whenever any `src/**/*.ts` runtime source changes.
8. Complete exact-SHA branch CI, protected release and independent live Gate B before claiming acceptance.

## 8. Acceptance and deployment boundaries

The only MCP CI/release workflow is `.github/workflows/browser-mcp.yml`. It builds/tests only this Worker. Feature branches do not publish. The protected `mcp` branch and existing GitHub Environment own release authorization.

The Browser MCP gates are separate from CRM gates:
- Branch Gate A: existing MCP CI on exact PR head.
- No separate pre-merge Branch Gate B: feature branches cannot publish and there is no preview Worker.
- Integrated Gate A: coordinator verifies the integrated `mcp` SHA.
- Production Gate B: coordinator verifies the published MCP endpoint and real user-level workflows.

Never treat a unit test, dry-run, successful deployment or cached tool schema as proof of live browser behavior.

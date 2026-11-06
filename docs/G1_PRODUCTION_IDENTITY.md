# G1 — Production Identity and Browser Sessions (implementation work in progress)

Status: **IN PROGRESS / NOT ACCEPTED / NO PRODUCTION LOGIN YET**  
Issue: #311; parent gates: #310; accepted baseline: `main@cd5fc1bc3ff2a92e4a265a40a1bb816648ec2e17`  
Scope: **one production authentication boundary**, not new business modules or identity microservices.

## Actual existing architecture

- `crm-capability-ingress::RequestAuthenticator` is the **only** authentication-to-principal boundary used by both HTTP and gRPC. `AuthenticatedPrincipal` binds one Actor to explicit Tenant IDs. ADR-029 governs it.
- `crm-application-runtime::assemble` currently creates an in-memory hashed `AccessTokenStore`, a process-configured bootstrap bearer, and in-memory live authorization/query-visibility stores. It is a development/process bootstrap, **not** a multiuser production identity system.
- `packages/client/src/session.ts` and `apps/web/src/developmentSession.ts` currently implement an explicit browser development bearer. Real production login was absent from main.
- Business owner modules and governed mutation/query gates are correct architectural destinations. **No direct writes or bypass** from identity code.

## Selected production design

Use **external OIDC Authorization Code + PKCE with confidential backend (BFF) integration in existing `crm-api`**, one configurable standards-compliant provider per deployment, no in-process password store, no new service, no token in browser JS or URL after callback. Fallback to local dev bearer is **strictly DEV-only** and must fail closed in production.

### Required external configuration

An HTTPS public origin, issuer metadata/JWKS, client ID, confidential client credential (server secret), allowed callback URL, and explicitly provisioned users/tenant memberships/roles. No provider account/secret is present in the GitHub repository and no user-specific IdP choice is inferred. The IdP is **a deployment dependency**; no client-side bypass if it is unavailable.

### Browser ↔ server contract (implemented first; server implementation pending)

- `GET /auth/session`, same-origin credentials, no-store: `200` with `{status:"authenticated",tenantId,actorLabel,tenantLabel,expiresAtUnixMillis,csrfToken,capabilities}` only after validated session, actor, tenant and server-side permissions; `401/403` without privileged payload otherwise; errors fail closed.
- `GET /auth/login`: server starts state+nonce+PKCE, redirect to configured IdP. Fail closed when absent/invalid config.
- `GET /auth/callback`: validate exact state, one-time login attempt, nonce, issuer, audience, signature, time bounds and code exchange **server-side**. Session rotation on login; prevent session fixation.
- `POST /auth/logout`: same-origin authenticated session + CSRF, revoke on server, expire session cookie, return `204` only on confirmed local revocation.
- Gateway gRPC-web uses same-origin **HttpOnly Secure SameSite** cookie; JS sees only a non-credential CSRF value and display snapshot. Cookie-based gateway requests use `x-crm-csrf`, no `authorization` header from JS.
- All auth endpoints and gRPC-web must reside behind the **same origin HTTPS reverse proxy** in #313. Browser may never hold `CRM_API_BEARER_TOKEN`.

## Mandatory server implementation before G1 acceptance

- Secure OIDC code exchange+identity verification, no unverified JWT parsing. Whitelisted issuer/endpoints; explicit algorithm; key rotation; bounded network failures; correct state/nonce/PKCE; safe redirect targets.
- Durable tenant membership, actor identity mapping, roles/grants, revocation/session expiry and restart-safe session storage through a **platform-owned** existing PostgreSQL boundary, not business owner tables. Hash session handles at rest, bound expiration and rotation, require cryptographic randomness, no cookie/log secret leak.
- The same authenticated principal and live authorization applies to HTTP/gRPC ingress, privacy, search and background/worker access. A browser cannot select another actor or tenant by sending metadata.
- Strict origin, Host and proxy trust, CSRF protection on state-changing cookie requests, secure cookie attributes and safe redirects; no direct unauthenticated access to application gateway.
- Explicit first-admin provisioning: bind a known IdP subject to a tenant and scoped permissions from a server-side operator action, never via a browser bootstrap token.
- Built web user journey: login → authorized navigation → logout/revocation/expiry → failed/disabled IdP and offline cases.
- Negative tests: invalid issuer/signature/audience/nonce/state, replayed callback, disabled user/tenant, revoked grant after login, expired/forged cookie/CSRF, cross-tenant requests, network failure, restart, concurrent logout and no-leak assertions.
- Match one exact head SHA, browser **production build** and real Postgres, then full applicable CI and independent acceptance. **No G1 checkbox may be checked on browser adapter tests alone.**

## Incremental draft implementation notes

The browser adapter and its unit tests are intentionally fail-closed and are a **partial G1 implementation only**. The backend `/auth/*` handlers, signed identity, durable store and production E2E do **not yet exist**. Until those are completed and independently tested, this branch remains draft and no G1 readiness is asserted.

## Extension complexity measurement

Target **zero new Rust crates**, no new business owner, no edits to generic router/worker algorithms, no duplicate permissions service. If any new adapter dependency becomes unavoidable, explain its trust boundary, security review and lockfile impact in #311. No Step 22R requirement is waived.

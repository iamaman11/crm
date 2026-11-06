# G1 — One simple sign-in path for the closed CRM pilot

State: **IN PROGRESS — NOT ACCEPTED**. Issue #311, plan #310, draft PR #318.
Baseline: `main@cd5fc1bc3ff2a92e4a265a40a1bb816648ec2e17`.

## Principle

**One entry, one session, one existing permission boundary.** No OIDC/SSO platform, JWT subsystem, identity microservice, custom role editor, extra worker, new business crate, background refresh daemon or second authorization engine just to test the first useful CRM.

The existing `crm-capability-ingress::RequestAuthenticator` and live authorization/query checks remain the **only** actor/tenant/capability authority. The existing `crm-api` process owns login and session. The browser receives no process bootstrap bearer; it only gets a server-verified display snapshot and a secure, HttpOnly same-origin cookie.

## Smallest useful closed-pilot implementation

1. **Operator setup:** explicitly provision two pilot test operators and tenant memberships in server-side configuration, outside the repository or built frontend. No self-sign-up, user administration UI, or reusable password database. Secrets are never committed, displayed in logs or shipped to the browser.
2. **Login:** a single server-validated sign-in flow on `/auth/login`. Successful login issues a short-lived, unguessable, HttpOnly/Secure/SameSite cookie. Server, not browser metadata, chooses the actor and permitted tenants. Failed login does not start a session.
3. **Session:** `GET /auth/session` returns only actor/tenant display information, expiry, CSRF value and a server-derived allowlist of visible capabilities; no bearer or password. The existing `RequestAuthenticator` and governed gateway re-check every data request. Access checks, tenant isolation, FORCE RLS and audit are unchanged.
4. **Logout:** `POST /auth/logout` with CSRF invalidates the server session and cookie. Expiry, revocation or process restart cause **re-login**; silent session reconstruction is not required for the pilot. The user remains signed out on errors.
5. **Browser:** no production fallback to `VITE_CRM_DEV_*`, hardcoded user or administrative `CRM_API_BEARER_TOKEN`; one visible **Sign in / Sign out** experience. All protected routes fail closed unless the server attests to a valid session.

**Release boundary:** the initial implementation remains deliberately small. Before actual production launch, verify only the **mandatory baseline** for safe use: authenticated access, tenant isolation, existing authorization/RLS, secure cookie/CSRF/HTTPS, release installation, backup and recovery, and absence of critical defects (#313/#314). **Additional strengthening and optional complexity belong strictly AFTER the CRM is in real production operation**, prioritized from observed incidents, measurements and actual needs—not during pilot development or pre-production preparation. OIDC/SSO, extra role tooling and refresh infrastructure are not prerequisites unless a fundamental launch blocker is proved.

## Essential, not optional, safety

- No identity/tenant comes from browser claims, URL, or developer feature flags.
- Cookie material is unpredictable, never readable by JavaScript, expires, is revoked on logout; server side verifies it on every protected HTTP **and gRPC-web** request.
- Unsafe cookie requests require CSRF protection; only trusted same-origin HTTPS entry; no auth leakage into logs or frontend artifacts.
- Actor/tenant permissions remain enforced by the existing gateway and PostgreSQL RLS; admin/service bootstrap bearer is not exposed.
- Even pilot tests must prove permitted and denied reads/writes, two actors and tenants, forbidden cross-tenant access, bad cookie, expiry, logout, restart, and server failure. No UI-only sign-in simulation.

## No extra architecture

Reuse `crates/crm-application-runtime`, `crates/crm-capability-ingress`, `packages/client` and `apps/web`. Keep all code close to its owner. Zero new business owner modules; no alternative CRUD API or duplicate policy tables. Do not add OIDC, refresh-token lifecycles, optional persistence layers or other cross-process machinery as speculative preparation. **Additional improvements are evaluated only AFTER real production launch**, based on actual use; necessary correctness, security and recovery checks remain part of the initial working implementation.

## Current implementation, accurately

Draft PR #318 now contains the fail-closed browser adapter **and the initial server implementation** in existing `crm-application-runtime`: configured pilot operators, cryptographically random 8-hour HttpOnly/Secure/SameSite cookie sessions, `GET/POST /auth/login`, `GET /auth/session`, `POST /auth/logout`, and conversion of an authenticated session to an existing token grant for **both HTTP and gRPC-web**. Authorized capability IDs are selected from the existing production composition; the established gateway/authorization/RLS remains authoritative. Basic session negative/unit tests are included.

**Configuration for a closed test installation:** set `CRM_PUBLIC_ORIGIN` to the exact public HTTPS origin and provide `CRM_PILOT_OPERATORS` as a server-side JSON array of `{username,password,actor_id,tenant_id,capabilities}`. Passwords must be independent randomly generated secrets of **at least 32 bytes**, entered only over HTTPS, and never checked into the repository or browser bundles. `tenant_id` must already be declared under `CRM_API_TENANTS`; allowed `capabilities` must match production capability identifiers. Up to 16 explicit operators and 128 simultaneous sessions are supported. The existing process bootstrap bearer stays server-only. Users sign in at `/auth/login`; after process restart they sign in again. Do **not** enable public sign-up or expose internal runtime ports.

**Remaining G1 work / NO-GO:** record the complete production build and real-process HTTP/gRPC-web integration tests on an actual HTTPS same-origin reverse proxy with PostgreSQL, two operators and two tenants, including negative cross-tenant attempts and CSRF/logout/restart. Validate full CI on one unchanged head and resolve the permanent Step13 complexity budget; do not disable or silently relax that gate. The G1 PR remains draft, and neither successful unit tests nor a rendered sign-in page establishes production readiness. No public deployment is permitted on this draft.

## Measured acceptance blockers (CI evidence, 2026-11-06)

G1 code is **not accepted by CI**. Exact-head intermediate runs measured the old ADR-031 Step13 gate as **blocking**: workspace public Rust items **5,389 vs 5,377**, `crm-application-runtime` non-comment LOC **7,879 vs 7,269**, and direct dependency declarations **271 vs 270**. This is measured growth for the necessary real user-facing identity feature. The G1 draft now **proposes an explicit, reviewable exact-value budget recalibration** in `step13-complexity-policy.json`, tracking #194 / ADR-031, with original ceilings preserved in `pilot_g1_measurement.previous_accepted_budget` and new precise ceilings 5,389 / 7,922 / 271. **Not accepted until reviewed and all exact-head CI pass**; checks remain blocking and no generic headroom, suppression or new crate is granted. Code-shuffling solely to conceal LOC is not simplification.

Customer Privacy Operations currently pins the exact `runtime.rs` Git blob SHA in `scripts/customer_privacy_operations.py` and must verify the **same** source used in G1; do not delete the guard or weaken accepted checks. Rust Generated Sync can format changed files; only the final **human-authored** unchanged SHA with complete applicable checks may be accepted. Historical CI passes on earlier heads are not acceptance of later heads.

**Mandatory functional proof not yet supplied:** running HTTPS same-origin built browser + process HTTP/gRPC-web + PostgreSQL with two genuinely distinct operators/tenants, successful permitted operations, denied cross-tenant and revoked operations, malformed cookie/CSRF/expiry/restart and a safe backup/recovery run. A login form and unit-level token assertions alone do not pass G1. Additional enhancements beyond the necessary safe working CRM are deferred until **after actual real production launch**, not before.

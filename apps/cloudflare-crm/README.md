# CRM — native Cloudflare pilot (not deployed)

Canonical work: #321 architecture, #322 chat-based development and browser testing.
Dedicated Cloudflare account: `CRM`. Dev D1: `crm-dev`. The Worker name is `crm-dev`.

**Current scope:** minimal independently testable API foundation for customers.
It uses verified Cloudflare Access identity (`ctx.access.getIdentity()`), maps
the identity to an enabled tenant membership in D1, scopes every SQL query
by that server-resolved tenant and writes business mutations + audit in one
D1 batch. No testing identity is accepted from a request header.

No real users or CRM records are provisioned by the migration. Administrators
must enroll test identities under an explicit tenant from a private control
procedure; never expose a public self-signup endpoint.

## Before deployment

- Enable Cloudflare Access for the new account. Protect the `crm-dev`
  Worker (including the assets) before deploying business pages. A Worker
  script that rejects unauthenticated API requests does not replace an Access
  policy for the SPA.
- Cloudflare `cf` CLI and `cloudflare.config.ts` are the native project
  interface. Once the Cloudflare account is prepared, run `cf init .` in
  this directory to add the official build/Vite setup, pin the generated
  dependencies and lockfile, and commit the generated files.
- Run the D1 migration for **crm-dev**, never another account. Initialize
  the UI, enable Access, then build/deploy and independently read back the
  deployed source SHA and D1 schema version. No production claim without
  deployed browser tests and independent D1 recovery.
- To validate the backend source now, use `node --test test/*.test.mjs`.
  It has no npm dependencies. Browser/Playwright scenarios are the next slice.

The existing G2 Rust/PostgreSQL code remains unchanged on main. This
separate Cloudflare experiment does **not** replace G2 acceptance evidence.

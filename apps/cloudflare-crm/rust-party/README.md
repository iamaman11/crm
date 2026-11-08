# Native Rust Party Worker — local integration gate

This is a **non-deployed** first adapter proof. It links the existing
`modules/crm-parties` crate directly, uses the first-party Cloudflare
`worker` (workers-rs) SDK, and persists through a local D1 binding.

The `ci-local` feature deliberately accepts **only** `tenant-a` or
`tenant-b` from an isolated GitHub Actions test, gated by
`CRM_CI_LOCAL_ONLY=yes`. Without that feature, the Worker denies every request.
**Never deploy this test harness to the public internet, and never use
this fake CI tenant header as production authentication.**

This harness neither edits the production JS CRM Worker nor adds a deployed
Cloudflare service or a second user-visible customer master. Production
integration must derive actor/tenant from verified Cloudflare Access identity
and replace the JS customer authority only after full physical proof.
Schema is intentionally separate from crm-dev D1.

A single D1 trigger atomically writes canonical state, an immutable audit row
and an idempotency receipt for create or versioned update. A duplicate
idempotency key is enforced by SQLite unique constraint and rolls back.
The original Rust owner applies version and input invariants.

The pinned `Cargo.lock` is committed; CI must not regenerate the dependency graph.
Build locally under GitHub CI with `worker-build --release --features ci-local`;
test with `wrangler dev --local` and `wrangler d1 execute --local`.
Nothing from this directory is pushed as a second production Worker.

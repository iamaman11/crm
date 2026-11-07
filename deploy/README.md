# G3 — minimal closed-pilot server package (IMPLEMENTING)

Execution issue: https://github.com/iamaman11/crm/issues/313.

This packet adds a normal Compose layout: **Caddy (HTTPS + built React + same-origin proxy), one crm-api process, PostgreSQL 17**, and two *exiting* initialization jobs (migration and provisioning). No Kubernetes, new authorization service, or duplicated business storage.

## Scope and current acceptance boundary

A first-install **provision** one-shot now creates the persistent tenant, service actor,
configured active user actors, real published compiled module manifests and capabilities,
module installations, and one audit/outbox/idempotency-backed installation transaction.
It reads the exact compiled capability definitions from the real `crm-api` binary and
uses the existing canonical manifest compiler. It never loads test fixtures,
demo records or debug identities. Reruns verify the existing installation without
rewriting user data. The API still runs with `CRM_BOOTSTRAP_ALLOW_PHASE6=false`.

**This implementation is NOT yet accepted:** prove a built installation against
fresh PostgreSQL, browser HTTPS CRUD, cross-tenant denial, complete data recovery
and rollback drills. Keep G3 NO-GO until that evidence exists.

## Infrastructure preview (not GO for user data)

1. Use a fresh checkout and an isolated server with Docker Compose v2. Copy `deploy/.env.example` to `deploy/.env`; fill every placeholder with real values. Never commit or print the file. Set its mode to 0600.
2. Ensure `CRM_SITE_HOST` resolves to this machine, with ports 80/443 permitted. Caddy handles public TLS and forwards both `/auth/*` and `/v1/*` to the existing HTTP listener; gRPC-web goes to its existing HTTP/1.1-compatible gRPC listener. API and PostgreSQL have **no published ports**.
3. `docker compose --env-file deploy/.env -f deploy/compose.yaml config --quiet` checks configuration, not runtime readiness.
4. `docker compose --env-file deploy/.env -f deploy/compose.yaml up -d --build` runs two exiting jobs: migrations/role creation and native first-install provisioning, then starts the API and web services. Reruns verify persisted identities and module installations without changing business records.
5. Check `docker compose --env-file deploy/.env -f deploy/compose.yaml ps` and `docker compose --env-file deploy/.env -f deploy/compose.yaml logs api` for diagnostics. Never post secrets or logs containing passwords.

## Recovery prototype

- `bash deploy/backup.sh /secure/private/directory` creates a custom-format PostgreSQL archive plus a portable basename-only SHA-256 checksum with filesystem mode 0600. Keep archives off the public web host and apply access controls; this is not an offsite-backup solution.
- `bash deploy/restore.sh /secure/private/directory/crm-YYYYMMDDTHHMMSSZ.dump crm-restore-drill` creates a **distinct** PostgreSQL volume under a separately named Compose project and refuses to overwrite any existing CRM schema. It then replays the idempotent migration journal and provisions the separate database runtime login role (not included in `pg_dump`) with `NOBYPASSRLS`; it never starts the restored API by default.
- Verify actual customer/contact/deal/task, tenant isolation, audit/outbox and independent startup after restoration. A successful `pg_restore` exit alone is NOT G3 recovery acceptance.

## Upgrade and rollback boundary

Record the exact git commit + built image digest; take and independently restore a backup before a schema-changing upgrade. Compose recreates the same three long-running services. The migration journal rejects changed scripts; the migrator executes each migration in a single transaction plus checksum checkpoint (both originally wrapped and unwrapped published SQL migrations). **No automatic destructive database rollback**: application rollback is allowed only while compatible with migrated schema; otherwise restore a separately verified pre-upgrade backup in an isolated environment. Full acceptance and measured recovery are tracked in #313.

## Next required G3 code

Prove first-install provisioning against clean PostgreSQL, then production-built HTTPS browser CRUD, persistence after restart, cross-tenant denial, full business/audit/outbox recovery, and exact-head CI. Keep this PR draft/NO-GO until all are proven.

## Installation-level HTTPS smoke

The existing Governance CI builds the actual production React and Caddy images,
starts the API with a fresh provisioned PostgreSQL and executes `deploy/smoke_https.py`
against `https://localhost` with a CI-local certificate. It checks origin rejection,
valid/invalid operator authentication, cookie session, CSRF and sign-out revocation.
`--local-insecure-tls` is **CI-only** and rejects non-local origins; it is never a
production TLS bypass. A real host must use a DNS-backed trusted certificate.

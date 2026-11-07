# G3 — minimal closed-pilot server package (IMPLEMENTING)

Execution issue: https://github.com/iamaman11/crm/issues/313.

This packet adds a normal Compose layout: **Caddy (HTTPS + built React + same-origin proxy), one crm-api process, PostgreSQL 17**, and one *exiting* migration job. No Kubernetes, new authorization service, or duplicated business storage.

## Scope and current hard blocker

The infrastructure files are **not yet an accepted installation**. A fresh schema alone cannot authenticate a useful administrator: the existing G1 runtime needs durable active `crm.tenants`, `crm.actors` and published module/activation state. Current tests populate these through **test fixtures**, including mock manifest digests and synthetic bootstrap transactions. Those fixtures MUST NOT be copied to a real deployment. G3 must implement a small first-install operator/tenant/module provisioning mechanism that preserves the project's transactional audit/RLS requirements, then demonstrate it on a fresh database. Until that is proved: **NO-GO**.

The Compose file deliberately sets `CRM_BOOTSTRAP_ALLOW_PHASE6=false`: the legacy test/bootstrap switch cannot become the production installation mechanism.

## Infrastructure preview (not GO for user data)

1. Use a fresh checkout and an isolated server with Docker Compose v2. Copy `deploy/.env.example` to `deploy/.env`; fill every placeholder with real values. Never commit or print the file. Set its mode to 0600.
2. Ensure `CRM_SITE_HOST` resolves to this machine, with ports 80/443 permitted. Caddy handles public TLS and forwards both `/auth/*` and `/v1/*` to the existing HTTP listener; gRPC-web goes to its existing HTTP/1.1-compatible gRPC listener. API and PostgreSQL have **no published ports**.
3. `docker compose --env-file deploy/.env -f deploy/compose.yaml config --quiet` checks configuration, not runtime readiness.
4. `docker compose --env-file deploy/.env -f deploy/compose.yaml up -d --build` executes schema migrations once per digest, creates the non-superuser runtime role and starts the services. This **will not pass G3 acceptance** until the authoritative first-install provisioning action exists and passes live tests.
5. Check `docker compose --env-file deploy/.env -f deploy/compose.yaml ps` and `docker compose --env-file deploy/.env -f deploy/compose.yaml logs api` for diagnostics. Never post secrets or logs containing passwords.

## Recovery prototype

- `bash deploy/backup.sh /secure/private/directory` creates a custom-format PostgreSQL archive plus a portable basename-only SHA-256 checksum with filesystem mode 0600. Keep archives off the public web host and apply access controls; this is not an offsite-backup solution.
- `bash deploy/restore.sh /secure/private/directory/crm-YYYYMMDDTHHMMSSZ.dump crm-restore-drill` creates a **distinct** PostgreSQL volume under a separately named Compose project and refuses to overwrite any existing CRM schema. It then replays the idempotent migration journal and provisions the separate database runtime login role (not included in `pg_dump`) with `NOBYPASSRLS`; it never starts the restored API by default.
- Verify actual customer/contact/deal/task, tenant isolation, audit/outbox and independent startup after restoration. A successful `pg_restore` exit alone is NOT G3 recovery acceptance.

## Upgrade and rollback boundary

Record the exact git commit + built image digest; take and independently restore a backup before a schema-changing upgrade. Compose recreates the same three long-running services. The migration journal rejects changed scripts; the migrator executes each migration in a single transaction plus checksum checkpoint (both originally wrapped and unwrapped published SQL migrations). **No automatic destructive database rollback**: application rollback is allowed only while compatible with migrated schema; otherwise restore a separately verified pre-upgrade backup in an isolated environment. Full acceptance and measured recovery are tracked in #313.

## Next required G3 code

Implement the first-install operator/actor/tenant/module provisioning without demo/test fixtures; wire a release-equivalent fresh-host HTTPS browser test, restart and independent restore, and exact-head CI. Keep this PR draft/NO-GO until all are proven.

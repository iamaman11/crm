#!/usr/bin/env bash
# CI-only additive-schema and pinned-image drill; never run on real user data.
set -euo pipefail
umask 077
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
[[ -f "$here/.env" ]] || exit 2
work="$(mktemp -d "${RUNNER_TEMP:-/tmp}/crm-g3-upgrade.XXXXXX")"
api_tag="crm-g3-rollback-api:${GITHUB_RUN_ID:-local}"
web_tag="crm-g3-rollback-web:${GITHUB_RUN_ID:-local}"
cleanup() {
  rm -rf "$work"
  docker image rm "$api_tag" "$web_tag" >/dev/null 2>&1 || true
}
trap cleanup EXIT
c=(docker compose --env-file "$here/.env" -f "$here/compose.yaml")
api_id="$(docker inspect --format '{{.Image}}' "$("${c[@]}" ps -q api)")"
web_id="$(docker inspect --format '{{.Image}}' "$("${c[@]}" ps -q web)")"
[[ "$api_id" == sha256:* && "$web_id" == sha256:* ]] || exit 2
docker image tag "$api_id" "$api_tag"
docker image tag "$web_id" "$web_tag"
rows() {
  "${c[@]}" exec -T database psql -X -qAt -v ON_ERROR_STOP=1 -U postgres -d crm -c \
    'SELECT count(*) FROM public.crm_schema_migrations'
}
before="$(rows)"
[[ "$before" =~ ^[0-9]+$ ]] || exit 2
mkdir "$work/migrations"
cp "$root"/database/migrations/*.up.sql "$work/migrations/"
cat >"$work/migrations/zzzz_g3_additive_upgrade_probe.up.sql" <<'SQL'
BEGIN;
CREATE TABLE IF NOT EXISTS crm.g3_additive_upgrade_probe (
  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  applied_at timestamptz NOT NULL DEFAULT now()
);
COMMIT;
SQL
printf 'services:\n  migrate:\n    volumes:\n      - %s:/migrations:ro\n' "$work/migrations" >"$work/upgrade.yml"
up=(docker compose --env-file "$here/.env" -f "$here/compose.yaml" -f "$work/upgrade.yml")
"${up[@]}" run --rm migrate
after="$(rows)"
[[ "$after" -eq $((before + 1)) ]] || { echo "Migration journal count mismatch" >&2; exit 3; }
"${up[@]}" run --rm migrate
[[ "$(rows)" == "$after" ]] || { echo "Migration replay not idempotent" >&2; exit 3; }
printf '\n-- changed after apply\n' >> "$work/migrations/zzzz_g3_additive_upgrade_probe.up.sql"
if "${up[@]}" run --rm migrate >"$work/rejected.log" 2>&1; then
  echo "Modified applied migration accepted" >&2; exit 3
fi
grep -q 'Migration was modified after applying:' "$work/rejected.log" || exit 3
[[ "$(rows)" == "$after" ]] || exit 3
# Recreate EXACT original images with no build; persistent PostgreSQL unchanged.
printf 'services:\n  api:\n    image: %s\n  web:\n    image: %s\n' "$api_tag" "$web_tag" >"$work/rollback.yml"
rollback=(docker compose --env-file "$here/.env" -f "$here/compose.yaml" -f "$work/rollback.yml")
"${rollback[@]}" up -d --no-build --no-deps --force-recreate --wait api web
[[ "$(docker inspect --format '{{.Image}}' "$("${c[@]}" ps -q api)")" == "$api_id" ]] || exit 4
[[ "$(docker inspect --format '{{.Image}}' "$("${c[@]}" ps -q web)")" == "$web_id" ]] || exit 4
echo "PASS: additive migration, idempotence, checksum refusal, pinned-image recreation."
echo "LIMIT: genuine older application binary compatibility remains unproven."

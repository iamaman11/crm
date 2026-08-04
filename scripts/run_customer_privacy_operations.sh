#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

eval "$(python scripts/customer_privacy_operations.py shell-env)"

ARTIFACT_DIR="${CRM_OPERATIONS_ARTIFACT_DIR:-build/customer-privacy-operations}"
POSTGRES_PORT="${CRM_OPERATIONS_POSTGRES_PORT:-55434}"
CONTAINER_NAME="crm-privacy-operations-${GITHUB_RUN_ID:-local}-$$"
BACKUP_IN_CONTAINER="/tmp/customer-privacy-operations.dump"
BACKUP_PATH="${ARTIFACT_DIR}/customer-privacy-operations.dump"
LATENCY_PATH="${ARTIFACT_DIR}/readiness-latencies.txt"
METRICS_PATH="${ARTIFACT_DIR}/metrics.prom"
SUPPLY_CHAIN_PATH="${ARTIFACT_DIR}/supply-chain.sha256"
REPORT_PATH="${ARTIFACT_DIR}/report.json"
API_LOG_PATH="${ARTIFACT_DIR}/crm-api.log"
VITE_LOG_PATH="${ARTIFACT_DIR}/vite.log"
TOKEN="phase20b-operations-bearer-token-0123456789abcdef0123456789abcdef"
HTTP_PORT="${CRM_OPERATIONS_HTTP_PORT:-18080}"
GRPC_PORT="${CRM_OPERATIONS_GRPC_PORT:-19090}"
VITE_PORT=5173
API_PID=""
VITE_PID=""

mkdir -p "$ARTIFACT_DIR"
rm -f "$BACKUP_PATH" "$LATENCY_PATH" "$METRICS_PATH" "$REPORT_PATH" \
  "$API_LOG_PATH" "$VITE_LOG_PATH" "$SUPPLY_CHAIN_PATH"

kill_tree() {
  local target_pid="${1:-}"
  if [ -z "$target_pid" ]; then return; fi
  local children
  children="$(pgrep -P "$target_pid" 2>/dev/null || true)"
  for child in $children; do kill_tree "$child"; done
  kill -TERM "$target_pid" 2>/dev/null || true
  sleep 0.2
  kill -KILL "$target_pid" 2>/dev/null || true
}

cleanup() {
  set +e
  kill_tree "$VITE_PID"
  kill_tree "$API_PID"
  docker rm --force "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

admin_psql() {
  local database="$1"; shift
  docker exec -i "$CONTAINER_NAME" psql --username postgres --dbname "$database" \
    --no-psqlrc --set ON_ERROR_STOP=1 "$@"
}

apply_database_inputs() {
  local database="$1"
  for path in database/migrations/*up.sql; do admin_psql "$database" < "$path"; done
  admin_psql "$database" < database/tests/0001_platform_foundation.sql
  admin_psql "$database" < database/tests/0003_sales_activities_adapters.sql
  admin_psql "$database" < database/tests/0004_search_runtime_role_grants.sql
}

echo "Starting immutable PostgreSQL operations target ${CONTAINER_NAME}..."
docker run --detach --rm --name "$CONTAINER_NAME" \
  --publish "127.0.0.1:${POSTGRES_PORT}:5432" \
  --env "POSTGRES_DB=${OPS_SOURCE_DATABASE}" --env POSTGRES_USER=postgres \
  --env POSTGRES_PASSWORD=postgres "$OPS_POSTGRES_IMAGE" >/dev/null
for _ in $(seq 1 120); do
  docker exec "$CONTAINER_NAME" pg_isready --username postgres \
    --dbname "$OPS_SOURCE_DATABASE" >/dev/null 2>&1 && break
  sleep 0.5
done
docker exec "$CONTAINER_NAME" pg_isready --username postgres \
  --dbname "$OPS_SOURCE_DATABASE" >/dev/null

apply_database_inputs "$OPS_SOURCE_DATABASE"
admin_psql "$OPS_SOURCE_DATABASE" <<'SQL'
DO $operations$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'crm_app_test') THEN
    CREATE ROLE crm_app_test LOGIN PASSWORD 'crm_app_test';
  ELSE
    ALTER ROLE crm_app_test LOGIN PASSWORD 'crm_app_test';
  END IF;
END
$operations$;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA crm TO crm_app_test;
SQL

SOURCE_APP_URL="postgres://crm_app_test:crm_app_test@127.0.0.1:${POSTGRES_PORT}/${OPS_SOURCE_DATABASE}"
SOURCE_ADMIN_URL="postgres://postgres:postgres@127.0.0.1:${POSTGRES_PORT}/${OPS_SOURCE_DATABASE}"

echo "Verifying fail-closed application-role persistence privileges..."
MISSING_PRIVILEGES="$(admin_psql "$OPS_SOURCE_DATABASE" --tuples-only --no-align <<'SQL'
WITH required(table_name, privileges) AS (
  VALUES
    ('records', ARRAY['SELECT','INSERT','UPDATE']::text[]),
    ('idempotency_records', ARRAY['SELECT','INSERT','UPDATE']::text[]),
    ('outbox_events', ARRAY['INSERT']::text[]),
    ('audit_records', ARRAY['SELECT','INSERT']::text[]),
    ('business_transactions', ARRAY['INSERT']::text[])
), missing AS (
  SELECT table_name, privilege
  FROM required CROSS JOIN LATERAL unnest(privileges) AS privilege
  WHERE NOT has_table_privilege('crm_app_test', format('crm.%I', table_name), privilege)
)
SELECT table_name || ':' || privilege FROM missing ORDER BY table_name, privilege;
SQL
)"
if [ -n "$MISSING_PRIVILEGES" ]; then
  echo "application role is missing persistence privileges: ${MISSING_PRIVILEGES}" >&2
  exit 1
fi

echo "Creating the governed Customer Privacy fixture through assembled production mutations..."
DATABASE_URL="$SOURCE_APP_URL" ADMIN_DATABASE_URL="$SOURCE_ADMIN_URL" RUST_BACKTRACE=1 \
  cargo test -p crm-api --test seed_e2e_fixture -- --nocapture

echo "Creating logical backup evidence with owner replay disabled..."
docker exec "$CONTAINER_NAME" pg_dump --username postgres \
  --dbname "$OPS_SOURCE_DATABASE" --format custom --compress 9 --no-owner \
  --file "$BACKUP_IN_CONTAINER"
docker cp "$CONTAINER_NAME:$BACKUP_IN_CONTAINER" "$BACKUP_PATH" >/dev/null
chmod 600 "$BACKUP_PATH"
[ "$(stat -c '%a' "$BACKUP_PATH")" = "600" ] || { echo "backup artifact permissions are not 0600" >&2; exit 1; }
BACKUP_SHA256="$(sha256sum "$BACKUP_PATH" | awk '{print $1}')"

echo "Restoring the logical backup into an independent database..."
admin_psql postgres --command "CREATE DATABASE ${OPS_RESTORE_DATABASE};"
docker exec "$CONTAINER_NAME" pg_restore --username postgres \
  --dbname "$OPS_RESTORE_DATABASE" --exit-on-error --no-owner "$BACKUP_IN_CONTAINER"
SOURCE_TABLE_COUNT="$(admin_psql "$OPS_SOURCE_DATABASE" --tuples-only --no-align --command \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema NOT IN ('pg_catalog', 'information_schema');" | tr -d '[:space:]')"
RESTORE_TABLE_COUNT="$(admin_psql "$OPS_RESTORE_DATABASE" --tuples-only --no-align --command \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema NOT IN ('pg_catalog', 'information_schema');" | tr -d '[:space:]')"
[ -n "$SOURCE_TABLE_COUNT" ] && [ "$SOURCE_TABLE_COUNT" = "$RESTORE_TABLE_COUNT" ] || {
  echo "restored table inventory does not match source: ${SOURCE_TABLE_COUNT} != ${RESTORE_TABLE_COUNT}" >&2; exit 1;
}
RESTORE_APP_URL="postgres://crm_app_test:crm_app_test@127.0.0.1:${POSTGRES_PORT}/${OPS_RESTORE_DATABASE}"

STARTED_AT="$(python -c 'import time; print(time.monotonic())')"
echo "Starting assembled crm-api against the restored database..."
CRM_DATABASE_URL="$RESTORE_APP_URL" CRM_API_BEARER_TOKEN="$TOKEN" \
CRM_API_ACTOR_ID=actor-a CRM_API_TENANTS=tenant-a CRM_BOOTSTRAP_ALLOW_PHASE6=true \
CRM_CURSOR_SIGNING_KEY=phase20b-cursor-signing-key-0123456789abcdef0123456789abcdef \
CRM_APPROVAL_SIGNING_KEY=phase20b-approval-signing-key-0123456789abcdef0123456789abcdef \
CRM_GRPC_BIND="127.0.0.1:${GRPC_PORT}" CRM_HTTP_BIND="127.0.0.1:${HTTP_PORT}" \
./target/debug/crm-api >"$API_LOG_PATH" 2>&1 &
API_PID=$!
for _ in $(seq 1 $((OPS_STARTUP_SLO_SECONDS * 4))); do
  curl --fail --silent --show-error "http://127.0.0.1:${HTTP_PORT}/readyz" >/dev/null 2>&1 && break
  kill -0 "$API_PID" 2>/dev/null || { cat "$API_LOG_PATH" >&2; exit 1; }
  sleep 0.25
done
curl --fail --silent --show-error "http://127.0.0.1:${HTTP_PORT}/healthz" | python -c 'import json,sys; assert json.load(sys.stdin)=={"status":"ok"}'
curl --fail --silent --show-error "http://127.0.0.1:${HTTP_PORT}/readyz" | python -c 'import json,sys; assert json.load(sys.stdin)=={"status":"ready"}'
READY_AT="$(python -c 'import time; print(time.monotonic())')"
STARTUP_SECONDS="$(python - "$STARTED_AT" "$READY_AT" <<'PY'
import sys
print(f"{float(sys.argv[2])-float(sys.argv[1]):.6f}")
PY
)"

PROBE_FAILURES=0
for _ in $(seq 1 "$OPS_PROBE_COUNT"); do
  if latency="$(curl --fail --silent --show-error --output /dev/null --write-out '%{time_total}' "http://127.0.0.1:${HTTP_PORT}/readyz")"; then
    printf '%s\n' "$latency" >> "$LATENCY_PATH"
  else
    PROBE_FAILURES=$((PROBE_FAILURES + 1)); printf '9999\n' >> "$LATENCY_PATH"
  fi
done

echo "Starting product plane against the restored crm-api..."
rm -rf apps/web/node_modules/.vite
VITE_CRM_GRPC_WEB_TARGET="http://127.0.0.1:${GRPC_PORT}" \
VITE_CRM_DEV_BEARER_TOKEN="$TOKEN" VITE_CRM_DEV_TENANT_ID=tenant-a \
VITE_CRM_DEV_CAPABILITIES=search.global.query,customer_privacy.case.list,customer_privacy.case.get,metadata.activation.get \
pnpm --filter @ultimate-crm/web dev --force --host 127.0.0.1 --port "$VITE_PORT" >"$VITE_LOG_PATH" 2>&1 &
VITE_PID=$!
for _ in $(seq 1 120); do
  curl --fail --silent --show-error "http://127.0.0.1:${VITE_PORT}" >/dev/null 2>&1 && break
  kill -0 "$VITE_PID" 2>/dev/null || { cat "$VITE_LOG_PATH" >&2; exit 1; }
  sleep 0.5
done
curl --fail --silent --show-error "http://127.0.0.1:${VITE_PORT}" >/dev/null
pnpm --filter @ultimate-crm/web exec playwright test e2e/customer-privacy.spec.ts \
  --config=playwright.config.ts --timeout="$((OPS_BROWSER_TIMEOUT_SECONDS * 1000))"

curl --fail --silent --show-error "http://127.0.0.1:${HTTP_PORT}/metrics" > "$METRICS_PATH"
grep --fixed-strings --quiet "$TOKEN" "$METRICS_PATH" && { echo "metrics output contains the bearer token" >&2; exit 1; }
sha256sum Cargo.lock pnpm-lock.yaml rust-toolchain.toml \
  .github/workflows/customer-privacy-operations.yml \
  scripts/customer_privacy_operations.py scripts/run_customer_privacy_operations.sh > "$SUPPLY_CHAIN_PATH"
python scripts/customer_privacy_operations.py report \
  --startup-seconds "$STARTUP_SECONDS" --latencies "$LATENCY_PATH" \
  --probe-failures "$PROBE_FAILURES" --metrics "$METRICS_PATH" \
  --supply-chain "$SUPPLY_CHAIN_PATH" --backup "$BACKUP_PATH" \
  --backup-sha256 "$BACKUP_SHA256" --output "$REPORT_PATH"
echo "Customer Privacy Step 20B operations acceptance passed."

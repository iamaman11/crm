#!/bin/bash
set -euo pipefail

E2E_DB_HOST="127.0.0.1"
E2E_DB_PORT="5433"
E2E_DB_USER="postgres"
E2E_DB_PASSWORD="postgres"
DB_NAME="crm_test"

CONTAINER_NAME="crm-postgres-e2e-${GITHUB_RUN_ID:-local}-$$"

API_PID=""
VITE_PID=""

log() {
  printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"
}

kill_descendants() {
  local target_pid=${1:-}
  if [ -z "$target_pid" ]; then
    return
  fi

  local children
  children=$(pgrep -P "$target_pid" 2>/dev/null || true)
  for child in $children; do
    kill_descendants "$child"
  done
  kill -9 "$target_pid" 2>/dev/null || true
}

cleanup() {
  log "Cleaning up E2E-owned background services..."

  if [ -n "$API_PID" ]; then
    log "Stopping crm-api process tree (PID: $API_PID)..."
    kill_descendants "$API_PID"
  fi

  if [ -n "$VITE_PID" ]; then
    log "Stopping Vite process tree (PID: $VITE_PID)..."
    kill_descendants "$VITE_PID"
  fi

  jobs -p | xargs -r kill 2>/dev/null || true

  if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    log "Stopping ephemeral PostgreSQL container ${CONTAINER_NAME}..."
    docker stop --time 5 "${CONTAINER_NAME}" >/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

wait_for_command() {
  local description=$1
  local timeout_seconds=$2
  shift 2

  local deadline=$((SECONDS + timeout_seconds))
  until "$@" >/dev/null 2>&1; do
    if (( SECONDS >= deadline )); then
      log "Timed out after ${timeout_seconds}s waiting for ${description}."
      return 1
    fi
    sleep 0.5
  done
}

wait_for_http_process() {
  local description=$1
  local url=$2
  local pid=$3
  local timeout_seconds=$4
  local deadline=$((SECONDS + timeout_seconds))

  until curl --fail --silent --show-error "$url" >/dev/null 2>&1; do
    if ! kill -0 "$pid" 2>/dev/null; then
      log "${description} exited before becoming ready."
      wait "$pid" || true
      return 1
    fi
    if (( SECONDS >= deadline )); then
      log "Timed out after ${timeout_seconds}s waiting for ${description} at ${url}."
      return 1
    fi
    sleep 0.5
  done
}

log "Starting ephemeral PostgreSQL container ${CONTAINER_NAME} on port ${E2E_DB_PORT}..."
docker run --rm --name "${CONTAINER_NAME}" -p "${E2E_DB_PORT}:5432" -e POSTGRES_PASSWORD="${E2E_DB_PASSWORD}" -d postgres:16-alpine >/dev/null

wait_for_command "ephemeral PostgreSQL" 60 docker exec "${CONTAINER_NAME}" pg_isready -U "${E2E_DB_USER}"
log "PostgreSQL is ready."

docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -c "CREATE DATABASE ${DB_NAME};"

log "Applying migrations and deterministic test fixtures..."
for f in database/migrations/*up.sql; do
  docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < "$f"
done
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < database/tests/0001_platform_foundation.sql
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < database/tests/0003_sales_activities_adapters.sql
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < database/tests/0004_search_runtime_role_grants.sql

log "Enabling LOGIN and setting password for crm_app_test..."
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" -c "
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'crm_app_test') THEN
    CREATE ROLE crm_app_test WITH LOGIN PASSWORD 'crm_app_test';
  ELSE
    ALTER ROLE crm_app_test WITH LOGIN PASSWORD 'crm_app_test';
  END IF;
END
\$\$;
"

# The integration test references CARGO_BIN_EXE_crm-api, so Cargo builds the crm-api binary
# as part of this single compilation path. A separate `cargo build -p crm-api` is redundant.
log "Building crm-api once and seeding the ephemeral database via seed_e2e_fixture..."
DATABASE_URL="postgres://crm_app_test:crm_app_test@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
ADMIN_DATABASE_URL="postgres://${E2E_DB_USER}:${E2E_DB_PASSWORD}@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
timeout --signal=TERM --kill-after=30s 40m cargo test -p crm-api --test seed_e2e_fixture -- --nocapture

log "Starting crm-api service..."
CRM_DATABASE_URL="postgres://crm_app_test:crm_app_test@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
CRM_API_BEARER_TOKEN=phase6l-process-bearer-token-0123456789abcdef0123456789abcdef \
CRM_API_ACTOR_ID=actor-a \
CRM_API_TENANTS=tenant-a \
CRM_BOOTSTRAP_ALLOW_PHASE6=true \
CRM_CURSOR_SIGNING_KEY=phase6l-cursor-signing-key-0123456789abcdef0123456789abcdef \
CRM_APPROVAL_SIGNING_KEY=phase6l-approval-signing-key-0123456789abcdef0123456789abcdef \
CRM_GRPC_BIND=127.0.0.1:9090 \
CRM_HTTP_BIND=127.0.0.1:8080 \
./target/debug/crm-api &
API_PID=$!

wait_for_http_process "crm-api" "http://127.0.0.1:8080/readyz" "$API_PID" 60
log "crm-api is ready."

log "Clearing Vite cache..."
rm -rf apps/web/node_modules/.vite

log "Starting Vite dev server..."
VITE_CRM_GRPC_WEB_TARGET=http://127.0.0.1:9090 \
VITE_CRM_DEV_BEARER_TOKEN=phase6l-process-bearer-token-0123456789abcdef0123456789abcdef \
VITE_CRM_DEV_TENANT_ID=tenant-a \
VITE_CRM_DEV_CAPABILITIES=search.global.query \
pnpm --filter @ultimate-crm/web dev --force &
VITE_PID=$!

wait_for_http_process "Vite dev server" "http://127.0.0.1:5173" "$VITE_PID" 60
log "Vite dev server is ready."

log "Installing Chromium for Playwright..."
timeout --signal=TERM --kill-after=30s 10m pnpm exec playwright install chromium

log "Running Playwright E2E tests..."
timeout --signal=TERM --kill-after=30s 10m pnpm --filter @ultimate-crm/web exec playwright test --config=playwright.config.ts

log "E2E tests passed successfully."

#!/bin/bash
set -euo pipefail

# Configuration fallback
E2E_DB_HOST="127.0.0.1"
E2E_DB_PORT="5433"
E2E_DB_USER="postgres"
E2E_DB_PASSWORD="postgres"
DB_NAME="crm_test"

CONTAINER_NAME="crm-postgres-e2e-$(date +%s)"

echo "Starting ephemeral PostgreSQL container ${CONTAINER_NAME} on port ${E2E_DB_PORT}..."
docker run --rm --name "${CONTAINER_NAME}" -p "${E2E_DB_PORT}:5432" -e POSTGRES_PASSWORD="${E2E_DB_PASSWORD}" -d postgres:16-alpine

API_PID=""
VITE_PID=""
PREVIEW_PID=""
G2_TMP=""

# Helper to recursively kill child processes of a target PID safely
kill_descendants() {
  local target_pid=$1
  if [ -z "$target_pid" ]; then
    return
  fi
  # Find child PIDs
  local children
  children=$(pgrep -P "$target_pid" 2>/dev/null || true)
  for child in $children; do
    kill_descendants "$child"
  done
  kill -9 "$target_pid" 2>/dev/null || true
}

# Ensure cleanup of background services and Docker container on exit
cleanup() {
  local exit_status=$?
  if [ "$exit_status" -ne 0 ]; then
    echo "E2E failed. Last PostgreSQL server errors:" >&2
    docker logs --tail 160 "$CONTAINER_NAME" 2>&1 | tail -n 160 || true
  fi
  echo "Cleaning up background services..."
  
  if [ -n "$API_PID" ]; then
    echo "Stopping crm-api process group recursively (PID: $API_PID)..."
    kill_descendants "$API_PID"
  fi

  if [ -n "$PREVIEW_PID" ]; then
    kill_descendants "$PREVIEW_PID"
  fi

  if [ -n "$G2_TMP" ]; then
    rm -rf "$G2_TMP"
  fi

  if [ -n "$VITE_PID" ]; then
    echo "Stopping Vite process group recursively (PID: $VITE_PID)..."
    kill_descendants "$VITE_PID"
  fi

  # Terminate any remaining background jobs of this shell session
  jobs -p | xargs -r kill 2>/dev/null || true
  sleep 1
  
  # Clean up ephemeral PostgreSQL container if running
  if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo "Stopping ephemeral PostgreSQL container ${CONTAINER_NAME}..."
    docker stop "${CONTAINER_NAME}"
  fi
}
trap cleanup EXIT

# Wait for database container to be ready
until docker exec "${CONTAINER_NAME}" pg_isready -U "${E2E_DB_USER}" >/dev/null 2>&1; do
  echo "Waiting for ephemeral postgres to be ready..."
  sleep 0.5
done
# postgres:16-alpine briefly starts an initdb-only server before restarting
# the final TCP server. pg_isready during initdb is not reliable acceptance.
until docker logs "${CONTAINER_NAME}" 2>&1 | grep -q 'PostgreSQL init process complete; ready for start up.'; do
  if [ "$(docker inspect -f '{{.State.Running}}' "${CONTAINER_NAME}" 2>/dev/null)" != "true" ]; then
    echo "PostgreSQL container exited during initialization." >&2
    exit 1
  fi
  sleep 0.5
done
until docker exec "${CONTAINER_NAME}" pg_isready -U "${E2E_DB_USER}" >/dev/null 2>&1; do
  sleep 0.5
done
echo "PostgreSQL final server is ready!"

# Create test database
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -c "CREATE DATABASE ${DB_NAME};"

echo "Applying migrations and seeds..."
for f in database/migrations/*up.sql; do
  docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < "$f"
done
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < database/tests/0001_platform_foundation.sql
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < database/tests/0003_sales_activities_adapters.sql
docker exec -i "${CONTAINER_NAME}" psql -U "${E2E_DB_USER}" -d "${DB_NAME}" < database/tests/0004_search_runtime_role_grants.sql

# Alter test role to make sure it has LOGIN capability and the password is set
echo "Enabling LOGIN and setting password for crm_app_test..."
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

# Compile backend
cargo build -p crm-api

# Seed database by running seed_e2e_fixture once against our ephemeral postgres
echo "Seeding ephemeral database via seed_e2e_fixture..."
DATABASE_URL="postgres://crm_app_test:crm_app_test@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
ADMIN_DATABASE_URL="postgres://${E2E_DB_USER}:${E2E_DB_PASSWORD}@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
cargo test -p crm-api --test seed_e2e_fixture

# Start crm-api with the existing real owner modules, two isolated tenants
# and the original pilot cookie identity mechanism. These credentials are
# test-only and are never compiled into the browser.
G2_PILOT_OPERATORS='[{"username":"alice","password":"01234567890123456789012345678901","actor_id":"actor-a","tenant_id":"tenant-a","capabilities":["parties.party.create","parties.party.get","parties.party.list","parties.party.update","accounts.account.create","accounts.account.list","contact-points.contact-point.create","contact-points.contact-point.get","contact-points.contact-point.list","contact-points.contact-point.update","sales.deal.create","sales.deal.get","sales.deal.list","sales.deal.update","sales.deal.advance_stage","activities.task.create","activities.task.get","activities.task.list","activities.task.update","activities.task.complete"]},{"username":"bob","password":"abcdefghijklmnopqrstuvwxyzABCDEF","actor_id":"actor-b","tenant_id":"tenant-b","capabilities":["parties.party.list","accounts.account.list","contact-points.contact-point.list","sales.deal.list","activities.task.list"]}]'
start_api() {
  CRM_DATABASE_URL="postgres://crm_app_test:crm_app_test@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
  CRM_API_BEARER_TOKEN=phase6l-process-bearer-token-0123456789abcdef0123456789abcdef \
  CRM_API_ACTOR_ID=actor-a \
  CRM_API_TENANTS=tenant-a,tenant-b \
  CRM_BOOTSTRAP_ALLOW_PHASE6=true \
  CRM_PUBLIC_ORIGIN=https://127.0.0.1:5174 \
  CRM_PILOT_OPERATORS="$G2_PILOT_OPERATORS" \
  CRM_CURSOR_SIGNING_KEY=phase6l-cursor-signing-key-0123456789abcdef0123456789abcdef \
  CRM_APPROVAL_SIGNING_KEY=phase6l-approval-signing-key-0123456789abcdef0123456789abcdef \
  CRM_GRPC_BIND=127.0.0.1:9090 \
  CRM_HTTP_BIND=127.0.0.1:8080 \
  ./target/debug/crm-api &
  API_PID=$!
}
echo "Starting crm-api service..."
start_api

# Wait for backend to be ready
echo "Waiting for crm-api to be ready..."
TIMEOUT=60
COUNTER=0
until curl -fsS http://127.0.0.1:8080/readyz >/dev/null 2>&1; do
  if [ $COUNTER -ge $TIMEOUT ]; then
    echo "Timed out after ${TIMEOUT}s waiting for crm-api to be ready."
    exit 1
  fi
  if ! kill -0 "$API_PID" 2>/dev/null; then
    echo "crm-api exited before becoming ready."
    exit 1
  fi
  sleep 1
  COUNTER=$((COUNTER + 1))
done
echo "crm-api is ready!"

# Clear Vite cache to prevent stale bundle caching of packages/client
echo "Clearing Vite cache..."
rm -rf apps/web/node_modules/.vite

# Start Vite dev server in background
echo "Starting Vite dev server..."
VITE_CRM_GRPC_WEB_TARGET=http://127.0.0.1:9090 \
VITE_CRM_DEV_BEARER_TOKEN=phase6l-process-bearer-token-0123456789abcdef0123456789abcdef \
VITE_CRM_DEV_TENANT_ID=tenant-a \
VITE_CRM_DEV_CAPABILITIES=search.global.query,customer_privacy.case.list,customer_privacy.case.get,metadata.activation.get \
pnpm --filter @ultimate-crm/web dev --force --host 127.0.0.1 &
VITE_PID=$!

# Wait for Vite dev server to be ready
echo "Waiting for Vite to start..."
TIMEOUT=60
COUNTER=0
until curl -fsS http://127.0.0.1:5173 >/dev/null 2>&1; do
  if [ $COUNTER -ge $TIMEOUT ]; then
    echo "Timed out after ${TIMEOUT}s waiting for Vite dev server at http://127.0.0.1:5173."
    exit 1
  fi
  if ! kill -0 "$VITE_PID" 2>/dev/null; then
    echo "Vite dev server exited before becoming ready."
    exit 1
  fi
  sleep 1
  COUNTER=$((COUNTER + 1))
done
echo "Vite dev server is ready!"

# Run Playwright E2E tests
echo "Running Playwright E2E tests..."
pnpm --filter @ultimate-crm/web exec playwright install chromium
pnpm --filter @ultimate-crm/web exec playwright test --config=playwright.config.ts

echo "Legacy E2E tests passed successfully!"

# G2 must run the compiled browser with real cookies/CSRF, not the DEV session
# used by older E2E tests. The self-signed HTTPS key exists only inside this
# disposable CI workspace and is never stored in the repository.
echo "Starting production-build G2 browser acceptance..."
test -f apps/web/dist/index.html
G2_TMP="$(mktemp -d)"
openssl req -x509 -nodes -newkey rsa:2048 -days 1 \
  -subj '/CN=127.0.0.1' -addext 'subjectAltName=IP:127.0.0.1' \
  -keyout "$G2_TMP/key.pem" -out "$G2_TMP/cert.pem" >/dev/null 2>&1
G2_RUN_ID="g2-$(date +%s)-${GITHUB_RUN_ID:-local}"
export G2_E2E G2_RUN_ID
G2_E2E=1
G2_TEST_PHASE=before
export G2_TEST_PHASE
G2_TLS_KEY="$G2_TMP/key.pem" G2_TLS_CERT="$G2_TMP/cert.pem" \
VITE_CRM_GRPC_WEB_TARGET=http://127.0.0.1:9090 \
G2_E2E=1 pnpm --filter @ultimate-crm/web preview --host 127.0.0.1 --port 5174 --strictPort &
PREVIEW_PID=$!

for _ in $(seq 1 60); do
  if curl --insecure --fail --silent https://127.0.0.1:5174/ >/dev/null 2>&1; then break; fi
  if ! kill -0 "$PREVIEW_PID" 2>/dev/null; then echo "G2 preview exited"; exit 1; fi
  sleep 1
done
curl --insecure --fail --silent https://127.0.0.1:5174/ >/dev/null
pnpm --filter @ultimate-crm/web exec playwright test e2e/g2-crm.spec.ts --reporter=list

echo "Restarting crm-api against the SAME PostgreSQL database..."
kill "$API_PID"
wait "$API_PID" || true
API_PID=""
start_api
for _ in $(seq 1 60); do
  if curl --fail --silent http://127.0.0.1:8080/readyz >/dev/null 2>&1; then break; fi
  if ! kill -0 "$API_PID" 2>/dev/null; then echo "crm-api restart failed"; exit 1; fi
  sleep 1
done
curl --fail --silent http://127.0.0.1:8080/readyz >/dev/null
export G2_TEST_PHASE=after
pnpm --filter @ultimate-crm/web exec playwright test e2e/g2-crm.spec.ts --reporter=list
echo "G2 built-browser PostgreSQL, tenant and restart acceptance passed."


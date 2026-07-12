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
  echo "Cleaning up background services..."
  
  if [ -n "$API_PID" ]; then
    echo "Stopping crm-api process group recursively (PID: $API_PID)..."
    kill_descendants "$API_PID"
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
echo "PostgreSQL is ready!"

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

# Start crm-api in background pointing to our ephemeral database
echo "Starting crm-api service..."
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
VITE_CRM_DEV_CAPABILITIES=search.global.query \
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

echo "E2E tests passed successfully!"

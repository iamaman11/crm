#!/bin/bash
set -euo pipefail

# Ephemeral unique DB name
DB_NAME="crm_e2e_$(date +%s)_$RANDOM"

# Configuration fallback
E2E_DB_HOST="${E2E_DB_HOST:-127.0.0.1}"
E2E_DB_PORT="${E2E_DB_PORT:-5432}"
E2E_DB_USER="${E2E_DB_USER:-postgres}"
E2E_DB_PASSWORD="${E2E_DB_PASSWORD:-postgres}"

echo "Using unique ephemeral E2E database: ${DB_NAME}"

# Ensure cleanup of our unique database on exit
cleanup() {
  echo "Cleaning up background services..."
  jobs -p | xargs -r kill 2>/dev/null || true
  sleep 1
  echo "Dropping ephemeral database ${DB_NAME}..."
  PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d postgres -c "DROP DATABASE IF EXISTS ${DB_NAME};" || true
}
trap cleanup EXIT

echo "Starting local integration E2E prep..."

# 1. Create unique database
PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d postgres -c "CREATE DATABASE ${DB_NAME};"

# 2. Apply migrations and seeds to the new database
echo "Applying migrations and seeds..."
for f in database/migrations/*up.sql; do
  PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d "${DB_NAME}" -f "$f"
done
PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d "${DB_NAME}" -f database/tests/0001_platform_foundation.sql
PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d "${DB_NAME}" -f database/tests/0003_sales_activities_adapters.sql
PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d "${DB_NAME}" -f database/tests/0004_search_runtime_role_grants.sql

# 3. Create the test role only if it does not exist, without changing any global password settings
PGPASSWORD="${E2E_DB_PASSWORD}" psql -h "${E2E_DB_HOST}" -p "${E2E_DB_PORT}" -U "${E2E_DB_USER}" -d "${DB_NAME}" -c "
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'crm_app_test') THEN
    CREATE ROLE crm_app_test WITH LOGIN PASSWORD 'crm_app_test';
  END IF;
END
\$\$;
"

# 4. Compile backend
cargo build -p crm-api

# 5. Seed database by running seed_e2e_fixture once against our unique database
echo "Seeding unique database via seed_e2e_fixture..."
DATABASE_URL="postgres://crm_app_test:crm_app_test@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
ADMIN_DATABASE_URL="postgres://${E2E_DB_USER}:${E2E_DB_PASSWORD}@${E2E_DB_HOST}:${E2E_DB_PORT}/${DB_NAME}" \
cargo test -p crm-api --test seed_e2e_fixture

# 6. Start crm-api in background pointing to our unique database
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

# Wait for backend to be ready
until curl -s http://127.0.0.1:8080/readyz > /dev/null; do
  echo "Waiting for crm-api to be ready..."
  sleep 0.5
done
echo "crm-api is ready!"

# 7. Start Vite dev server in background
echo "Starting Vite dev server..."
VITE_CRM_GRPC_WEB_TARGET=http://127.0.0.1:9090 \
VITE_CRM_DEV_BEARER_TOKEN=phase6l-process-bearer-token-0123456789abcdef0123456789abcdef \
VITE_CRM_DEV_TENANT_ID=tenant-a \
VITE_CRM_DEV_CAPABILITIES=search.global.query \
pnpm --filter @ultimate-crm/web dev &

# Wait for Vite dev server to be ready
until curl -s http://127.0.0.1:5173 > /dev/null; do
  echo "Waiting for Vite to start..."
  sleep 0.5
done
echo "Vite dev server is ready!"

# 8. Run Playwright E2E tests
echo "Running Playwright E2E tests..."
pnpm exec playwright install chromium
pnpm --filter @ultimate-crm/web exec playwright test --config=playwright.config.ts

echo "E2E tests passed successfully!"

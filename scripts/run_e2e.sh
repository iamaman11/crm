#!/bin/bash
set -euo pipefail

# Ensure cleanup on exit
cleanup() {
  echo "Cleaning up background services..."
  jobs -p | xargs -r kill 2>/dev/null || true
}
trap cleanup EXIT

echo "Starting local integration E2E prep..."

# 1. Reset database to a clean state
echo "Resetting database crm_test..."
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d postgres -c "DROP DATABASE IF EXISTS crm_test;"
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d postgres -c "CREATE DATABASE crm_test;"

echo "Applying migrations and seeds..."
for f in database/migrations/*up.sql; do
  PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d crm_test -f "$f"
done
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d crm_test -f database/tests/0001_platform_foundation.sql
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d crm_test -f database/tests/0003_sales_activities_adapters.sql
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d crm_test -f database/tests/0004_search_runtime_role_grants.sql
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d crm_test -c "ALTER ROLE crm_app_test LOGIN PASSWORD 'crm_app_test'" || true
PGPASSWORD=postgres psql -h 127.0.0.1 -U postgres -d crm_test -c "ALTER ROLE postgres PASSWORD 'postgres'" || true

# 2. Compile backend
cargo build -p crm-api

# 3. Seed database by running process_e2e once
DATABASE_URL=postgres://crm_app_test:crm_app_test@127.0.0.1:5432/crm_test \
ADMIN_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/crm_test \
cargo test -p crm-api --test process_e2e

# 4. Start crm-api in background
echo "Starting crm-api service..."
CRM_DATABASE_URL=postgres://crm_app_test:crm_app_test@127.0.0.1:5432/crm_test \
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

# 5. Start Vite dev server in background
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

# 6. Run Playwright E2E tests
echo "Running Playwright E2E tests..."
pnpm exec playwright install chromium
pnpm --filter @ultimate-crm/web exec playwright test --config=playwright.config.ts

echo "E2E tests passed successfully!"

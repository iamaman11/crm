#!/usr/bin/env bash
set -euo pipefail

: "${PGHOST:?PGHOST is required}"
: "${PGPORT:?PGPORT is required}"
: "${PGDATABASE:?PGDATABASE is required}"
: "${PGUSER:?PGUSER is required}"
: "${PGPASSWORD:?PGPASSWORD is required}"
: "${PROCESS_SUITE:?PROCESS_SUITE is required}"
: "${ARTIFACT_DIR:?ARTIFACT_DIR is required}"

case "${PGDATABASE}" in
  crm_process_*_test) ;;
  *)
    echo "refusing to prepare non-isolated database: ${PGDATABASE}" >&2
    exit 2
    ;;
esac

mkdir -p "${ARTIFACT_DIR}"
log_path="${ARTIFACT_DIR}/database-setup.log"

{
  echo "suite=${PROCESS_SUITE}"
  echo "database=${PGDATABASE}"
  echo "host=${PGHOST}"
  echo "port=${PGPORT}"

  actual_database="$(psql --tuples-only --no-align --set ON_ERROR_STOP=1 --command 'SELECT current_database()')"
  if [[ "${actual_database}" != "${PGDATABASE}" ]]; then
    echo "connected database ${actual_database} does not match isolated database ${PGDATABASE}" >&2
    exit 3
  fi

  psql --set ON_ERROR_STOP=1 --command "DROP SCHEMA IF EXISTS crm CASCADE"

  migration_count=0
  while IFS= read -r migration; do
    psql --set ON_ERROR_STOP=1 --file "${migration}"
    migration_count=$((migration_count + 1))
  done < <(find database/migrations -maxdepth 1 -type f -name '*.up.sql' | sort)

  psql --set ON_ERROR_STOP=1 --file database/tests/0001_platform_foundation.sql
  psql --set ON_ERROR_STOP=1 --file database/tests/0003_sales_activities_adapters.sql
  psql --set ON_ERROR_STOP=1 --file database/tests/0004_search_runtime_role_grants.sql
  psql --set ON_ERROR_STOP=1 --command "ALTER ROLE crm_app_test LOGIN PASSWORD 'crm_app_test'"

  echo "migration_count=${migration_count}"
  psql --tuples-only --no-align --set ON_ERROR_STOP=1 --command \
    "SELECT current_database() || ':' || count(*) FROM information_schema.tables WHERE table_schema = 'crm'"
} 2>&1 | tee "${log_path}"

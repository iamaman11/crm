#!/usr/bin/env bash
set -euo pipefail

apply_migrations() {
  while IFS= read -r migration; do
    psql --set ON_ERROR_STOP=1 --file "${migration}"
  done < <(find database/migrations -maxdepth 1 -type f -name '*.up.sql' | sort)
}

rollback_migrations() {
  while IFS= read -r migration; do
    psql --set ON_ERROR_STOP=1 --file "${migration}"
  done < <(find database/migrations -maxdepth 1 -type f -name '*.down.sql' | sort -r)
}

verify_hardening() {
  psql --set ON_ERROR_STOP=1 --file database/tests/0001_platform_foundation.sql
  psql --set ON_ERROR_STOP=1 --file database/tests/0018_customer_enrichment_force_rls.sql
}

psql --set ON_ERROR_STOP=1 --command "DROP SCHEMA IF EXISTS crm CASCADE"
apply_migrations
verify_hardening

rollback_migrations
schema_count="$(
  psql --tuples-only --no-align --command \
    "SELECT count(*) FROM pg_namespace WHERE nspname = 'crm'"
)"
if [[ "${schema_count}" != "0" ]]; then
  echo "customer enrichment rollback left the crm schema present" >&2
  exit 1
fi

apply_migrations
verify_hardening

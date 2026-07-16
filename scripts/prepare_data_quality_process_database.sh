#!/usr/bin/env bash
set -euo pipefail

psql --set ON_ERROR_STOP=1 --command "DROP SCHEMA IF EXISTS crm CASCADE"
while IFS= read -r migration; do
  psql --set ON_ERROR_STOP=1 --file "${migration}"
done < <(find database/migrations -maxdepth 1 -type f -name '*.up.sql' | sort)

psql --set ON_ERROR_STOP=1 --file database/tests/0001_platform_foundation.sql
psql --set ON_ERROR_STOP=1 --file database/tests/0003_sales_activities_adapters.sql
psql --set ON_ERROR_STOP=1 --file database/tests/0004_search_runtime_role_grants.sql
psql --set ON_ERROR_STOP=1 --file database/tests/0014_data_quality_adapter.sql
psql --set ON_ERROR_STOP=1 --file database/tests/0015_data_quality_completeness_profile_adapter.sql
psql --set ON_ERROR_STOP=1 --command "ALTER ROLE crm_app_test LOGIN PASSWORD 'crm_app_test'"

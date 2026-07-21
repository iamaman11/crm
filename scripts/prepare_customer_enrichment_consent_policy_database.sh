#!/usr/bin/env bash
set -euo pipefail

bash scripts/prepare_customer_enrichment_worker_process_database.sh
psql --set ON_ERROR_STOP=1 --file database/tests/0019_customer_enrichment_consent_policy.sql
psql --set ON_ERROR_STOP=1 --file database/tests/0020_customer_enrichment_crm_api_process.sql

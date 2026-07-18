#!/usr/bin/env bash
set -euo pipefail
bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-customer-enrichment-review-composition --features postgres-integration --test postgres_review_process -- --nocapture

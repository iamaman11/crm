#!/usr/bin/env bash
set -euo pipefail

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-customer-enrichment-review-composition --features postgres-integration --test postgres_review_process -- --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-customer-enrichment-application-composition --features postgres-integration --test postgres_application_process -- --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-customer-enrichment-application-composition --features postgres-integration --test postgres_application_orchestration_process -- --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-customer-enrichment-application-composition --features postgres-integration --test postgres_application_worker_process process::accepted_review_worker_checkpoints_and_applies_exactly_once -- --exact --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-application-runtime --test postgres_customer_enrichment_application_worker -- --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-application-runtime --test postgres_customer_enrichment_provider_worker process::provider_worker_requires_dispatch_and_response_grants_and_recovers -- --exact --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-application-runtime --test postgres_customer_enrichment_suggestion_get -- --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-application-runtime --test postgres_customer_enrichment_suggestion_reject -- --nocapture

bash scripts/prepare_customer_enrichment_worker_process_database.sh
cargo test -p crm-application-runtime --test postgres_customer_enrichment_suggestion_accept -- --nocapture

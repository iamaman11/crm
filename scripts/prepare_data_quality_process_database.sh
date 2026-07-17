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
psql --set ON_ERROR_STOP=1 --command "
  INSERT INTO crm.module_versions (
    module_id, version, canonicalization_profile, manifest_sha256,
    normalized_manifest_json, published_at, publisher_id
  ) VALUES (
    'crm.parties', '0.3.0', 'crm.cjson/v1', decode(repeat('c3', 32), 'hex'),
    '{}'::jsonb, clock_timestamp(), 'platform'
  ) ON CONFLICT (module_id, version) DO NOTHING;

  INSERT INTO crm.capability_registry (
    capability_id, capability_version, owner_module_id, owner_module_version,
    service_name, method_name, input_descriptor_hash, output_descriptor_hash,
    risk_level, idempotency_required, audit_required, approval_required,
    ai_callable, marketplace_callable, bulk_allowed, export_allowed,
    data_classes_touched
  ) VALUES (
    'parties.party.create', '1.0.0', 'crm.parties', '0.3.0',
    'crm.parties.v1.PartiesService', 'CreateParty',
    decode(repeat('c4', 32), 'hex'), decode(repeat('c5', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['personal']::text[]
  ) ON CONFLICT (capability_id, capability_version) DO NOTHING;
"
psql --set ON_ERROR_STOP=1 --command "ALTER ROLE crm_app_test LOGIN PASSWORD 'crm_app_test'"

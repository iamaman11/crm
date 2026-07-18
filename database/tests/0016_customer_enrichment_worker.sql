\set ON_ERROR_STOP on

INSERT INTO crm.module_versions (
  module_id,
  version,
  canonicalization_profile,
  manifest_sha256,
  normalized_manifest_json,
  published_at,
  publisher_id
)
VALUES (
  'crm.customer-enrichment',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('e1', 32), 'hex'),
  '{"test_fixture":"customer_enrichment_worker"}'::jsonb,
  clock_timestamp(),
  'platform'
)
ON CONFLICT (module_id, version) DO NOTHING;

INSERT INTO crm.capability_registry (
  capability_id,
  capability_version,
  owner_module_id,
  owner_module_version,
  service_name,
  method_name,
  input_descriptor_hash,
  output_descriptor_hash,
  risk_level,
  idempotency_required,
  audit_required,
  approval_required,
  ai_callable,
  marketplace_callable,
  bulk_allowed,
  export_allowed,
  required_permissions,
  data_classes_touched
)
VALUES
  (
    'customer_enrichment.request.seed',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.test.WorkerFixtureService',
    'SeedRequest',
    decode(repeat('e2', 32), 'hex'),
    decode(repeat('e3', 32), 'hex'),
    'medium',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['customer_enrichment.request.seed']::text[],
    ARRAY['personal']::text[]
  ),
  (
    'customer_enrichment.request.dispatch',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentWorkerService',
    'DispatchEnrichmentRequest',
    decode(repeat('e4', 32), 'hex'),
    decode(repeat('e5', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['customer_enrichment.request.dispatch']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.response.record',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentWorkerService',
    'RecordProviderResponse',
    decode(repeat('e6', 32), 'hex'),
    decode(repeat('e7', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['customer_enrichment.response.record']::text[],
    ARRAY['personal', 'confidential']::text[]
  )
ON CONFLICT (capability_id, capability_version) DO NOTHING;

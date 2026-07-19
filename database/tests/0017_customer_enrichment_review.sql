\set ON_ERROR_STOP on

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
    'customer_enrichment.review.seed',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.test.ReviewFixtureService',
    'SeedSuggestion',
    decode(repeat('61', 32), 'hex'),
    decode(repeat('64', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.review.seed']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.suggestion.accept',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService',
    'AcceptSuggestion',
    decode(repeat('62', 32), 'hex'),
    decode(repeat('63', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.suggestion.accept']::text[],
    ARRAY['personal', 'confidential']::text[]
  )
ON CONFLICT (capability_id, capability_version) DO NOTHING;

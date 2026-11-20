BEGIN;

-- The real crm-api process persists audit evidence against the authoritative
-- capability registry. Publish the exact successful mutation coordinates used
-- by the Customer Enrichment transport-surface process acceptance instead of
-- weakening the audit foreign key or using a test-only persistence bypass.
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
    'customer_enrichment.provider_profile.publish',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentDefinitionService',
    'PublishProviderProfileVersion',
    decode(repeat('81', 32), 'hex'),
    decode(repeat('82', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.provider_profile.publish']::text[],
    ARRAY['confidential']::text[]
  ),
  (
    'customer_enrichment.mapping.publish',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentDefinitionService',
    'PublishMappingVersion',
    decode(repeat('83', 32), 'hex'),
    decode(repeat('84', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.mapping.publish']::text[],
    ARRAY['confidential']::text[]
  ),
  (
    'customer_enrichment.request.create',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentRequestService',
    'CreateEnrichmentRequest',
    decode(repeat('85', 32), 'hex'),
    decode(repeat('86', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.request.create']::text[],
    ARRAY['personal', 'confidential']::text[]
  )
ON CONFLICT (capability_id, capability_version) DO NOTHING;

DO $$
DECLARE
  published_count integer;
BEGIN
  SELECT count(*)
  INTO published_count
  FROM crm.capability_registry
  WHERE capability_version = '1.0.0'
    AND capability_id IN (
      'customer_enrichment.provider_profile.publish',
      'customer_enrichment.mapping.publish',
      'customer_enrichment.request.create'
    );

  IF published_count <> 3 THEN
    RAISE EXCEPTION
      'expected three Customer Enrichment crm-api process capabilities, found %',
      published_count;
  END IF;
END
$$;

COMMIT;

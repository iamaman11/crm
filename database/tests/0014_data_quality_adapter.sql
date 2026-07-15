-- Production-process acceptance fixture for the Phase 8A.9 Data Quality owner slice.
-- The in-process runtime catalog remains authoritative for request contract validation.
-- These rows satisfy durable module/capability registry foreign keys and audit lineage
-- for the public immutable Party rule-set publication exercised by the real crm-api process.

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
  'crm.data-quality',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('d9', 32), 'hex'),
  '{}'::jsonb,
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
  data_classes_touched
)
VALUES (
  'data_quality.party.rule_set.publish',
  '1.0.0',
  'crm.data-quality',
  '0.1.0',
  'crm.data_quality.v1.DataQualityService',
  'PublishPartyRuleSetVersion',
  decode(repeat('da', 32), 'hex'),
  decode(repeat('db', 32), 'hex'),
  'medium',
  true,
  true,
  false,
  false,
  false,
  false,
  false,
  ARRAY['confidential']::text[]
)
ON CONFLICT (capability_id, capability_version) DO NOTHING;

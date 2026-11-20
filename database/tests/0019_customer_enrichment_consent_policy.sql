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
  'crm.consents',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('74', 32), 'hex'),
  '{"test_fixture":"customer_enrichment_consent_policy"}'::jsonb,
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
VALUES (
  'consents.authorization.withdraw',
  '1.0.0',
  'crm.consents',
  '0.1.0',
  'crm.consents.v1.ConsentAuthorizationService',
  'WithdrawConsentAuthorization',
  decode(repeat('75', 32), 'hex'),
  decode(repeat('76', 32), 'hex'),
  'high',
  true,
  true,
  false,
  false,
  false,
  false,
  false,
  ARRAY['consents.authorization.withdraw']::text[],
  ARRAY['personal']::text[]
)
ON CONFLICT (capability_id, capability_version) DO NOTHING;

SELECT 'Customer Enrichment Consent policy fixture PASS' AS result;

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
  'crm.customer-privacy',
  '0.2.0',
  'crm.cjson/v1',
  decode(repeat('68', 32), 'hex'),
  '{"module_id":"crm.customer-privacy","version":"0.2.0"}'::jsonb,
  clock_timestamp(),
  'customer-platform'
)
ON CONFLICT (module_id, version) DO UPDATE
SET canonicalization_profile = EXCLUDED.canonicalization_profile,
    manifest_sha256 = EXCLUDED.manifest_sha256,
    normalized_manifest_json = EXCLUDED.normalized_manifest_json,
    publisher_id = EXCLUDED.publisher_id;

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
  export_allowed
)
VALUES (
  'customer_privacy.case.create',
  '1.0.0',
  'crm.customer-privacy',
  '0.2.0',
  'crm.customer_privacy.v1.CustomerPrivacyCaseService',
  'CreatePrivacyCase',
  decode(repeat('69', 32), 'hex'),
  decode(repeat('6a', 32), 'hex'),
  'high',
  true,
  true,
  false,
  false,
  false,
  false,
  false
)
ON CONFLICT (capability_id, capability_version) DO UPDATE
SET owner_module_id = EXCLUDED.owner_module_id,
    owner_module_version = EXCLUDED.owner_module_version,
    service_name = EXCLUDED.service_name,
    method_name = EXCLUDED.method_name,
    input_descriptor_hash = EXCLUDED.input_descriptor_hash,
    output_descriptor_hash = EXCLUDED.output_descriptor_hash,
    risk_level = EXCLUDED.risk_level,
    idempotency_required = EXCLUDED.idempotency_required,
    audit_required = EXCLUDED.audit_required,
    approval_required = EXCLUDED.approval_required,
    ai_callable = EXCLUDED.ai_callable,
    marketplace_callable = EXCLUDED.marketplace_callable,
    bulk_allowed = EXCLUDED.bulk_allowed,
    export_allowed = EXCLUDED.export_allowed;

SELECT 'Customer Privacy persistence fixture PASS' AS result;

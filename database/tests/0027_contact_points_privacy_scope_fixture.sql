\set ON_ERROR_STOP on

-- Minimal durable registry fixture required only to create, verify and update
-- authoritative Contact Point records for the non-runtime owner privacy-scope
-- PostgreSQL acceptance suite. Party creation remains supplied by fixture 0024.
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
  'crm.contact-points',
  '0.3.0',
  'crm.cjson/v1',
  decode(repeat('94', 32), 'hex'),
  '{"module_id":"crm.contact-points","version":"0.3.0"}'::jsonb,
  clock_timestamp(),
  'customer-platform'
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
VALUES
  (
    'contact-points.contact-point.create',
    '1.0.0',
    'crm.contact-points',
    '0.3.0',
    'crm.contact_points.v1.ContactPointService',
    'CreateContactPoint',
    decode(repeat('95', 32), 'hex'),
    decode(repeat('96', 32), 'hex'),
    'medium',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['personal']::text[]
  ),
  (
    'contact-points.contact-point.verify',
    '1.0.0',
    'crm.contact-points',
    '0.3.0',
    'crm.contact_points.v1.ContactPointService',
    'VerifyContactPoint',
    decode(repeat('97', 32), 'hex'),
    decode(repeat('98', 32), 'hex'),
    'medium',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['personal']::text[]
  ),
  (
    'contact-points.contact-point.update',
    '1.0.0',
    'crm.contact-points',
    '0.3.0',
    'crm.contact_points.v1.ContactPointService',
    'UpdateContactPoint',
    decode(repeat('99', 32), 'hex'),
    decode(repeat('9a', 32), 'hex'),
    'medium',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['personal']::text[]
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
    export_allowed = EXCLUDED.export_allowed,
    data_classes_touched = EXCLUDED.data_classes_touched;

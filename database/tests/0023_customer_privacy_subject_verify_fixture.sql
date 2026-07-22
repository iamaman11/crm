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
  'crm.parties',
  '0.2.0',
  'crm.cjson/v1',
  decode(repeat('73', 32), 'hex'),
  '{"module_id":"crm.parties","version":"0.2.0"}'::jsonb,
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
    'parties.party.create',
    '1.0.0',
    'crm.parties',
    '0.2.0',
    'crm.parties.v1.PartyService',
    'CreateParty',
    decode(repeat('74', 32), 'hex'),
    decode(repeat('75', 32), 'hex'),
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
    'customer_privacy.case.subject.verify',
    '1.0.0',
    'crm.customer-privacy',
    '0.2.0',
    'crm.customer_privacy.v1.CustomerPrivacyCaseService',
    'VerifyPrivacyCaseSubject',
    decode(repeat('76', 32), 'hex'),
    decode(repeat('77', 32), 'hex'),
    'critical',
    true,
    true,
    false,
    false,
    false,
    false,
    false,
    ARRAY['personal', 'confidential']::text[]
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

-- The isolated process proof deliberately converts shared-subject contention into a
-- bounded PostgreSQL lock-timeout error instead of allowing an unbounded test hang.
ALTER ROLE crm_app_test SET lock_timeout = '500ms';

SELECT 'Customer Privacy subject-verification fixture PASS' AS result;

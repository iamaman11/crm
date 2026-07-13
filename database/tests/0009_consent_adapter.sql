-- Production-adapter fixture for the Phase 8A.4 Consent and Communication Authorization slice.
-- Runtime contracts remain authoritative; these durable registry rows satisfy
-- module/capability foreign keys and audit lineage for real PostgreSQL process
-- acceptance. Publication is immutable and idempotent via DO NOTHING.

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
  decode(repeat('a4', 32), 'hex'),
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
VALUES
  (
    'consents.authorization.create',
    '1.0.0',
    'crm.consents',
    '0.1.0',
    'crm.consents.v1.ConsentAuthorizationService',
    'CreateConsentAuthorization',
    decode('c991f42f85f5f2c592fb723a128d1f176b4c91acc47b83bfafc5a3d35572bad0', 'hex'),
    decode('f2b161c122a93994c5253e47bd90aa264e6e03ff306665e8f3f8233b7b3ecfb5', 'hex'),
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
    'consents.authorization.withdraw',
    '1.0.0',
    'crm.consents',
    '0.1.0',
    'crm.consents.v1.ConsentAuthorizationService',
    'WithdrawConsentAuthorization',
    decode('81068ae3de4cf66acbc82bc675bbfc31f7dbe022279d0e7766ed8948e6c54c3c', 'hex'),
    decode('107a5360ecd7661f8345406278fe1bd7e95cfbef747d1e604e6ceca0369a09e5', 'hex'),
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
ON CONFLICT (capability_id, capability_version) DO NOTHING;

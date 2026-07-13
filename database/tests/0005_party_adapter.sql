-- Production-adapter fixture for the Phase 8A Party lifecycle slice.
-- The runtime contract catalog remains authoritative for request validation;
-- these rows satisfy the durable module/capability registry foreign-key and
-- audit lineage required by transactional PostgreSQL evidence.
-- The fixture is intentionally idempotent because both CI setup and focused
-- acceptance tests may apply it before exercising the same production path.
-- Focused acceptance reuses bootstrap tenant/actor registry identities so the
-- test proves Party behavior rather than bypassing authoritative identity FKs.
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
  '0.3.0',
  'crm.cjson/v1',
  decode(repeat('ae', 32), 'hex'),
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
    'parties.party.create',
    '1.0.0',
    'crm.parties',
    '0.3.0',
    'crm.parties.v1.PartyService',
    'CreateParty',
    decode('33c7898d550a0b6501844042fda297fc921f33c4a1a9f8342504a67c87e27ac0', 'hex'),
    decode('9dda6fb3cba295aa7eecd184fe2bb3dcebaf37a511248bda32cb883dce409699', 'hex'),
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
    'parties.party.update',
    '1.0.0',
    'crm.parties',
    '0.3.0',
    'crm.parties.v1.PartyService',
    'UpdateParty',
    decode('2a99e2e203ad1756c72d0f015bf1e370b9a06776a8563bd25e0ad5f3d993b16b', 'hex'),
    decode('9192dbb260a7b2a0c3a5ed16b029c65c6c9b8b2330bbfe2f2916bb63f0b012a8', 'hex'),
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
ON CONFLICT (capability_id, capability_version)
DO UPDATE SET
  owner_module_id = EXCLUDED.owner_module_id,
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

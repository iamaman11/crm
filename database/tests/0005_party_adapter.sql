-- Production-adapter fixture for the Phase 8A.2 Party vertical slice.
-- The runtime contract catalog remains authoritative for request validation;
-- these rows satisfy the durable module/capability registry foreign-key and
-- audit lineage required by transactional PostgreSQL evidence.
-- The fixture is intentionally idempotent because both CI setup and focused
-- acceptance tests may apply it before exercising the same production path.
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
VALUES (
  'parties.party.create',
  '1.0.0',
  'crm.parties',
  '0.2.0',
  'crm.parties.v1.PartyService',
  'CreateParty',
  decode('2b46dca49090a9ef3ff4426aaedd46fffff6c3d120a5174b084a6a686bec3c2a', 'hex'),
  decode('a750a7dba57ad912ef9cd1cc7f1039acc0e6c8ffe68707403655c24aebc85911', 'hex'),
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
SET
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

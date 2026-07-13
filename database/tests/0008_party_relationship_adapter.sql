-- Production-adapter fixture for the Phase 8A.3c Party Relationship lifecycle slice.
-- Runtime contracts remain authoritative; these durable registry rows satisfy
-- module/capability foreign keys and audit lineage for real PostgreSQL process
-- acceptance. Publication is immutable and therefore idempotent via DO NOTHING.
-- The Party Relationship process acceptance also exercises tenant-B Parties
-- through the same service actor used by crm-api without weakening isolation.
BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'party-relationship-process-actor-bootstrap-request';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'party-relationship-process-actor-bootstrap';

INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES (
  'tenant-b',
  'actor-a',
  'service',
  'active',
  'Party Relationship acceptance cross-tenant actor',
  'party-relationship-process-actor-bootstrap'
)
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;

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
  'crm.party-relationships',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('c3', 32), 'hex'),
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
    'party-relationships.party-relationship.create',
    '1.0.0',
    'crm.party-relationships',
    '0.1.0',
    'crm.party_relationships.v1.PartyRelationshipService',
    'CreatePartyRelationship',
    decode('c2556490b4e91d80a034976beea58f9915451041a513a279fb500e31533e0fbf', 'hex'),
    decode('8f81c303c664fd0373cbe123b61cf265e1112dbfd514150e93020a24ce1fa1c2', 'hex'),
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
    'party-relationships.party-relationship.update',
    '1.0.0',
    'crm.party-relationships',
    '0.1.0',
    'crm.party_relationships.v1.PartyRelationshipService',
    'UpdatePartyRelationship',
    decode('7bdc666f4ec9ae80478a6dbb8cda404db93882fc0cf4ff0c547bf7008c9dac4c', 'hex'),
    decode('218616f027a168f994207cfdde48f77f39672bcfdfafd64a6a6454efd55aa774', 'hex'),
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

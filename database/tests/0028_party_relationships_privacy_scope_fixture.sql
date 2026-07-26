\set ON_ERROR_STOP on

-- Minimal durable registry fixture required only to create and update
-- authoritative Party Relationship records for the non-runtime owner privacy-scope
-- PostgreSQL acceptance suite. Party creation remains supplied by fixture 0024.
INSERT INTO crm.module_versions (
  module_id, version, canonicalization_profile, manifest_sha256,
  normalized_manifest_json, published_at, publisher_id
)
VALUES (
  'crm.party-relationships', '0.2.0', 'crm.cjson/v1',
  decode(repeat('a1', 32), 'hex'),
  '{"module_id":"crm.party-relationships","version":"0.2.0"}'::jsonb,
  clock_timestamp(), 'customer-platform'
)
ON CONFLICT (module_id, version) DO NOTHING;

INSERT INTO crm.capability_registry (
  capability_id, capability_version, owner_module_id, owner_module_version,
  service_name, method_name, input_descriptor_hash, output_descriptor_hash,
  risk_level, idempotency_required, audit_required, approval_required,
  ai_callable, marketplace_callable, bulk_allowed, export_allowed,
  data_classes_touched
)
VALUES
  (
    'party-relationships.party-relationship.create', '1.0.0',
    'crm.party-relationships', '0.2.0',
    'crm.party_relationships.v1.PartyRelationshipService',
    'CreatePartyRelationship', decode(repeat('a2', 32), 'hex'),
    decode(repeat('a3', 32), 'hex'), 'medium', true, true, false,
    false, false, false, false, ARRAY['personal']::text[]
  ),
  (
    'party-relationships.party-relationship.update', '1.0.0',
    'crm.party-relationships', '0.2.0',
    'crm.party_relationships.v1.PartyRelationshipService',
    'UpdatePartyRelationship', decode(repeat('a4', 32), 'hex'),
    decode(repeat('a5', 32), 'hex'), 'medium', true, true, false,
    false, false, false, false, ARRAY['personal']::text[]
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

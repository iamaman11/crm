-- Production-adapter fixture for the Phase 8A.3b Contact Point lifecycle slice.
-- Runtime contracts remain authoritative; these durable registry rows satisfy
-- module/capability foreign keys and audit lineage for real PostgreSQL process
-- acceptance. Publication is immutable and therefore idempotent via DO NOTHING.
-- The Contact Point process acceptance also exercises a tenant-B Party through
-- the same service actor used by crm-api, without weakening tenant isolation.
BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'contact-point-process-actor-bootstrap-request';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'contact-point-process-actor-bootstrap';

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
  'Contact Point acceptance cross-tenant actor',
  'contact-point-process-actor-bootstrap'
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
  'crm.contact-points',
  '0.2.0',
  'crm.cjson/v1',
  decode(repeat('b7', 32), 'hex'),
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
    'contact-points.contact-point.create',
    '1.0.0',
    'crm.contact-points',
    '0.2.0',
    'crm.contact_points.v1.ContactPointService',
    'CreateContactPoint',
    decode('563d811f66b9519045b23b67ec3e4d9fa6c656c6f4b0f2f1a34960079c05c5ec', 'hex'),
    decode('ce4ebd53aa0b2878476205f7a182d5e315fbbea92ec2d60b259be2eba0e7d6ab', 'hex'),
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
    '0.2.0',
    'crm.contact_points.v1.ContactPointService',
    'UpdateContactPoint',
    decode('2a0bbf95985a8f84e7ad5ed422568e354f3f9c3205c9540733704f30cbd51f47', 'hex'),
    decode('f4e64c264e38187f0a2bf5d4d77c49f4dcd726ce7500a59724827e5e29ba6922', 'hex'),
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
    '0.2.0',
    'crm.contact_points.v1.ContactPointService',
    'VerifyContactPoint',
    decode('625d928550ece703da5d677ae4a8a952e523b3f964d1b037dbdb00d3025733b4', 'hex'),
    decode('4d18a0dae432b91f50330b74a5ba6362b69d30953c7441de2e5001403d1f8961', 'hex'),
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

-- Production-adapter fixture for the Phase 8A.6 Identity Resolution candidate and reversible merge slice.
-- Runtime contracts remain authoritative; these durable registry rows satisfy
-- module/capability foreign keys and audit lineage for real PostgreSQL process
-- acceptance. Publication is immutable and idempotent via DO NOTHING.

-- Cross-tenant process acceptance uses the same authenticated service actor in both tenants.
BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'identity-resolution-process-actor-bootstrap-request';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'identity-resolution-process-actor-bootstrap';

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
  'Identity Resolution acceptance cross-tenant actor',
  'identity-resolution-process-actor-bootstrap'
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
  'crm.identity-resolution',
  '0.2.0',
  'crm.cjson/v1',
  decode(repeat('a5', 32), 'hex'),
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
    'identity_resolution.candidate.register',
    '1.0.0',
    'crm.identity-resolution',
    '0.2.0',
    'crm.identity_resolution.v1.IdentityResolutionService',
    'RegisterDuplicateCandidate',
    decode('65e25433e1c43334f5ba6ab5f112e25e307cdcf38eb45c813aa60076cf9d6a28', 'hex'),
    decode('b9fa086eb4577391d31e52855c8ec152f9addbe73bcb24c9fe772ec14f642fea', 'hex'),
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
    'identity_resolution.candidate.evidence.refresh',
    '1.0.0',
    'crm.identity-resolution',
    '0.2.0',
    'crm.identity_resolution.v1.IdentityResolutionService',
    'RefreshDuplicateCandidateEvidence',
    decode('d7b82b5a0c1f8be504a00484a287af1e730603a0911a31ccde79549fd026b36e', 'hex'),
    decode('b495c4cabc21e2204d41f6ade51f794b4caabd1bdbe9cda4217105c126a535e4', 'hex'),
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
    'identity_resolution.candidate.dismiss',
    '1.0.0',
    'crm.identity-resolution',
    '0.2.0',
    'crm.identity_resolution.v1.IdentityResolutionService',
    'DismissDuplicateCandidate',
    decode('753ba7974a28ed9df76d8ca1fb39390a0445cea77ea012ad38f48d202f47cd51', 'hex'),
    decode('d5ea468e3d4d274e7e30a282979404b84f1a619c589257896f1fa34266fbb6a0', 'hex'),
    'high',
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
    'identity_resolution.candidate.confirm_duplicate',
    '1.0.0',
    'crm.identity-resolution',
    '0.2.0',
    'crm.identity_resolution.v1.IdentityResolutionService',
    'ConfirmDuplicateCandidate',
    decode('ee026ecfef300b48c2edf7ae47c74b2f2c3511c36cb9d2cda128efe52485d3c1', 'hex'),
    decode('4b45930d1924ef5c0e9d61793db989dde35398d1f67e02737a1992707af24d63', 'hex'),
    'high',
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
    'identity_resolution.merge.execute',
    '1.0.0',
    'crm.identity-resolution',
    '0.2.0',
    'crm.identity_resolution.v1.IdentityResolutionService',
    'MergeParty',
    decode('8789c039f580c2e211ab64aeb45c5861390a99c49f31b9b5d9c08c5f07ad4a9e', 'hex'),
    decode('54019fe1aa4c98300eca2d46355ec557c64c0b5eed0694a62c6d6d42817e033f', 'hex'),
    'high',
    true,
    true,
    true,
    false,
    false,
    false,
    false,
    ARRAY['personal']::text[]
  ),
  (
    'identity_resolution.merge.unmerge',
    '1.0.0',
    'crm.identity-resolution',
    '0.2.0',
    'crm.identity_resolution.v1.IdentityResolutionService',
    'UnmergeParty',
    decode('2b14276a28de06bea0da6550984777626b32daba434d4442bb076ee853d8e74b', 'hex'),
    decode('4f835ec6a6fa65cf14e311a872d8cec43be9e5963af0d46ae84798498de989fb', 'hex'),
    'high',
    true,
    true,
    true,
    false,
    false,
    false,
    false,
    ARRAY['personal']::text[]
  )
ON CONFLICT (capability_id, capability_version) DO NOTHING;

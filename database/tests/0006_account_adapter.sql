-- Production-adapter fixture for the Phase 8A.3a Account lifecycle slice.
-- Runtime contracts remain authoritative; these durable registry rows satisfy
-- module/capability foreign keys and audit lineage for real PostgreSQL process
-- acceptance. Publication is immutable and therefore idempotent via DO NOTHING.
-- The Account process acceptance also exercises a tenant-B Party through the
-- same service actor used by crm-api. Provision that actor through the same
-- transaction-local write-context guard required by normal platform writes,
-- without weakening the global platform fixture's tenant-isolation proof.
BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'account-process-actor-bootstrap-request';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'account-process-actor-bootstrap';

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
  'Account acceptance cross-tenant actor',
  'account-process-actor-bootstrap'
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
  'crm.customer-accounts',
  '0.2.0',
  'crm.cjson/v1',
  decode(repeat('af', 32), 'hex'),
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
    'accounts.account.create',
    '1.0.0',
    'crm.customer-accounts',
    '0.2.0',
    'crm.accounts.v1.AccountService',
    'CreateAccount',
    decode('d63af8e0e9e02abb4e24b874674e3b1212fd060d952efe561f450f7dceb3ef18', 'hex'),
    decode('e9d597c66a06795e7cec24e6a3dd5245465c937ed22b266664980c98e468e1e7', 'hex'),
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
    'accounts.account.update',
    '1.0.0',
    'crm.customer-accounts',
    '0.2.0',
    'crm.accounts.v1.AccountService',
    'UpdateAccount',
    decode('8b12d9c75abcc079c284df65d67bd061bcd21e77699ef9df5a32e66f583594d4', 'hex'),
    decode('a6fbbce1c04edeb1070cf7ce338992c9625be0fcebc056ed952db7613422c3ae', 'hex'),
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

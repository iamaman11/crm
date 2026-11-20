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
  '0.3.0',
  'crm.cjson/v1',
  decode(repeat('73', 32), 'hex'),
  '{"module_id":"crm.parties","version":"0.3.0"}'::jsonb,
  clock_timestamp(),
  'customer-platform'
)
ON CONFLICT (module_id, version) DO NOTHING;

-- This shared submit/subject/query/cancel process database must register every
-- audited mutation coordinate exercised by the real crm-api process. The
-- runtime registry remains authoritative for execution; these rows satisfy the
-- durable audit foreign key without weakening or bypassing that constraint.
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
  ),
  (
    'customer_privacy.case.cancel',
    '1.0.0',
    'crm.customer-privacy',
    '0.2.0',
    'crm.customer_privacy.v1.CustomerPrivacyCaseService',
    'CancelPrivacyCase',
    decode(repeat('7b', 32), 'hex'),
    decode(repeat('7c', 32), 'hex'),
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

-- The process bearer grants both tenant-a and tenant-b to one authenticated actor.
-- Register that exact actor in tenant-b through complete governed evidence rather
-- than bypassing the tenant-bound actor/audit foreign key in the E2E itself.
SET ROLE crm_app_test;
BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-b';
SET LOCAL app.request_id = 'request-privacy-subject-actor-a-b';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-privacy-subject-actor-a-b';

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
  'Cross-tenant privacy subject process actor',
  'tx-privacy-subject-actor-a-b'
)
ON CONFLICT (tenant_id, actor_id) DO NOTHING;

INSERT INTO crm.idempotency_records (
  tenant_id,
  idempotency_scope,
  idempotency_key,
  request_hash,
  status,
  business_transaction_id,
  expires_at
)
VALUES (
  'tenant-b',
  'test.record.mutate@1.0.0',
  'privacy-subject-actor-a-b',
  decode(repeat('78', 32), 'hex'),
  'completed',
  'tx-privacy-subject-actor-a-b',
  clock_timestamp() + interval '1 day'
);

INSERT INTO crm.outbox_events (
  tenant_id,
  event_id,
  business_transaction_id,
  aggregate_type,
  aggregate_id,
  aggregate_version,
  event_sequence,
  event_type,
  deduplication_key,
  schema_id,
  schema_version,
  descriptor_hash,
  data_class,
  payload_encoding,
  maximum_payload_size,
  retention_policy_id,
  payload_bytes,
  occurred_at
)
VALUES (
  'tenant-b',
  'event-privacy-subject-actor-a-b',
  'tx-privacy-subject-actor-a-b',
  'crm.actor',
  'actor-a',
  1,
  1,
  'actor.created',
  'privacy-subject-actor-a-b',
  'crm.actor.created.v1',
  '1.0.0',
  decode(repeat('79', 32), 'hex'),
  'internal',
  'protobuf',
  16,
  'standard',
  decode('01', 'hex'),
  clock_timestamp()
);

INSERT INTO crm.audit_records (
  tenant_id,
  audit_sequence,
  audit_record_id,
  business_transaction_id,
  actor_id,
  capability_id,
  capability_version,
  canonicalization_profile,
  previous_hash,
  record_hash,
  canonical_envelope,
  occurred_at
)
SELECT
  'tenant-b',
  audit_sequence + 1,
  'audit-privacy-subject-actor-a-b',
  'tx-privacy-subject-actor-a-b',
  'actor-b',
  'test.record.mutate',
  '1.0.0',
  'crm.cjson/v1',
  record_hash,
  decode(repeat('7a', 32), 'hex'),
  convert_to('{"actor":"actor-a","tenant":"tenant-b"}', 'UTF8'),
  clock_timestamp()
FROM crm.audit_records
WHERE tenant_id = 'tenant-b'
ORDER BY audit_sequence DESC
LIMIT 1;

INSERT INTO crm.business_transactions (
  tenant_id,
  business_transaction_id,
  actor_id,
  request_id,
  capability_id,
  capability_version,
  expected_outbox_events,
  expected_audit_records,
  expected_idempotency_records
)
VALUES (
  'tenant-b',
  'tx-privacy-subject-actor-a-b',
  'actor-b',
  'request-privacy-subject-actor-a-b',
  'test.record.mutate',
  '1.0.0',
  1,
  1,
  1
);

SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
RESET ROLE;

-- Subject-lock contention is bounded by the shared fail-fast SQL primitive itself;
-- the process proof intentionally relies on no role-specific lock timeout.
SELECT 'Customer Privacy subject-verification fixture PASS' AS result;

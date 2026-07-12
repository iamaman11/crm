\set ON_ERROR_STOP on

INSERT INTO crm.tenants (tenant_id, status, data_region)
VALUES
  ('tenant-a', 'active', 'eu-central'),
  ('tenant-b', 'active', 'eu-central');

INSERT INTO crm.module_versions (
  module_id,
  version,
  canonicalization_profile,
  manifest_sha256,
  normalized_manifest_json,
  published_at,
  publisher_id
)
VALUES
  (
    'crm.test',
    '1.0.0',
    'crm.cjson/v1',
    decode(repeat('ab', 32), 'hex'),
    '{}'::jsonb,
    clock_timestamp(),
    'platform'
  ),
  (
    'crm.metadata',
    '1.0.0',
    'crm.cjson/v1',
    decode(repeat('cd', 32), 'hex'),
    '{"platform":"metadata"}'::jsonb,
    clock_timestamp(),
    'platform'
  );

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
  export_allowed
)
VALUES
  (
    'test.record.mutate',
    '1.0.0',
    'crm.test',
    '1.0.0',
    'crm.test.v1.TestService',
    'Mutate',
    decode(repeat('01', 32), 'hex'),
    decode(repeat('02', 32), 'hex'),
    'medium',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'metadata.bundle.publish',
    '1.0.0',
    'crm.metadata',
    '1.0.0',
    'crm.metadata.v1.MetadataCapabilityService',
    'PublishMetadataBundle',
    decode(repeat('03', 32), 'hex'),
    decode(repeat('04', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'metadata.revision.activate',
    '1.0.0',
    'crm.metadata',
    '1.0.0',
    'crm.metadata.v1.MetadataCapabilityService',
    'ActivateMetadataRevision',
    decode(repeat('05', 32), 'hex'),
    decode(repeat('06', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'metadata.revision.rollback',
    '1.0.0',
    'crm.metadata',
    '1.0.0',
    'crm.metadata.v1.MetadataCapabilityService',
    'RollbackMetadataRevision',
    decode(repeat('07', 32), 'hex'),
    decode(repeat('08', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  );

DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'crm_app_test') THEN
    CREATE ROLE crm_app_test NOLOGIN NOSUPERUSER NOBYPASSRLS;
  END IF;
END;
$$;

GRANT USAGE ON SCHEMA crm TO crm_app_test;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA crm TO crm_app_test;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA crm TO crm_app_test;
REVOKE INSERT, UPDATE, DELETE ON crm.audit_heads FROM crm_app_test;

SET ROLE crm_app_test;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-bootstrap-a';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-bootstrap-a';

INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES (
  'tenant-a',
  'actor-a',
  'service',
  'active',
  'Tenant A bootstrap actor',
  'tx-bootstrap-a'
);

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
  'tenant-a',
  'test.record.mutate@1.0.0',
  'bootstrap-a',
  decode(repeat('10', 32), 'hex'),
  'completed',
  'tx-bootstrap-a',
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
  'tenant-a',
  'event-bootstrap-a',
  'tx-bootstrap-a',
  'crm.actor',
  'actor-a',
  1,
  1,
  'actor.created',
  'bootstrap-a',
  'crm.actor.created.v1',
  '1.0.0',
  decode(repeat('20', 32), 'hex'),
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
VALUES (
  'tenant-a',
  1,
  'audit-bootstrap-a',
  'tx-bootstrap-a',
  'actor-a',
  'test.record.mutate',
  '1.0.0',
  'crm.cjson/v1',
  decode(repeat('00', 32), 'hex'),
  decode(repeat('11', 32), 'hex'),
  convert_to('{"audit":"bootstrap-a"}', 'UTF8'),
  clock_timestamp()
);

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
  'tenant-a',
  'tx-bootstrap-a',
  'actor-a',
  'request-bootstrap-a',
  'test.record.mutate',
  '1.0.0',
  1,
  1,
  1
);
COMMIT;

DO $$
DECLARE
  visible_count integer;
BEGIN
  SELECT count(*) INTO visible_count FROM crm.actors;
  IF visible_count <> 0 THEN
    RAISE EXCEPTION 'tenant rows were visible without transaction-local tenant context';
  END IF;
END;
$$;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-b';
SET LOCAL app.request_id = 'request-read-b';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-read-b';
DO $$
DECLARE
  visible_count integer;
BEGIN
  SELECT count(*) INTO visible_count FROM crm.actors WHERE actor_id = 'actor-a';
  IF visible_count <> 0 THEN
    RAISE EXCEPTION 'cross-tenant actor row leaked through RLS';
  END IF;
END;
$$;
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-mismatch';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-mismatch';
DO $$
DECLARE
  rejected boolean := false;
BEGIN
  BEGIN
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
      'cross-tenant-actor',
      'service',
      'active',
      'Must not be inserted',
      'tx-mismatch'
    );
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'cross-tenant write was not rejected';
  END IF;
END;
$$;
ROLLBACK;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-second-audit';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-second-audit';

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
  'tenant-a',
  'test.record.mutate@1.0.0',
  'second-audit',
  decode(repeat('30', 32), 'hex'),
  'completed',
  'tx-second-audit',
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
  'tenant-a',
  'event-second-audit',
  'tx-second-audit',
  'crm.actor',
  'actor-a',
  2,
  2,
  'actor.observed',
  'second-audit',
  'crm.actor.observed.v1',
  '1.0.0',
  decode(repeat('31', 32), 'hex'),
  'internal',
  'protobuf',
  16,
  'standard',
  decode('02', 'hex'),
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
VALUES (
  'tenant-a',
  2,
  'audit-second-a',
  'tx-second-audit',
  'actor-a',
  'test.record.mutate',
  '1.0.0',
  'crm.cjson/v1',
  decode(repeat('11', 32), 'hex'),
  decode(repeat('22', 32), 'hex'),
  convert_to('{"audit":"second-a"}', 'UTF8'),
  clock_timestamp()
);

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
  'tenant-a',
  'tx-second-audit',
  'actor-a',
  'request-second-audit',
  'test.record.mutate',
  '1.0.0',
  1,
  1,
  1
);
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-bad-chain';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-bad-chain';
DO $$
DECLARE
  rejected boolean := false;
BEGIN
  BEGIN
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
    VALUES (
      'tenant-a',
      3,
      'audit-invalid-chain',
      'tx-bad-chain',
      'actor-a',
      'test.record.mutate',
      '1.0.0',
      'crm.cjson/v1',
      decode(repeat('ff', 32), 'hex'),
      decode(repeat('33', 32), 'hex'),
      convert_to('{"audit":"invalid"}', 'UTF8'),
      clock_timestamp()
    );
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'audit chain discontinuity was not rejected';
  END IF;
END;
$$;
ROLLBACK;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-missing-evidence';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-missing-evidence';
DO $$
DECLARE
  rejected boolean := false;
BEGIN
  BEGIN
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
      'tenant-a',
      'tx-missing-evidence',
      'actor-a',
      'request-missing-evidence',
      'test.record.mutate',
      '1.0.0',
      1,
      1,
      1
    );
    SET CONSTRAINTS ALL IMMEDIATE;
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'business transaction without evidence was not rejected';
  END IF;
END;
$$;
ROLLBACK;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-size-check';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-size-check';
DO $$
DECLARE
  rejected boolean := false;
BEGIN
  BEGIN
    INSERT INTO crm.records (
      tenant_id,
      record_type,
      record_id,
      version,
      owner_module_id,
      schema_id,
      schema_version,
      descriptor_hash,
      data_class,
      payload_encoding,
      maximum_payload_size,
      retention_policy_id,
      payload_bytes,
      last_business_transaction_id
    )
    VALUES (
      'tenant-a',
      'test.record',
      'oversized',
      1,
      'crm.test',
      'test.record.v1',
      '1.0.0',
      decode(repeat('44', 32), 'hex'),
      'internal',
      'protobuf',
      0,
      'standard',
      decode('01', 'hex'),
      'tx-size-check'
    );
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'oversized typed payload was not rejected';
  END IF;
END;
$$;
ROLLBACK;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-immutable-audit';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-immutable-audit';
DO $$
DECLARE
  rejected boolean := false;
BEGIN
  BEGIN
    UPDATE crm.audit_records
       SET canonical_envelope = convert_to('{}', 'UTF8')
     WHERE tenant_id = 'tenant-a' AND audit_sequence = 1;
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'audit record update was not rejected';
  END IF;
END;
$$;
ROLLBACK;

RESET ROLE;

DO $$
DECLARE
  rejected boolean := false;
BEGIN
  BEGIN
    UPDATE crm.module_versions
       SET publisher_id = 'mutated'
     WHERE module_id = 'crm.test' AND version = '1.0.0';
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'published module version mutation was not rejected';
  END IF;
END;
$$;

DO $$
DECLARE
  expected_tables text[] := ARRAY[
    'tenants', 'actors', 'teams', 'team_memberships', 'business_transactions',
    'module_installations', 'tenant_capability_grants', 'metadata_packages',
    'object_definitions', 'field_definitions', 'records', 'relationships',
    'idempotency_records', 'outbox_events', 'outbox_delivery', 'audit_heads',
    'audit_records', 'workflow_definitions', 'workflow_runs', 'module_state'
  ];
  secured_count integer;
BEGIN
  SELECT count(*)
    INTO secured_count
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
   WHERE n.nspname = 'crm'
     AND c.relname = ANY(expected_tables)
     AND c.relrowsecurity
     AND c.relforcerowsecurity;

  IF secured_count <> cardinality(expected_tables) THEN
    RAISE EXCEPTION 'not all tenant tables have forced RLS: % of %', secured_count, cardinality(expected_tables);
  END IF;
END;
$$;

DO $$
DECLARE
  head_sequence bigint;
  head_hash bytea;
BEGIN
  SELECT next_sequence, last_hash
    INTO head_sequence, head_hash
    FROM crm.audit_heads
   WHERE tenant_id = 'tenant-a';

  IF head_sequence <> 3 OR head_hash <> decode(repeat('22', 32), 'hex') THEN
    RAISE EXCEPTION 'tenant audit head does not match the committed audit chain';
  END IF;
END;
$$;

SELECT 'PostgreSQL platform foundation PASS' AS result;

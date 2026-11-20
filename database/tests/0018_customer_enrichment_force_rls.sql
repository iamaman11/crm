\set ON_ERROR_STOP on

DO $$
DECLARE
  unsecured_tables text[];
  incomplete_policies text[];
  tenant_table_count integer;
  role_is_superuser boolean;
  role_bypasses_rls boolean;
BEGIN
  SELECT count(*)
    INTO tenant_table_count
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
   WHERE n.nspname = 'crm'
     AND c.relkind IN ('r', 'p')
     AND EXISTS (
       SELECT 1
         FROM pg_attribute a
        WHERE a.attrelid = c.oid
          AND a.attname = 'tenant_id'
          AND a.attnum > 0
          AND NOT a.attisdropped
     );

  IF tenant_table_count = 0 THEN
    RAISE EXCEPTION 'no tenant-bearing CRM tables were discovered';
  END IF;

  SELECT array_agg(
           format('%I(enable=%s,force=%s)', c.relname, c.relrowsecurity, c.relforcerowsecurity)
           ORDER BY c.relname
         )
    INTO unsecured_tables
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
   WHERE n.nspname = 'crm'
     AND c.relkind IN ('r', 'p')
     AND EXISTS (
       SELECT 1
         FROM pg_attribute a
        WHERE a.attrelid = c.oid
          AND a.attname = 'tenant_id'
          AND a.attnum > 0
          AND NOT a.attisdropped
     )
     AND (NOT c.relrowsecurity OR NOT c.relforcerowsecurity);

  IF cardinality(unsecured_tables) > 0 THEN
    RAISE EXCEPTION 'tenant-bearing CRM tables lack ENABLE+FORCE RLS: %', unsecured_tables;
  END IF;

  SELECT array_agg(c.relname ORDER BY c.relname)
    INTO incomplete_policies
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
   WHERE n.nspname = 'crm'
     AND c.relkind IN ('r', 'p')
     AND EXISTS (
       SELECT 1
         FROM pg_attribute a
        WHERE a.attrelid = c.oid
          AND a.attname = 'tenant_id'
          AND a.attnum > 0
          AND NOT a.attisdropped
     )
     AND NOT EXISTS (
       SELECT 1
         FROM pg_policy p
        WHERE p.polrelid = c.oid
          AND p.polname = 'tenant_isolation'
          AND p.polqual IS NOT NULL
          AND p.polwithcheck IS NOT NULL
          AND pg_get_expr(p.polqual, p.polrelid) LIKE '%current_tenant_id%'
          AND pg_get_expr(p.polwithcheck, p.polrelid) LIKE '%current_tenant_id%'
     );

  IF cardinality(incomplete_policies) > 0 THEN
    RAISE EXCEPTION 'tenant-bearing CRM tables lack complete tenant_isolation policy: %', incomplete_policies;
  END IF;

  SELECT rolsuper, rolbypassrls
    INTO role_is_superuser, role_bypasses_rls
    FROM pg_roles
   WHERE rolname = 'crm_app_test';

  IF NOT FOUND THEN
    RAISE EXCEPTION 'crm_app_test role is missing';
  END IF;
  IF role_is_superuser OR role_bypasses_rls THEN
    RAISE EXCEPTION 'crm_app_test must remain NOSUPERUSER NOBYPASSRLS';
  END IF;
END;
$$;

SET ROLE crm_app_test;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-customer-enrichment-force-rls';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-customer-enrichment-force-rls';

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
  'customer-enrichment-force-rls',
  decode(repeat('51', 32), 'hex'),
  'completed',
  'tx-customer-enrichment-force-rls',
  clock_timestamp() + interval '1 day'
);

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
  typed_projection,
  last_business_transaction_id
)
VALUES (
  'tenant-a',
  'customer_enrichment.rls_probe',
  'customer-enrichment-force-rls-a',
  1,
  'crm.customer-enrichment',
  'crm.customer_enrichment.rls_probe.v1',
  '1.0.0',
  decode(repeat('52', 32), 'hex'),
  'personal',
  'protobuf',
  64,
  'crm.customer_enrichment.lifecycle',
  decode('01', 'hex'),
  '{"status":"isolated"}'::jsonb,
  'tx-customer-enrichment-force-rls'
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
  'event-customer-enrichment-force-rls',
  'tx-customer-enrichment-force-rls',
  'customer_enrichment.rls_probe',
  'customer-enrichment-force-rls-a',
  1,
  1,
  'customer_enrichment.rls_probe.created',
  'customer-enrichment-force-rls',
  'crm.customer_enrichment.rls_probe.created.v1',
  '1.0.0',
  decode(repeat('53', 32), 'hex'),
  'personal',
  'protobuf',
  64,
  'crm.customer_enrichment.lifecycle',
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
  3,
  'audit-customer-enrichment-force-rls',
  'tx-customer-enrichment-force-rls',
  'actor-a',
  'test.record.mutate',
  '1.0.0',
  'crm.cjson/v1',
  decode(repeat('22', 32), 'hex'),
  decode(repeat('54', 32), 'hex'),
  convert_to('{"customer_enrichment":"force_rls"}', 'UTF8'),
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
  'tx-customer-enrichment-force-rls',
  'actor-a',
  'request-customer-enrichment-force-rls',
  'test.record.mutate',
  '1.0.0',
  1,
  1,
  1
);

SET CONSTRAINTS ALL IMMEDIATE;

DO $$
DECLARE
  visible_count integer;
BEGIN
  SELECT count(*)
    INTO visible_count
    FROM crm.records
   WHERE owner_module_id = 'crm.customer-enrichment'
     AND record_id = 'customer-enrichment-force-rls-a';
  IF visible_count <> 1 THEN
    RAISE EXCEPTION 'tenant-a could not read its Customer Enrichment record';
  END IF;
END;
$$;

SELECT set_config('app.tenant_id', 'tenant-b', true);
SELECT set_config('app.actor_id', 'actor-b', true);
SELECT set_config('app.request_id', 'request-customer-enrichment-force-rls-b', true);
SELECT set_config('app.business_transaction_id', 'tx-customer-enrichment-force-rls-b', true);

DO $$
DECLARE
  visible_count integer;
  affected_count integer;
  rejected boolean := false;
BEGIN
  SELECT count(*)
    INTO visible_count
    FROM crm.records
   WHERE owner_module_id = 'crm.customer-enrichment'
     AND record_id = 'customer-enrichment-force-rls-a';
  IF visible_count <> 0 THEN
    RAISE EXCEPTION 'tenant-b read tenant-a Customer Enrichment state';
  END IF;

  UPDATE crm.records
     SET typed_projection = '{"status":"mutated"}'::jsonb
   WHERE tenant_id = 'tenant-a'
     AND record_type = 'customer_enrichment.rls_probe'
     AND record_id = 'customer-enrichment-force-rls-a';
  GET DIAGNOSTICS affected_count = ROW_COUNT;
  IF affected_count <> 0 THEN
    RAISE EXCEPTION 'tenant-b updated tenant-a Customer Enrichment state';
  END IF;

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
      'customer_enrichment.rls_probe',
      'customer-enrichment-cross-tenant-write',
      1,
      'crm.customer-enrichment',
      'crm.customer_enrichment.rls_probe.v1',
      '1.0.0',
      decode(repeat('55', 32), 'hex'),
      'personal',
      'protobuf',
      64,
      'crm.customer_enrichment.lifecycle',
      decode('01', 'hex'),
      'tx-customer-enrichment-force-rls-b'
    );
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'cross-tenant Customer Enrichment insert was not rejected';
  END IF;
END;
$$;

SELECT set_config('app.tenant_id', '', true);
DO $$
DECLARE
  visible_count integer;
BEGIN
  SELECT count(*)
    INTO visible_count
    FROM crm.records
   WHERE owner_module_id = 'crm.customer-enrichment';
  IF visible_count <> 0 THEN
    RAISE EXCEPTION 'Customer Enrichment state was visible without tenant context';
  END IF;
END;
$$;

SELECT set_config('app.tenant_id', 'tenant-b', true);
DO $$
DECLARE
  bypassed boolean := false;
BEGIN
  PERFORM set_config('row_security', 'off', true);
  BEGIN
    PERFORM count(*)
      FROM crm.records
     WHERE owner_module_id = 'crm.customer-enrichment';
  EXCEPTION WHEN OTHERS THEN
    bypassed := true;
  END;
  PERFORM set_config('row_security', 'on', true);
  IF NOT bypassed THEN
    RAISE EXCEPTION 'crm_app_test bypassed FORCE RLS with row_security=off';
  END IF;
END;
$$;

ROLLBACK;
RESET ROLE;

SELECT 'Customer Enrichment dynamic FORCE RLS PASS' AS result;

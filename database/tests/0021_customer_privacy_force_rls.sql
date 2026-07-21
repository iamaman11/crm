\set ON_ERROR_STOP on

DO $$
DECLARE
  unsecured_tables text[];
  incomplete_policies text[];
  role_is_superuser boolean;
  role_bypasses_rls boolean;
BEGIN
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
SET LOCAL app.tenant_id = 'tenant-privacy-a';
SET LOCAL app.actor_id = 'privacy-actor-a';
SET LOCAL app.request_id = 'request-customer-privacy-force-rls';
SET LOCAL app.capability_id = 'customer_privacy.persistence.probe';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-customer-privacy-force-rls';

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
  'tenant-privacy-a',
  'tx-customer-privacy-force-rls',
  'privacy-actor-a',
  'request-customer-privacy-force-rls',
  'customer_privacy.persistence.probe',
  '1.0.0',
  1,
  1,
  1
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
VALUES
  (
    'tenant-privacy-a',
    'customer-privacy.case',
    'privacy-case-force-rls',
    1,
    'crm.customer-privacy',
    'crm.customer-privacy.case.state',
    '1.0.0',
    decode(repeat('61', 32), 'hex'),
    'personal',
    'json',
    65536,
    'crm.customer_privacy.case',
    convert_to('{"canonicalization_profile":"crm.cjson/v1","case_id":"privacy-case-force-rls","created_at_unix_nanos":"10","kind":"erasure","last_transition_at_unix_nanos":"10","policy_version":"privacy-policy/1","status":{"code":"draft"},"tenant_id":"tenant-privacy-a","version":"1"}', 'UTF8'),
    '{"kind":"erasure","status":"draft"}'::jsonb,
    'tx-customer-privacy-force-rls'
  ),
  (
    'tenant-privacy-a',
    'customer-privacy.restriction',
    'privacy-restriction-force-rls',
    1,
    'crm.customer-privacy',
    'crm.customer-privacy.processing_restriction.state',
    '1.0.0',
    decode(repeat('62', 32), 'hex'),
    'personal',
    'json',
    16384,
    'crm.customer_privacy.restriction',
    convert_to('{"canonicalization_profile":"crm.cjson/v1","canonical_party_id":"party-force-rls","effective_from_unix_nanos":"20","placed_at_unix_nanos":"20","placed_by":"privacy-actor-a","policy_version":"privacy-policy/1","restriction_id":"privacy-restriction-force-rls","scope":"processing_and_communication","status":"active","tenant_id":"tenant-privacy-a","version":"1"}', 'UTF8'),
    '{"canonical_party_id":"party-force-rls","scope":"processing_and_communication","status":"active"}'::jsonb,
    'tx-customer-privacy-force-rls'
  ),
  (
    'tenant-privacy-a',
    'customer-privacy.legal-hold',
    'privacy-legal-hold-force-rls',
    1,
    'crm.customer-privacy',
    'crm.customer-privacy.legal_hold.state',
    '1.0.0',
    decode(repeat('63', 32), 'hex'),
    'personal',
    'json',
    16384,
    'crm.customer_privacy.legal_hold',
    convert_to('{"authority_reference":"authority-force-rls","canonical_party_id":"party-force-rls","canonicalization_profile":"crm.cjson/v1","effective_from_unix_nanos":"30","hold_id":"privacy-legal-hold-force-rls","placed_by":"legal-actor-a","policy_version":"privacy-policy/1","reason_code":"LITIGATION_HOLD","scope":{"kind":"all_customer_data"},"status":"active","tenant_id":"tenant-privacy-a","version":"1"}', 'UTF8'),
    '{"canonical_party_id":"party-force-rls","reason_code":"LITIGATION_HOLD","status":"active"}'::jsonb,
    'tx-customer-privacy-force-rls'
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
  'tenant-privacy-a',
  'customer_privacy.persistence.probe@1.0.0',
  'customer-privacy-force-rls',
  decode(repeat('65', 32), 'hex'),
  'completed',
  'tx-customer-privacy-force-rls',
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
  'tenant-privacy-a',
  'event-customer-privacy-force-rls',
  'tx-customer-privacy-force-rls',
  'customer-privacy.case',
  'privacy-case-force-rls',
  1,
  1,
  'customer_privacy.persistence.probe.recorded',
  'customer-privacy-force-rls',
  'crm.customer_privacy.persistence_probe.recorded',
  '1.0.0',
  decode(repeat('66', 32), 'hex'),
  'personal',
  'protobuf',
  64,
  'crm.customer_privacy.case',
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
  'tenant-privacy-a',
  1,
  'audit-customer-privacy-force-rls',
  'tx-customer-privacy-force-rls',
  'privacy-actor-a',
  'customer_privacy.persistence.probe',
  '1.0.0',
  'crm.cjson/v1',
  decode(repeat('00', 32), 'hex'),
  decode(repeat('67', 32), 'hex'),
  convert_to('{"customer_privacy":"force_rls"}', 'UTF8'),
  clock_timestamp()
);

SET CONSTRAINTS ALL IMMEDIATE;

DO $$
DECLARE
  visible_count integer;
BEGIN
  SELECT count(*)
    INTO visible_count
    FROM crm.records
   WHERE owner_module_id = 'crm.customer-privacy';
  IF visible_count <> 3 THEN
    RAISE EXCEPTION 'tenant-privacy-a could not read its three Customer Privacy records';
  END IF;
END;
$$;

SELECT set_config('app.tenant_id', 'tenant-privacy-b', true);
SELECT set_config('app.actor_id', 'privacy-actor-b', true);
SELECT set_config('app.request_id', 'request-customer-privacy-force-rls-b', true);
SELECT set_config('app.business_transaction_id', 'tx-customer-privacy-force-rls-b', true);

DO $$
DECLARE
  visible_count integer;
  affected_count integer;
  rejected boolean := false;
BEGIN
  SELECT count(*)
    INTO visible_count
    FROM crm.records
   WHERE owner_module_id = 'crm.customer-privacy';
  IF visible_count <> 0 THEN
    RAISE EXCEPTION 'tenant-privacy-b read tenant-privacy-a Customer Privacy state';
  END IF;

  UPDATE crm.records
     SET typed_projection = '{"status":"mutated"}'::jsonb
   WHERE tenant_id = 'tenant-privacy-a'
     AND owner_module_id = 'crm.customer-privacy';
  GET DIAGNOSTICS affected_count = ROW_COUNT;
  IF affected_count <> 0 THEN
    RAISE EXCEPTION 'tenant-privacy-b updated tenant-privacy-a Customer Privacy state';
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
      'tenant-privacy-a',
      'customer-privacy.case',
      'privacy-cross-tenant-write',
      1,
      'crm.customer-privacy',
      'crm.customer-privacy.case.state',
      '1.0.0',
      decode(repeat('64', 32), 'hex'),
      'personal',
      'json',
      65536,
      'crm.customer_privacy.case',
      convert_to('{}', 'UTF8'),
      'tx-customer-privacy-force-rls'
    );
  EXCEPTION WHEN OTHERS THEN
    rejected := true;
  END;
  IF NOT rejected THEN
    RAISE EXCEPTION 'cross-tenant Customer Privacy insert was not rejected';
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
   WHERE owner_module_id = 'crm.customer-privacy';
  IF visible_count <> 0 THEN
    RAISE EXCEPTION 'Customer Privacy state was visible without tenant context';
  END IF;
END;
$$;

SELECT set_config('app.tenant_id', 'tenant-privacy-b', true);
DO $$
DECLARE
  blocked boolean := false;
BEGIN
  PERFORM set_config('row_security', 'off', true);
  BEGIN
    PERFORM count(*)
      FROM crm.records
     WHERE owner_module_id = 'crm.customer-privacy';
  EXCEPTION WHEN OTHERS THEN
    blocked := true;
  END;
  PERFORM set_config('row_security', 'on', true);
  IF NOT blocked THEN
    RAISE EXCEPTION 'crm_app_test bypassed Customer Privacy FORCE RLS with row_security=off';
  END IF;
END;
$$;

ROLLBACK;
RESET ROLE;

SELECT 'Customer Privacy FORCE RLS envelope PASS' AS result;

\set ON_ERROR_STOP on

DO $$
DECLARE
  forced_count integer;
BEGIN
  SELECT count(*)
  INTO forced_count
  FROM pg_class c
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname = 'crm'
    AND c.relname IN (
      'customer_privacy_discovery_attempts',
      'customer_privacy_discovery_owner_pages',
      'customer_privacy_discovery_checkpoints',
      'customer_privacy_discovery_snapshots',
      'customer_privacy_discovery_audit'
    )
    AND c.relrowsecurity
    AND c.relforcerowsecurity;
  IF forced_count <> 5 THEN
    RAISE EXCEPTION 'expected FORCE RLS on all five discovery evidence tables, found %', forced_count;
  END IF;
END;
$$;

GRANT USAGE ON SCHEMA crm TO crm_app_test;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE
  crm.customer_privacy_discovery_attempts,
  crm.customer_privacy_discovery_owner_pages,
  crm.customer_privacy_discovery_checkpoints,
  crm.customer_privacy_discovery_snapshots,
  crm.customer_privacy_discovery_audit
TO crm_app_test;

SET ROLE crm_app_test;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
INSERT INTO crm.customer_privacy_discovery_attempts (
  tenant_id,
  attempt_digest,
  privacy_case_id,
  canonical_party_id,
  identity_resolution_generation,
  registry_version,
  registry_digest,
  purpose_code,
  effective_request_at_unix_ms,
  captured_at_unix_nanos
) VALUES (
  'tenant-a',
  decode(repeat('11', 32), 'hex'),
  'privacy-case-discovery-1',
  'party-discovery-1',
  1,
  '1.0.0',
  decode(repeat('22', 32), 'hex'),
  'ERASURE',
  1000,
  1000000
);

INSERT INTO crm.customer_privacy_discovery_owner_pages (
  tenant_id,
  attempt_digest,
  owner_module_id,
  capability_id,
  capability_version,
  lineage_digest,
  page_number,
  request_cursor_digest,
  response_cursor_digest,
  owner_cursor_digest,
  page_digest,
  scanned_resource_count,
  emitted_resource_count,
  terminal_complete,
  response_bytes,
  response_digest
) VALUES (
  'tenant-a',
  decode(repeat('11', 32), 'hex'),
  'crm.parties',
  'parties.privacy.scope.contribute',
  '1.0.0',
  decode(repeat('33', 32), 'hex'),
  1,
  decode(repeat('44', 32), 'hex'),
  decode(repeat('55', 32), 'hex'),
  decode(repeat('66', 32), 'hex'),
  decode(repeat('77', 32), 'hex'),
  1,
  1,
  true,
  decode('010203', 'hex'),
  decode('039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81', 'hex')
);

INSERT INTO crm.customer_privacy_discovery_checkpoints (
  tenant_id,
  attempt_digest,
  owner_module_id,
  contiguous_page_number,
  terminal_complete
) VALUES (
  'tenant-a',
  decode(repeat('11', 32), 'hex'),
  'crm.parties',
  1,
  true
);

DO $$
BEGIN
  IF (SELECT count(*) FROM crm.customer_privacy_discovery_attempts) <> 1 THEN
    RAISE EXCEPTION 'same-tenant attempt visibility failed';
  END IF;
  IF (SELECT count(*) FROM crm.customer_privacy_discovery_owner_pages) <> 1 THEN
    RAISE EXCEPTION 'same-tenant owner page visibility failed';
  END IF;
  BEGIN
    UPDATE crm.customer_privacy_discovery_owner_pages
    SET emitted_resource_count = 2
    WHERE tenant_id = 'tenant-a';
    RAISE EXCEPTION 'immutable owner page update unexpectedly succeeded';
  EXCEPTION WHEN object_not_in_prerequisite_state THEN
    NULL;
  END;
END;
$$;
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
DO $$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM crm.customer_privacy_discovery_attempts
    WHERE tenant_id = 'tenant-a'
  ) THEN
    RAISE EXCEPTION 'cross-tenant attempt visibility was not denied';
  END IF;
  IF EXISTS (
    SELECT 1
    FROM crm.customer_privacy_discovery_owner_pages
    WHERE tenant_id = 'tenant-a'
  ) THEN
    RAISE EXCEPTION 'cross-tenant owner page visibility was not denied';
  END IF;
  BEGIN
    INSERT INTO crm.customer_privacy_discovery_attempts (
      tenant_id,
      attempt_digest,
      privacy_case_id,
      canonical_party_id,
      identity_resolution_generation,
      registry_version,
      registry_digest,
      purpose_code,
      effective_request_at_unix_ms,
      captured_at_unix_nanos
    ) VALUES (
      'tenant-a',
      decode(repeat('88', 32), 'hex'),
      'privacy-case-cross-tenant',
      'party-cross-tenant',
      1,
      '1.0.0',
      decode(repeat('99', 32), 'hex'),
      'ERASURE',
      1000,
      1000000
    );
    RAISE EXCEPTION 'cross-tenant insert unexpectedly succeeded';
  EXCEPTION WHEN insufficient_privilege THEN
    NULL;
  END;
END;
$$;
ROLLBACK;

RESET ROLE;

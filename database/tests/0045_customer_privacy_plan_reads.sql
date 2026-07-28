\set ON_ERROR_STOP on

DO $$
DECLARE
  forced_count integer;
  policy_count integer;
  immutable_trigger_count integer;
BEGIN
  SELECT count(*)
  INTO forced_count
  FROM pg_class c
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname = 'crm'
    AND c.relname = 'customer_privacy_plan_read_audit'
    AND c.relrowsecurity
    AND c.relforcerowsecurity;
  IF forced_count <> 1 THEN
    RAISE EXCEPTION 'expected FORCE RLS on Customer Privacy read audit, found %', forced_count;
  END IF;

  SELECT count(*)
  INTO policy_count
  FROM pg_policies
  WHERE schemaname = 'crm'
    AND tablename = 'customer_privacy_plan_read_audit'
    AND policyname = 'tenant_isolation'
    AND qual = '(tenant_id = crm.current_tenant_id())'
    AND with_check = '(tenant_id = crm.current_tenant_id())';
  IF policy_count <> 1 THEN
    RAISE EXCEPTION 'expected canonical tenant_isolation on read audit, found %', policy_count;
  END IF;

  SELECT count(*)
  INTO immutable_trigger_count
  FROM pg_trigger t
  JOIN pg_class c ON c.oid = t.tgrelid
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE NOT t.tgisinternal
    AND n.nspname = 'crm'
    AND c.relname = 'customer_privacy_plan_read_audit'
    AND t.tgname = 'customer_privacy_plan_read_audit_immutable';
  IF immutable_trigger_count <> 1 THEN
    RAISE EXCEPTION 'expected immutable read-audit trigger, found %', immutable_trigger_count;
  END IF;

  IF to_regclass('crm.customer_privacy_owner_outcomes') IS NOT NULL THEN
    RAISE EXCEPTION 'owner-outcome persistence must not exist in the read-only packet';
  END IF;
END;
$$;

GRANT USAGE ON SCHEMA crm TO crm_app_test;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE crm.customer_privacy_plan_read_audit TO crm_app_test;

SET ROLE crm_app_test;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';

INSERT INTO crm.customer_privacy_plan_read_audit (
  tenant_id, audit_digest, capability_id, privacy_case_id,
  plan_id, plan_digest, authorization_digest, allowed, result_code,
  actor_id, request_id, correlation_id, trace_id, occurred_at_unix_nanos
) VALUES (
  'tenant-a', decode(repeat('11', 32), 'hex'),
  'customer_privacy.case.plan.get', 'privacy-case-read-1',
  'privacy-action-plan-read-1', decode(repeat('22', 32), 'hex'),
  decode(repeat('33', 32), 'hex'), true, 'plan_read_allowed',
  'privacy-reader', 'request-read-1', 'correlation-read-1', 'trace-read-1', 1000000000
);

INSERT INTO crm.customer_privacy_plan_read_audit (
  tenant_id, audit_digest, capability_id, privacy_case_id,
  owner_module_filter, page_size, page_digest, terminal_digest,
  authorization_digest, allowed, result_code,
  actor_id, request_id, correlation_id, trace_id, occurred_at_unix_nanos
) VALUES (
  'tenant-a', decode(repeat('44', 32), 'hex'),
  'customer_privacy.case.owner_outcomes.list', 'privacy-case-concealed',
  'crm.parties', NULL, NULL, NULL,
  decode(repeat('55', 32), 'hex'), false, 'case_visibility_denied',
  'privacy-reader', 'request-read-2', 'correlation-read-2', 'trace-read-2', 2000000000
);

INSERT INTO crm.customer_privacy_plan_read_audit (
  tenant_id, audit_digest, capability_id, privacy_case_id,
  plan_id, plan_digest, owner_module_filter, page_size,
  page_digest, terminal_digest, authorization_digest, allowed, result_code,
  actor_id, request_id, correlation_id, trace_id, occurred_at_unix_nanos
) VALUES (
  'tenant-a', decode(repeat('66', 32), 'hex'),
  'customer_privacy.case.owner_outcomes.list', 'privacy-case-read-1',
  'privacy-action-plan-read-1', decode(repeat('22', 32), 'hex'),
  'crm.parties', 64, decode(repeat('77', 32), 'hex'), decode(repeat('88', 32), 'hex'),
  decode(repeat('99', 32), 'hex'), true, 'owner_outcomes_empty_terminal_allowed',
  'privacy-reader', 'request-read-3', 'correlation-read-3', 'trace-read-3', 3000000000
);

DO $$
BEGIN
  IF (SELECT count(*) FROM crm.customer_privacy_plan_read_audit) <> 3 THEN
    RAISE EXCEPTION 'same-tenant read-audit visibility failed';
  END IF;
  IF EXISTS (
    SELECT 1 FROM crm.customer_privacy_plan_read_audit
    WHERE result_code = 'owner_outcomes_empty_terminal_allowed'
      AND (page_size <> 64 OR page_digest IS NULL OR terminal_digest IS NULL)
  ) THEN
    RAISE EXCEPTION 'empty terminal outcome-page evidence is incomplete';
  END IF;

  BEGIN
    UPDATE crm.customer_privacy_plan_read_audit
    SET result_code = 'evidence_invalid'
    WHERE request_id = 'request-read-1';
    RAISE EXCEPTION 'immutable read-audit update unexpectedly succeeded';
  EXCEPTION WHEN object_not_in_prerequisite_state THEN
    NULL;
  END;

  BEGIN
    DELETE FROM crm.customer_privacy_plan_read_audit
    WHERE request_id = 'request-read-2';
    RAISE EXCEPTION 'immutable read-audit delete unexpectedly succeeded';
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
    SELECT 1 FROM crm.customer_privacy_plan_read_audit
    WHERE tenant_id = 'tenant-a'
  ) THEN
    RAISE EXCEPTION 'cross-tenant read-audit visibility was not denied';
  END IF;

  BEGIN
    INSERT INTO crm.customer_privacy_plan_read_audit (
      tenant_id, audit_digest, capability_id, privacy_case_id,
      authorization_digest, allowed, result_code,
      actor_id, request_id, correlation_id, trace_id, occurred_at_unix_nanos
    ) VALUES (
      'tenant-a', decode(repeat('aa', 32), 'hex'),
      'customer_privacy.case.plan.get', 'privacy-case-cross-tenant',
      decode(repeat('bb', 32), 'hex'), false, 'source_not_found',
      'privacy-reader', 'request-cross-tenant', 'correlation-cross-tenant',
      'trace-cross-tenant', 4000000000
    );
    RAISE EXCEPTION 'cross-tenant read-audit insert unexpectedly succeeded';
  EXCEPTION WHEN insufficient_privilege THEN
    NULL;
  END;
END;
$$;
ROLLBACK;

RESET ROLE;

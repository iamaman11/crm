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
    AND c.relname IN (
      'customer_privacy_action_plans',
      'customer_privacy_planning_audit'
    )
    AND c.relrowsecurity
    AND c.relforcerowsecurity;
  IF forced_count <> 2 THEN
    RAISE EXCEPTION 'expected FORCE RLS on both planning evidence tables, found %', forced_count;
  END IF;

  SELECT count(*)
  INTO policy_count
  FROM pg_policies
  WHERE schemaname = 'crm'
    AND tablename IN (
      'customer_privacy_action_plans',
      'customer_privacy_planning_audit'
    )
    AND policyname = 'tenant_isolation'
    AND qual = '(tenant_id = crm.current_tenant_id())'
    AND with_check = '(tenant_id = crm.current_tenant_id())';
  IF policy_count <> 2 THEN
    RAISE EXCEPTION 'expected canonical tenant_isolation policies on both planning tables, found %', policy_count;
  END IF;

  SELECT count(*)
  INTO immutable_trigger_count
  FROM pg_trigger t
  JOIN pg_class c ON c.oid = t.tgrelid
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE NOT t.tgisinternal
    AND n.nspname = 'crm'
    AND t.tgname IN (
      'customer_privacy_action_plans_immutable',
      'customer_privacy_planning_audit_immutable',
      'customer_privacy_action_plan_records_immutable'
    );
  IF immutable_trigger_count <> 3 THEN
    RAISE EXCEPTION 'expected all three planning immutability triggers, found %', immutable_trigger_count;
  END IF;
END;
$$;

GRANT USAGE ON SCHEMA crm TO crm_app_test;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE
  crm.customer_privacy_action_plans,
  crm.customer_privacy_planning_audit
TO crm_app_test;

SET ROLE crm_app_test;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';

INSERT INTO crm.customer_privacy_action_plans (
  tenant_id,
  privacy_case_id,
  source_case_version,
  resulting_case_version,
  scope_snapshot_id,
  plan_id,
  plan_digest,
  approval_required,
  planned_at_unix_nanos
) VALUES (
  'tenant-a',
  'privacy-case-plan-1',
  4,
  5,
  'privacy-discovery-scope-test',
  'privacy-action-plan-test',
  decode(repeat('11', 32), 'hex'),
  true,
  1000000001
);

INSERT INTO crm.customer_privacy_planning_audit (
  tenant_id,
  audit_digest,
  event_type,
  privacy_case_id,
  plan_id,
  plan_digest,
  resulting_case_version,
  actor_id,
  request_id,
  occurred_at_unix_nanos
) VALUES (
  'tenant-a',
  decode(repeat('22', 32), 'hex'),
  'planning_finalized',
  'privacy-case-plan-1',
  'privacy-action-plan-test',
  decode(repeat('11', 32), 'hex'),
  5,
  'privacy-worker',
  'request-plan-1',
  1000000001
);

DO $$
BEGIN
  IF (SELECT count(*) FROM crm.customer_privacy_action_plans) <> 1 THEN
    RAISE EXCEPTION 'same-tenant action-plan visibility failed';
  END IF;
  IF (SELECT count(*) FROM crm.customer_privacy_planning_audit) <> 1 THEN
    RAISE EXCEPTION 'same-tenant planning-audit visibility failed';
  END IF;

  BEGIN
    UPDATE crm.customer_privacy_action_plans
    SET approval_required = false
    WHERE privacy_case_id = 'privacy-case-plan-1';
    RAISE EXCEPTION 'immutable action-plan link update unexpectedly succeeded';
  EXCEPTION WHEN object_not_in_prerequisite_state THEN
    NULL;
  END;

  BEGIN
    DELETE FROM crm.customer_privacy_planning_audit
    WHERE privacy_case_id = 'privacy-case-plan-1';
    RAISE EXCEPTION 'immutable planning audit delete unexpectedly succeeded';
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
    FROM crm.customer_privacy_action_plans
    WHERE tenant_id = 'tenant-a'
  ) THEN
    RAISE EXCEPTION 'cross-tenant action-plan visibility was not denied';
  END IF;
  IF EXISTS (
    SELECT 1
    FROM crm.customer_privacy_planning_audit
    WHERE tenant_id = 'tenant-a'
  ) THEN
    RAISE EXCEPTION 'cross-tenant planning-audit visibility was not denied';
  END IF;

  BEGIN
    INSERT INTO crm.customer_privacy_action_plans (
      tenant_id,
      privacy_case_id,
      source_case_version,
      resulting_case_version,
      scope_snapshot_id,
      plan_id,
      plan_digest,
      approval_required,
      planned_at_unix_nanos
    ) VALUES (
      'tenant-a',
      'privacy-case-cross-tenant',
      1,
      2,
      'privacy-discovery-scope-cross-tenant',
      'privacy-action-plan-cross-tenant',
      decode(repeat('33', 32), 'hex'),
      false,
      2000000001
    );
    RAISE EXCEPTION 'cross-tenant planning insert unexpectedly succeeded';
  EXCEPTION WHEN insufficient_privilege THEN
    NULL;
  END;
END;
$$;
ROLLBACK;

RESET ROLE;

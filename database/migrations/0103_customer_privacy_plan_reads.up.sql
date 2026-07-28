CREATE TABLE crm.customer_privacy_plan_read_audit (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  audit_digest bytea NOT NULL CHECK (octet_length(audit_digest) = 32),
  capability_id text NOT NULL CHECK (capability_id IN (
    'customer_privacy.case.plan.get',
    'customer_privacy.case.owner_outcomes.list'
  )),
  privacy_case_id text NOT NULL,
  plan_id text,
  plan_digest bytea CHECK (plan_digest IS NULL OR octet_length(plan_digest) = 32),
  owner_module_filter text,
  page_size integer CHECK (page_size IS NULL OR page_size BETWEEN 1 AND 128),
  page_digest bytea CHECK (page_digest IS NULL OR octet_length(page_digest) = 32),
  terminal_digest bytea CHECK (terminal_digest IS NULL OR octet_length(terminal_digest) = 32),
  authorization_digest bytea NOT NULL CHECK (octet_length(authorization_digest) = 32),
  allowed boolean NOT NULL,
  result_code text NOT NULL CHECK (result_code IN (
    'plan_read_allowed',
    'owner_outcomes_empty_terminal_allowed',
    'case_visibility_denied',
    'source_not_found',
    'party_visibility_denied',
    'plan_visibility_denied',
    'evidence_invalid'
  )),
  actor_id text NOT NULL,
  request_id text NOT NULL,
  correlation_id text NOT NULL,
  trace_id text NOT NULL,
  occurred_at_unix_nanos bigint NOT NULL CHECK (occurred_at_unix_nanos > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, audit_digest),
  CHECK ((allowed AND plan_id IS NOT NULL AND plan_digest IS NOT NULL)
      OR (NOT allowed)),
  CHECK ((capability_id = 'customer_privacy.case.owner_outcomes.list'
          AND allowed
          AND page_size IS NOT NULL
          AND page_digest IS NOT NULL
          AND terminal_digest IS NOT NULL)
      OR capability_id <> 'customer_privacy.case.owner_outcomes.list'
      OR NOT allowed),
  CHECK (result_code <> 'owner_outcomes_empty_terminal_allowed'
      OR (page_digest IS NOT NULL AND terminal_digest IS NOT NULL))
);

CREATE INDEX customer_privacy_plan_read_audit_case_idx
  ON crm.customer_privacy_plan_read_audit (
    tenant_id,
    privacy_case_id,
    occurred_at_unix_nanos,
    audit_digest
  );

ALTER TABLE crm.customer_privacy_plan_read_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_plan_read_audit FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation
  ON crm.customer_privacy_plan_read_audit
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE FUNCTION crm.reject_customer_privacy_plan_read_audit_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'customer privacy read audit evidence is immutable'
    USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER customer_privacy_plan_read_audit_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_plan_read_audit
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_plan_read_audit_mutation();

CREATE TABLE crm.customer_privacy_action_plans (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  privacy_case_id text NOT NULL,
  source_case_version bigint NOT NULL CHECK (source_case_version > 0),
  resulting_case_version bigint NOT NULL CHECK (
    resulting_case_version = source_case_version + 1
  ),
  scope_snapshot_id text NOT NULL,
  plan_id text NOT NULL,
  plan_digest bytea NOT NULL CHECK (octet_length(plan_digest) = 32),
  approval_required boolean NOT NULL,
  planned_at_unix_nanos bigint NOT NULL CHECK (planned_at_unix_nanos > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, privacy_case_id),
  UNIQUE (tenant_id, plan_id)
);

CREATE TABLE crm.customer_privacy_planning_audit (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  audit_digest bytea NOT NULL CHECK (octet_length(audit_digest) = 32),
  event_type text NOT NULL CHECK (event_type IN (
    'planning_finalized',
    'planning_replayed'
  )),
  privacy_case_id text NOT NULL,
  plan_id text NOT NULL,
  plan_digest bytea NOT NULL CHECK (octet_length(plan_digest) = 32),
  resulting_case_version bigint NOT NULL CHECK (resulting_case_version > 0),
  actor_id text NOT NULL,
  request_id text NOT NULL,
  occurred_at_unix_nanos bigint NOT NULL CHECK (occurred_at_unix_nanos > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, audit_digest)
);

CREATE INDEX customer_privacy_action_plans_snapshot_idx
  ON crm.customer_privacy_action_plans (
    tenant_id,
    scope_snapshot_id,
    privacy_case_id
  );

CREATE INDEX customer_privacy_planning_audit_case_idx
  ON crm.customer_privacy_planning_audit (
    tenant_id,
    privacy_case_id,
    occurred_at_unix_nanos,
    audit_digest
  );

ALTER TABLE crm.customer_privacy_action_plans ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_action_plans FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_planning_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_planning_audit FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation
  ON crm.customer_privacy_action_plans
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE POLICY tenant_isolation
  ON crm.customer_privacy_planning_audit
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

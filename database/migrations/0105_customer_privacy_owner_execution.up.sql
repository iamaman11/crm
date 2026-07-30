CREATE TABLE crm.customer_privacy_owner_execution_checkpoints (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  privacy_case_id text NOT NULL,
  source_case_version bigint NOT NULL CHECK (source_case_version > 0),
  executing_case_version bigint NOT NULL CHECK (executing_case_version = source_case_version + 1),
  converging_case_version bigint CHECK (
    converging_case_version IS NULL OR converging_case_version = executing_case_version + 1
  ),
  action_plan_id text NOT NULL,
  action_plan_digest bytea NOT NULL CHECK (octet_length(action_plan_digest) = 32),
  retention_decision_id text NOT NULL,
  retention_decision_digest bytea NOT NULL CHECK (octet_length(retention_decision_digest) = 32),
  total_items integer NOT NULL CHECK (total_items BETWEEN 0 AND 16384),
  next_sequence integer NOT NULL CHECK (next_sequence BETWEEN 1 AND 16385),
  started_at_unix_nanos bigint NOT NULL CHECK (started_at_unix_nanos > 0),
  completed_at_unix_nanos bigint CHECK (
    completed_at_unix_nanos IS NULL OR completed_at_unix_nanos >= started_at_unix_nanos
  ),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, privacy_case_id),
  UNIQUE (tenant_id, action_plan_id),
  UNIQUE (tenant_id, retention_decision_id),
  CHECK (next_sequence <= total_items + 1),
  CHECK ((next_sequence = total_items + 1) = (completed_at_unix_nanos IS NOT NULL)),
  CHECK ((completed_at_unix_nanos IS NOT NULL) = (converging_case_version IS NOT NULL))
);

CREATE TABLE crm.customer_privacy_owner_action_attempts (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  privacy_case_id text NOT NULL,
  action_plan_id text NOT NULL,
  action_plan_digest bytea NOT NULL CHECK (octet_length(action_plan_digest) = 32),
  retention_decision_id text NOT NULL,
  retention_decision_digest bytea NOT NULL CHECK (octet_length(retention_decision_digest) = 32),
  item_sequence integer NOT NULL CHECK (item_sequence BETWEEN 1 AND 16384),
  attempt_generation integer NOT NULL CHECK (attempt_generation BETWEEN 0 AND 100),
  attempt_id text NOT NULL,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  item_digest bytea NOT NULL CHECK (octet_length(item_digest) = 32),
  owner_module_id text NOT NULL,
  owner_capability_id text NOT NULL,
  owner_capability_version text NOT NULL CHECK (owner_capability_version = '1.0.0'),
  target_idempotency_key text NOT NULL,
  resource_type text NOT NULL,
  resource_id text NOT NULL,
  resource_version bigint NOT NULL CHECK (resource_version > 0),
  action_code text NOT NULL,
  decision_reason text NOT NULL,
  schema_id text NOT NULL CHECK (schema_id = 'crm.customer-privacy.owner_action_attempt.state'),
  schema_version text NOT NULL CHECK (schema_version = '1.0.0'),
  descriptor_hash bytea NOT NULL CHECK (octet_length(descriptor_hash) = 32),
  maximum_payload_size bigint NOT NULL CHECK (maximum_payload_size = 32768),
  retention_policy_id text NOT NULL CHECK (
    retention_policy_id = 'crm.customer_privacy.owner_action_attempt'
  ),
  payload_bytes bytea NOT NULL CHECK (octet_length(payload_bytes) BETWEEN 1 AND 32768),
  planned_at_unix_nanos bigint NOT NULL CHECK (planned_at_unix_nanos > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, privacy_case_id, item_sequence, attempt_generation),
  UNIQUE (tenant_id, attempt_id),
  FOREIGN KEY (tenant_id, privacy_case_id)
    REFERENCES crm.customer_privacy_owner_execution_checkpoints (tenant_id, privacy_case_id)
    ON DELETE RESTRICT
);

CREATE TABLE crm.customer_privacy_owner_action_outcomes (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  privacy_case_id text NOT NULL,
  action_plan_id text NOT NULL,
  retention_decision_id text NOT NULL,
  item_sequence integer NOT NULL CHECK (item_sequence BETWEEN 1 AND 16384),
  attempt_generation integer NOT NULL CHECK (attempt_generation BETWEEN 0 AND 100),
  outcome_id text NOT NULL,
  outcome_digest bytea NOT NULL CHECK (octet_length(outcome_digest) = 32),
  attempt_id text NOT NULL,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  owner_module_id text NOT NULL,
  action_code text NOT NULL,
  status text NOT NULL CHECK (status IN (
    'succeeded',
    'retained',
    'blocked_by_hold',
    'blocked_by_retention',
    'failed_retryable',
    'failed_terminal'
  )),
  safe_failure_code text,
  schema_id text NOT NULL CHECK (schema_id = 'crm.customer-privacy.owner_action_outcome.state'),
  schema_version text NOT NULL CHECK (schema_version = '1.0.0'),
  descriptor_hash bytea NOT NULL CHECK (octet_length(descriptor_hash) = 32),
  maximum_payload_size bigint NOT NULL CHECK (maximum_payload_size = 32768),
  retention_policy_id text NOT NULL CHECK (
    retention_policy_id = 'crm.customer_privacy.owner_action_outcome'
  ),
  payload_bytes bytea NOT NULL CHECK (octet_length(payload_bytes) BETWEEN 1 AND 32768),
  recorded_at_unix_nanos bigint NOT NULL CHECK (recorded_at_unix_nanos > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, privacy_case_id, item_sequence, attempt_generation),
  UNIQUE (tenant_id, outcome_id),
  FOREIGN KEY (tenant_id, privacy_case_id, item_sequence, attempt_generation)
    REFERENCES crm.customer_privacy_owner_action_attempts (
      tenant_id, privacy_case_id, item_sequence, attempt_generation
    ) ON DELETE RESTRICT,
  CHECK ((status IN ('failed_retryable', 'failed_terminal') AND safe_failure_code IS NOT NULL)
      OR (status NOT IN ('failed_retryable', 'failed_terminal') AND safe_failure_code IS NULL))
);

CREATE TABLE crm.customer_privacy_owner_execution_audit (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  audit_digest bytea NOT NULL CHECK (octet_length(audit_digest) = 32),
  event_type text NOT NULL CHECK (event_type IN (
    'execution_started',
    'attempt_prepared',
    'outcome_recorded',
    'checkpoint_advanced',
    'execution_complete'
  )),
  privacy_case_id text NOT NULL,
  item_sequence integer CHECK (item_sequence IS NULL OR item_sequence BETWEEN 1 AND 16384),
  attempt_generation integer CHECK (
    attempt_generation IS NULL OR attempt_generation BETWEEN 0 AND 100
  ),
  attempt_id text,
  outcome_id text,
  next_sequence integer CHECK (next_sequence IS NULL OR next_sequence BETWEEN 1 AND 16385),
  actor_id text NOT NULL,
  request_id text NOT NULL,
  correlation_id text NOT NULL,
  trace_id text NOT NULL,
  occurred_at_unix_nanos bigint NOT NULL CHECK (occurred_at_unix_nanos > 0),
  PRIMARY KEY (tenant_id, audit_digest)
);

CREATE INDEX customer_privacy_owner_action_outcomes_list_idx
  ON crm.customer_privacy_owner_action_outcomes (
    tenant_id, privacy_case_id, owner_module_id, item_sequence, attempt_generation, outcome_id
  );

ALTER TABLE crm.customer_privacy_owner_execution_checkpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_execution_checkpoints FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_action_attempts ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_action_attempts FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_action_outcomes ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_action_outcomes FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_execution_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_owner_execution_audit FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON crm.customer_privacy_owner_execution_checkpoints
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());
CREATE POLICY tenant_isolation ON crm.customer_privacy_owner_action_attempts
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());
CREATE POLICY tenant_isolation ON crm.customer_privacy_owner_action_outcomes
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());
CREATE POLICY tenant_isolation ON crm.customer_privacy_owner_execution_audit
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE FUNCTION crm.guard_customer_privacy_owner_execution_checkpoint()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  IF NEW.tenant_id <> OLD.tenant_id
     OR NEW.privacy_case_id <> OLD.privacy_case_id
     OR NEW.source_case_version <> OLD.source_case_version
     OR NEW.executing_case_version <> OLD.executing_case_version
     OR NEW.action_plan_id <> OLD.action_plan_id
     OR NEW.action_plan_digest <> OLD.action_plan_digest
     OR NEW.retention_decision_id <> OLD.retention_decision_id
     OR NEW.retention_decision_digest <> OLD.retention_decision_digest
     OR NEW.total_items <> OLD.total_items
     OR NEW.started_at_unix_nanos <> OLD.started_at_unix_nanos
     OR NEW.next_sequence < OLD.next_sequence
     OR (OLD.converging_case_version IS NOT NULL
         AND NEW.converging_case_version <> OLD.converging_case_version)
     OR (OLD.completed_at_unix_nanos IS NOT NULL
         AND NEW.completed_at_unix_nanos <> OLD.completed_at_unix_nanos) THEN
    RAISE EXCEPTION 'customer privacy owner execution checkpoint lineage or progress is immutable'
      USING ERRCODE = '55000';
  END IF;
  NEW.updated_at := clock_timestamp();
  RETURN NEW;
END;
$$;

CREATE TRIGGER customer_privacy_owner_execution_checkpoint_guard
BEFORE UPDATE ON crm.customer_privacy_owner_execution_checkpoints
FOR EACH ROW EXECUTE FUNCTION crm.guard_customer_privacy_owner_execution_checkpoint();

CREATE FUNCTION crm.reject_customer_privacy_owner_execution_evidence_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'customer privacy owner execution evidence is immutable'
    USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER customer_privacy_owner_action_attempt_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_owner_action_attempts
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_owner_execution_evidence_mutation();

CREATE TRIGGER customer_privacy_owner_action_outcome_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_owner_action_outcomes
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_owner_execution_evidence_mutation();

CREATE TRIGGER customer_privacy_owner_execution_audit_immutable
BEFORE UPDATE OR DELETE ON crm.customer_privacy_owner_execution_audit
FOR EACH ROW EXECUTE FUNCTION crm.reject_customer_privacy_owner_execution_evidence_mutation();

ALTER TABLE crm.customer_privacy_plan_read_audit
  DROP CONSTRAINT IF EXISTS customer_privacy_plan_read_audit_result_code_check;
ALTER TABLE crm.customer_privacy_plan_read_audit
  ADD CONSTRAINT customer_privacy_plan_read_audit_result_code_check CHECK (result_code IN (
    'plan_read_allowed',
    'owner_outcomes_empty_terminal_allowed',
    'owner_outcomes_page_allowed',
    'case_visibility_denied',
    'source_not_found',
    'party_visibility_denied',
    'plan_visibility_denied',
    'evidence_invalid'
  ));

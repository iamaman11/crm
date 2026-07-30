ALTER TABLE crm.customer_privacy_plan_read_audit
  DROP CONSTRAINT IF EXISTS customer_privacy_plan_read_audit_result_code_check;
ALTER TABLE crm.customer_privacy_plan_read_audit
  ADD CONSTRAINT customer_privacy_plan_read_audit_result_code_check CHECK (result_code IN (
    'plan_read_allowed',
    'owner_outcomes_empty_terminal_allowed',
    'case_visibility_denied',
    'source_not_found',
    'party_visibility_denied',
    'plan_visibility_denied',
    'evidence_invalid'
  ));

DROP TABLE IF EXISTS crm.customer_privacy_owner_execution_audit;
DROP TABLE IF EXISTS crm.customer_privacy_owner_action_outcomes;
DROP TABLE IF EXISTS crm.customer_privacy_owner_action_attempts;
DROP TABLE IF EXISTS crm.customer_privacy_owner_execution_checkpoints;
DROP FUNCTION IF EXISTS crm.reject_customer_privacy_owner_execution_evidence_mutation();
DROP FUNCTION IF EXISTS crm.guard_customer_privacy_owner_execution_checkpoint();

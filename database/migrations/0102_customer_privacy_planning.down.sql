DROP TRIGGER IF EXISTS customer_privacy_action_plan_records_immutable ON crm.records;
DROP TABLE IF EXISTS crm.customer_privacy_planning_audit;
DROP TABLE IF EXISTS crm.customer_privacy_action_plans;
DROP FUNCTION IF EXISTS crm.reject_customer_privacy_planning_evidence_mutation();

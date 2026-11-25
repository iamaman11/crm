-- SK-08A/F4: bounded manual attempt control on the existing immutable run.
-- Keep the 0039 pending/committed storage contract; explicit cancellation is
-- a separate terminal fact, never a fabricated Task receipt.
ALTER TABLE crm_activity_automation_runs ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 5);
ALTER TABLE crm_activity_automation_runs ADD COLUMN lease_request_id TEXT;
ALTER TABLE crm_activity_automation_runs ADD COLUMN lease_actor TEXT;
ALTER TABLE crm_activity_automation_runs ADD COLUMN lease_until_unix_millis INTEGER NOT NULL DEFAULT 0;
ALTER TABLE crm_activity_automation_runs ADD COLUMN next_attempt_at_unix_millis INTEGER NOT NULL DEFAULT 0;
ALTER TABLE crm_activity_automation_runs ADD COLUMN cancel_key TEXT;
ALTER TABLE crm_activity_automation_runs ADD COLUMN cancel_digest TEXT;
ALTER TABLE crm_activity_automation_runs ADD COLUMN cancel_request_id TEXT;
ALTER TABLE crm_activity_automation_runs ADD COLUMN cancelled_by TEXT;
ALTER TABLE crm_activity_automation_runs ADD COLUMN cancelled_at_unix_millis INTEGER;

-- Fence the original Task write by the exact currently leased request.
-- The earlier 0040 rule guard remains authoritative for revoked rules.
CREATE TRIGGER crm_activity_automation_task_lease_guard
BEFORE INSERT ON crm_activity_task_records
WHEN EXISTS(
 SELECT 1 FROM crm_activity_automation_runs a
 JOIN crm_activity_automation_rules r ON r.tenant=a.tenant AND r.id=a.rule_id
 WHERE a.tenant=NEW.tenant AND a.task_id=NEW.id AND r.enabled=1 AND r.version=a.rule_version
 AND (a.cancel_request_id IS NOT NULL OR a.lease_request_id IS NOT NEW.request_id
      OR a.lease_actor IS NOT NEW.actor OR a.lease_until_unix_millis<=unixepoch('now')*1000)
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_TASK_LEASE_REQUIRED'); END;

CREATE TRIGGER crm_activity_automation_attempt_audit
AFTER UPDATE OF attempt_count ON crm_activity_automation_runs
WHEN NEW.attempt_count>OLD.attempt_count
BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'activities.automation.run.attempt',
  NEW.task_id,datetime('now'),NEW.lease_request_id,NEW.lease_actor);
END;

-- SQLite serializes these writes with the original Task/receipt statement.
-- Cancellation may win during an in-flight attempt, but never after a Task
-- or its receipt exists. No new Task may be inserted after cancellation.
CREATE TRIGGER crm_activity_automation_cancel_guard
BEFORE UPDATE OF cancel_request_id,cancel_key,cancel_digest,cancelled_by,cancelled_at_unix_millis
ON crm_activity_automation_runs
WHEN OLD.cancel_request_id IS NOT NULL OR NEW.cancel_request_id IS NULL
 OR OLD.status<>'pending' OR NEW.status<>'pending'
 OR NEW.cancel_key IS NULL OR length(NEW.cancel_key) NOT BETWEEN 8 AND 120
 OR NEW.cancel_digest IS NULL OR length(NEW.cancel_digest)<>64
 OR NEW.cancelled_by IS NULL OR NEW.cancelled_at_unix_millis IS NULL
 OR EXISTS(SELECT 1 FROM crm_activity_task_records t WHERE t.tenant=OLD.tenant AND t.id=OLD.task_id)
 OR EXISTS(SELECT 1 FROM crm_activity_task_receipts t WHERE t.tenant=OLD.tenant AND t.key=OLD.task_id)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_CANCEL_NOT_ALLOWED'); END;

CREATE TRIGGER crm_activity_automation_cancel_receipt
AFTER UPDATE OF cancel_request_id ON crm_activity_automation_runs
WHEN OLD.cancel_request_id IS NULL AND NEW.cancel_request_id IS NOT NULL
BEGIN
 INSERT INTO crm_activity_automation_run_requests(tenant,key,rule_id,source_event_id,digest)
 VALUES(NEW.tenant,NEW.cancel_key,NEW.rule_id,NEW.source_event_id,NEW.cancel_digest);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'activities.automation.run.cancel',
  NEW.task_id,datetime('now'),NEW.cancel_request_id,NEW.cancelled_by);
END;

CREATE TRIGGER crm_activity_automation_cancelled_commit_guard
BEFORE UPDATE OF status ON crm_activity_automation_runs
WHEN OLD.cancel_request_id IS NOT NULL AND NEW.status<>'pending'
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RUN_CANCELLED'); END;

-- All lifecycle updates retain the same immutable source and Task intent.
CREATE TRIGGER crm_activity_automation_run_intent_immutable
BEFORE UPDATE ON crm_activity_automation_runs
WHEN NEW.seq IS NOT OLD.seq OR NEW.tenant IS NOT OLD.tenant
 OR NEW.rule_id IS NOT OLD.rule_id OR NEW.rule_version IS NOT OLD.rule_version
 OR NEW.source_event_id IS NOT OLD.source_event_id OR NEW.interaction_id IS NOT OLD.interaction_id
 OR NEW.task_id IS NOT OLD.task_id OR NEW.task_intent IS NOT OLD.task_intent
 OR NEW.run_request_key IS NOT OLD.run_request_key OR NEW.run_request_digest IS NOT OLD.run_request_digest
 OR NEW.requested_by IS NOT OLD.requested_by OR NEW.original_request_id IS NOT OLD.original_request_id
 OR NEW.created_at_unix_millis IS NOT OLD.created_at_unix_millis
 OR NEW.attempt_count<OLD.attempt_count OR NEW.attempt_count>OLD.attempt_count+1
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RUN_INTENT_IMMUTABLE'); END;

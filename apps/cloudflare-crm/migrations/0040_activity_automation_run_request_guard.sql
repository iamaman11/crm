-- Forward-only: bind every run request key to exactly one immutable intent.
-- Multiple explicit requests may observe the same logical rule/source run.
-- A historical ambiguous key fails this migration rather than losing evidence.
CREATE TABLE crm_activity_automation_run_requests (
 tenant TEXT NOT NULL,
 key TEXT NOT NULL CHECK(length(key) BETWEEN 8 AND 120),
 rule_id TEXT NOT NULL,
 source_event_id TEXT NOT NULL,
 digest TEXT NOT NULL CHECK(length(digest)=64),
 PRIMARY KEY(tenant,key),
 FOREIGN KEY(tenant,rule_id,source_event_id)
  REFERENCES crm_activity_automation_runs(tenant,rule_id,source_event_id)
);
INSERT INTO crm_activity_automation_run_requests(tenant,key,rule_id,source_event_id,digest)
 SELECT tenant,run_request_key,rule_id,source_event_id,run_request_digest
 FROM crm_activity_automation_runs;

-- Initial claim and its first request binding are atomic.
CREATE TRIGGER crm_activity_automation_run_request_guard
BEFORE INSERT ON crm_activity_automation_runs
WHEN EXISTS(
 SELECT 1 FROM crm_activity_automation_run_requests k
 WHERE k.tenant=NEW.tenant AND k.key=NEW.run_request_key
 AND (k.rule_id<>NEW.rule_id OR k.source_event_id<>NEW.source_event_id
      OR k.digest<>NEW.run_request_digest)
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RUN_REQUEST_CONFLICT'); END;

CREATE TRIGGER crm_activity_automation_run_request_bind
AFTER INSERT ON crm_activity_automation_runs
BEGIN
 INSERT INTO crm_activity_automation_run_requests(tenant,key,rule_id,source_event_id,digest)
 VALUES(NEW.tenant,NEW.run_request_key,NEW.rule_id,NEW.source_event_id,NEW.run_request_digest);
END;

CREATE TRIGGER crm_activity_automation_run_request_immutable
BEFORE UPDATE ON crm_activity_automation_run_requests
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RUN_REQUEST_IMMUTABLE'); END;

-- The original Task owner still creates the Task and its receipt. The
-- automation rule CAS must hold at that exact D1 insert, not at an earlier
-- asynchronous Rust read. Existing Task receipt replay performs no insert.
CREATE TRIGGER crm_activity_automation_task_rule_guard
BEFORE INSERT ON crm_activity_task_records
WHEN EXISTS(
 SELECT 1 FROM crm_activity_automation_runs a
 WHERE a.tenant=NEW.tenant AND a.task_id=NEW.id
 AND (a.status<>'pending' OR NEW.idem_key<>a.task_id OR NOT EXISTS(
  SELECT 1 FROM crm_activity_automation_rules r
  WHERE r.tenant=a.tenant AND r.id=a.rule_id
  AND r.version=a.rule_version AND r.enabled=1
 ))
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RULE_NOT_ENABLED_OR_STALE'); END;

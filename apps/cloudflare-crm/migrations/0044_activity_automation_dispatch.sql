-- SK-08A.3: extend the existing Activities rule/run ledger to the two
-- autonomous source types. Keep one Worker, one D1, one Task owner and one
-- run/request/receipt ledger. The old trigger column is retained as a v2
-- compatibility sentinel; trigger_kind is the canonical typed trigger.

ALTER TABLE crm_activity_automation_rules
 ADD COLUMN trigger_kind TEXT NOT NULL DEFAULT 'interaction_created'
 CHECK(trigger_kind IN ('interaction_created','deal_created','task_overdue'));
ALTER TABLE crm_activity_automation_rules
 ADD COLUMN published_at_unix_nanos INTEGER NOT NULL DEFAULT 0
 CHECK(published_at_unix_nanos>=0);
UPDATE crm_activity_automation_rules
 SET published_at_unix_nanos=
   CAST(strftime('%s','now') AS INTEGER)*1000000000+
   CAST(substr(strftime('%f','now'),4,3) AS INTEGER)*1000000
 WHERE published_at_unix_nanos=0;

DROP TRIGGER crm_activity_automation_rule_insert_receipt;
DROP TRIGGER crm_activity_automation_rule_update_receipt;

CREATE TRIGGER crm_activity_automation_rule_insert_receipt
AFTER INSERT ON crm_activity_automation_rules
BEGIN
 INSERT INTO crm_activity_automation_rule_versions(tenant,rule_id,version,state,actor,request_id)
 VALUES(NEW.tenant,NEW.id,NEW.version,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger_kind,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds,
   'published_at_unix_nanos',NEW.published_at_unix_nanos),
  NEW.actor,NEW.request_id);
 INSERT INTO crm_activity_automation_rule_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger_kind,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds,
   'published_at_unix_nanos',NEW.published_at_unix_nanos),NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
  'activities.automation.rule.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_activity_automation_rule_update_receipt
AFTER UPDATE ON crm_activity_automation_rules WHEN NEW.version > OLD.version
BEGIN
 SELECT CASE WHEN NEW.version<>OLD.version+1
 THEN RAISE(ABORT,'AUTOMATION_RULE_VERSION_GAP') END;
 INSERT INTO crm_activity_automation_rule_versions(tenant,rule_id,version,state,actor,request_id)
 VALUES(NEW.tenant,NEW.id,NEW.version,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger_kind,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds,
   'published_at_unix_nanos',NEW.published_at_unix_nanos),
  NEW.actor,NEW.request_id);
 INSERT INTO crm_activity_automation_rule_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger_kind,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds,
   'published_at_unix_nanos',NEW.published_at_unix_nanos),NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
  'activities.automation.rule.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

-- The original 0039/0040/0041 run table required every source to be an
-- Interaction. Rebuild that SAME ledger (and its child request bindings)
-- so the exact same lease, retry, cancellation and receipt reconciliation
-- applies to Interaction, Deal-created and Task-overdue sources.
DROP TRIGGER crm_activity_automation_run_claim_audit;
DROP TRIGGER crm_activity_automation_run_commit_guard;
DROP TRIGGER crm_activity_automation_run_commit_audit;
DROP TRIGGER crm_activity_automation_run_request_guard;
DROP TRIGGER crm_activity_automation_run_request_bind;
DROP TRIGGER crm_activity_automation_run_request_immutable;
DROP TRIGGER crm_activity_automation_task_rule_guard;
DROP TRIGGER crm_activity_automation_task_lease_guard;
DROP TRIGGER crm_activity_automation_attempt_audit;
DROP TRIGGER crm_activity_automation_cancel_guard;
DROP TRIGGER crm_activity_automation_cancel_receipt;
DROP TRIGGER crm_activity_automation_cancelled_commit_guard;
DROP TRIGGER crm_activity_automation_run_intent_immutable;

ALTER TABLE crm_activity_automation_run_requests
 RENAME TO crm_activity_automation_run_requests_legacy;
ALTER TABLE crm_activity_automation_runs
 RENAME TO crm_activity_automation_runs_legacy;

CREATE TABLE crm_activity_automation_runs (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 rule_id TEXT NOT NULL,
 rule_version INTEGER NOT NULL CHECK(rule_version>0),
 source_event_id TEXT NOT NULL CHECK(length(source_event_id) BETWEEN 1 AND 180),
 source_type TEXT NOT NULL CHECK(source_type IN ('interaction_created','deal_created','task_overdue')),
 source_id TEXT NOT NULL CHECK(length(source_id) BETWEEN 1 AND 180),
 source_version INTEGER NOT NULL CHECK(source_version>0),
 source_checked_at_unix_nanos INTEGER NOT NULL DEFAULT 0 CHECK(source_checked_at_unix_nanos>=0),
 interaction_id TEXT,
 task_id TEXT NOT NULL,
 task_intent TEXT NOT NULL CHECK(json_valid(task_intent)),
 run_request_key TEXT NOT NULL CHECK(length(run_request_key) BETWEEN 8 AND 120),
 run_request_digest TEXT NOT NULL CHECK(length(run_request_digest)=64),
 status TEXT NOT NULL CHECK(status IN ('pending','committed')),
 requested_by TEXT NOT NULL CHECK(length(trim(requested_by))>0),
 original_request_id TEXT NOT NULL,
 task_request_id TEXT,
 created_at_unix_millis INTEGER NOT NULL,
 completed_at_unix_millis INTEGER,
 attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 5),
 lease_request_id TEXT,
 lease_actor TEXT,
 lease_until_unix_millis INTEGER NOT NULL DEFAULT 0,
 next_attempt_at_unix_millis INTEGER NOT NULL DEFAULT 0,
 cancel_key TEXT,
 cancel_digest TEXT,
 cancel_request_id TEXT,
 cancelled_by TEXT,
 cancelled_at_unix_millis INTEGER,
 source_stale INTEGER NOT NULL DEFAULT 0 CHECK(source_stale IN (0,1)),
 UNIQUE(tenant,rule_id,source_event_id),
 UNIQUE(tenant,rule_id,source_type,source_id,rule_version),
 UNIQUE(tenant,task_id),
 FOREIGN KEY(tenant,rule_id) REFERENCES crm_activity_automation_rules(tenant,id)
);

INSERT INTO crm_activity_automation_runs(
 seq,tenant,rule_id,rule_version,source_event_id,source_type,source_id,
 source_version,interaction_id,task_id,task_intent,run_request_key,
 run_request_digest,status,requested_by,original_request_id,task_request_id,
 created_at_unix_millis,completed_at_unix_millis,attempt_count,lease_request_id,
 lease_actor,lease_until_unix_millis,next_attempt_at_unix_millis,cancel_key,
 cancel_digest,cancel_request_id,cancelled_by,cancelled_at_unix_millis,source_stale
)
SELECT seq,tenant,rule_id,rule_version,source_event_id,'interaction_created',
 interaction_id,COALESCE((SELECT source_version FROM crm_activity_automation_source_events s
  WHERE s.tenant=old.tenant AND s.event_id=old.source_event_id),1),
 interaction_id,task_id,task_intent,run_request_key,
 run_request_digest,status,requested_by,original_request_id,task_request_id,
 created_at_unix_millis,completed_at_unix_millis,attempt_count,lease_request_id,
 lease_actor,lease_until_unix_millis,next_attempt_at_unix_millis,cancel_key,
 cancel_digest,cancel_request_id,cancelled_by,cancelled_at_unix_millis,0
FROM crm_activity_automation_runs_legacy old;

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
 SELECT tenant,key,rule_id,source_event_id,digest
 FROM crm_activity_automation_run_requests_legacy;

DROP TABLE crm_activity_automation_run_requests_legacy;
DROP TABLE crm_activity_automation_runs_legacy;

CREATE INDEX idx_crm_activity_automation_run_tenant_seq
 ON crm_activity_automation_runs(tenant,seq);
CREATE INDEX idx_crm_activity_automation_run_source
 ON crm_activity_automation_runs(tenant,rule_id,source_type,source_id);

CREATE TRIGGER crm_activity_automation_run_claim_audit
AFTER INSERT ON crm_activity_automation_runs
BEGIN
 SELECT CASE WHEN NEW.status<>'pending'
 THEN RAISE(ABORT,'AUTOMATION_RUN_MUST_START_PENDING') END;
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   'activities.automation.run.claim',NEW.task_id,datetime('now'),
   NEW.original_request_id,NEW.requested_by);
END;

CREATE TRIGGER crm_activity_automation_run_commit_guard
BEFORE UPDATE ON crm_activity_automation_runs
WHEN NEW.status='committed'
BEGIN
 SELECT CASE WHEN OLD.status<>'pending'
  OR NEW.task_id IS NOT OLD.task_id
  OR NEW.task_intent IS NOT OLD.task_intent
  OR NEW.run_request_key IS NOT OLD.run_request_key
  OR NEW.run_request_digest IS NOT OLD.run_request_digest
  OR NEW.requested_by IS NOT OLD.requested_by
  OR NEW.original_request_id IS NOT OLD.original_request_id
  OR NEW.interaction_id IS NOT OLD.interaction_id
  OR NEW.source_type IS NOT OLD.source_type
  OR NEW.source_id IS NOT OLD.source_id
  OR NEW.source_version IS NOT OLD.source_version
  OR NEW.source_checked_at_unix_nanos IS NOT OLD.source_checked_at_unix_nanos
  OR NEW.source_stale<>0
  OR NEW.rule_version IS NOT OLD.rule_version
  OR NEW.source_event_id IS NOT OLD.source_event_id
  OR NEW.task_request_id IS NULL
  OR NOT EXISTS(
   SELECT 1 FROM crm_activity_task_records t
   WHERE t.tenant=NEW.tenant AND t.id=NEW.task_id
  )
  OR NOT EXISTS(
   SELECT 1 FROM crm_activity_task_receipts r
   WHERE r.tenant=NEW.tenant AND r.key=NEW.task_id
   AND r.original_request_id=NEW.task_request_id
  )
 THEN RAISE(ABORT,'AUTOMATION_TASK_RECEIPT_REQUIRED') END;
END;

CREATE TRIGGER crm_activity_automation_run_commit_audit
AFTER UPDATE ON crm_activity_automation_runs
WHEN OLD.status='pending' AND NEW.status='committed'
BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   'activities.automation.run.committed',NEW.task_id,datetime('now'),
   NEW.task_request_id,NEW.requested_by);
END;

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

-- The same original Task owner remains the only writer. A scheduled Task is
-- accepted only while the exact overdue source version is still open and due.
CREATE TRIGGER crm_activity_automation_task_rule_guard
BEFORE INSERT ON crm_activity_task_records
WHEN EXISTS(
 SELECT 1 FROM crm_activity_automation_runs a
 WHERE a.tenant=NEW.tenant AND a.task_id=NEW.id
 AND (a.status<>'pending' OR a.source_stale=1 OR NEW.idem_key<>a.task_id OR NOT EXISTS(
  SELECT 1 FROM crm_activity_automation_rules r
  WHERE r.tenant=a.tenant AND r.id=a.rule_id
  AND r.version=a.rule_version AND r.enabled=1
 ))
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RULE_NOT_ENABLED_OR_STALE'); END;

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

CREATE TRIGGER crm_activity_automation_task_overdue_source_guard
BEFORE INSERT ON crm_activity_task_records
WHEN EXISTS(
 SELECT 1 FROM crm_activity_automation_runs a
 WHERE a.tenant=NEW.tenant AND a.task_id=NEW.id AND a.source_type='task_overdue'
 AND NOT EXISTS(
  SELECT 1 FROM crm_activity_task_records source
  WHERE source.tenant=a.tenant AND source.id=a.source_id
   AND source.version=a.source_version
   AND json_extract(source.state,'$.status')='open'
   AND json_type(source.state,'$.due_at_unix_nanos')='integer'
   AND CAST(json_extract(source.state,'$.due_at_unix_nanos') AS INTEGER)
       <= a.source_checked_at_unix_nanos
 )
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_TASK_SOURCE_STALE'); END;

CREATE TRIGGER crm_activity_automation_deal_source_guard
BEFORE INSERT ON crm_activity_task_records
WHEN EXISTS(
 SELECT 1 FROM crm_activity_automation_runs a
 WHERE a.tenant=NEW.tenant AND a.task_id=NEW.id AND a.source_type='deal_created'
 AND NOT EXISTS(
  SELECT 1 FROM crm_sales_deal_events e
  WHERE e.tenant=a.tenant AND e.deal_id=a.source_id
   AND e.deal_version=a.source_version AND e.action='created'
 )
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_DEAL_SOURCE_MISSING'); END;

CREATE TRIGGER crm_activity_automation_attempt_audit
AFTER UPDATE OF attempt_count ON crm_activity_automation_runs
WHEN NEW.attempt_count>OLD.attempt_count
BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'activities.automation.run.attempt',
  NEW.task_id,datetime('now'),NEW.lease_request_id,NEW.lease_actor);
END;

CREATE TRIGGER crm_activity_automation_cancel_guard
BEFORE UPDATE OF cancel_request_id,cancel_key,cancel_digest,cancelled_by,cancelled_at_unix_millis
ON crm_activity_automation_runs
WHEN OLD.cancel_request_id IS NOT NULL OR NEW.cancel_request_id IS NULL
 OR OLD.status<>'pending' OR NEW.status<>'pending'
 OR NEW.source_stale=1
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

CREATE TRIGGER crm_activity_automation_run_source_stale_audit
AFTER UPDATE OF source_stale ON crm_activity_automation_runs
WHEN OLD.source_stale=0 AND NEW.source_stale=1
BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'activities.automation.run.source_stale',
  NEW.task_id,datetime('now'),NEW.original_request_id,NEW.requested_by);
END;

CREATE TRIGGER crm_activity_automation_run_intent_immutable
BEFORE UPDATE ON crm_activity_automation_runs
WHEN NEW.seq IS NOT OLD.seq OR NEW.tenant IS NOT OLD.tenant
 OR NEW.rule_id IS NOT OLD.rule_id OR NEW.rule_version IS NOT OLD.rule_version
 OR NEW.source_event_id IS NOT OLD.source_event_id OR NEW.source_type IS NOT OLD.source_type
 OR NEW.source_id IS NOT OLD.source_id OR NEW.source_version IS NOT OLD.source_version
 OR NEW.source_checked_at_unix_nanos IS NOT OLD.source_checked_at_unix_nanos
 OR NEW.interaction_id IS NOT OLD.interaction_id
 OR NEW.task_id IS NOT OLD.task_id OR NEW.task_intent IS NOT OLD.task_intent
 OR NEW.run_request_key IS NOT OLD.run_request_key OR NEW.run_request_digest IS NOT OLD.run_request_digest
 OR NEW.requested_by IS NOT OLD.requested_by OR NEW.original_request_id IS NOT OLD.original_request_id
 OR NEW.created_at_unix_millis IS NOT OLD.created_at_unix_millis
 OR NEW.attempt_count<OLD.attempt_count OR NEW.attempt_count>OLD.attempt_count+1
 OR NEW.source_stale<OLD.source_stale OR NEW.source_stale>1
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RUN_INTENT_IMMUTABLE'); END;

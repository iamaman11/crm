-- SK-08A/2: the existing Activities module owns one rule and run ledger.
-- No scripts, dynamic SQL rule bodies, runtime ACLs, extra Worker or second D1.
-- 0037/0038 remain immutable; this is the forward-only next migration.
CREATE TABLE crm_activity_automation_rules (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 trigger TEXT NOT NULL CHECK(trigger='interaction_created'),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 task_subject TEXT NOT NULL CHECK(length(trim(task_subject)) BETWEEN 1 AND 240),
 assignee_email TEXT NOT NULL CHECK(length(trim(assignee_email)) BETWEEN 3 AND 254),
 due_after_seconds INTEGER NOT NULL CHECK(due_after_seconds BETWEEN 0 AND 31536000),
 idem_key TEXT NOT NULL CHECK(length(idem_key) BETWEEN 8 AND 120),
 idem_digest TEXT NOT NULL CHECK(length(idem_digest)=64),
 request_id TEXT NOT NULL CHECK(length(request_id)>0),
 actor TEXT NOT NULL CHECK(length(actor)>0),
 PRIMARY KEY(tenant,id)
);
CREATE INDEX idx_crm_automation_rules_tenant_order
 ON crm_activity_automation_rules(tenant,id);

CREATE TABLE crm_activity_automation_rule_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TABLE crm_activity_automation_rule_versions (
 tenant TEXT NOT NULL,
 rule_id TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version>0),
 state TEXT NOT NULL CHECK(json_valid(state)),
 actor TEXT NOT NULL,
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,rule_id,version),
 FOREIGN KEY(tenant,rule_id) REFERENCES crm_activity_automation_rules(tenant,id)
);

-- Existing Access member table is the single human-authority registry. A
-- disabled rule can survive assignee revocation so admins can repair it.
CREATE TRIGGER crm_activity_automation_rule_assignee_insert
BEFORE INSERT ON crm_activity_automation_rules
WHEN NEW.enabled=1 AND NOT EXISTS(
 SELECT 1 FROM crm_users WHERE tenant_id=NEW.tenant
 AND email=NEW.assignee_email AND active=1 AND role IN ('admin','viewer')
 AND email NOT LIKE 'service:%'
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_ASSIGNEE_NOT_ACTIVE'); END;
CREATE TRIGGER crm_activity_automation_rule_assignee_update
BEFORE UPDATE ON crm_activity_automation_rules
WHEN NEW.enabled=1 AND NOT EXISTS(
 SELECT 1 FROM crm_users WHERE tenant_id=NEW.tenant
 AND email=NEW.assignee_email AND active=1 AND role IN ('admin','viewer')
 AND email NOT LIKE 'service:%'
)
BEGIN SELECT RAISE(ABORT,'AUTOMATION_ASSIGNEE_NOT_ACTIVE'); END;

CREATE TRIGGER crm_activity_automation_rule_insert_receipt
AFTER INSERT ON crm_activity_automation_rules
BEGIN
 INSERT INTO crm_activity_automation_rule_versions(tenant,rule_id,version,state,actor,request_id)
 VALUES(NEW.tenant,NEW.id,NEW.version,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds),
  NEW.actor,NEW.request_id);
 INSERT INTO crm_activity_automation_rule_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds),NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
  'activities.automation.rule.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_activity_automation_rule_version_guard
BEFORE UPDATE ON crm_activity_automation_rules
WHEN NEW.version<>OLD.version+1 OR NEW.id IS NOT OLD.id
 OR NEW.tenant IS NOT OLD.tenant
BEGIN SELECT RAISE(ABORT,'AUTOMATION_RULE_VERSION_CONFLICT'); END;
CREATE TRIGGER crm_activity_automation_rule_update_receipt
AFTER UPDATE ON crm_activity_automation_rules WHEN NEW.version > OLD.version
BEGIN
 SELECT CASE WHEN NEW.version<>OLD.version+1
 THEN RAISE(ABORT,'AUTOMATION_RULE_VERSION_GAP') END;
 INSERT INTO crm_activity_automation_rule_versions(tenant,rule_id,version,state,actor,request_id)
 VALUES(NEW.tenant,NEW.id,NEW.version,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds),
  NEW.actor,NEW.request_id);
 INSERT INTO crm_activity_automation_rule_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,
  json_object('rule_id',NEW.id,'version',NEW.version,'trigger',NEW.trigger,
   'enabled',NEW.enabled,'task_subject',NEW.task_subject,
   'assignee_email',NEW.assignee_email,'due_after_seconds',NEW.due_after_seconds),NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
  'activities.automation.rule.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

-- The signed administrative runner snapshots the COMPLETE deterministic Task
-- CreateInput BEFORE invoking the original Task owner. A lost acknowledgement
-- or rule revision must not change the Task mutation intent on replay.
CREATE TABLE crm_activity_automation_runs (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 rule_id TEXT NOT NULL,
 rule_version INTEGER NOT NULL CHECK(rule_version>0),
 source_event_id TEXT NOT NULL,
 interaction_id TEXT NOT NULL,
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
 UNIQUE(tenant,rule_id,source_event_id),
 UNIQUE(tenant,task_id),
 FOREIGN KEY(tenant,rule_id) REFERENCES crm_activity_automation_rules(tenant,id),
 FOREIGN KEY(tenant,source_event_id) REFERENCES crm_activity_automation_source_events(tenant,event_id),
 FOREIGN KEY(tenant,interaction_id) REFERENCES crm_activity_interactions(tenant,id)
);
CREATE INDEX idx_crm_activity_automation_run_tenant_seq
 ON crm_activity_automation_runs(tenant,seq);
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
-- A run never becomes committed merely because an RPC returned 200. The
-- original Task and its SAME-transaction receipt must exist in physical D1.
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

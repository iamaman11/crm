-- Preserve accepted 0002. Classify the committed transition, not any SQL UPDATE.
-- The existing task table atomically writes state, receipt and audit in this trigger.
DROP TRIGGER IF EXISTS crm_activity_task_update_audit;
CREATE TRIGGER crm_activity_task_update_audit
AFTER UPDATE ON crm_activity_task_records BEGIN
  INSERT INTO crm_activity_task_receipts(tenant,key,digest,state)
    VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
  INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
    VALUES(
      lower(hex(randomblob(16))),
      NEW.tenant,
      CASE
        WHEN json_extract(OLD.state,'$.status')='open'
         AND json_extract(NEW.state,'$.status')='completed'
        THEN 'activity_tasks.complete'
        ELSE 'activity_tasks.update'
      END,
      NEW.id,
      datetime('now'),
      NEW.request_id
    );
END;

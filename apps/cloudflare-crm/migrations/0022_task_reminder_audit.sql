-- M1-08: preserve the original ScheduleReminder event on the SAME Task
-- update trigger, receipt and D1 transaction. Older migrations are immutable.
-- A same-time no-op never UPDATEs the row, so creates no receipt or audit.
DROP TRIGGER crm_activity_task_update_audit;
CREATE TRIGGER crm_activity_task_update_audit
AFTER UPDATE ON crm_activity_task_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_activity_task_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(
    lower(hex(randomblob(16))),
    NEW.tenant,
    CASE
      WHEN json_extract(OLD.state,'$.status')='open'
       AND json_extract(NEW.state,'$.status')='completed'
        THEN 'activity_tasks.complete'
      WHEN json_extract(OLD.state,'$.reminder_at_unix_nanos')
           IS NOT json_extract(NEW.state,'$.reminder_at_unix_nanos')
       AND json_extract(OLD.state,'$.status')='open'
       AND json_extract(NEW.state,'$.status')='open'
        THEN 'activity_tasks.reminder_scheduled'
      ELSE 'activity_tasks.update'
    END,
    NEW.id,datetime('now'),NEW.request_id,NEW.actor
  );
END;

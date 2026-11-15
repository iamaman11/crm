-- Verified actor attribution for existing Account owner. Historical audit rows stay NULL.
ALTER TABLE crm_account_records ADD COLUMN actor TEXT;
DROP TRIGGER crm_account_create_audit;
DROP TRIGGER crm_account_update_audit;
CREATE TRIGGER crm_account_create_audit
AFTER INSERT ON crm_account_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACCOUNT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_account_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_account_update_audit
AFTER UPDATE ON crm_account_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACCOUNT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_account_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

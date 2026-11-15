-- Existing ContactPoint owner writes signed Access attribution atomically (#407).
-- Historical audit rows remain actor NULL; earlier migrations are immutable.
ALTER TABLE crm_contact_point_records ADD COLUMN actor TEXT;
DROP TRIGGER crm_contact_point_create_audit;
DROP TRIGGER crm_contact_point_update_audit;
CREATE TRIGGER crm_contact_point_create_audit
AFTER INSERT ON crm_contact_point_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'CONTACT_POINT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'contact_points.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_contact_point_update_audit
AFTER UPDATE ON crm_contact_point_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'CONTACT_POINT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'contact_points.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

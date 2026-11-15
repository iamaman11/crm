-- Historical rows and owners not yet migrated retain unknown attribution.
-- Only the verified server context supplies the actor for new Party writes.
ALTER TABLE crm_audit ADD COLUMN actor TEXT;
ALTER TABLE crm_party_records ADD COLUMN actor TEXT;

DROP TRIGGER crm_party_create_audit;
DROP TRIGGER crm_party_update_audit;

CREATE TRIGGER crm_party_create_audit
AFTER INSERT ON crm_party_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
   THEN RAISE(ABORT, 'PARTY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_party_update_audit
AFTER UPDATE ON crm_party_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
   THEN RAISE(ABORT, 'PARTY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

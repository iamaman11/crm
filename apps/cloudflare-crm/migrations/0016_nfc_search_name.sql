-- #471 S2: NFC search projection, atomically owned by existing Rust writers.
-- Deploy verifies all pre-existing display names already are NFC before copying.
-- Backfill derived columns only, with no synthetic business audit or receipt.
ALTER TABLE crm_party_records ADD COLUMN search_name TEXT NOT NULL DEFAULT '';
DROP TRIGGER crm_party_create_audit;
DROP TRIGGER crm_party_update_audit;
UPDATE crm_party_records SET search_name=json_extract(state,'$.display_name');
CREATE TRIGGER crm_party_create_audit
AFTER INSERT ON crm_party_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
   THEN RAISE(ABORT, 'PARTY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_party_update_audit
AFTER UPDATE ON crm_party_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
   THEN RAISE(ABORT, 'PARTY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

ALTER TABLE crm_account_records ADD COLUMN search_name TEXT NOT NULL DEFAULT '';
DROP TRIGGER crm_account_create_audit;
DROP TRIGGER crm_account_update_audit;
UPDATE crm_account_records SET search_name=json_extract(state,'$.name');
CREATE TRIGGER crm_account_create_audit
AFTER INSERT ON crm_account_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACCOUNT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_account_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_account_update_audit
AFTER UPDATE ON crm_account_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACCOUNT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_account_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

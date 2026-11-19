-- #471 S5a: original request provenance in the existing atomic owner receipts.
-- A historical receipt has no trustworthy request ID: leave its new column NULL,
-- never reconstruct a committed request from mutable record state or audit order.
-- Same native Worker, same D1, original Rust owners and unchanged receipt digests.
ALTER TABLE crm_party_receipts ADD COLUMN original_request_id TEXT;
ALTER TABLE crm_account_receipts ADD COLUMN original_request_id TEXT;
ALTER TABLE crm_contact_point_receipts ADD COLUMN original_request_id TEXT;
ALTER TABLE crm_party_relationship_receipts ADD COLUMN original_request_id TEXT;
ALTER TABLE crm_activity_task_receipts ADD COLUMN original_request_id TEXT;
DROP TRIGGER crm_party_create_audit;
DROP TRIGGER crm_party_update_audit;
DROP TRIGGER crm_account_create_audit;
DROP TRIGGER crm_account_update_audit;
DROP TRIGGER crm_contact_point_create_audit;
DROP TRIGGER crm_contact_point_update_audit;
DROP TRIGGER crm_party_relationship_create_audit;
DROP TRIGGER crm_party_relationship_update_audit;
DROP TRIGGER crm_activity_task_create_audit;
DROP TRIGGER crm_activity_task_update_audit;

CREATE TRIGGER crm_party_create_audit
AFTER INSERT ON crm_party_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
   THEN RAISE(ABORT, 'PARTY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_party_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_party_update_audit
AFTER UPDATE ON crm_party_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
   THEN RAISE(ABORT, 'PARTY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_party_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_account_create_audit
AFTER INSERT ON crm_account_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACCOUNT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_account_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_account_update_audit
AFTER UPDATE ON crm_account_records BEGIN
 SELECT CASE WHEN length(NEW.search_name)=0
 THEN RAISE(ABORT,'NFC_SEARCH_NAME_REQUIRED') END;
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACCOUNT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_account_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_contact_point_create_audit
AFTER INSERT ON crm_contact_point_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'CONTACT_POINT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'contact_points.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_contact_point_update_audit
AFTER UPDATE ON crm_contact_point_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'CONTACT_POINT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,
    datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_party_relationship_create_audit
AFTER INSERT ON crm_party_relationship_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_party_relationship_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'party_relationships.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_party_relationship_update_audit
AFTER UPDATE ON crm_party_relationship_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_party_relationship_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'party_relationships.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_activity_task_create_audit
AFTER INSERT ON crm_activity_task_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_activity_task_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'activity_tasks.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

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
        ELSE 'activity_tasks.update'
      END,
      NEW.id,
      datetime('now'),
      NEW.request_id,
      NEW.actor
    );
END;

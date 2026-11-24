-- #471 S5b: tenant-bound Consent DENY/GRANT/WITHDRAW provenance.
-- Historical receipts remain NULL and never certify a commit.
-- Existing signed-human GRANT authorization and operator audit rules unchanged.
ALTER TABLE crm_consent_receipts ADD COLUMN original_request_id TEXT;
DROP TRIGGER crm_consent_operator_suppression_audit;
DROP TRIGGER crm_consent_update_audit;
CREATE TRIGGER crm_consent_operator_suppression_audit
AFTER INSERT ON crm_consent_records BEGIN
 SELECT CASE WHEN NEW.operator_identity IS NULL OR length(trim(NEW.operator_identity))=0
  THEN RAISE(ABORT,'CONSENTS_SIGNED_ACTOR_REQUIRED') END;
 INSERT INTO crm_consent_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,
    datetime('now'),NEW.request_id,NEW.operator_identity);
END;
CREATE TRIGGER crm_consent_update_audit
AFTER UPDATE ON crm_consent_records BEGIN
 SELECT CASE WHEN NEW.operator_identity IS NULL OR length(trim(NEW.operator_identity))=0
  THEN RAISE(ABORT,'CONSENTS_SIGNED_ACTOR_REQUIRED') END;
 INSERT INTO crm_consent_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,
    datetime('now'),NEW.request_id,NEW.operator_identity);
END;

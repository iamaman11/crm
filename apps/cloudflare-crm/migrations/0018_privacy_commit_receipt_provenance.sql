-- #471 S5b Privacy: original commit provenance, atomic with the owner receipt.
-- Historical receipts are unverifiable and deliberately have NULL provenance.
ALTER TABLE crm_privacy_case_receipts ADD COLUMN original_request_id TEXT;
DROP TRIGGER crm_privacy_case_create_audit;
DROP TRIGGER crm_privacy_case_transition_audit;
CREATE TRIGGER crm_privacy_case_create_audit
AFTER INSERT ON crm_privacy_case_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'PRIVACY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_privacy.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_privacy_case_transition_audit
AFTER UPDATE ON crm_privacy_case_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'PRIVACY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        CASE WHEN json_extract(NEW.state,'$.status.code')='submitted'
             THEN 'customer_privacy.submit'
             ELSE 'customer_privacy.cancel' END,
        NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

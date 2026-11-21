-- R4a #352: one canonical audit label for the verified-human subject
-- transition. Preserve receipt provenance and all atomic trigger behavior.
-- Older applied migrations are immutable.
DROP TRIGGER crm_privacy_case_transition_audit;
CREATE TRIGGER crm_privacy_case_transition_audit
AFTER UPDATE ON crm_privacy_case_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'PRIVACY_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   CASE
     WHEN json_extract(OLD.state,'$.status.code')='draft'
      AND json_extract(NEW.state,'$.status.code')='submitted'
       THEN 'customer_privacy.submit'
     WHEN json_extract(OLD.state,'$.status.code')='submitted'
      AND json_extract(NEW.state,'$.status.code')='subject_verified'
       THEN 'customer_privacy.subject.verify_own'
     WHEN json_extract(OLD.state,'$.status.code') IN (
          'draft','submitted','subject_verified','scoping','scoped','planned',
          'awaiting_approval','rescope_required','failed_retryable')
      AND json_extract(NEW.state,'$.status.code')='cancelled'
       THEN 'customer_privacy.cancel'
     ELSE RAISE(ABORT,'PRIVACY_AUDIT_TRANSITION_UNRECOGNIZED')
   END,
   NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

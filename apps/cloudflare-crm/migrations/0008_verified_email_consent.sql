-- R3a: verified email self-consent and immediate revocation in the
-- existing ContactPoint and Consent D1 owners. Historical rows preserved.
-- A single original trigger per operation writes receipt + signed actor audit.
ALTER TABLE crm_contact_point_records
  ADD COLUMN last_action TEXT NOT NULL DEFAULT 'contact_points.update';
ALTER TABLE crm_consent_records
  ADD COLUMN last_action TEXT NOT NULL DEFAULT 'consents.operator_suppress';

DROP TRIGGER crm_contact_point_update_audit;
CREATE TRIGGER crm_contact_point_update_audit
AFTER UPDATE ON crm_contact_point_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'CONTACT_POINT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,
    datetime('now'),NEW.request_id,NEW.actor);
END;

DROP TRIGGER crm_consent_operator_suppression_audit;
CREATE TRIGGER crm_consent_operator_suppression_audit
AFTER INSERT ON crm_consent_records BEGIN
 SELECT CASE WHEN NEW.operator_identity IS NULL OR length(trim(NEW.operator_identity))=0
  THEN RAISE(ABORT,'CONSENTS_SIGNED_ACTOR_REQUIRED') END;
 INSERT INTO crm_consent_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,
    datetime('now'),NEW.request_id,NEW.operator_identity);
END;
CREATE TRIGGER crm_consent_update_audit
AFTER UPDATE ON crm_consent_records BEGIN
 SELECT CASE WHEN NEW.operator_identity IS NULL OR length(trim(NEW.operator_identity))=0
  THEN RAISE(ABORT,'CONSENTS_SIGNED_ACTOR_REQUIRED') END;
 INSERT INTO crm_consent_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,
    datetime('now'),NEW.request_id,NEW.operator_identity);
END;

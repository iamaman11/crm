-- Correlate a real native Party mutation with the existing D1 audit.
-- Existing rows remain untouched; this is the same CRM database.
ALTER TABLE crm_party_records ADD COLUMN request_id TEXT;
ALTER TABLE crm_audit ADD COLUMN request_id TEXT;
CREATE INDEX IF NOT EXISTS idx_crm_audit_request_id ON crm_audit(request_id);
DROP TRIGGER IF EXISTS crm_party_create_audit;
DROP TRIGGER IF EXISTS crm_party_update_audit;
CREATE TRIGGER crm_party_create_audit
AFTER INSERT ON crm_party_records BEGIN
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER crm_party_update_audit
AFTER UPDATE ON crm_party_records BEGIN
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.update',NEW.id,datetime('now'),NEW.request_id);
END;

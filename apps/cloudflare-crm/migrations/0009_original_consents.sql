-- Original crm-consents state. Explicit verified-operator DENY suppressions only;
-- no positive grant can enter this API without trusted subject evidence.
CREATE TABLE IF NOT EXISTS crm_consent_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 party_id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 operator_identity TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,party_id) REFERENCES crm_party_records(tenant,id)
);
CREATE INDEX IF NOT EXISTS idx_crm_consent_party
 ON crm_consent_records(tenant,party_id);
CREATE TABLE IF NOT EXISTS crm_consent_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
-- Same D1 statement commits state, idempotency receipt and honest audit.
CREATE TRIGGER IF NOT EXISTS crm_consent_operator_suppression_audit
AFTER INSERT ON crm_consent_records BEGIN
 INSERT INTO crm_consent_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'consents.operator_suppress',NEW.id,
         datetime('now'),NEW.request_id);
END;

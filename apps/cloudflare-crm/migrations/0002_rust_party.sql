-- Original Rust crm.parties state in the existing CRM D1. Additive only.
-- Party is identity; existing crm_customers are commercial test accounts.
CREATE TABLE IF NOT EXISTS crm_party_records(
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 PRIMARY KEY(tenant,id)
);
CREATE TABLE IF NOT EXISTS crm_party_receipts(
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_party_create_audit
AFTER INSERT ON crm_party_records BEGIN
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.create',NEW.id,datetime('now'));
END;
CREATE TRIGGER IF NOT EXISTS crm_party_update_audit
AFTER UPDATE ON crm_party_records BEGIN
 INSERT INTO crm_party_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'parties.update',NEW.id,datetime('now'));
END;

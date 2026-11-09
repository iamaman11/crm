-- Original crm.customer-accounts authoritative state, using the existing CRM D1.
-- New owner schema; crm_customers stays strictly legacy test pilot until final cutover.
CREATE TABLE IF NOT EXISTS crm_account_records (
  tenant TEXT NOT NULL REFERENCES crm_tenants(id),
  id TEXT NOT NULL,
  primary_party TEXT NOT NULL,
  state TEXT NOT NULL,
  version INTEGER NOT NULL CHECK(version > 0),
  idem_key TEXT NOT NULL,
  idem_digest TEXT NOT NULL,
  request_id TEXT NOT NULL,
  PRIMARY KEY (tenant, id),
  FOREIGN KEY (tenant, primary_party) REFERENCES crm_party_records(tenant, id)
);
CREATE INDEX IF NOT EXISTS idx_crm_account_primary_party
  ON crm_account_records(tenant, primary_party);
CREATE TABLE IF NOT EXISTS crm_account_receipts (
  tenant TEXT NOT NULL REFERENCES crm_tenants(id),
  key TEXT NOT NULL,
  digest TEXT NOT NULL,
  state TEXT NOT NULL,
  PRIMARY KEY (tenant, key)
);
-- SQLite triggers are in the SAME atomic D1 mutation as state writes:
-- duplicate receipt/id or missing tenant/Party aborts the mutation itself.
CREATE TRIGGER IF NOT EXISTS crm_account_create_audit
AFTER INSERT ON crm_account_records BEGIN
  INSERT INTO crm_account_receipts(tenant,key,digest,state)
    VALUES(NEW.tenant, NEW.idem_key, NEW.idem_digest, NEW.state);
  INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
    VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_account_update_audit
AFTER UPDATE ON crm_account_records BEGIN
  INSERT INTO crm_account_receipts(tenant,key,digest,state)
    VALUES(NEW.tenant, NEW.idem_key, NEW.idem_digest, NEW.state);
  INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
    VALUES(lower(hex(randomblob(16))),NEW.tenant,'accounts.update',NEW.id,datetime('now'),NEW.request_id);
END;

-- ORIGINAL crm-sales Deals: the same Worker, the same D1. Never crm_deals (pilot).
-- Exact domain state is serialized by crm-sales; money uses integer minor units.
CREATE TABLE IF NOT EXISTS crm_sales_deal_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 account_id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,account_id) REFERENCES crm_account_records(tenant,id)
);
CREATE INDEX IF NOT EXISTS idx_crm_sales_deals_account
 ON crm_sales_deal_records(tenant,account_id);
CREATE TABLE IF NOT EXISTS crm_sales_deal_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_sales_deal_create_audit
AFTER INSERT ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_deals.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_sales_deal_update_audit
AFTER UPDATE ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_deals.advance',NEW.id,datetime('now'),NEW.request_id);
END;

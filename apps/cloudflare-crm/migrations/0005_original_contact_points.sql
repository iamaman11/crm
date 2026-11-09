-- Original crm.contact-points aggregates. One Cloudflare D1, one Rust Worker.
-- A contact point belongs to crm.parties, never directly to legacy crm_customers.
CREATE TABLE IF NOT EXISTS crm_contact_point_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 party_id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,party_id) REFERENCES crm_party_records(tenant,id)
);
CREATE INDEX IF NOT EXISTS idx_crm_contact_points_party
 ON crm_contact_point_records(tenant,party_id);
CREATE TABLE IF NOT EXISTS crm_contact_point_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
-- Domain state, idempotency receipt, and request-correlated audit are atomic.
CREATE TRIGGER IF NOT EXISTS crm_contact_point_create_audit
AFTER INSERT ON crm_contact_point_records BEGIN
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'contact_points.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_contact_point_update_audit
AFTER UPDATE ON crm_contact_point_records BEGIN
 INSERT INTO crm_contact_point_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'contact_points.update',NEW.id,datetime('now'),NEW.request_id);
END;

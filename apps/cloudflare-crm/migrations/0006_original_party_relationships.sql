-- Authoritative original crm.party-relationships state in SAME CRM D1.
-- Both Party ends remain scoped to the Access-proven tenant.
CREATE TABLE IF NOT EXISTS crm_party_relationship_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 from_party TEXT NOT NULL,
 to_party TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,from_party) REFERENCES crm_party_records(tenant,id),
 FOREIGN KEY(tenant,to_party) REFERENCES crm_party_records(tenant,id)
);
CREATE INDEX IF NOT EXISTS idx_crm_relationship_from
 ON crm_party_relationship_records(tenant,from_party);
CREATE INDEX IF NOT EXISTS idx_crm_relationship_to
 ON crm_party_relationship_records(tenant,to_party);
CREATE TABLE IF NOT EXISTS crm_party_relationship_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_party_relationship_create_audit
AFTER INSERT ON crm_party_relationship_records BEGIN
 INSERT INTO crm_party_relationship_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'party_relationships.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_party_relationship_update_audit
AFTER UPDATE ON crm_party_relationship_records BEGIN
 INSERT INTO crm_party_relationship_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'party_relationships.update',NEW.id,datetime('now'),NEW.request_id);
END;

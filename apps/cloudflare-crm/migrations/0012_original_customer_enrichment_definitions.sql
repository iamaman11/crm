-- Original immutable Customer Enrichment provider/mapping definitions in one D1.
-- No external provider calls, credentials, suggestion dispatch or Party mutations.
CREATE TABLE IF NOT EXISTS crm_enrichment_profiles (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL,
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 created_by TEXT NOT NULL,
 PRIMARY KEY (tenant,id)
);
CREATE TABLE IF NOT EXISTS crm_enrichment_mappings (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 provider_profile_id TEXT NOT NULL,
 state TEXT NOT NULL,
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 created_by TEXT NOT NULL,
 PRIMARY KEY (tenant,id),
 FOREIGN KEY (tenant,provider_profile_id) REFERENCES crm_enrichment_profiles(tenant,id)
);
CREATE TABLE IF NOT EXISTS crm_enrichment_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('profile','mapping')),
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY (tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_enrichment_profile_audit
AFTER INSERT ON crm_enrichment_profiles BEGIN
 INSERT INTO crm_enrichment_receipts(tenant,key,kind,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,'profile',NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_enrichment.profile_publish',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_enrichment_mapping_audit
AFTER INSERT ON crm_enrichment_mappings BEGIN
 INSERT INTO crm_enrichment_receipts(tenant,key,kind,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,'mapping',NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_enrichment.mapping_publish',NEW.id,datetime('now'),NEW.request_id);
END;

-- Fresh CRM D1 bootstrap; all CRM records are disposable test data.
-- Later migrations evolve this bootstrap through the standard Wrangler ledger.
-- Apply to an EMPTY D1 after the explicit, one-time dev reset.
-- No pilot crm_customers/crm_contacts/crm_deals/crm_tasks tables,
-- no data copying, no legacy compatibility or rehydration.
PRAGMA foreign_keys = ON;
CREATE TABLE crm_tenants (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL
);
CREATE TABLE crm_users (
  email TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
  role TEXT NOT NULL DEFAULT 'admin',
  active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE crm_audit (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL REFERENCES crm_tenants(id),
  action TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  request_id TEXT
);
CREATE INDEX idx_crm_audit_tenant ON crm_audit(tenant_id, created_at);
CREATE INDEX idx_crm_audit_request_id ON crm_audit(request_id);

-- The existing original Rust Parties aggregate is the only Party owner.
CREATE TABLE crm_party_records(
  tenant TEXT NOT NULL REFERENCES crm_tenants(id),
  id TEXT NOT NULL,
  state TEXT NOT NULL,
  version INTEGER NOT NULL CHECK(version > 0),
  idem_key TEXT NOT NULL,
  idem_digest TEXT NOT NULL,
  request_id TEXT,
  PRIMARY KEY(tenant,id)
);
CREATE TABLE crm_party_receipts(
  tenant TEXT NOT NULL REFERENCES crm_tenants(id),
  key TEXT NOT NULL,
  digest TEXT NOT NULL,
  state TEXT NOT NULL,
  PRIMARY KEY(tenant,key)
);
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


-- original customer accounts (original Rust domain owner)
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


-- original contact points (original Rust domain owner)
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


-- original party relationships (original Rust domain owner)
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


-- original activity tasks (original Rust domain owner)
CREATE TABLE IF NOT EXISTS crm_activity_task_records (
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
CREATE INDEX IF NOT EXISTS idx_crm_activity_tasks_account
 ON crm_activity_task_records(tenant,account_id);
CREATE TABLE IF NOT EXISTS crm_activity_task_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_activity_task_create_audit
AFTER INSERT ON crm_activity_task_records BEGIN
 INSERT INTO crm_activity_task_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'activity_tasks.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_activity_task_update_audit
AFTER UPDATE ON crm_activity_task_records BEGIN
 INSERT INTO crm_activity_task_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'activity_tasks.complete',NEW.id,datetime('now'),NEW.request_id);
END;


-- original sales deals (original Rust domain owner)
CREATE TABLE IF NOT EXISTS crm_sales_deal_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 account_id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 last_action TEXT NOT NULL DEFAULT 'sales_deals.create'
   CHECK(last_action IN ('sales_deals.create','sales_deals.update','sales_deals.advance')),
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
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,datetime('now'),NEW.request_id);
END;


-- original consents (original Rust domain owner)
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
CREATE TRIGGER IF NOT EXISTS crm_consent_operator_suppression_audit
AFTER INSERT ON crm_consent_records BEGIN
 INSERT INTO crm_consent_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'consents.operator_suppress',NEW.id,
         datetime('now'),NEW.request_id);
END;


-- original sales activities link (original Rust domain owner)
CREATE TABLE IF NOT EXISTS crm_sales_link_delivery_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 delivery_id TEXT NOT NULL,
 event_id TEXT NOT NULL,
 deal_id TEXT NOT NULL,
 deal_version INTEGER NOT NULL CHECK(deal_version > 0),
 task_id TEXT,
 disposition TEXT NOT NULL CHECK(disposition IN ('applied','ignored')),
 state_json TEXT NOT NULL,
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,delivery_id),
 UNIQUE(tenant,deal_id,deal_version),
 FOREIGN KEY(tenant,deal_id) REFERENCES crm_sales_deal_records(tenant,id),
 FOREIGN KEY(tenant,task_id) REFERENCES crm_activity_task_records(tenant,id),
 CHECK((disposition='applied' AND task_id IS NOT NULL) OR
       (disposition='ignored' AND task_id IS NULL))
);
CREATE INDEX IF NOT EXISTS idx_crm_sales_link_deal
 ON crm_sales_link_delivery_records(tenant,deal_id,deal_version);
CREATE TRIGGER IF NOT EXISTS crm_sales_link_source_version
BEFORE INSERT ON crm_sales_link_delivery_records BEGIN
 SELECT CASE WHEN
  (SELECT version FROM crm_sales_deal_records
    WHERE tenant=NEW.tenant AND id=NEW.deal_id) IS NOT NEW.deal_version
 THEN RAISE(ABORT,'SALES_LINK_SOURCE_VERSION_CONFLICT') END;
END;
CREATE TRIGGER IF NOT EXISTS crm_sales_link_delivery_audit
AFTER INSERT ON crm_sales_link_delivery_records BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   CASE WHEN NEW.disposition='applied' THEN 'sales_activities_link.applied'
        ELSE 'sales_activities_link.ignored' END,
   NEW.delivery_id,datetime('now'),NEW.request_id);
END;


-- original customer privacy intake (original Rust domain owner)
CREATE TABLE IF NOT EXISTS crm_privacy_case_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 created_by TEXT NOT NULL,
 PRIMARY KEY(tenant,id)
);
CREATE TABLE IF NOT EXISTS crm_privacy_case_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER IF NOT EXISTS crm_privacy_case_create_audit
AFTER INSERT ON crm_privacy_case_records BEGIN
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_privacy.create',NEW.id,datetime('now'),NEW.request_id);
END;
CREATE TRIGGER IF NOT EXISTS crm_privacy_case_transition_audit
AFTER UPDATE ON crm_privacy_case_records BEGIN
 INSERT INTO crm_privacy_case_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        CASE WHEN json_extract(NEW.state,'$.status.code')='submitted'
             THEN 'customer_privacy.submit'
             ELSE 'customer_privacy.cancel' END,
        NEW.id,datetime('now'),NEW.request_id);
END;


-- original customer enrichment definitions (original Rust domain owner)
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

-- Bounded create-only import receipt, committed atomically with original Party rows.
-- This is business idempotency state in the same D1, not an import-job subsystem.
CREATE TABLE IF NOT EXISTS crm_party_import_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 source_sha256 TEXT NOT NULL,
 row_count INTEGER NOT NULL CHECK(row_count BETWEEN 1 AND 20),
 request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);

-- Original Sales quota CAS, atomic immutable revision, receipt and audit.
CREATE TABLE crm_sales_quota_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 version INTEGER NOT NULL CHECK(version>0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,id)
);
CREATE INDEX idx_crm_sales_quota_series ON crm_sales_quota_records(
 tenant,json_extract(state,'$.scope'),json_extract(state,'$.pipeline_id'),
 json_extract(state,'$.currency_code'),json_extract(state,'$.period.time_zone'),
 json_extract(state,'$.period.cadence')
) WHERE json_extract(state,'$.published')=1;
CREATE TABLE crm_sales_quota_revisions (
 tenant TEXT NOT NULL,
 id TEXT NOT NULL,
 version INTEGER NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 PRIMARY KEY(tenant,id,version),
 FOREIGN KEY(tenant,id) REFERENCES crm_sales_quota_records(tenant,id)
);
CREATE TABLE crm_sales_quota_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_sales_quota_create_audit AFTER INSERT ON crm_sales_quota_records BEGIN
 INSERT INTO crm_sales_quota_revisions(tenant,id,version,state) VALUES(NEW.tenant,NEW.id,NEW.version,NEW.state);
 INSERT INTO crm_sales_quota_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.quota.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_quota_update_audit AFTER UPDATE ON crm_sales_quota_records BEGIN
 INSERT INTO crm_sales_quota_revisions(tenant,id,version,state) VALUES(NEW.tenant,NEW.id,NEW.version,NEW.state);
 INSERT INTO crm_sales_quota_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.quota.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

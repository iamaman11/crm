-- Immutable commercial report summary and separately paged forecast evidence.
-- Report row is its own permanent commit receipt; no mutable duplicate state.
CREATE TABLE crm_sales_performance_reports (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id) ON DELETE CASCADE,
 id TEXT NOT NULL,
 quota_id TEXT NOT NULL,
 quota_version INTEGER NOT NULL CHECK(quota_version>0),
 state TEXT NOT NULL CHECK(json_valid(state)),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,id),
 UNIQUE(tenant,idem_key),
 FOREIGN KEY(tenant,quota_id,quota_version)
   REFERENCES crm_sales_quota_revisions(tenant,id,version),
 CHECK(json_extract(state,'$.report_id')=id),
 CHECK(json_extract(state,'$.quota.quota_id')=quota_id),
 CHECK(json_extract(state,'$.quota.version')=quota_version)
);
CREATE INDEX idx_crm_sales_report_quota ON crm_sales_performance_reports(tenant,quota_id,id);
CREATE TABLE crm_sales_performance_forecast_sources (
 tenant TEXT NOT NULL,
 report_id TEXT NOT NULL,
 deal_id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 PRIMARY KEY(tenant,report_id,deal_id),
 FOREIGN KEY(tenant,report_id) REFERENCES crm_sales_performance_reports(tenant,id) ON DELETE CASCADE,
 CHECK(json_extract(state,'$.deal_id')=deal_id)
);
CREATE TRIGGER crm_sales_performance_report_create_audit AFTER INSERT ON crm_sales_performance_reports BEGIN
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.performance_report.save',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

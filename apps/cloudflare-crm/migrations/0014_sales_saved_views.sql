-- R7.3 named saved Sales filters. State and signed mutation receipts are
-- tenant-scoped, audited in the SAME D1 write, with monotonic CAS versions.
-- Archived views are hidden by default; no independent browser storage owner.
CREATE TABLE crm_sales_saved_view_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 version INTEGER NOT NULL CHECK(version>0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL CHECK(length(trim(actor))>0),
 last_action TEXT NOT NULL CHECK(last_action IN ('sales_view.create','sales_view.update')),
 PRIMARY KEY(tenant,id),
 CHECK(json_extract(state,'$.view_id')=id),
 CHECK(CAST(json_extract(state,'$.version') AS INTEGER)=version)
);
CREATE TABLE crm_sales_saved_view_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_sales_saved_view_create_audit
AFTER INSERT ON crm_sales_saved_view_records BEGIN
 INSERT INTO crm_sales_saved_view_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_view.create',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_saved_view_update_audit
AFTER UPDATE ON crm_sales_saved_view_records WHEN NEW.version>OLD.version BEGIN
 INSERT INTO crm_sales_saved_view_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_view.update',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

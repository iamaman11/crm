-- #471 S5: immutable commit provenance for the existing Sales pipeline and
-- saved-view owners. Historical receipts remain NULL; never infer provenance
-- from mutable records or audit ordering.
ALTER TABLE crm_sales_pipeline_receipts ADD COLUMN original_request_id TEXT;
ALTER TABLE crm_sales_saved_view_receipts ADD COLUMN original_request_id TEXT;

DROP TRIGGER crm_sales_pipeline_create_audit;
DROP TRIGGER crm_sales_pipeline_update_audit;
CREATE TRIGGER crm_sales_pipeline_create_audit
AFTER INSERT ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_pipeline_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_pipeline.create',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_pipeline_update_audit
AFTER UPDATE ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_pipeline_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_pipeline.update',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

DROP TRIGGER crm_sales_saved_view_create_audit;
DROP TRIGGER crm_sales_saved_view_update_audit;
CREATE TRIGGER crm_sales_saved_view_create_audit
AFTER INSERT ON crm_sales_saved_view_records BEGIN
 INSERT INTO crm_sales_saved_view_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_view.create',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_saved_view_update_audit
AFTER UPDATE ON crm_sales_saved_view_records WHEN NEW.version>OLD.version BEGIN
 INSERT INTO crm_sales_saved_view_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_view.update',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

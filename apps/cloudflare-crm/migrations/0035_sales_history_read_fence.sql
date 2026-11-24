-- Extend the existing technical Sales read fence to lifecycle retention and catalog membership.
-- No second business state, event store, or materialized metrics.
CREATE TRIGGER crm_sales_history_event_insert AFTER INSERT ON crm_sales_deal_events BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_sales_history_event_update AFTER UPDATE ON crm_sales_deal_events BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE OLD.tenant<>NEW.tenant AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_sales_history_event_delete AFTER DELETE ON crm_sales_deal_events BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_sales_history_catalog_insert AFTER INSERT ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_sales_history_catalog_update AFTER UPDATE ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE OLD.tenant<>NEW.tenant AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_sales_history_catalog_delete AFTER DELETE ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;

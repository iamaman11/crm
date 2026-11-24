-- Technical optimistic read fence, not a second business aggregate.
-- Original owners and their receipts/audit remain unchanged. Existing tenants
-- begin at revision 0 until the first relevant committed mutation.
CREATE TABLE crm_sales_forecast_revisions (
 tenant TEXT PRIMARY KEY REFERENCES crm_tenants(id) ON DELETE CASCADE,
 revision INTEGER NOT NULL CHECK(typeof(revision)='integer' AND revision>0)
);
CREATE INDEX idx_crm_forecast_open_deal
 ON crm_sales_deal_records(tenant,id)
 WHERE json_extract(state,'$.status')='open';
CREATE INDEX idx_crm_forecast_open_pipeline
 ON crm_sales_deal_records(tenant,json_extract(state,'$.stage.pipeline_id'),id)
 WHERE json_extract(state,'$.status')='open';
CREATE TRIGGER crm_forecast_deal_insert AFTER INSERT ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_deal_update AFTER UPDATE ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE OLD.tenant<>NEW.tenant AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_deal_delete AFTER DELETE ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_team_insert AFTER INSERT ON crm_access_teams BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_team_update AFTER UPDATE ON crm_access_teams BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE OLD.tenant<>NEW.tenant AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_team_delete AFTER DELETE ON crm_access_teams BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_member_insert AFTER INSERT ON crm_users BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant_id,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant_id)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_member_update AFTER UPDATE ON crm_users BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant_id,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant_id)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant_id,1 WHERE OLD.tenant_id<>NEW.tenant_id AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant_id)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_member_delete AFTER DELETE ON crm_users BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant_id,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant_id)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_assignment_insert AFTER INSERT ON crm_sales_account_assignment_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_assignment_update AFTER UPDATE ON crm_sales_account_assignment_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE OLD.tenant<>NEW.tenant AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_assignment_delete AFTER DELETE ON crm_sales_account_assignment_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_territory_insert AFTER INSERT ON crm_sales_territory_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_territory_update AFTER UPDATE ON crm_sales_territory_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT NEW.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=NEW.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE OLD.tenant<>NEW.tenant AND EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER crm_forecast_territory_delete AFTER DELETE ON crm_sales_territory_records BEGIN
 INSERT INTO crm_sales_forecast_revisions(tenant,revision)
 SELECT OLD.tenant,1 WHERE EXISTS(SELECT 1 FROM crm_tenants WHERE id=OLD.tenant)
 ON CONFLICT(tenant) DO UPDATE SET revision=revision+1;
END;

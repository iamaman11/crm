-- Original SalesActivitiesLink processing is part of the same D1 mutation.
-- Immutable typed delivery receipts survive later Deal version changes.
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
-- Inserting a delivery for a stale (unsuccessfully updated) Deal must abort the
-- entire D1 batch, including the Deal update and task/audit/receipts. This
-- verifies the version only at insertion, not as a mutable foreign key.
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

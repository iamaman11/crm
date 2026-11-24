-- Preserve immutable commit membership independently of commercial event time.
-- This index contains keys only; the original lifecycle event remains the
-- sole owner of commercial facts. No historical attribution is synthesized.
CREATE TABLE crm_sales_commercial_event_order (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 tenant TEXT NOT NULL,
 deal_id TEXT NOT NULL,
 deal_version INTEGER NOT NULL,
 UNIQUE(tenant,deal_id,deal_version),
 FOREIGN KEY(tenant,deal_id,deal_version)
   REFERENCES crm_sales_deal_events(tenant,deal_id,deal_version) ON DELETE CASCADE
);
CREATE INDEX idx_crm_sales_commercial_order_tenant
 ON crm_sales_commercial_event_order(tenant,sequence);
INSERT INTO crm_sales_commercial_event_order(tenant,deal_id,deal_version)
 SELECT tenant,deal_id,deal_version FROM crm_sales_deal_events
 WHERE action IN ('closed','reopened') ORDER BY tenant,deal_id,deal_version;
CREATE TRIGGER crm_sales_commercial_event_order_insert
AFTER INSERT ON crm_sales_deal_events WHEN NEW.action IN ('closed','reopened') BEGIN
 INSERT INTO crm_sales_commercial_event_order(tenant,deal_id,deal_version)
 VALUES(NEW.tenant,NEW.deal_id,NEW.deal_version);
END;

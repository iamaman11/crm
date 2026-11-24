-- Preserve the original close snapshot on close and reopen; no historical inference.
ALTER TABLE crm_sales_deal_events ADD COLUMN commercial_close_fact TEXT
 CHECK(commercial_close_fact IS NULL OR json_valid(commercial_close_fact));
CREATE INDEX idx_crm_sales_commercial_period ON crm_sales_deal_events(tenant,occurred_at_unix_nanos,deal_id,deal_version)
 WHERE action IN ('closed','reopened');
DROP TRIGGER crm_sales_deal_changed_event;
CREATE TRIGGER crm_sales_deal_changed_event
AFTER UPDATE ON crm_sales_deal_records WHEN NEW.version>OLD.version BEGIN
 INSERT INTO crm_sales_deal_events(
   tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,
   stage_id,previous_stage_ordinal,stage_ordinal,previous_status,status,
   reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version,commercial_close_fact
 ) VALUES(
   NEW.tenant,NEW.id,NEW.version,
   CASE
     WHEN json_extract(OLD.state,'$.status')='open'
      AND json_extract(NEW.state,'$.status') IN ('won','lost') THEN 'closed'
     WHEN json_extract(OLD.state,'$.status') IN ('won','lost')
      AND json_extract(NEW.state,'$.status')='open' THEN 'reopened'
     WHEN NEW.last_action='sales_deals.advance' THEN 'moved'
     ELSE 'updated'
   END,
   json_extract(NEW.state,'$.stage.pipeline_id'),
   json_extract(OLD.state,'$.stage.stage_id'),
   json_extract(NEW.state,'$.stage.stage_id'),
   json_extract(OLD.state,'$.stage.ordinal'),
   json_extract(NEW.state,'$.stage.ordinal'),
   json_extract(OLD.state,'$.status'),
   json_extract(NEW.state,'$.status'),
   CASE
     WHEN json_extract(OLD.state,'$.status')='open'
      AND json_extract(NEW.state,'$.status') IN ('won','lost')
      THEN json_extract(NEW.state,'$.close_outcome.reason_code')
     WHEN json_extract(OLD.state,'$.status') IN ('won','lost')
      AND json_extract(NEW.state,'$.status')='open'
      THEN NEW.last_reason_code
     ELSE NULL END,
   NEW.actor,NEW.request_id,
   json_extract(NEW.state,'$.updated_at_unix_nanos'),
   (SELECT version FROM crm_sales_pipeline_records p
      WHERE p.tenant=NEW.tenant AND p.id=json_extract(NEW.state,'$.stage.pipeline_id')),
   CASE
     WHEN json_extract(OLD.state,'$.status')='open'
      AND json_extract(NEW.state,'$.status') IN ('won','lost')
      THEN json_extract(NEW.state,'$.close_outcome.commercial_fact')
     WHEN json_extract(OLD.state,'$.status') IN ('won','lost')
      AND json_extract(NEW.state,'$.status')='open'
      THEN json_extract(OLD.state,'$.close_outcome.commercial_fact')
     ELSE NULL END
 );
END;

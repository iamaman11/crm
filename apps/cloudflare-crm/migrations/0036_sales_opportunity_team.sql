-- One Deal aggregate, immutable roster-change evidence in its original event.
-- Existing records have no grants. No synthetic history or second owner.
ALTER TABLE crm_sales_deal_events ADD COLUMN opportunity_team_after TEXT
 CHECK(opportunity_team_after IS NULL OR json_valid(opportunity_team_after));
DROP TRIGGER crm_sales_deal_changed_event;
CREATE TRIGGER crm_sales_deal_changed_event
AFTER UPDATE ON crm_sales_deal_records WHEN NEW.version>OLD.version BEGIN
 INSERT INTO crm_sales_deal_events(
   tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,
   stage_id,previous_stage_ordinal,stage_ordinal,previous_status,status,
   reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version,commercial_close_fact,opportunity_team_after
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
     ELSE NULL END,
   CASE WHEN COALESCE(json_extract(OLD.state,'$.opportunity_team'),'[]')<>COALESCE(json_extract(NEW.state,'$.opportunity_team'),'[]')
     THEN COALESCE(json_extract(NEW.state,'$.opportunity_team'),'[]') ELSE NULL END
 );
END;

DROP TRIGGER crm_sales_deal_update_audit;
CREATE TRIGGER crm_sales_deal_update_audit
AFTER UPDATE ON crm_sales_deal_records WHEN NEW.version>OLD.version BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   CASE
     WHEN json_extract(OLD.state,'$.status')='open'
      AND json_extract(NEW.state,'$.status') IN ('won','lost') THEN 'sales_deals.close'
     WHEN json_extract(OLD.state,'$.status') IN ('won','lost')
      AND json_extract(NEW.state,'$.status')='open' THEN 'sales_deals.reopen'
     WHEN COALESCE(json_extract(OLD.state,'$.opportunity_team'),'[]')<>COALESCE(json_extract(NEW.state,'$.opportunity_team'),'[]') THEN 'sales_deals.opportunity_team'
     ELSE NEW.last_action
   END,NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

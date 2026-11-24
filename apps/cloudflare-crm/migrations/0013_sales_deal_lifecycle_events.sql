-- R7.2b: immutable original Sales Deal lifecycle history.
-- Existing receipt/audit triggers remain the only owners of their state;
-- these AFTER triggers run inside the SAME Deal D1 transaction.
-- No fabricated backfill for pre-migration test deals.
ALTER TABLE crm_sales_deal_records ADD COLUMN last_reason_code TEXT;

-- Preserve the existing single receipt/audit owner and D1 CHECK constraint.
-- Close/reopen are distinguished by the ORIGINAL persisted status transition,
-- not by inventing unsupported last_action column values.
DROP TRIGGER crm_sales_deal_update_audit;
CREATE TRIGGER crm_sales_deal_update_audit
AFTER UPDATE ON crm_sales_deal_records WHEN NEW.version>OLD.version BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   CASE
     WHEN json_extract(OLD.state,'$.status')='open'
      AND json_extract(NEW.state,'$.status') IN ('won','lost') THEN 'sales_deals.close'
     WHEN json_extract(OLD.state,'$.status') IN ('won','lost')
      AND json_extract(NEW.state,'$.status')='open' THEN 'sales_deals.reopen'
     ELSE NEW.last_action
   END,NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TABLE crm_sales_deal_events (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 deal_id TEXT NOT NULL,
 deal_version INTEGER NOT NULL CHECK(deal_version>0),
 action TEXT NOT NULL CHECK(action IN ('created','updated','moved','closed','reopened')),
 pipeline_id TEXT NOT NULL,
 previous_stage_id TEXT,
 stage_id TEXT NOT NULL,
 previous_stage_ordinal INTEGER,
 stage_ordinal INTEGER NOT NULL,
 previous_status TEXT,
 status TEXT NOT NULL CHECK(status IN ('open','won','lost')),
 reason_code TEXT,
 actor TEXT NOT NULL CHECK(length(actor)>0),
 request_id TEXT NOT NULL,
 occurred_at_unix_nanos INTEGER NOT NULL CHECK(occurred_at_unix_nanos>0),
 catalog_version INTEGER,
 PRIMARY KEY(tenant,deal_id,deal_version),
 FOREIGN KEY(tenant,deal_id) REFERENCES crm_sales_deal_records(tenant,id)
);
-- The composite primary key is already the tenant-scoped history index.

-- A concurrent pipeline archival must not re-enable a closed Deal in an
-- invalid stage. This guard is in the same D1 transaction as the CAS write.
CREATE TRIGGER crm_sales_deal_reopen_catalog_guard
BEFORE UPDATE ON crm_sales_deal_records
WHEN NEW.version>OLD.version
 AND json_extract(OLD.state,'$.status') IN ('won','lost')
 AND json_extract(NEW.state,'$.status')='open' BEGIN
 SELECT CASE WHEN NOT EXISTS (
   SELECT 1 FROM crm_sales_pipeline_records p
   JOIN json_each(p.state,'$.stages') s
   WHERE p.tenant=NEW.tenant
     AND p.id=json_extract(NEW.state,'$.stage.pipeline_id')
     AND json_extract(p.state,'$.archived')=0
     AND json_extract(s.value,'$.stage_id')=json_extract(NEW.state,'$.stage.stage_id')
     AND json_extract(s.value,'$.ordinal')=json_extract(NEW.state,'$.stage.ordinal')
     AND json_extract(s.value,'$.archived')=0
 ) THEN RAISE(ABORT,'SALES_REOPEN_INACTIVE_STAGE') END;
END;

CREATE TRIGGER crm_sales_deal_created_event
AFTER INSERT ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_deal_events(
   tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,
   stage_id,previous_stage_ordinal,stage_ordinal,previous_status,status,
   reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version
 ) VALUES(
   NEW.tenant,NEW.id,NEW.version,'created',
   json_extract(NEW.state,'$.stage.pipeline_id'),
   NULL,json_extract(NEW.state,'$.stage.stage_id'),
   NULL,json_extract(NEW.state,'$.stage.ordinal'),
   NULL,json_extract(NEW.state,'$.status'),
   NULL,NEW.actor,NEW.request_id,
   json_extract(NEW.state,'$.updated_at_unix_nanos'),
   (SELECT version FROM crm_sales_pipeline_records p
      WHERE p.tenant=NEW.tenant AND p.id=json_extract(NEW.state,'$.stage.pipeline_id'))
 );
END;

CREATE TRIGGER crm_sales_deal_changed_event
AFTER UPDATE ON crm_sales_deal_records WHEN NEW.version>OLD.version BEGIN
 INSERT INTO crm_sales_deal_events(
   tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,
   stage_id,previous_stage_ordinal,stage_ordinal,previous_status,status,
   reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version
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
      WHERE p.tenant=NEW.tenant AND p.id=json_extract(NEW.state,'$.stage.pipeline_id'))
 );
END;

-- #471 S1: immutable commit evidence and the existing cross-owner batch guard.
-- Pre-migration receipts deliberately remain NULL; never fabricate provenance.
ALTER TABLE crm_sales_deal_receipts ADD COLUMN original_request_id TEXT;

DROP TRIGGER crm_sales_deal_create_audit;
CREATE TRIGGER crm_sales_deal_create_audit
AFTER INSERT ON crm_sales_deal_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state,original_request_id)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_deals.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
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
     ELSE NEW.last_action
   END,NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

-- Matching a version is insufficient: another command may have won the CAS
-- and arrived at exactly that version. Bind the delivery to the actual receipt
-- in the SAME transaction; abort rolls back a staged Task as well.
DROP TRIGGER crm_sales_link_source_version;
CREATE TRIGGER crm_sales_link_source_version
BEFORE INSERT ON crm_sales_link_delivery_records BEGIN
 SELECT CASE WHEN NOT EXISTS (
  SELECT 1 FROM crm_sales_deal_records d
  JOIN crm_sales_deal_receipts r
    ON r.tenant=d.tenant AND r.key=d.idem_key AND r.digest=d.idem_digest
  WHERE d.tenant=NEW.tenant AND d.id=NEW.deal_id
    AND d.version=NEW.deal_version AND d.request_id=NEW.request_id
    AND r.original_request_id=NEW.request_id
 ) THEN RAISE(ABORT,'SALES_LINK_SOURCE_COMMIT_MISMATCH') END;
END;

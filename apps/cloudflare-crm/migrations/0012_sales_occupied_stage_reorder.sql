-- R7.1: atomic catalog-rank rebase for occupied stages. Immutable 0011 stays applied.
DROP TRIGGER crm_sales_pipeline_references_guard;
DROP TRIGGER crm_sales_deal_update_audit;

-- Stage ID is stable. Used stages cannot be deleted; open Deal stages cannot
-- be archived, and a pipeline with open Deals cannot be archived.
CREATE TRIGGER crm_sales_pipeline_references_guard
BEFORE UPDATE ON crm_sales_pipeline_records BEGIN
 SELECT CASE WHEN EXISTS (
  SELECT 1 FROM crm_sales_deal_records d
  WHERE d.tenant=NEW.tenant
    AND json_extract(d.state,'$.stage.pipeline_id')=NEW.id
    AND (
      (json_extract(NEW.state,'$.archived')=1
        AND json_extract(d.state,'$.status')='open')
      OR NOT EXISTS (
        SELECT 1 FROM json_each(NEW.state,'$.stages') s
        WHERE json_extract(s.value,'$.stage_id')=json_extract(d.state,'$.stage.stage_id')
          AND (json_extract(d.state,'$.status')!='open'
            OR json_extract(s.value,'$.archived')=0)
      )
    )
 ) THEN RAISE(ABORT,'SALES_PIPELINE_REFERENCED_STAGE_CHANGED') END;
END;

-- Only a catalog-derived ordinal can change without a Deal version increment.
-- Immutable metadata cannot be forged by this internal rank-projection path.
CREATE TRIGGER crm_sales_deal_catalog_rank_only_guard
BEFORE UPDATE ON crm_sales_deal_records
WHEN NEW.version=OLD.version BEGIN
 SELECT CASE WHEN
    NEW.tenant IS NOT OLD.tenant
 OR NEW.id IS NOT OLD.id
 OR NEW.account_id IS NOT OLD.account_id
 OR NEW.idem_key IS NOT OLD.idem_key
 OR NEW.idem_digest IS NOT OLD.idem_digest
 OR NEW.request_id IS NOT OLD.request_id
 OR NEW.actor IS NOT OLD.actor
 OR NEW.last_action IS NOT OLD.last_action
 OR NEW.state IS NOT json_set(OLD.state,'$.stage.ordinal',(
    SELECT json_extract(s.value,'$.ordinal')
    FROM crm_sales_pipeline_records p, json_each(p.state,'$.stages') s
    WHERE p.tenant=OLD.tenant
      AND p.id=json_extract(OLD.state,'$.stage.pipeline_id')
      AND json_extract(s.value,'$.stage_id')=json_extract(OLD.state,'$.stage.stage_id')
    LIMIT 1
 ))
 THEN RAISE(ABORT,'SALES_CATALOG_RANK_ONLY_REQUIRED') END;
END;

-- Business mutations still create exactly one signed Deal receipt and audit;
-- a catalog rank rebase creates neither a new Deal version nor a Deal event.
CREATE TRIGGER crm_sales_deal_update_audit
AFTER UPDATE ON crm_sales_deal_records WHEN NEW.version>OLD.version BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor))=0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,
        NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

-- Runs in the SAME SQLite transaction as the catalog CAS + pipeline audit.
-- A failure rolls back the entire catalog mutation and every rank rebase.
CREATE TRIGGER crm_sales_pipeline_rebase_deal_ranks
AFTER UPDATE OF state ON crm_sales_pipeline_records BEGIN
 UPDATE crm_sales_deal_records
 SET state=json_set(state,'$.stage.ordinal',(
   SELECT json_extract(s.value,'$.ordinal')
   FROM json_each(NEW.state,'$.stages') s
   WHERE json_extract(s.value,'$.stage_id')=
     json_extract(crm_sales_deal_records.state,'$.stage.stage_id')
   LIMIT 1
 ))
 WHERE tenant=NEW.tenant
   AND json_extract(state,'$.stage.pipeline_id')=NEW.id
   AND json_extract(state,'$.stage.ordinal') IS NOT (
     SELECT json_extract(s.value,'$.ordinal')
     FROM json_each(NEW.state,'$.stages') s
     WHERE json_extract(s.value,'$.stage_id')=
       json_extract(crm_sales_deal_records.state,'$.stage.stage_id')
     LIMIT 1
   );
END;

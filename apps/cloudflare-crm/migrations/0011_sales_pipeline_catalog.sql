-- R7.1 authoritative tenant-scoped Sales pipeline catalog, signed actor,
-- exact replay and transactional audit. Existing deal snapshots are preserved;
-- they require explicit reconciliation before a new movement or creation.
CREATE TABLE crm_sales_pipeline_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL CHECK(length(trim(actor)) > 0),
 last_action TEXT NOT NULL CHECK(last_action IN ('sales_pipeline.create','sales_pipeline.update')),
 PRIMARY KEY(tenant,id)
);
CREATE TABLE crm_sales_pipeline_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_sales_pipeline_create_audit
AFTER INSERT ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_pipeline_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_pipeline.create',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_pipeline_update_audit
AFTER UPDATE ON crm_sales_pipeline_records BEGIN
 INSERT INTO crm_sales_pipeline_receipts(tenant,key,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_pipeline.update',
 NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

-- A referenced stage cannot be deleted, reordered, or archived while used by
-- an open Deal. Closed Deal history also preserves its previous rank and ID.
-- This guard runs at write time: a concurrent Deal write cannot evade the
-- preceding Rust read check.
CREATE TRIGGER crm_sales_pipeline_references_guard
BEFORE UPDATE ON crm_sales_pipeline_records BEGIN
 SELECT CASE WHEN EXISTS (
  SELECT 1 FROM crm_sales_deal_records d
  WHERE d.tenant=NEW.tenant
    AND json_extract(d.state,'$.stage.pipeline_id')=NEW.id
    AND (
      (json_extract(NEW.state,'$.archived')=1 AND json_extract(d.state,'$.status')='open')
      OR NOT EXISTS (
        SELECT 1 FROM json_each(NEW.state,'$.stages') ns
        WHERE json_extract(ns.value,'$.stage_id')=json_extract(d.state,'$.stage.stage_id')
          AND json_extract(ns.value,'$.ordinal')=json_extract(d.state,'$.stage.ordinal')
          AND (json_extract(d.state,'$.status')!='open'
               OR json_extract(ns.value,'$.archived')=0)
      )
    )
 ) THEN RAISE(ABORT,'SALES_PIPELINE_REFERENCED_STAGE_CHANGED') END;
END;

-- Authoritative stage and rank: never trust caller-supplied ordinal.
-- Legacy records remain readable; only new writes must use the catalog.
CREATE TRIGGER crm_sales_deal_catalog_create_guard
BEFORE INSERT ON crm_sales_deal_records BEGIN
 SELECT CASE WHEN NOT EXISTS (
  SELECT 1 FROM crm_sales_pipeline_records p
  JOIN json_each(p.state,'$.stages') s
  WHERE p.tenant=NEW.tenant
    AND p.id=json_extract(NEW.state,'$.stage.pipeline_id')
    AND json_extract(p.state,'$.archived')=0
    AND json_extract(s.value,'$.stage_id')=json_extract(NEW.state,'$.stage.stage_id')
    AND json_extract(s.value,'$.ordinal')=json_extract(NEW.state,'$.stage.ordinal')
    AND json_extract(s.value,'$.archived')=0
 ) THEN RAISE(ABORT,'SALES_DEAL_STAGE_NOT_IN_CATALOG') END;
END;
CREATE TRIGGER crm_sales_deal_catalog_move_guard
BEFORE UPDATE ON crm_sales_deal_records
WHEN NEW.last_action='sales_deals.advance' BEGIN
 SELECT CASE WHEN NOT EXISTS (
  SELECT 1 FROM crm_sales_pipeline_records p
  JOIN json_each(p.state,'$.stages') s
  WHERE p.tenant=NEW.tenant
    AND p.id=json_extract(NEW.state,'$.stage.pipeline_id')
    AND json_extract(p.state,'$.archived')=0
    AND json_extract(s.value,'$.stage_id')=json_extract(NEW.state,'$.stage.stage_id')
    AND json_extract(s.value,'$.ordinal')=json_extract(NEW.state,'$.stage.ordinal')
    AND json_extract(s.value,'$.archived')=0
 ) THEN RAISE(ABORT,'SALES_DEAL_STAGE_NOT_IN_CATALOG') END;
END;

-- Reconcile pre-catalog Deal snapshots without changing a single Deal/version.
-- Legacy tenants may have only a rank-2 stage: fill missing ranks with explicit
-- placeholders so the original aggregate remains valid and links stay intact.
-- Ambiguous stage IDs/ranks and >24-stage legacy pipelines are NOT guessed:
-- those are skipped for explicit operator reconciliation (no silent mutation).
-- The provenance is a system migration identity, never a forged human actor.
WITH RECURSIVE
 legacy(tenant,pipeline,stage,ordinal) AS (
  SELECT tenant,
    json_extract(state,'$.stage.pipeline_id'),
    json_extract(state,'$.stage.stage_id'),
    json_extract(state,'$.stage.ordinal')
  FROM crm_sales_deal_records
 ),
 candidates(tenant,pipeline,max_rank) AS (
  SELECT tenant,pipeline,MAX(ordinal)
  FROM legacy
  WHERE pipeline IS NOT NULL AND stage IS NOT NULL
  GROUP BY tenant,pipeline
  HAVING MIN(ordinal)>=1 AND MAX(ordinal)<=24
     AND COUNT(DISTINCT stage||'#'||ordinal)=COUNT(DISTINCT ordinal)
     AND COUNT(DISTINCT stage||'#'||ordinal)=COUNT(DISTINCT stage)
 ),
 slot(tenant,pipeline,ordinal,max_rank) AS (
  SELECT tenant,pipeline,1,max_rank FROM candidates
  UNION ALL
  SELECT tenant,pipeline,ordinal+1,max_rank FROM slot WHERE ordinal<max_rank
 ),
 stage_slot(tenant,pipeline,ordinal,stage_id) AS (
  SELECT slot.tenant,slot.pipeline,slot.ordinal,
    COALESCE((SELECT stage FROM legacy
      WHERE tenant=slot.tenant AND pipeline=slot.pipeline AND ordinal=slot.ordinal
      LIMIT 1),'legacy_unassigned_'||slot.ordinal)
  FROM slot
 ),
 assembled(tenant,pipeline,step,stages) AS (
  SELECT tenant,pipeline,0,json('[]') FROM candidates
  UNION ALL
  SELECT assembled.tenant,assembled.pipeline,assembled.step+1,
    json_insert(assembled.stages,'$[#]',json_object(
      'stage_id',stage_slot.stage_id,'name',stage_slot.stage_id,
      'ordinal',stage_slot.ordinal,'probability_basis_points',0,
      'archived',json('false')))
  FROM assembled JOIN stage_slot ON
    stage_slot.tenant=assembled.tenant AND stage_slot.pipeline=assembled.pipeline
    AND stage_slot.ordinal=assembled.step+1
 )
INSERT INTO crm_sales_pipeline_records
 (tenant,id,state,version,idem_key,idem_digest,request_id,actor,last_action)
SELECT assembled.tenant,assembled.pipeline,
  json_object('pipeline_id',assembled.pipeline,'name',assembled.pipeline,
    'version',1,'archived',json('false'),'stages',json(assembled.stages)),
  1,'migration:0011:'||lower(hex(randomblob(12))),'legacy-reconciliation',
  'migration-0011','system:migration-0011','sales_pipeline.create'
FROM assembled JOIN candidates ON assembled.tenant=candidates.tenant
  AND assembled.pipeline=candidates.pipeline
WHERE assembled.step=candidates.max_rank;

-- Stage-local keyset queries use a single tenant/pipeline/stage covering prefix.
CREATE INDEX idx_crm_sales_board_stage
ON crm_sales_deal_records(
 tenant,
 json_extract(state,'$.stage.pipeline_id'),
 json_extract(state,'$.stage.stage_id'),
 id
);

-- R2a: Enrichment definitions are immutable, and created_by already records
-- the signed enrolled Access actor. Record that original actor in the same SQL
-- statement as the definition and idempotency receipt. Do not backfill old audit.
DROP TRIGGER crm_enrichment_profile_audit;
DROP TRIGGER crm_enrichment_mapping_audit;

CREATE TRIGGER crm_enrichment_profile_audit
AFTER INSERT ON crm_enrichment_profiles BEGIN
 SELECT CASE WHEN NEW.created_by IS NULL OR length(trim(NEW.created_by))=0
  THEN RAISE(ABORT,'ENRICHMENT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_enrichment_receipts(tenant,key,kind,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,'profile',NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_enrichment.profile_publish',NEW.id,datetime('now'),
        NEW.request_id,NEW.created_by);
END;

CREATE TRIGGER crm_enrichment_mapping_audit
AFTER INSERT ON crm_enrichment_mappings BEGIN
 SELECT CASE WHEN NEW.created_by IS NULL OR length(trim(NEW.created_by))=0
  THEN RAISE(ABORT,'ENRICHMENT_AUDIT_ACTOR_REQUIRED') END;
 INSERT INTO crm_enrichment_receipts(tenant,key,kind,digest,state)
 VALUES(NEW.tenant,NEW.idem_key,'mapping',NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
        'customer_enrichment.mapping_publish',NEW.id,datetime('now'),
        NEW.request_id,NEW.created_by);
END;

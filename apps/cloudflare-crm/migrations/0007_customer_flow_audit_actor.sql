-- R2a #408: one additive migration for the linked Relationship -> Task -> Sales flow.
-- Keep previously applied SQL immutable. Historical rows and audit actors stay NULL.
ALTER TABLE crm_party_relationship_records ADD COLUMN actor TEXT;
ALTER TABLE crm_activity_task_records ADD COLUMN actor TEXT;
ALTER TABLE crm_sales_deal_records ADD COLUMN actor TEXT;
ALTER TABLE crm_sales_link_delivery_records ADD COLUMN actor TEXT;

DROP TRIGGER crm_party_relationship_create_audit;
DROP TRIGGER crm_party_relationship_update_audit;
DROP TRIGGER crm_activity_task_create_audit;
DROP TRIGGER crm_activity_task_update_audit;
DROP TRIGGER crm_sales_deal_create_audit;
DROP TRIGGER crm_sales_deal_update_audit;
DROP TRIGGER crm_sales_link_delivery_audit;

CREATE TRIGGER crm_party_relationship_create_audit
AFTER INSERT ON crm_party_relationship_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_party_relationship_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'party_relationships.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_party_relationship_update_audit
AFTER UPDATE ON crm_party_relationship_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_party_relationship_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'party_relationships.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_activity_task_create_audit
AFTER INSERT ON crm_activity_task_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_activity_task_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'activity_tasks.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_activity_task_update_audit
AFTER UPDATE ON crm_activity_task_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
  INSERT INTO crm_activity_task_receipts(tenant,key,digest,state)
    VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
  INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
    VALUES(
      lower(hex(randomblob(16))),
      NEW.tenant,
      CASE
        WHEN json_extract(OLD.state,'$.status')='open'
         AND json_extract(NEW.state,'$.status')='completed'
        THEN 'activity_tasks.complete'
        ELSE 'activity_tasks.update'
      END,
      NEW.id,
      datetime('now'),
      NEW.request_id,
      NEW.actor
    );
END;

CREATE TRIGGER crm_sales_deal_create_audit
AFTER INSERT ON crm_sales_deal_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales_deals.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_sales_deal_update_audit
AFTER UPDATE ON crm_sales_deal_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_sales_deal_receipts(tenant,key,digest,state)
  VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
  VALUES(lower(hex(randomblob(16))),NEW.tenant,NEW.last_action,NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

CREATE TRIGGER crm_sales_link_delivery_audit
AFTER INSERT ON crm_sales_link_delivery_records BEGIN
 SELECT CASE WHEN NEW.actor IS NULL OR length(trim(NEW.actor)) = 0
  THEN RAISE(ABORT,'ACTOR_AUDIT_REQUIRED') END;
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   CASE WHEN NEW.disposition='applied' THEN 'sales_activities_link.applied'
        ELSE 'sales_activities_link.ignored' END,
   NEW.delivery_id,datetime('now'),NEW.request_id,NEW.actor);
END;


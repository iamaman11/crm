-- SK-05: immutable human interactions in the existing Activities owner and D1.
-- Author is the signed actor; the real occurrence time differs from D1 recording time.
-- Atomic INSERT trigger creates receipt and audit or commits none of them.
CREATE TABLE crm_activity_interactions (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 account_id TEXT NOT NULL,
 deal_id TEXT,
 state TEXT NOT NULL,
 occurred_at_key TEXT NOT NULL CHECK(length(occurred_at_key)=20),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL CHECK(length(trim(actor))>0),
 PRIMARY KEY(tenant,id),
 FOREIGN KEY(tenant,account_id) REFERENCES crm_account_records(tenant,id),
 FOREIGN KEY(tenant,deal_id) REFERENCES crm_sales_deal_records(tenant,id)
);
CREATE INDEX idx_crm_interactions_account_time
 ON crm_activity_interactions(tenant,account_id,occurred_at_key DESC,id DESC);
CREATE INDEX idx_crm_interactions_deal_time
 ON crm_activity_interactions(tenant,deal_id,occurred_at_key DESC,id DESC);

CREATE TABLE crm_activity_interaction_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);

-- Keep a linked Deal in its actual parent Account, including direct SQL writes.
CREATE TRIGGER crm_activity_interaction_account_guard
BEFORE INSERT ON crm_activity_interactions
WHEN NEW.deal_id IS NOT NULL
BEGIN
 SELECT CASE WHEN
   (SELECT account_id FROM crm_sales_deal_records
      WHERE tenant=NEW.tenant AND id=NEW.deal_id) IS NOT NEW.account_id
 THEN RAISE(ABORT,'ACTIVITIES_INTERACTION_DEAL_ACCOUNT_MISMATCH') END;
END;

CREATE TRIGGER crm_activity_interaction_create_audit
AFTER INSERT ON crm_activity_interactions
BEGIN
 INSERT INTO crm_activity_interaction_receipts(
   tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,
   'activities.interaction.recorded',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;

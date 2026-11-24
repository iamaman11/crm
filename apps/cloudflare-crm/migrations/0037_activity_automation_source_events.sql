-- SK-08A foundation: one immutable Activities-owned source event per committed
-- Interaction. This is an outbox, not a second Interaction record or an active
-- automation engine. No rules, executions or scheduled triggers are enabled.
-- The original INSERT, original receipt, audit and this event commit atomically.
CREATE TABLE crm_activity_automation_source_events (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 event_id TEXT NOT NULL,
 event_type TEXT NOT NULL CHECK(event_type='crm.activities.interaction_created.v1'),
 source_id TEXT NOT NULL,
 source_version INTEGER NOT NULL CHECK(source_version=1),
 account_id TEXT NOT NULL,
 deal_id TEXT,
 recorded_at_key TEXT NOT NULL CHECK(length(recorded_at_key)=20),
 source_actor TEXT NOT NULL CHECK(length(trim(source_actor))>0),
 origin_request_id TEXT NOT NULL CHECK(length(origin_request_id)>0),
 origin_intent_digest TEXT NOT NULL CHECK(length(origin_intent_digest)=64),
 PRIMARY KEY(tenant,event_id),
 UNIQUE(tenant,event_type,source_id,source_version),
 FOREIGN KEY(tenant,source_id) REFERENCES crm_activity_interactions(tenant,id),
 FOREIGN KEY(tenant,account_id) REFERENCES crm_account_records(tenant,id),
 FOREIGN KEY(tenant,deal_id) REFERENCES crm_sales_deal_records(tenant,id)
);
CREATE INDEX idx_crm_activity_automation_source_order
 ON crm_activity_automation_source_events(tenant,event_type,recorded_at_key,event_id);

CREATE TRIGGER crm_activity_interaction_automation_source
AFTER INSERT ON crm_activity_interactions
BEGIN
 INSERT INTO crm_activity_automation_source_events(
   tenant,event_id,event_type,source_id,source_version,account_id,deal_id,
   recorded_at_key,source_actor,origin_request_id,origin_intent_digest
 ) VALUES(
   NEW.tenant,'crm.activities.interaction_created.v1:'||NEW.id,
   'crm.activities.interaction_created.v1',
   NEW.id,CAST(json_extract(NEW.state,'$.version') AS INTEGER),
   NEW.account_id,NEW.deal_id,
   printf('%020d',CAST(json_extract(NEW.state,'$.recorded_at_nanos') AS INTEGER)),
   NEW.actor,NEW.request_id,NEW.idem_digest
 );
END;

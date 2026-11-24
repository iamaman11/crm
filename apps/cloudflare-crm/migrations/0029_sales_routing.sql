-- Sales rule storage, receipts/audit, and immutable initial attribution.
CREATE TABLE crm_sales_routing_rule_records (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(json_valid(state)),
 version INTEGER NOT NULL CHECK(version>0),
 idem_key TEXT NOT NULL,
 idem_digest TEXT NOT NULL,
 request_id TEXT NOT NULL,
 actor TEXT NOT NULL,
 PRIMARY KEY(tenant,id)
);
CREATE UNIQUE INDEX idx_crm_sales_routing_active_priority
 ON crm_sales_routing_rule_records(tenant,json_extract(state,'$.priority'))
 WHERE json_extract(state,'$.archived')=0;
CREATE TABLE crm_sales_routing_rule_receipts (
 tenant TEXT NOT NULL REFERENCES crm_tenants(id),
 key TEXT NOT NULL,
 digest TEXT NOT NULL,
 state TEXT NOT NULL,
 original_request_id TEXT NOT NULL,
 PRIMARY KEY(tenant,key)
);
CREATE TRIGGER crm_sales_routing_rule_create_audit AFTER INSERT ON crm_sales_routing_rule_records BEGIN
 INSERT INTO crm_sales_routing_rule_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.routing_rule.create',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
CREATE TRIGGER crm_sales_routing_rule_update_audit AFTER UPDATE ON crm_sales_routing_rule_records BEGIN
 INSERT INTO crm_sales_routing_rule_receipts(tenant,key,digest,state,original_request_id)
 VALUES(NEW.tenant,NEW.idem_key,NEW.idem_digest,NEW.state,NEW.request_id);
 INSERT INTO crm_audit(id,tenant_id,action,subject_id,created_at,request_id,actor)
 VALUES(lower(hex(randomblob(16))),NEW.tenant,'sales.routing_rule.update',NEW.id,datetime('now'),NEW.request_id,NEW.actor);
END;
ALTER TABLE crm_sales_deal_events ADD COLUMN initial_routing TEXT CHECK(initial_routing IS NULL OR json_valid(initial_routing));
DROP TRIGGER crm_sales_deal_created_event;
CREATE TRIGGER crm_sales_deal_created_event
AFTER INSERT ON crm_sales_deal_records BEGIN
 INSERT INTO crm_sales_deal_events(
   tenant,deal_id,deal_version,action,pipeline_id,previous_stage_id,
   stage_id,previous_stage_ordinal,stage_ordinal,previous_status,status,
   reason_code,actor,request_id,occurred_at_unix_nanos,catalog_version,initial_routing
 ) VALUES(
   NEW.tenant,NEW.id,NEW.version,'created',
   json_extract(NEW.state,'$.stage.pipeline_id'),
   NULL,json_extract(NEW.state,'$.stage.stage_id'),
   NULL,json_extract(NEW.state,'$.stage.ordinal'),
   NULL,json_extract(NEW.state,'$.status'),
   NULL,NEW.actor,NEW.request_id,
   json_extract(NEW.state,'$.updated_at_unix_nanos'),
   (SELECT version FROM crm_sales_pipeline_records p
      WHERE p.tenant=NEW.tenant AND p.id=json_extract(NEW.state,'$.stage.pipeline_id')),
   json_extract(NEW.state,'$.initial_routing')
 );
END;
CREATE INDEX idx_crm_sales_routing_actor_load
 ON crm_sales_deal_records(tenant,json_extract(state,'$.owner.actor_id'))
 WHERE json_extract(state,'$.status')='open';

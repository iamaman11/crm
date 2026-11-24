-- Once applied, D1 migrations are immutable. This additive migration repairs
-- existing 0037 event IDs and upgrades future Activity inserts in BOTH the
-- already-deployed D1 and a freshly created D1. Never edit applied 0037.
-- The original intent digest includes the Interaction ID and signed actor.
-- It is SHA-256, so a bounded event ID also works for 180-character sources.
UPDATE crm_activity_automation_source_events
 SET event_id='crm.activities.e1:'||origin_intent_digest
 WHERE event_id<>'crm.activities.e1:'||origin_intent_digest;

DROP TRIGGER crm_activity_interaction_automation_source;
CREATE TRIGGER crm_activity_interaction_automation_source
AFTER INSERT ON crm_activity_interactions
BEGIN
 INSERT INTO crm_activity_automation_source_events(
   tenant,event_id,event_type,source_id,source_version,account_id,deal_id,
   recorded_at_key,source_actor,origin_request_id,origin_intent_digest
 ) VALUES(
   NEW.tenant,'crm.activities.e1:'||NEW.idem_digest,
   'crm.activities.interaction_created.v1',
   NEW.id,CAST(json_extract(NEW.state,'$.version') AS INTEGER),
   NEW.account_id,NEW.deal_id,
   printf('%020d',CAST(json_extract(NEW.state,'$.recorded_at_nanos') AS INTEGER)),
   NEW.actor,NEW.request_id,NEW.idem_digest
 );
END;

-- The EventId bound is the existing crm-module-sdk identifier bound. Keep
-- enforcement in D1 for direct SQL writes as well as the original Rust owner.
CREATE TRIGGER crm_activity_automation_event_id_guard
BEFORE INSERT ON crm_activity_automation_source_events
WHEN length(NEW.event_id)<1 OR length(NEW.event_id)>180
BEGIN
 SELECT RAISE(ABORT,'ACTIVITIES_AUTOMATION_EVENT_ID_INVALID');
END;

BEGIN;

DROP TRIGGER IF EXISTS outbox_events_delivery_metadata ON crm.outbox_events;
DROP FUNCTION IF EXISTS crm.populate_outbox_delivery_metadata();

ALTER TABLE crm.outbox_events
  DROP COLUMN trace_id,
  DROP COLUMN correlation_id,
  DROP COLUMN source_actor_id,
  DROP COLUMN event_version,
  DROP COLUMN source_module_id;

COMMIT;

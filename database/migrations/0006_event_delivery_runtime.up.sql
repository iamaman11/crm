BEGIN;

ALTER TABLE crm.outbox_events
  ADD COLUMN source_module_id text,
  ADD COLUMN event_version text,
  ADD COLUMN source_actor_id text,
  ADD COLUMN correlation_id text,
  ADD COLUMN trace_id text;

ALTER TABLE crm.outbox_events DISABLE TRIGGER outbox_events_immutable;
ALTER TABLE crm.outbox_events DISABLE TRIGGER require_write_context;

UPDATE crm.outbox_events AS event
   SET source_module_id = capability.owner_module_id,
       event_version = event.schema_version,
       source_actor_id = business_transaction.actor_id,
       correlation_id = business_transaction.request_id,
       trace_id = business_transaction.request_id
  FROM crm.business_transactions AS business_transaction
  JOIN crm.capability_registry AS capability
    ON capability.capability_id = business_transaction.capability_id
   AND capability.capability_version = business_transaction.capability_version
 WHERE event.tenant_id = business_transaction.tenant_id
   AND event.business_transaction_id = business_transaction.business_transaction_id;

ALTER TABLE crm.outbox_events ENABLE TRIGGER require_write_context;
ALTER TABLE crm.outbox_events ENABLE TRIGGER outbox_events_immutable;

ALTER TABLE crm.outbox_events
  ALTER COLUMN source_module_id SET NOT NULL,
  ALTER COLUMN event_version SET NOT NULL,
  ALTER COLUMN source_actor_id SET NOT NULL,
  ALTER COLUMN correlation_id SET NOT NULL,
  ALTER COLUMN trace_id SET NOT NULL,
  ADD CONSTRAINT outbox_source_module_id_length
    CHECK (length(source_module_id) BETWEEN 1 AND 180),
  ADD CONSTRAINT outbox_event_version_length
    CHECK (length(event_version) BETWEEN 1 AND 80),
  ADD CONSTRAINT outbox_source_actor_id_length
    CHECK (length(source_actor_id) BETWEEN 1 AND 180),
  ADD CONSTRAINT outbox_correlation_id_length
    CHECK (length(correlation_id) BETWEEN 1 AND 180),
  ADD CONSTRAINT outbox_trace_id_length
    CHECK (length(trace_id) BETWEEN 1 AND 180);

CREATE FUNCTION crm.populate_outbox_delivery_metadata()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
  resolved_source_module_id text := crm.context_value('app.module_id');
  resolved_actor_id text := crm.current_actor_id();
  resolved_correlation_id text := crm.context_value('app.correlation_id');
  resolved_trace_id text := crm.context_value('app.trace_id');
BEGIN
  IF resolved_source_module_id IS NULL THEN
    SELECT owner_module_id
      INTO resolved_source_module_id
      FROM crm.capability_registry
     WHERE capability_id = crm.current_capability_id()
       AND capability_version = crm.current_capability_version();
  END IF;

  NEW.source_module_id := COALESCE(NEW.source_module_id, resolved_source_module_id);
  NEW.event_version := COALESCE(NEW.event_version, NEW.schema_version);
  NEW.source_actor_id := COALESCE(NEW.source_actor_id, resolved_actor_id);
  NEW.correlation_id := COALESCE(
    NEW.correlation_id,
    resolved_correlation_id,
    crm.current_request_id()
  );
  NEW.trace_id := COALESCE(
    NEW.trace_id,
    resolved_trace_id,
    crm.current_request_id()
  );

  IF NEW.source_module_id IS NULL
     OR NEW.event_version IS NULL
     OR NEW.source_actor_id IS NULL
     OR NEW.correlation_id IS NULL
     OR NEW.trace_id IS NULL THEN
    RAISE EXCEPTION USING
      ERRCODE = '28000',
      MESSAGE = 'complete event delivery metadata is required';
  END IF;

  RETURN NEW;
END;
$$;

CREATE TRIGGER outbox_events_delivery_metadata
BEFORE INSERT ON crm.outbox_events
FOR EACH ROW EXECUTE FUNCTION crm.populate_outbox_delivery_metadata();

COMMENT ON COLUMN crm.outbox_events.source_module_id IS
  'Immutable owner module that published the event payload.';
COMMENT ON COLUMN crm.outbox_events.event_version IS
  'Published event contract version, independent from storage implementation details.';
COMMENT ON COLUMN crm.outbox_events.source_actor_id IS
  'Actor responsible for the source business transaction.';
COMMENT ON COLUMN crm.outbox_events.correlation_id IS
  'Persisted correlation lineage used for governed downstream delivery.';
COMMENT ON COLUMN crm.outbox_events.trace_id IS
  'Persisted trace lineage used for governed downstream delivery.';

COMMIT;

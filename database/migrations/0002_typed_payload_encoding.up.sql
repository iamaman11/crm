BEGIN;

ALTER TABLE crm.records
  ADD COLUMN payload_encoding text NOT NULL DEFAULT 'protobuf'
  CHECK (payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary'));
ALTER TABLE crm.records ALTER COLUMN payload_encoding DROP DEFAULT;

ALTER TABLE crm.outbox_events
  ADD COLUMN payload_encoding text NOT NULL DEFAULT 'protobuf'
  CHECK (payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary'));
ALTER TABLE crm.outbox_events ALTER COLUMN payload_encoding DROP DEFAULT;

ALTER TABLE crm.outbox_events
  ADD COLUMN deduplication_key text;
UPDATE crm.outbox_events
SET deduplication_key = 'legacy:' || event_id
WHERE deduplication_key IS NULL;
ALTER TABLE crm.outbox_events
  ALTER COLUMN deduplication_key SET NOT NULL,
  ADD CONSTRAINT outbox_deduplication_key_length
    CHECK (length(deduplication_key) BETWEEN 1 AND 240);
CREATE UNIQUE INDEX outbox_deduplication_idx
  ON crm.outbox_events (tenant_id, event_type, deduplication_key);

ALTER TABLE crm.workflow_runs
  ADD COLUMN state_encoding text NOT NULL DEFAULT 'protobuf'
  CHECK (state_encoding IN ('protobuf', 'json', 'utf8_text', 'binary'));
ALTER TABLE crm.workflow_runs ALTER COLUMN state_encoding DROP DEFAULT;

ALTER TABLE crm.module_state
  ADD COLUMN payload_encoding text NOT NULL DEFAULT 'protobuf'
  CHECK (payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary'));
ALTER TABLE crm.module_state ALTER COLUMN payload_encoding DROP DEFAULT;

ALTER TABLE crm.idempotency_records
  ADD COLUMN response_payload_encoding text
  CHECK (
    response_payload_encoding IS NULL
    OR response_payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary')
  );

COMMIT;

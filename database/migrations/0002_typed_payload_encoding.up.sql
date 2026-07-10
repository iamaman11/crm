BEGIN;

ALTER TABLE crm.records
  ADD COLUMN payload_encoding text NOT NULL DEFAULT 'protobuf'
  CHECK (payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary'));
ALTER TABLE crm.records ALTER COLUMN payload_encoding DROP DEFAULT;

ALTER TABLE crm.outbox_events
  ADD COLUMN payload_encoding text NOT NULL DEFAULT 'protobuf'
  CHECK (payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary'));
ALTER TABLE crm.outbox_events ALTER COLUMN payload_encoding DROP DEFAULT;

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

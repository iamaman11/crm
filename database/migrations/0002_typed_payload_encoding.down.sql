BEGIN;

ALTER TABLE crm.idempotency_records DROP COLUMN response_payload_encoding;
ALTER TABLE crm.module_state DROP COLUMN payload_encoding;
ALTER TABLE crm.workflow_runs DROP COLUMN state_encoding;
DROP INDEX crm.outbox_deduplication_idx;
ALTER TABLE crm.outbox_events DROP COLUMN deduplication_key;
ALTER TABLE crm.outbox_events DROP COLUMN payload_encoding;
ALTER TABLE crm.records DROP COLUMN payload_encoding;

COMMIT;

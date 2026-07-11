BEGIN;

ALTER TABLE crm.business_transactions
  DROP CONSTRAINT business_transactions_expected_outbox_events_check;

ALTER TABLE crm.business_transactions
  ADD CONSTRAINT business_transactions_expected_outbox_events_check
  CHECK (expected_outbox_events >= 0);

COMMENT ON COLUMN crm.business_transactions.expected_outbox_events IS
  'Exact transactional outbox evidence count; zero is valid only for an explicit audited aggregate no-op.';

COMMIT;

BEGIN;

DO $$
BEGIN
  IF EXISTS (
    SELECT 1
      FROM crm.business_transactions
     WHERE expected_outbox_events = 0
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '55000',
      MESSAGE = 'cannot roll back audited no-op support while zero-outbox transactions exist';
  END IF;
END;
$$;

ALTER TABLE crm.business_transactions
  DROP CONSTRAINT business_transactions_expected_outbox_events_check;

ALTER TABLE crm.business_transactions
  ADD CONSTRAINT business_transactions_expected_outbox_events_check
  CHECK (expected_outbox_events > 0);

COMMENT ON COLUMN crm.business_transactions.expected_outbox_events IS
  'Exact transactional outbox evidence count; every transaction requires at least one event.';

COMMIT;

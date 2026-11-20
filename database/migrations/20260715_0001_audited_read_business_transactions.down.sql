-- Restoring the previous mutation-only invariant is safe only before any read-only business
-- transaction has been persisted. Refuse a destructive rollback rather than rewriting evidence.

DO $$
BEGIN
  IF EXISTS (
    SELECT 1
      FROM crm.business_transactions
     WHERE expected_idempotency_records = 0
  ) THEN
    RAISE EXCEPTION
      'cannot restore mutation-only business transaction invariant while audited read-only transactions exist';
  END IF;
END;
$$;

ALTER TABLE crm.business_transactions
  DROP CONSTRAINT business_transactions_expected_idempotency_records_check;

ALTER TABLE crm.business_transactions
  ADD CONSTRAINT business_transactions_expected_idempotency_records_check
  CHECK (expected_idempotency_records = 1);

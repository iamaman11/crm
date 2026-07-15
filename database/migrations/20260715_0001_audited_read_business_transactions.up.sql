-- Allow the business-transaction evidence model to represent both mutation batches and
-- governed read-only disclosures without synthesizing a false idempotency record.
--
-- Existing mutation execution remains unchanged and continues to declare exactly one
-- idempotency record. Read-only audited operations may declare zero.

ALTER TABLE crm.business_transactions
  DROP CONSTRAINT business_transactions_expected_idempotency_records_check;

ALTER TABLE crm.business_transactions
  ADD CONSTRAINT business_transactions_expected_idempotency_records_check
  CHECK (expected_idempotency_records IN (0, 1));

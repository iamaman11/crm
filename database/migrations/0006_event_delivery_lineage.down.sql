BEGIN;

ALTER TABLE crm.business_transactions
  DROP CONSTRAINT IF EXISTS business_transactions_trace_id_length_check,
  DROP CONSTRAINT IF EXISTS business_transactions_correlation_id_length_check,
  DROP COLUMN IF EXISTS trace_id,
  DROP COLUMN IF EXISTS correlation_id;

COMMIT;

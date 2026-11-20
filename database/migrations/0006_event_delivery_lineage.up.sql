BEGIN;

ALTER TABLE crm.business_transactions
  ADD COLUMN correlation_id text,
  ADD COLUMN trace_id text;

-- Existing history predates persisted correlation/trace lineage. Request identity
-- is the only durable lineage anchor available for those rows, so use it as the
-- deterministic compatibility fallback. New runtime writes persist exact values.
UPDATE crm.business_transactions
SET correlation_id = request_id,
    trace_id = request_id
WHERE correlation_id IS NULL
   OR trace_id IS NULL;

ALTER TABLE crm.business_transactions
  ALTER COLUMN correlation_id SET NOT NULL,
  ALTER COLUMN trace_id SET NOT NULL,
  ALTER COLUMN correlation_id SET DEFAULT crm.current_request_id(),
  ALTER COLUMN trace_id SET DEFAULT crm.current_request_id(),
  ADD CONSTRAINT business_transactions_correlation_id_length_check
    CHECK (length(correlation_id) BETWEEN 1 AND 180),
  ADD CONSTRAINT business_transactions_trace_id_length_check
    CHECK (length(trace_id) BETWEEN 1 AND 180);

COMMENT ON COLUMN crm.business_transactions.correlation_id IS
  'Persisted request correlation lineage used to reconstruct restart-safe event deliveries.';
COMMENT ON COLUMN crm.business_transactions.trace_id IS
  'Persisted distributed trace lineage used to reconstruct restart-safe event deliveries.';

COMMIT;

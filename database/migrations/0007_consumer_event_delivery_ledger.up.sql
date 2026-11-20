BEGIN;

CREATE TABLE crm.event_deliveries (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  consumer_module_id text NOT NULL CHECK (length(consumer_module_id) BETWEEN 1 AND 180),
  event_id text NOT NULL,
  delivery_id text NOT NULL CHECK (length(delivery_id) BETWEEN 1 AND 180),
  status text NOT NULL CHECK (
    status IN ('pending', 'processing', 'applied', 'ignored', 'retry', 'dead_letter')
  ),
  attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  next_attempt_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  lease_owner text,
  lease_expires_at timestamptz,
  last_error_code text,
  completed_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, consumer_module_id, event_id),
  UNIQUE (tenant_id, delivery_id),
  FOREIGN KEY (tenant_id, event_id)
    REFERENCES crm.outbox_events (tenant_id, event_id)
    ON DELETE CASCADE,
  CHECK (
    (status = 'processing' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
    OR
    (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
  ),
  CHECK (
    (status IN ('applied', 'ignored') AND completed_at IS NOT NULL)
    OR
    (status NOT IN ('applied', 'ignored') AND completed_at IS NULL)
  )
);

ALTER TABLE crm.event_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.event_deliveries FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON crm.event_deliveries
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE INDEX event_deliveries_ready_idx
  ON crm.event_deliveries (tenant_id, consumer_module_id, status, next_attempt_at);
CREATE INDEX event_deliveries_lease_idx
  ON crm.event_deliveries (tenant_id, status, lease_expires_at)
  WHERE status = 'processing';

COMMENT ON TABLE crm.event_deliveries IS
  'Rebuildable consumer-scoped inbox ledger for at-least-once event delivery; business exactly-once effects remain enforced by target capability idempotency.';
COMMENT ON COLUMN crm.event_deliveries.delivery_id IS
  'Deterministic tenant+consumer+source-event delivery identity reused as downstream idempotency identity.';

COMMIT;

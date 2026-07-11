BEGIN;

CREATE TABLE crm.projection_checkpoints (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  projection_id text NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 180),
  last_occurred_at timestamptz NOT NULL,
  last_event_id text NOT NULL CHECK (length(last_event_id) BETWEEN 1 AND 180),
  applied_event_count bigint NOT NULL CHECK (applied_event_count >= 0),
  status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'failed')),
  failure_event_id text,
  failure_code text,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, projection_id),
  CHECK (
    (status = 'active' AND failure_event_id IS NULL AND failure_code IS NULL)
    OR
    (status = 'failed' AND failure_event_id IS NOT NULL AND failure_code IS NOT NULL)
  )
);

CREATE TABLE crm.projection_applied_events (
  tenant_id text NOT NULL,
  projection_id text NOT NULL,
  event_id text NOT NULL CHECK (length(event_id) BETWEEN 1 AND 180),
  occurred_at timestamptz NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, projection_id, event_id),
  FOREIGN KEY (tenant_id, projection_id)
    REFERENCES crm.projection_checkpoints (tenant_id, projection_id)
    ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, event_id)
    REFERENCES crm.outbox_events (tenant_id, event_id)
    ON DELETE CASCADE
);

CREATE TABLE crm.projection_documents (
  tenant_id text NOT NULL,
  projection_id text NOT NULL,
  resource_type text NOT NULL CHECK (length(resource_type) BETWEEN 1 AND 180),
  resource_id text NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 360),
  source_event_id text NOT NULL CHECK (length(source_event_id) BETWEEN 1 AND 180),
  source_version bigint NOT NULL CHECK (source_version > 0),
  document jsonb NOT NULL CHECK (jsonb_typeof(document) = 'object'),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, projection_id, resource_type, resource_id),
  FOREIGN KEY (tenant_id, projection_id)
    REFERENCES crm.projection_checkpoints (tenant_id, projection_id)
    ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, source_event_id)
    REFERENCES crm.outbox_events (tenant_id, event_id)
    ON DELETE CASCADE
);

ALTER TABLE crm.projection_checkpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.projection_checkpoints FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.projection_applied_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.projection_applied_events FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.projection_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.projection_documents FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON crm.projection_checkpoints
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());
CREATE POLICY tenant_isolation ON crm.projection_applied_events
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());
CREATE POLICY tenant_isolation ON crm.projection_documents
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE INDEX projection_applied_events_order_idx
  ON crm.projection_applied_events (tenant_id, projection_id, occurred_at, event_id);
CREATE INDEX projection_documents_resource_idx
  ON crm.projection_documents (tenant_id, projection_id, resource_type, resource_id);

COMMENT ON TABLE crm.projection_checkpoints IS
  'Rebuildable tenant-scoped event-history checkpoint; never authoritative business state.';
COMMENT ON TABLE crm.projection_applied_events IS
  'Rebuildable per-projection source-event deduplication evidence.';
COMMENT ON TABLE crm.projection_documents IS
  'Generic rebuildable JSON read models materialized only from immutable event history.';

COMMIT;

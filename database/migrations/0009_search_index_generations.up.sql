BEGIN;

CREATE TABLE crm.search_index_generations (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  index_id text NOT NULL CHECK (length(index_id) BETWEEN 1 AND 180),
  generation_id text NOT NULL CHECK (length(generation_id) BETWEEN 1 AND 180),
  projection_id text NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 180),
  schema_version text NOT NULL CHECK (length(schema_version) BETWEEN 1 AND 120),
  status text NOT NULL CHECK (status IN ('building', 'active', 'retired')),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  activated_at timestamptz,
  PRIMARY KEY (tenant_id, index_id, generation_id),
  UNIQUE (tenant_id, projection_id),
  CHECK (
    (status = 'active' AND activated_at IS NOT NULL)
    OR
    (status <> 'active')
  )
);

CREATE UNIQUE INDEX search_index_one_active_generation_idx
  ON crm.search_index_generations (tenant_id, index_id)
  WHERE status = 'active';

CREATE INDEX search_index_generation_projection_idx
  ON crm.search_index_generations (tenant_id, index_id, projection_id);

CREATE INDEX projection_documents_search_fts_idx
  ON crm.projection_documents
  USING gin (to_tsvector('simple', COALESCE(document ->> 'search_text', '')))
  WHERE document ? 'search_text';

ALTER TABLE crm.search_index_generations ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.search_index_generations FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON crm.search_index_generations
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

COMMENT ON TABLE crm.search_index_generations IS
  'Tenant-scoped logical search-index generation registry. Documents remain rebuildable projection state; only an explicitly activated generation is queryable.';
COMMENT ON INDEX projection_documents_search_fts_idx IS
  'GIN acceleration for rebuildable search projection documents; never authoritative business state.';

COMMIT;

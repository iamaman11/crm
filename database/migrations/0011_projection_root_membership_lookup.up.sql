BEGIN;

-- Rebuildable composition projections may carry a canonical `root_party_ids`
-- JSON array to identify the stable Party roots affected by one current source
-- aggregate contribution. The partial GIN index keeps root-scoped reads bounded
-- by indexed membership instead of tenant-wide projection scans.
CREATE INDEX projection_documents_root_party_ids_gin_idx
  ON crm.projection_documents
  USING GIN ((document -> 'root_party_ids') jsonb_path_ops)
  WHERE jsonb_typeof(document -> 'root_party_ids') = 'array';

COMMENT ON INDEX crm.projection_documents_root_party_ids_gin_idx IS
  'Indexed root Party membership for rebuildable composition projections; never authoritative customer state.';

COMMIT;

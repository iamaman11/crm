BEGIN;

-- Platform-owned immutable artifact storage. Business modules receive only the
-- governed crm-core-files port; they never receive raw PostgreSQL or object-storage clients.
CREATE TABLE crm.file_artifacts (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  file_id text NOT NULL CHECK (length(file_id) BETWEEN 1 AND 180),
  owner_module_id text NOT NULL CHECK (length(owner_module_id) BETWEEN 1 AND 180),
  media_type text NOT NULL CHECK (length(media_type) BETWEEN 1 AND 255),
  data_class text NOT NULL CHECK (
    data_class IN (
      'public',
      'internal',
      'confidential',
      'restricted',
      'personal',
      'sensitive_personal',
      'biometric',
      'financial',
      'credential'
    )
  ),
  retention_policy_id text NOT NULL CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  expected_size_bytes bigint NOT NULL CHECK (
    expected_size_bytes BETWEEN 0 AND 67108864
  ),
  expected_sha256 bytea NOT NULL CHECK (octet_length(expected_sha256) = 32),
  status text NOT NULL CHECK (status IN ('uploading', 'finalized')),
  next_chunk_index bigint NOT NULL DEFAULT 0 CHECK (next_chunk_index >= 0),
  received_size_bytes bigint NOT NULL DEFAULT 0 CHECK (
    received_size_bytes >= 0 AND received_size_bytes <= expected_size_bytes
  ),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  finalized_at timestamptz,
  PRIMARY KEY (tenant_id, file_id),
  CHECK (
    (status = 'uploading' AND finalized_at IS NULL)
    OR
    (
      status = 'finalized'
      AND finalized_at IS NOT NULL
      AND received_size_bytes = expected_size_bytes
    )
  )
);

CREATE TABLE crm.file_artifact_chunks (
  tenant_id text NOT NULL,
  file_id text NOT NULL,
  chunk_index bigint NOT NULL CHECK (chunk_index >= 0),
  chunk_size_bytes integer NOT NULL CHECK (chunk_size_bytes BETWEEN 1 AND 524288),
  chunk_sha256 bytea NOT NULL CHECK (octet_length(chunk_sha256) = 32),
  chunk_bytes bytea NOT NULL CHECK (octet_length(chunk_bytes) = chunk_size_bytes),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, file_id, chunk_index),
  FOREIGN KEY (tenant_id, file_id)
    REFERENCES crm.file_artifacts (tenant_id, file_id)
    ON DELETE CASCADE
);

CREATE INDEX file_artifacts_owner_status_idx
  ON crm.file_artifacts (tenant_id, owner_module_id, status, created_at, file_id);

ALTER TABLE crm.file_artifacts ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.file_artifacts FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.file_artifact_chunks ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.file_artifact_chunks FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON crm.file_artifacts
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE POLICY tenant_isolation ON crm.file_artifact_chunks
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

COMMENT ON TABLE crm.file_artifacts IS
  'Tenant-scoped immutable artifact metadata. Bytes become readable only after exact size and SHA-256 finalization.';
COMMENT ON TABLE crm.file_artifact_chunks IS
  'Tenant-scoped bounded non-empty upload chunks. Chunk order is exact and finalized artifacts are immutable through the crm-core-files port.';

COMMIT;

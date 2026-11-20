BEGIN;

CREATE TABLE crm.metadata_revisions_v2 (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  revision_id bytea NOT NULL CHECK (octet_length(revision_id) = 32),
  document_count integer NOT NULL CHECK (document_count > 0),
  published_by_actor_id text NOT NULL CHECK (length(published_by_actor_id) BETWEEN 1 AND 180),
  business_transaction_id text NOT NULL CHECK (length(business_transaction_id) BETWEEN 1 AND 180),
  published_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_id, revision_id)
);

CREATE TABLE crm.metadata_revision_documents (
  tenant_id text NOT NULL,
  revision_id bytea NOT NULL CHECK (octet_length(revision_id) = 32),
  metadata_kind text NOT NULL CHECK (
    metadata_kind IN (
      'object', 'field', 'relationship', 'layout',
      'view', 'pipeline', 'permission', 'workflow'
    )
  ),
  metadata_id text NOT NULL CHECK (length(metadata_id) BETWEEN 1 AND 180),
  schema_version text NOT NULL CHECK (length(schema_version) BETWEEN 1 AND 80),
  canonical_content bytea NOT NULL CHECK (
    octet_length(canonical_content) BETWEEN 1 AND 4194304
  ),
  PRIMARY KEY (tenant_id, revision_id, metadata_kind, metadata_id),
  FOREIGN KEY (tenant_id, revision_id)
    REFERENCES crm.metadata_revisions_v2 (tenant_id, revision_id)
    ON DELETE RESTRICT
);

CREATE TABLE crm.metadata_revision_dependencies (
  tenant_id text NOT NULL,
  revision_id bytea NOT NULL CHECK (octet_length(revision_id) = 32),
  metadata_kind text NOT NULL,
  metadata_id text NOT NULL,
  dependency_kind text NOT NULL CHECK (
    dependency_kind IN (
      'object', 'field', 'relationship', 'layout',
      'view', 'pipeline', 'permission', 'workflow'
    )
  ),
  dependency_id text NOT NULL CHECK (length(dependency_id) BETWEEN 1 AND 180),
  PRIMARY KEY (
    tenant_id, revision_id, metadata_kind, metadata_id,
    dependency_kind, dependency_id
  ),
  FOREIGN KEY (tenant_id, revision_id, metadata_kind, metadata_id)
    REFERENCES crm.metadata_revision_documents (
      tenant_id, revision_id, metadata_kind, metadata_id
    )
    ON DELETE RESTRICT
);

CREATE TABLE crm.metadata_activation_heads (
  tenant_id text PRIMARY KEY REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  generation bigint NOT NULL CHECK (generation >= 0),
  active_revision_id bytea CHECK (
    active_revision_id IS NULL OR octet_length(active_revision_id) = 32
  ),
  rollback_depth bigint NOT NULL CHECK (rollback_depth >= 0),
  last_business_transaction_id text NOT NULL CHECK (
    length(last_business_transaction_id) BETWEEN 1 AND 180
  ),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  FOREIGN KEY (tenant_id, active_revision_id)
    REFERENCES crm.metadata_revisions_v2 (tenant_id, revision_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.metadata_rollback_stack (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  depth bigint NOT NULL CHECK (depth > 0),
  revision_id bytea NOT NULL CHECK (octet_length(revision_id) = 32),
  pushed_generation bigint NOT NULL CHECK (pushed_generation > 0),
  last_business_transaction_id text NOT NULL CHECK (
    length(last_business_transaction_id) BETWEEN 1 AND 180
  ),
  PRIMARY KEY (tenant_id, depth),
  FOREIGN KEY (tenant_id, revision_id)
    REFERENCES crm.metadata_revisions_v2 (tenant_id, revision_id)
    ON DELETE RESTRICT
);

CREATE TABLE crm.metadata_transitions (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  transition_id text NOT NULL CHECK (length(transition_id) BETWEEN 1 AND 512),
  action text NOT NULL CHECK (action IN ('publish', 'activate', 'rollback')),
  generation bigint NOT NULL CHECK (generation >= 0),
  rollback_depth bigint NOT NULL CHECK (rollback_depth >= 0),
  from_revision_id bytea CHECK (
    from_revision_id IS NULL OR octet_length(from_revision_id) = 32
  ),
  to_revision_id bytea NOT NULL CHECK (octet_length(to_revision_id) = 32),
  actor_id text NOT NULL CHECK (length(actor_id) BETWEEN 1 AND 180),
  request_id text NOT NULL CHECK (length(request_id) BETWEEN 1 AND 180),
  capability_id text NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 180),
  capability_version text NOT NULL CHECK (length(capability_version) BETWEEN 1 AND 80),
  business_transaction_id text NOT NULL CHECK (
    length(business_transaction_id) BETWEEN 1 AND 180
  ),
  occurred_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_id, transition_id),
  FOREIGN KEY (tenant_id, from_revision_id)
    REFERENCES crm.metadata_revisions_v2 (tenant_id, revision_id)
    DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (tenant_id, to_revision_id)
    REFERENCES crm.metadata_revisions_v2 (tenant_id, revision_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE INDEX metadata_transitions_generation_idx
  ON crm.metadata_transitions (tenant_id, generation, occurred_at, transition_id);

CREATE TRIGGER metadata_revisions_v2_immutable
BEFORE UPDATE OR DELETE ON crm.metadata_revisions_v2
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER metadata_revision_documents_immutable
BEFORE UPDATE OR DELETE ON crm.metadata_revision_documents
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER metadata_revision_dependencies_immutable
BEFORE UPDATE OR DELETE ON crm.metadata_revision_dependencies
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER metadata_transitions_immutable
BEFORE UPDATE OR DELETE ON crm.metadata_transitions
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

DO $$
DECLARE
  table_name text;
  tenant_tables text[] := ARRAY[
    'metadata_revisions_v2',
    'metadata_revision_documents',
    'metadata_revision_dependencies',
    'metadata_activation_heads',
    'metadata_rollback_stack',
    'metadata_transitions'
  ];
BEGIN
  FOREACH table_name IN ARRAY tenant_tables LOOP
    EXECUTE format('ALTER TABLE crm.%I ENABLE ROW LEVEL SECURITY', table_name);
    EXECUTE format('ALTER TABLE crm.%I FORCE ROW LEVEL SECURITY', table_name);
    EXECUTE format(
      'CREATE POLICY tenant_isolation ON crm.%I USING (tenant_id = crm.current_tenant_id()) WITH CHECK (tenant_id = crm.current_tenant_id())',
      table_name
    );
    EXECUTE format(
      'CREATE TRIGGER require_write_context BEFORE INSERT OR UPDATE OR DELETE ON crm.%I FOR EACH ROW EXECUTE FUNCTION crm.require_write_context()',
      table_name
    );
  END LOOP;
END;
$$;

COMMENT ON TABLE crm.metadata_revisions_v2 IS
  'Tenant-scoped immutable metadata publication revisions keyed by deterministic SHA-256 content identity.';
COMMENT ON TABLE crm.metadata_revision_documents IS
  'Immutable canonical metadata documents contained in one complete published revision snapshot.';
COMMENT ON TABLE crm.metadata_revision_dependencies IS
  'Immutable explicit dependency edges for documents in a complete metadata revision snapshot.';
COMMENT ON TABLE crm.metadata_activation_heads IS
  'Mutable tenant-scoped active metadata pointer guarded by optimistic generation and rollback depth.';
COMMENT ON TABLE crm.metadata_rollback_stack IS
  'Operational rollback stack matching metadata runtime push/pop semantics; rows are removed only when rolled back.';
COMMENT ON TABLE crm.metadata_transitions IS
  'Append-only metadata publish/activate/rollback transition evidence bound to transaction-local execution context.';

COMMIT;

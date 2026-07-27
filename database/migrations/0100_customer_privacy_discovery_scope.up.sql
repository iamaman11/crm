CREATE TABLE crm.customer_privacy_discovery_attempts (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  privacy_case_id text NOT NULL,
  canonical_party_id text NOT NULL,
  identity_resolution_generation bigint NOT NULL CHECK (identity_resolution_generation > 0),
  registry_version text NOT NULL,
  registry_digest bytea NOT NULL CHECK (octet_length(registry_digest) = 32),
  purpose_code text NOT NULL,
  effective_request_at_unix_ms bigint NOT NULL CHECK (effective_request_at_unix_ms > 0),
  captured_at_unix_nanos bigint NOT NULL CHECK (captured_at_unix_nanos > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, attempt_digest),
  UNIQUE (
    tenant_id,
    privacy_case_id,
    canonical_party_id,
    identity_resolution_generation,
    registry_digest,
    purpose_code,
    effective_request_at_unix_ms
  )
);

CREATE TABLE crm.customer_privacy_discovery_owner_pages (
  tenant_id text NOT NULL,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  owner_module_id text NOT NULL,
  capability_id text NOT NULL,
  capability_version text NOT NULL,
  lineage_digest bytea NOT NULL CHECK (octet_length(lineage_digest) = 32),
  page_number integer NOT NULL CHECK (page_number > 0),
  request_cursor_digest bytea NOT NULL CHECK (octet_length(request_cursor_digest) = 32),
  response_cursor_digest bytea NOT NULL CHECK (octet_length(response_cursor_digest) = 32),
  owner_cursor_digest bytea NOT NULL CHECK (octet_length(owner_cursor_digest) = 32),
  page_digest bytea NOT NULL CHECK (octet_length(page_digest) = 32),
  scanned_resource_count bigint NOT NULL CHECK (scanned_resource_count >= 0),
  emitted_resource_count bigint NOT NULL CHECK (emitted_resource_count >= 0),
  terminal_complete boolean NOT NULL,
  response_bytes bytea NOT NULL CHECK (octet_length(response_bytes) <= 524288),
  response_digest bytea NOT NULL CHECK (octet_length(response_digest) = 32),
  accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (
    tenant_id,
    attempt_digest,
    owner_module_id,
    page_number,
    request_cursor_digest
  ),
  UNIQUE (tenant_id, attempt_digest, owner_module_id, page_number),
  FOREIGN KEY (tenant_id, attempt_digest)
    REFERENCES crm.customer_privacy_discovery_attempts (tenant_id, attempt_digest)
    ON DELETE RESTRICT
);

CREATE TABLE crm.customer_privacy_discovery_checkpoints (
  tenant_id text NOT NULL,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  owner_module_id text NOT NULL,
  contiguous_page_number integer NOT NULL CHECK (contiguous_page_number > 0),
  terminal_complete boolean NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, attempt_digest, owner_module_id),
  FOREIGN KEY (tenant_id, attempt_digest)
    REFERENCES crm.customer_privacy_discovery_attempts (tenant_id, attempt_digest)
    ON DELETE RESTRICT
);

CREATE TABLE crm.customer_privacy_discovery_snapshots (
  tenant_id text NOT NULL,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  snapshot_id text NOT NULL,
  snapshot_binding_digest bytea NOT NULL CHECK (octet_length(snapshot_binding_digest) = 32),
  finalized_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, attempt_digest),
  UNIQUE (tenant_id, snapshot_id),
  FOREIGN KEY (tenant_id, attempt_digest)
    REFERENCES crm.customer_privacy_discovery_attempts (tenant_id, attempt_digest)
    ON DELETE RESTRICT
);

CREATE TABLE crm.customer_privacy_discovery_audit (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE RESTRICT,
  audit_digest bytea NOT NULL CHECK (octet_length(audit_digest) = 32),
  event_type text NOT NULL CHECK (event_type IN (
    'discovery_started',
    'owner_page_accepted',
    'owner_terminal_complete',
    'discovery_failed',
    'snapshot_finalized',
    'snapshot_read_allowed',
    'snapshot_read_denied'
  )),
  privacy_case_id text NOT NULL,
  attempt_digest bytea NOT NULL CHECK (octet_length(attempt_digest) = 32),
  owner_module_id text,
  page_number integer CHECK (page_number IS NULL OR page_number > 0),
  snapshot_id text,
  safe_count bigint CHECK (safe_count IS NULL OR safe_count >= 0),
  policy_reference text,
  occurred_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_id, audit_digest)
);

CREATE INDEX customer_privacy_discovery_pages_owner_idx
  ON crm.customer_privacy_discovery_owner_pages (
    tenant_id,
    attempt_digest,
    owner_module_id,
    page_number
  );

ALTER TABLE crm.customer_privacy_discovery_attempts ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_attempts FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_owner_pages ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_owner_pages FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_checkpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_checkpoints FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_snapshots FORCE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.customer_privacy_discovery_audit FORCE ROW LEVEL SECURITY;

CREATE POLICY customer_privacy_discovery_attempts_tenant_policy
  ON crm.customer_privacy_discovery_attempts
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY customer_privacy_discovery_owner_pages_tenant_policy
  ON crm.customer_privacy_discovery_owner_pages
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY customer_privacy_discovery_checkpoints_tenant_policy
  ON crm.customer_privacy_discovery_checkpoints
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY customer_privacy_discovery_snapshots_tenant_policy
  ON crm.customer_privacy_discovery_snapshots
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY customer_privacy_discovery_audit_tenant_policy
  ON crm.customer_privacy_discovery_audit
  USING (tenant_id = current_setting('app.tenant_id', true))
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

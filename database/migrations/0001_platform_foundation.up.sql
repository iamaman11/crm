BEGIN;

CREATE SCHEMA IF NOT EXISTS crm;
COMMENT ON SCHEMA crm IS 'Ultimate CRM authoritative transactional schema';

CREATE FUNCTION crm.context_value(setting_name text)
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$
  SELECT NULLIF(current_setting(setting_name, true), '')
$$;

CREATE FUNCTION crm.current_tenant_id()
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$ SELECT crm.context_value('app.tenant_id') $$;

CREATE FUNCTION crm.current_actor_id()
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$ SELECT crm.context_value('app.actor_id') $$;

CREATE FUNCTION crm.current_request_id()
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$ SELECT crm.context_value('app.request_id') $$;

CREATE FUNCTION crm.current_capability_id()
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$ SELECT crm.context_value('app.capability_id') $$;

CREATE FUNCTION crm.current_capability_version()
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$ SELECT crm.context_value('app.capability_version') $$;

CREATE FUNCTION crm.current_business_transaction_id()
RETURNS text
LANGUAGE sql
STABLE
PARALLEL SAFE
AS $$ SELECT crm.context_value('app.business_transaction_id') $$;

CREATE FUNCTION crm.reject_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION USING
    ERRCODE = '55000',
    MESSAGE = format('%I.%I is immutable after insert', TG_TABLE_SCHEMA, TG_TABLE_NAME);
END;
$$;

CREATE FUNCTION crm.require_write_context()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
  row_value jsonb;
  bound_tenant text := crm.current_tenant_id();
  bound_actor text := crm.current_actor_id();
  bound_request text := crm.current_request_id();
  bound_capability text := crm.current_capability_id();
  bound_capability_version text := crm.current_capability_version();
  bound_transaction text := crm.current_business_transaction_id();
  row_tenant text;
  row_transaction text;
BEGIN
  IF TG_OP = 'DELETE' THEN
    row_value := to_jsonb(OLD);
  ELSE
    row_value := to_jsonb(NEW);
  END IF;

  IF bound_tenant IS NULL OR bound_actor IS NULL OR bound_request IS NULL
     OR bound_capability IS NULL OR bound_capability_version IS NULL
     OR bound_transaction IS NULL THEN
    RAISE EXCEPTION USING
      ERRCODE = '28000',
      MESSAGE = 'complete transaction-local execution context is required';
  END IF;

  row_tenant := NULLIF(row_value ->> 'tenant_id', '');
  IF row_tenant IS NULL OR row_tenant <> bound_tenant THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'row tenant does not match the bound tenant context';
  END IF;

  IF row_value ? 'business_transaction_id' THEN
    row_transaction := NULLIF(row_value ->> 'business_transaction_id', '');
  ELSIF row_value ? 'last_business_transaction_id' THEN
    row_transaction := NULLIF(row_value ->> 'last_business_transaction_id', '');
  END IF;

  IF row_transaction IS NOT NULL AND row_transaction <> bound_transaction THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'row business transaction does not match the bound transaction context';
  END IF;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;
  RETURN NEW;
END;
$$;

CREATE TABLE crm.tenants (
  tenant_id text PRIMARY KEY CHECK (length(tenant_id) BETWEEN 1 AND 180),
  status text NOT NULL CHECK (status IN ('provisioning', 'active', 'suspended', 'deleting')),
  data_region text NOT NULL CHECK (length(data_region) BETWEEN 2 AND 64),
  version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE crm.module_versions (
  module_id text NOT NULL CHECK (length(module_id) BETWEEN 1 AND 180),
  version text NOT NULL CHECK (length(version) BETWEEN 1 AND 80),
  canonicalization_profile text NOT NULL CHECK (length(canonicalization_profile) BETWEEN 1 AND 80),
  manifest_sha256 bytea NOT NULL CHECK (octet_length(manifest_sha256) = 32),
  normalized_manifest_json jsonb NOT NULL CHECK (jsonb_typeof(normalized_manifest_json) = 'object'),
  published_at timestamptz NOT NULL,
  publisher_id text NOT NULL CHECK (length(publisher_id) BETWEEN 1 AND 180),
  publisher_signature bytea,
  provenance_digest bytea CHECK (provenance_digest IS NULL OR octet_length(provenance_digest) = 32),
  PRIMARY KEY (module_id, version)
);

CREATE TABLE crm.module_dependencies (
  module_id text NOT NULL,
  module_version text NOT NULL,
  dependency_module_id text NOT NULL CHECK (length(dependency_module_id) BETWEEN 1 AND 180),
  version_requirement text NOT NULL CHECK (length(version_requirement) BETWEEN 1 AND 160),
  dependency_kind text NOT NULL CHECK (dependency_kind IN ('required', 'optional', 'conflict')),
  PRIMARY KEY (module_id, module_version, dependency_module_id, dependency_kind),
  FOREIGN KEY (module_id, module_version)
    REFERENCES crm.module_versions (module_id, version)
    ON DELETE RESTRICT
);

CREATE TABLE crm.capability_registry (
  capability_id text NOT NULL CHECK (length(capability_id) BETWEEN 1 AND 180),
  capability_version text NOT NULL CHECK (length(capability_version) BETWEEN 1 AND 80),
  owner_module_id text NOT NULL,
  owner_module_version text NOT NULL,
  service_name text NOT NULL CHECK (length(service_name) BETWEEN 1 AND 180),
  method_name text NOT NULL CHECK (length(method_name) BETWEEN 1 AND 180),
  input_descriptor_hash bytea NOT NULL CHECK (octet_length(input_descriptor_hash) = 32),
  output_descriptor_hash bytea NOT NULL CHECK (octet_length(output_descriptor_hash) = 32),
  risk_level text NOT NULL CHECK (risk_level IN ('low', 'medium', 'high', 'critical')),
  idempotency_required boolean NOT NULL,
  audit_required boolean NOT NULL,
  approval_required boolean NOT NULL,
  ai_callable boolean NOT NULL,
  marketplace_callable boolean NOT NULL,
  bulk_allowed boolean NOT NULL,
  export_allowed boolean NOT NULL,
  required_permissions text[] NOT NULL DEFAULT '{}',
  data_classes_touched text[] NOT NULL DEFAULT '{}',
  rate_limit_policy_id text,
  approval_policy_id text,
  PRIMARY KEY (capability_id, capability_version),
  FOREIGN KEY (owner_module_id, owner_module_version)
    REFERENCES crm.module_versions (module_id, version)
    ON DELETE RESTRICT
);

CREATE TABLE crm.actors (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  actor_id text NOT NULL CHECK (length(actor_id) BETWEEN 1 AND 180),
  actor_type text NOT NULL CHECK (actor_type IN ('user', 'service', 'workflow', 'integration', 'ai')),
  status text NOT NULL CHECK (status IN ('active', 'suspended', 'disabled')),
  display_name text NOT NULL CHECK (length(display_name) BETWEEN 1 AND 240),
  attributes jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(attributes) = 'object'),
  version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
  last_business_transaction_id text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, actor_id)
);

CREATE TABLE crm.teams (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  team_id text NOT NULL CHECK (length(team_id) BETWEEN 1 AND 180),
  name text NOT NULL CHECK (length(name) BETWEEN 1 AND 240),
  parent_team_id text,
  version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
  last_business_transaction_id text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, team_id),
  FOREIGN KEY (tenant_id, parent_team_id)
    REFERENCES crm.teams (tenant_id, team_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.team_memberships (
  tenant_id text NOT NULL,
  team_id text NOT NULL,
  actor_id text NOT NULL,
  membership_role text NOT NULL CHECK (membership_role IN ('member', 'manager', 'owner')),
  last_business_transaction_id text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, team_id, actor_id),
  FOREIGN KEY (tenant_id, team_id) REFERENCES crm.teams (tenant_id, team_id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, actor_id) REFERENCES crm.actors (tenant_id, actor_id) ON DELETE CASCADE
);

CREATE TABLE crm.business_transactions (
  tenant_id text NOT NULL,
  business_transaction_id text NOT NULL CHECK (length(business_transaction_id) BETWEEN 1 AND 180),
  actor_id text NOT NULL,
  request_id text NOT NULL CHECK (length(request_id) BETWEEN 1 AND 180),
  capability_id text NOT NULL,
  capability_version text NOT NULL,
  expected_outbox_events integer NOT NULL CHECK (expected_outbox_events > 0),
  expected_audit_records integer NOT NULL CHECK (expected_audit_records > 0),
  expected_idempotency_records integer NOT NULL CHECK (expected_idempotency_records > 0),
  committed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, business_transaction_id),
  FOREIGN KEY (tenant_id, actor_id)
    REFERENCES crm.actors (tenant_id, actor_id)
    DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (capability_id, capability_version)
    REFERENCES crm.capability_registry (capability_id, capability_version)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.module_installations (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  install_id text NOT NULL CHECK (length(install_id) BETWEEN 1 AND 180),
  module_id text NOT NULL,
  current_version text NOT NULL,
  status text NOT NULL CHECK (status IN ('installed', 'active', 'suspended', 'upgrading', 'rolling_back', 'uninstalling', 'failed')),
  previous_version text,
  pending_version text,
  generation bigint NOT NULL CHECK (generation > 0),
  failure_code text,
  grant_set_digest bytea NOT NULL CHECK (octet_length(grant_set_digest) = 32),
  last_business_transaction_id text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, module_id),
  UNIQUE (tenant_id, install_id),
  FOREIGN KEY (module_id, current_version) REFERENCES crm.module_versions (module_id, version),
  FOREIGN KEY (module_id, previous_version) REFERENCES crm.module_versions (module_id, version),
  FOREIGN KEY (module_id, pending_version) REFERENCES crm.module_versions (module_id, version),
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.tenant_capability_grants (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  capability_id text NOT NULL,
  capability_version text NOT NULL,
  grant_digest bytea NOT NULL CHECK (octet_length(grant_digest) = 32),
  enabled boolean NOT NULL DEFAULT true,
  last_business_transaction_id text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, capability_id, capability_version),
  FOREIGN KEY (capability_id, capability_version)
    REFERENCES crm.capability_registry (capability_id, capability_version),
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.metadata_packages (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  package_id text NOT NULL CHECK (length(package_id) BETWEEN 1 AND 180),
  package_version text NOT NULL CHECK (length(package_version) BETWEEN 1 AND 80),
  source_environment text NOT NULL CHECK (length(source_environment) BETWEEN 1 AND 80),
  target_platform_version text NOT NULL CHECK (length(target_platform_version) BETWEEN 1 AND 80),
  descriptor_set bytea NOT NULL,
  content_hash bytea NOT NULL CHECK (octet_length(content_hash) = 32),
  signature bytea,
  business_transaction_id text NOT NULL,
  published_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, package_id, package_version),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.object_definitions (
  tenant_id text NOT NULL,
  object_type text NOT NULL CHECK (length(object_type) BETWEEN 1 AND 180),
  definition_version text NOT NULL CHECK (length(definition_version) BETWEEN 1 AND 80),
  package_id text NOT NULL,
  package_version text NOT NULL,
  owner_module_id text NOT NULL CHECK (length(owner_module_id) BETWEEN 1 AND 180),
  label text NOT NULL CHECK (length(label) BETWEEN 1 AND 240),
  data_class text NOT NULL CHECK (length(data_class) BETWEEN 1 AND 80),
  retention_policy_id text NOT NULL CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  business_transaction_id text NOT NULL,
  PRIMARY KEY (tenant_id, object_type, definition_version),
  FOREIGN KEY (tenant_id, package_id, package_version)
    REFERENCES crm.metadata_packages (tenant_id, package_id, package_version),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.field_definitions (
  tenant_id text NOT NULL,
  object_type text NOT NULL,
  definition_version text NOT NULL,
  field_number integer NOT NULL CHECK (field_number > 0),
  api_name text NOT NULL CHECK (length(api_name) BETWEEN 1 AND 180),
  data_type text NOT NULL CHECK (length(data_type) BETWEEN 1 AND 80),
  cardinality text NOT NULL CHECK (cardinality IN ('optional', 'required', 'repeated')),
  data_class text NOT NULL CHECK (length(data_class) BETWEEN 1 AND 80),
  searchable boolean NOT NULL DEFAULT false,
  sortable boolean NOT NULL DEFAULT false,
  encrypted boolean NOT NULL DEFAULT false,
  retention_policy_id text NOT NULL CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  business_transaction_id text NOT NULL,
  PRIMARY KEY (tenant_id, object_type, definition_version, field_number),
  UNIQUE (tenant_id, object_type, definition_version, api_name),
  FOREIGN KEY (tenant_id, object_type, definition_version)
    REFERENCES crm.object_definitions (tenant_id, object_type, definition_version),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.records (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  record_type text NOT NULL CHECK (length(record_type) BETWEEN 1 AND 180),
  record_id text NOT NULL CHECK (length(record_id) BETWEEN 1 AND 180),
  version bigint NOT NULL CHECK (version > 0),
  owner_module_id text NOT NULL CHECK (length(owner_module_id) BETWEEN 1 AND 180),
  schema_id text NOT NULL CHECK (length(schema_id) BETWEEN 1 AND 180),
  schema_version text NOT NULL CHECK (length(schema_version) BETWEEN 1 AND 80),
  descriptor_hash bytea NOT NULL CHECK (octet_length(descriptor_hash) = 32),
  data_class text NOT NULL CHECK (length(data_class) BETWEEN 1 AND 80),
  maximum_payload_size bigint NOT NULL CHECK (maximum_payload_size BETWEEN 0 AND 67108864),
  retention_policy_id text NOT NULL CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  payload_bytes bytea NOT NULL CHECK (octet_length(payload_bytes) <= maximum_payload_size),
  typed_projection jsonb CHECK (typed_projection IS NULL OR jsonb_typeof(typed_projection) = 'object'),
  last_business_transaction_id text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  deleted_at timestamptz,
  PRIMARY KEY (tenant_id, record_type, record_id),
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.relationships (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  relationship_type text NOT NULL CHECK (length(relationship_type) BETWEEN 1 AND 180),
  source_record_type text NOT NULL,
  source_record_id text NOT NULL,
  target_record_type text NOT NULL,
  target_record_id text NOT NULL,
  version bigint NOT NULL CHECK (version > 0),
  attributes jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(attributes) = 'object'),
  last_business_transaction_id text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (
    tenant_id,
    relationship_type,
    source_record_type,
    source_record_id,
    target_record_type,
    target_record_id
  ),
  FOREIGN KEY (tenant_id, source_record_type, source_record_id)
    REFERENCES crm.records (tenant_id, record_type, record_id),
  FOREIGN KEY (tenant_id, target_record_type, target_record_id)
    REFERENCES crm.records (tenant_id, record_type, record_id),
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.idempotency_records (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  idempotency_scope text NOT NULL CHECK (length(idempotency_scope) BETWEEN 1 AND 180),
  idempotency_key text NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 240),
  request_hash bytea NOT NULL CHECK (octet_length(request_hash) = 32),
  status text NOT NULL CHECK (status IN ('in_progress', 'completed', 'failed')),
  response_schema_id text,
  response_schema_version text,
  response_descriptor_hash bytea CHECK (response_descriptor_hash IS NULL OR octet_length(response_descriptor_hash) = 32),
  response_payload bytea,
  business_transaction_id text NOT NULL,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, idempotency_scope, idempotency_key),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.outbox_events (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  event_id text NOT NULL CHECK (length(event_id) BETWEEN 1 AND 180),
  business_transaction_id text NOT NULL,
  aggregate_type text NOT NULL CHECK (length(aggregate_type) BETWEEN 1 AND 180),
  aggregate_id text NOT NULL CHECK (length(aggregate_id) BETWEEN 1 AND 180),
  aggregate_version bigint NOT NULL CHECK (aggregate_version > 0),
  event_sequence bigint NOT NULL CHECK (event_sequence > 0),
  event_type text NOT NULL CHECK (length(event_type) BETWEEN 1 AND 180),
  schema_id text NOT NULL CHECK (length(schema_id) BETWEEN 1 AND 180),
  schema_version text NOT NULL CHECK (length(schema_version) BETWEEN 1 AND 80),
  descriptor_hash bytea NOT NULL CHECK (octet_length(descriptor_hash) = 32),
  data_class text NOT NULL CHECK (length(data_class) BETWEEN 1 AND 80),
  maximum_payload_size bigint NOT NULL CHECK (maximum_payload_size BETWEEN 0 AND 67108864),
  retention_policy_id text NOT NULL CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  payload_bytes bytea NOT NULL CHECK (octet_length(payload_bytes) <= maximum_payload_size),
  occurred_at timestamptz NOT NULL,
  available_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, event_id),
  UNIQUE (tenant_id, aggregate_type, aggregate_id, aggregate_version),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.outbox_delivery (
  tenant_id text NOT NULL,
  event_id text NOT NULL,
  status text NOT NULL CHECK (status IN ('pending', 'publishing', 'published', 'retry', 'dead_letter')),
  attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  next_attempt_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  published_at timestamptz,
  last_error_code text,
  last_business_transaction_id text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, event_id),
  FOREIGN KEY (tenant_id, event_id) REFERENCES crm.outbox_events (tenant_id, event_id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.audit_heads (
  tenant_id text PRIMARY KEY REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  next_sequence bigint NOT NULL CHECK (next_sequence > 0),
  last_hash bytea NOT NULL CHECK (octet_length(last_hash) = 32),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE crm.audit_records (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  audit_sequence bigint NOT NULL CHECK (audit_sequence > 0),
  audit_record_id text NOT NULL CHECK (length(audit_record_id) BETWEEN 1 AND 180),
  business_transaction_id text NOT NULL,
  actor_id text NOT NULL,
  capability_id text NOT NULL,
  capability_version text NOT NULL,
  canonicalization_profile text NOT NULL CHECK (length(canonicalization_profile) BETWEEN 1 AND 80),
  previous_hash bytea NOT NULL CHECK (octet_length(previous_hash) = 32),
  record_hash bytea NOT NULL CHECK (octet_length(record_hash) = 32),
  canonical_envelope bytea NOT NULL,
  occurred_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_id, audit_sequence),
  UNIQUE (tenant_id, audit_record_id),
  FOREIGN KEY (tenant_id, actor_id) REFERENCES crm.actors (tenant_id, actor_id),
  FOREIGN KEY (capability_id, capability_version)
    REFERENCES crm.capability_registry (capability_id, capability_version),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.workflow_definitions (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  workflow_id text NOT NULL CHECK (length(workflow_id) BETWEEN 1 AND 180),
  workflow_version text NOT NULL CHECK (length(workflow_version) BETWEEN 1 AND 80),
  definition_sha256 bytea NOT NULL CHECK (octet_length(definition_sha256) = 32),
  definition_bytes bytea NOT NULL,
  business_transaction_id text NOT NULL,
  published_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, workflow_id, workflow_version),
  FOREIGN KEY (tenant_id, business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.workflow_runs (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  workflow_run_id text NOT NULL CHECK (length(workflow_run_id) BETWEEN 1 AND 180),
  workflow_id text NOT NULL,
  workflow_version text NOT NULL,
  status text NOT NULL CHECK (status IN ('pending', 'running', 'waiting', 'succeeded', 'failed', 'cancelled')),
  generation bigint NOT NULL CHECK (generation > 0),
  state_schema_id text NOT NULL,
  state_schema_version text NOT NULL,
  state_descriptor_hash bytea NOT NULL CHECK (octet_length(state_descriptor_hash) = 32),
  state_data_class text NOT NULL,
  maximum_state_size bigint NOT NULL CHECK (maximum_state_size BETWEEN 0 AND 67108864),
  retention_policy_id text NOT NULL,
  state_bytes bytea NOT NULL CHECK (octet_length(state_bytes) <= maximum_state_size),
  last_business_transaction_id text NOT NULL,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, workflow_run_id),
  FOREIGN KEY (tenant_id, workflow_id, workflow_version)
    REFERENCES crm.workflow_definitions (tenant_id, workflow_id, workflow_version),
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE crm.module_state (
  tenant_id text NOT NULL REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  module_id text NOT NULL,
  state_key text NOT NULL CHECK (length(state_key) BETWEEN 1 AND 240),
  version bigint NOT NULL CHECK (version > 0),
  schema_id text NOT NULL CHECK (length(schema_id) BETWEEN 1 AND 180),
  schema_version text NOT NULL CHECK (length(schema_version) BETWEEN 1 AND 80),
  descriptor_hash bytea NOT NULL CHECK (octet_length(descriptor_hash) = 32),
  data_class text NOT NULL CHECK (length(data_class) BETWEEN 1 AND 80),
  maximum_payload_size bigint NOT NULL CHECK (maximum_payload_size BETWEEN 0 AND 67108864),
  retention_policy_id text NOT NULL CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  payload_bytes bytea NOT NULL CHECK (octet_length(payload_bytes) <= maximum_payload_size),
  last_business_transaction_id text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_id, module_id, state_key),
  FOREIGN KEY (tenant_id, module_id)
    REFERENCES crm.module_installations (tenant_id, module_id)
    ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, last_business_transaction_id)
    REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE FUNCTION crm.enforce_audit_chain()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, crm
AS $$
DECLARE
  changed_rows integer;
  zero_hash bytea := decode(repeat('00', 32), 'hex');
BEGIN
  IF NEW.audit_sequence = 1 THEN
    IF NEW.previous_hash <> zero_hash THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'first audit record must use a zero previous hash';
    END IF;

    INSERT INTO crm.audit_heads (tenant_id, next_sequence, last_hash)
    VALUES (NEW.tenant_id, 2, NEW.record_hash)
    ON CONFLICT (tenant_id) DO NOTHING;
    GET DIAGNOSTICS changed_rows = ROW_COUNT;
    IF changed_rows <> 1 THEN
      RAISE EXCEPTION USING
        ERRCODE = '40001',
        MESSAGE = 'audit chain already exists for tenant';
    END IF;
  ELSE
    UPDATE crm.audit_heads
       SET next_sequence = NEW.audit_sequence + 1,
           last_hash = NEW.record_hash,
           updated_at = clock_timestamp()
     WHERE tenant_id = NEW.tenant_id
       AND next_sequence = NEW.audit_sequence
       AND last_hash = NEW.previous_hash;
    GET DIAGNOSTICS changed_rows = ROW_COUNT;
    IF changed_rows <> 1 THEN
      RAISE EXCEPTION USING
        ERRCODE = '40001',
        MESSAGE = 'audit sequence or previous hash does not match the tenant audit head';
    END IF;
  END IF;
  RETURN NEW;
END;
$$;

CREATE FUNCTION crm.verify_transaction_evidence()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
  outbox_count integer;
  audit_count integer;
  idempotency_count integer;
BEGIN
  SELECT count(*) INTO outbox_count
    FROM crm.outbox_events
   WHERE tenant_id = NEW.tenant_id
     AND business_transaction_id = NEW.business_transaction_id;

  SELECT count(*) INTO audit_count
    FROM crm.audit_records
   WHERE tenant_id = NEW.tenant_id
     AND business_transaction_id = NEW.business_transaction_id;

  SELECT count(*) INTO idempotency_count
    FROM crm.idempotency_records
   WHERE tenant_id = NEW.tenant_id
     AND business_transaction_id = NEW.business_transaction_id;

  IF outbox_count <> NEW.expected_outbox_events
     OR audit_count <> NEW.expected_audit_records
     OR idempotency_count <> NEW.expected_idempotency_records THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = format(
        'transaction evidence mismatch for %s: outbox %s/%s, audit %s/%s, idempotency %s/%s',
        NEW.business_transaction_id,
        outbox_count,
        NEW.expected_outbox_events,
        audit_count,
        NEW.expected_audit_records,
        idempotency_count,
        NEW.expected_idempotency_records
      );
  END IF;
  RETURN NULL;
END;
$$;

CREATE TRIGGER module_versions_immutable
BEFORE UPDATE OR DELETE ON crm.module_versions
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER module_dependencies_immutable
BEFORE UPDATE OR DELETE ON crm.module_dependencies
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER capability_registry_immutable
BEFORE UPDATE OR DELETE ON crm.capability_registry
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER metadata_packages_immutable
BEFORE UPDATE OR DELETE ON crm.metadata_packages
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER object_definitions_immutable
BEFORE UPDATE OR DELETE ON crm.object_definitions
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER field_definitions_immutable
BEFORE UPDATE OR DELETE ON crm.field_definitions
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER outbox_events_immutable
BEFORE UPDATE OR DELETE ON crm.outbox_events
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER audit_records_immutable
BEFORE UPDATE OR DELETE ON crm.audit_records
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER workflow_definitions_immutable
BEFORE UPDATE OR DELETE ON crm.workflow_definitions
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER business_transactions_immutable
BEFORE UPDATE OR DELETE ON crm.business_transactions
FOR EACH ROW EXECUTE FUNCTION crm.reject_mutation();

CREATE TRIGGER audit_records_chain
BEFORE INSERT ON crm.audit_records
FOR EACH ROW EXECUTE FUNCTION crm.enforce_audit_chain();

CREATE CONSTRAINT TRIGGER business_transactions_verify_evidence
AFTER INSERT ON crm.business_transactions
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION crm.verify_transaction_evidence();

DO $$
DECLARE
  table_name text;
  tenant_tables text[] := ARRAY[
    'tenants',
    'actors',
    'teams',
    'team_memberships',
    'business_transactions',
    'module_installations',
    'tenant_capability_grants',
    'metadata_packages',
    'object_definitions',
    'field_definitions',
    'records',
    'relationships',
    'idempotency_records',
    'outbox_events',
    'outbox_delivery',
    'audit_heads',
    'audit_records',
    'workflow_definitions',
    'workflow_runs',
    'module_state'
  ];
BEGIN
  FOREACH table_name IN ARRAY tenant_tables LOOP
    EXECUTE format('ALTER TABLE crm.%I ENABLE ROW LEVEL SECURITY', table_name);
    EXECUTE format('ALTER TABLE crm.%I FORCE ROW LEVEL SECURITY', table_name);
    EXECUTE format(
      'CREATE POLICY tenant_isolation ON crm.%I USING (tenant_id = crm.current_tenant_id()) WITH CHECK (tenant_id = crm.current_tenant_id())',
      table_name
    );
  END LOOP;
END;
$$;

DO $$
DECLARE
  table_name text;
  context_tables text[] := ARRAY[
    'actors',
    'teams',
    'team_memberships',
    'business_transactions',
    'module_installations',
    'tenant_capability_grants',
    'metadata_packages',
    'object_definitions',
    'field_definitions',
    'records',
    'relationships',
    'idempotency_records',
    'outbox_events',
    'outbox_delivery',
    'audit_records',
    'workflow_definitions',
    'workflow_runs',
    'module_state'
  ];
BEGIN
  FOREACH table_name IN ARRAY context_tables LOOP
    EXECUTE format(
      'CREATE TRIGGER require_write_context BEFORE INSERT OR UPDATE OR DELETE ON crm.%I FOR EACH ROW EXECUTE FUNCTION crm.require_write_context()',
      table_name
    );
  END LOOP;
END;
$$;

CREATE INDEX actors_status_idx ON crm.actors (tenant_id, status);
CREATE INDEX team_memberships_actor_idx ON crm.team_memberships (tenant_id, actor_id);
CREATE INDEX module_installations_status_idx ON crm.module_installations (tenant_id, status);
CREATE INDEX records_type_updated_idx ON crm.records (tenant_id, record_type, updated_at DESC);
CREATE INDEX records_projection_gin_idx ON crm.records USING gin (typed_projection);
CREATE INDEX relationships_source_idx ON crm.relationships (tenant_id, source_record_type, source_record_id);
CREATE INDEX relationships_target_idx ON crm.relationships (tenant_id, target_record_type, target_record_id);
CREATE INDEX idempotency_expiry_idx ON crm.idempotency_records (tenant_id, expires_at);
CREATE INDEX outbox_available_idx ON crm.outbox_delivery (tenant_id, status, next_attempt_at);
CREATE INDEX outbox_transaction_idx ON crm.outbox_events (tenant_id, business_transaction_id);
CREATE INDEX audit_transaction_idx ON crm.audit_records (tenant_id, business_transaction_id);
CREATE INDEX workflow_runs_status_idx ON crm.workflow_runs (tenant_id, status, updated_at);

COMMENT ON TABLE crm.module_versions IS 'Platform-global immutable published module catalog';
COMMENT ON TABLE crm.business_transactions IS 'Immutable commit marker verified by a deferred evidence constraint';
COMMENT ON TABLE crm.records IS 'Authoritative typed business payloads; typed_projection is rebuildable';
COMMENT ON TABLE crm.outbox_events IS 'Immutable transactional event envelopes awaiting delivery';
COMMENT ON TABLE crm.audit_records IS 'Immutable canonical audit envelopes chained per tenant';
COMMENT ON TABLE crm.audit_heads IS 'Internal tenant audit chain cursor; runtime roles must not receive direct mutation grants';

COMMIT;

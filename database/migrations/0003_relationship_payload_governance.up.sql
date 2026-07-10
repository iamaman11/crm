BEGIN;

ALTER TABLE crm.actors RENAME COLUMN attributes TO typed_projection;
COMMENT ON COLUMN crm.actors.typed_projection IS
  'Bounded rebuildable actor projection; not authoritative extension state';

ALTER TABLE crm.relationships RENAME COLUMN attributes TO typed_projection;
ALTER TABLE crm.relationships
  ADD COLUMN owner_module_id text NOT NULL DEFAULT 'platform'
    CHECK (length(owner_module_id) BETWEEN 1 AND 180),
  ADD COLUMN schema_id text NOT NULL DEFAULT 'crm.relationship.empty.v1'
    CHECK (length(schema_id) BETWEEN 1 AND 180),
  ADD COLUMN schema_version text NOT NULL DEFAULT '1.0.0'
    CHECK (length(schema_version) BETWEEN 1 AND 80),
  ADD COLUMN descriptor_hash bytea NOT NULL DEFAULT decode(repeat('01', 32), 'hex')
    CHECK (octet_length(descriptor_hash) = 32),
  ADD COLUMN data_class text NOT NULL DEFAULT 'internal'
    CHECK (length(data_class) BETWEEN 1 AND 80),
  ADD COLUMN payload_encoding text NOT NULL DEFAULT 'binary'
    CHECK (payload_encoding IN ('protobuf', 'json', 'utf8_text', 'binary')),
  ADD COLUMN maximum_payload_size bigint NOT NULL DEFAULT 0
    CHECK (maximum_payload_size BETWEEN 0 AND 67108864),
  ADD COLUMN retention_policy_id text NOT NULL DEFAULT 'standard'
    CHECK (length(retention_policy_id) BETWEEN 1 AND 180),
  ADD COLUMN payload_bytes bytea NOT NULL DEFAULT decode('', 'hex')
    CHECK (octet_length(payload_bytes) <= maximum_payload_size);

ALTER TABLE crm.relationships
  ALTER COLUMN owner_module_id DROP DEFAULT,
  ALTER COLUMN schema_id DROP DEFAULT,
  ALTER COLUMN schema_version DROP DEFAULT,
  ALTER COLUMN descriptor_hash DROP DEFAULT,
  ALTER COLUMN data_class DROP DEFAULT,
  ALTER COLUMN payload_encoding DROP DEFAULT,
  ALTER COLUMN maximum_payload_size DROP DEFAULT,
  ALTER COLUMN retention_policy_id DROP DEFAULT,
  ALTER COLUMN payload_bytes DROP DEFAULT;

COMMENT ON COLUMN crm.relationships.typed_projection IS
  'Bounded rebuildable relationship projection; authoritative extension data is payload_bytes';
COMMENT ON COLUMN crm.relationships.payload_bytes IS
  'Authoritative typed relationship payload governed by adjacent metadata columns';

CREATE INDEX relationships_owner_idx
  ON crm.relationships (tenant_id, owner_module_id, relationship_type);

COMMIT;

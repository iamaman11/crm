BEGIN;

DROP INDEX crm.relationships_owner_idx;

ALTER TABLE crm.relationships
  DROP COLUMN payload_bytes,
  DROP COLUMN retention_policy_id,
  DROP COLUMN maximum_payload_size,
  DROP COLUMN payload_encoding,
  DROP COLUMN data_class,
  DROP COLUMN descriptor_hash,
  DROP COLUMN schema_version,
  DROP COLUMN schema_id,
  DROP COLUMN owner_module_id;

ALTER TABLE crm.relationships RENAME COLUMN typed_projection TO attributes;
ALTER TABLE crm.actors RENAME COLUMN typed_projection TO attributes;

COMMIT;

CREATE FUNCTION crm.customer_privacy_restriction_canonical_party_id(payload_bytes bytea)
RETURNS text
LANGUAGE sql
IMMUTABLE
STRICT
PARALLEL SAFE
SET search_path = pg_catalog
AS $$
  SELECT convert_from(payload_bytes, 'UTF8')::jsonb ->> 'canonical_party_id'
$$;

CREATE INDEX customer_privacy_restriction_subject_idx
  ON crm.records (
    tenant_id,
    crm.customer_privacy_restriction_canonical_party_id(payload_bytes),
    record_id
  )
  WHERE owner_module_id = 'crm.customer-privacy'
    AND record_type = 'customer-privacy.restriction'
    AND deleted_at IS NULL;

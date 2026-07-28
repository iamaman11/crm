CREATE INDEX customer_privacy_restriction_subject_idx
  ON crm.records (
    tenant_id,
    ((convert_from(payload_bytes, 'UTF8')::jsonb ->> 'canonical_party_id')),
    record_id
  )
  WHERE owner_module_id = 'crm.customer-privacy'
    AND record_type = 'customer-privacy.restriction'
    AND deleted_at IS NULL;

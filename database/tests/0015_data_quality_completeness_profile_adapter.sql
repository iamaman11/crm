-- Additive production-process acceptance fixture for immutable Party completeness-profile publication.
-- The in-process application catalog remains authoritative for the exact request and response descriptors.

INSERT INTO crm.capability_registry (
  capability_id,
  capability_version,
  owner_module_id,
  owner_module_version,
  service_name,
  method_name,
  input_descriptor_hash,
  output_descriptor_hash,
  risk_level,
  idempotency_required,
  audit_required,
  approval_required,
  ai_callable,
  marketplace_callable,
  bulk_allowed,
  export_allowed,
  data_classes_touched
)
SELECT
  'data_quality.party.completeness_profile.publish',
  capability_version,
  owner_module_id,
  owner_module_version,
  service_name,
  'PublishPartyCompletenessProfileVersion',
  input_descriptor_hash,
  output_descriptor_hash,
  risk_level,
  idempotency_required,
  audit_required,
  approval_required,
  ai_callable,
  marketplace_callable,
  bulk_allowed,
  export_allowed,
  data_classes_touched
FROM crm.capability_registry
WHERE capability_id = 'data_quality.party.rule_set.publish'
  AND capability_version = '1.0.0'
ON CONFLICT (capability_id, capability_version) DO NOTHING;

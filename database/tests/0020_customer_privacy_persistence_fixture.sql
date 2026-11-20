\set ON_ERROR_STOP on

-- Production-process acceptance fixture for the published Customer Privacy module.
-- The in-process application catalog remains authoritative for exact descriptors;
-- this fixture supplies the durable module/capability registry rows required by
-- audit foreign keys without inserting any Customer Privacy business records.
-- Historical module versions remain append-only because accepted later fixtures
-- continue to reference their original published owner version.

INSERT INTO crm.module_versions (
  module_id,
  version,
  canonicalization_profile,
  manifest_sha256,
  normalized_manifest_json,
  published_at,
  publisher_id
)
VALUES
  (
    'crm.customer-privacy',
    '0.2.0',
    'crm.cjson/v1',
    decode(repeat('68', 32), 'hex'),
    '{"module_id":"crm.customer-privacy","version":"0.2.0"}'::jsonb,
    clock_timestamp(),
    'customer-platform'
  ),
  (
    'crm.customer-privacy',
    '0.3.0',
    'crm.cjson/v1',
    decode(repeat('7b', 32), 'hex'),
    '{"module_id":"crm.customer-privacy","version":"0.3.0"}'::jsonb,
    clock_timestamp(),
    'customer-platform'
  )
ON CONFLICT (module_id, version) DO UPDATE
SET canonicalization_profile = EXCLUDED.canonicalization_profile,
    manifest_sha256 = EXCLUDED.manifest_sha256,
    normalized_manifest_json = EXCLUDED.normalized_manifest_json,
    publisher_id = EXCLUDED.publisher_id;

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
  export_allowed
)
VALUES
  (
    'customer_privacy.case.create',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyCaseService',
    'CreatePrivacyCase',
    decode(repeat('69', 32), 'hex'),
    decode(repeat('6a', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.case.approve',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyCaseService',
    'ApprovePrivacyCase',
    decode(repeat('6b', 32), 'hex'),
    decode(repeat('6c', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.restriction.place',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'PlaceProcessingRestriction',
    decode(repeat('6d', 32), 'hex'),
    decode(repeat('6e', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.legal_hold.place',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'PlaceCustomerDataLegalHold',
    decode(repeat('6f', 32), 'hex'),
    decode(repeat('70', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.restriction.release',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'ReleaseProcessingRestriction',
    decode(repeat('71', 32), 'hex'),
    decode(repeat('72', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.restriction.get',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'GetProcessingRestriction',
    decode(repeat('73', 32), 'hex'),
    decode(repeat('74', 32), 'hex'),
    'low',
    false,
    false,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.legal_hold.release',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'ReleaseCustomerDataLegalHold',
    decode(repeat('75', 32), 'hex'),
    decode(repeat('76', 32), 'hex'),
    'high',
    true,
    true,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.legal_hold.get',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'GetCustomerDataLegalHold',
    decode(repeat('77', 32), 'hex'),
    decode(repeat('78', 32), 'hex'),
    'low',
    false,
    false,
    false,
    false,
    false,
    false,
    false
  ),
  (
    'customer_privacy.legal_hold.list_by_subject',
    '1.0.0',
    'crm.customer-privacy',
    '0.3.0',
    'crm.customer_privacy.v1.CustomerPrivacyControlService',
    'ListCustomerDataLegalHoldsBySubject',
    decode(repeat('79', 32), 'hex'),
    decode(repeat('7a', 32), 'hex'),
    'low',
    false,
    false,
    false,
    false,
    false,
    false,
    false
  )
ON CONFLICT (capability_id, capability_version) DO UPDATE
SET owner_module_id = EXCLUDED.owner_module_id,
    owner_module_version = EXCLUDED.owner_module_version,
    service_name = EXCLUDED.service_name,
    method_name = EXCLUDED.method_name,
    input_descriptor_hash = EXCLUDED.input_descriptor_hash,
    output_descriptor_hash = EXCLUDED.output_descriptor_hash,
    risk_level = EXCLUDED.risk_level,
    idempotency_required = EXCLUDED.idempotency_required,
    audit_required = EXCLUDED.audit_required,
    approval_required = EXCLUDED.approval_required,
    ai_callable = EXCLUDED.ai_callable,
    marketplace_callable = EXCLUDED.marketplace_callable,
    bulk_allowed = EXCLUDED.bulk_allowed,
    export_allowed = EXCLUDED.export_allowed;

SELECT 'Customer Privacy persistence fixture PASS' AS result;

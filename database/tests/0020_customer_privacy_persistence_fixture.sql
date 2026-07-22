\set ON_ERROR_STOP on

INSERT INTO crm.tenants (tenant_id, status, data_region)
VALUES ('tenant-privacy-a', 'active', 'eu-central')
ON CONFLICT (tenant_id) DO UPDATE
SET status = EXCLUDED.status,
    data_region = EXCLUDED.data_region;

INSERT INTO crm.module_versions (
  module_id,
  version,
  canonicalization_profile,
  manifest_sha256,
  normalized_manifest_json,
  published_at,
  publisher_id
)
VALUES (
  'crm.customer-privacy',
  '0.2.0',
  'crm.cjson/v1',
  decode(repeat('68', 32), 'hex'),
  '{"module_id":"crm.customer-privacy","version":"0.2.0"}'::jsonb,
  clock_timestamp(),
  'customer-platform'
)
ON CONFLICT (module_id, version) DO UPDATE
SET canonicalization_profile = EXCLUDED.canonicalization_profile,
    manifest_sha256 = EXCLUDED.manifest_sha256,
    normalized_manifest_json = EXCLUDED.normalized_manifest_json,
    publisher_id = EXCLUDED.publisher_id;

SELECT 'Customer Privacy persistence fixture PASS' AS result;

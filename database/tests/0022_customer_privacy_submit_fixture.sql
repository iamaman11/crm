\set ON_ERROR_STOP on

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
VALUES (
  'customer_privacy.case.submit',
  '1.0.0',
  'crm.customer-privacy',
  '0.2.0',
  'crm.customer_privacy.v1.CustomerPrivacyCaseService',
  'SubmitPrivacyCase',
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

SET ROLE crm_app_test;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-customer-privacy-submit-fixture-a';
SET LOCAL app.capability_id = 'customer_privacy.case.submit';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-customer-privacy-submit-fixture-a';

INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES (
  'tenant-a',
  'privacy-officer',
  'service',
  'active',
  'Tenant A privacy officer fixture',
  'tx-customer-privacy-submit-fixture-a'
);

INSERT INTO crm.idempotency_records (
  tenant_id,
  idempotency_scope,
  idempotency_key,
  request_hash,
  status,
  business_transaction_id,
  expires_at
)
VALUES (
  'tenant-a',
  'customer_privacy.case.submit@1.0.0',
  'customer-privacy-submit-fixture-a',
  decode(repeat('6d', 32), 'hex'),
  'completed',
  'tx-customer-privacy-submit-fixture-a',
  clock_timestamp() + interval '1 day'
);

INSERT INTO crm.outbox_events (
  tenant_id,
  event_id,
  business_transaction_id,
  aggregate_type,
  aggregate_id,
  aggregate_version,
  event_sequence,
  event_type,
  deduplication_key,
  schema_id,
  schema_version,
  descriptor_hash,
  data_class,
  payload_encoding,
  maximum_payload_size,
  retention_policy_id,
  payload_bytes,
  occurred_at
)
VALUES (
  'tenant-a',
  'event-customer-privacy-submit-fixture-a',
  'tx-customer-privacy-submit-fixture-a',
  'crm.actor',
  'privacy-officer',
  1,
  1,
  'actor.created',
  'customer-privacy-submit-fixture-a',
  'crm.actor.created.v1',
  '1.0.0',
  decode(repeat('6e', 32), 'hex'),
  'internal',
  'protobuf',
  16,
  'standard',
  decode('01', 'hex'),
  clock_timestamp()
);

INSERT INTO crm.audit_records (
  tenant_id,
  audit_sequence,
  audit_record_id,
  business_transaction_id,
  actor_id,
  capability_id,
  capability_version,
  canonicalization_profile,
  previous_hash,
  record_hash,
  canonical_envelope,
  occurred_at
)
SELECT
  'tenant-a',
  audit_sequence + 1,
  'audit-customer-privacy-submit-fixture-a',
  'tx-customer-privacy-submit-fixture-a',
  'privacy-officer',
  'customer_privacy.case.submit',
  '1.0.0',
  'crm.cjson/v1',
  record_hash,
  decode(repeat('6f', 32), 'hex'),
  convert_to('{"customer_privacy":"submit_fixture_a"}', 'UTF8'),
  clock_timestamp()
FROM crm.audit_records
WHERE tenant_id = 'tenant-a'
ORDER BY audit_sequence DESC
LIMIT 1;

INSERT INTO crm.business_transactions (
  tenant_id,
  business_transaction_id,
  actor_id,
  request_id,
  capability_id,
  capability_version,
  expected_outbox_events,
  expected_audit_records,
  expected_idempotency_records
)
VALUES (
  'tenant-a',
  'tx-customer-privacy-submit-fixture-a',
  'privacy-officer',
  'request-customer-privacy-submit-fixture-a',
  'customer_privacy.case.submit',
  '1.0.0',
  1,
  1,
  1
);

SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-b';
SET LOCAL app.actor_id = 'actor-b';
SET LOCAL app.request_id = 'request-customer-privacy-submit-fixture-b';
SET LOCAL app.capability_id = 'customer_privacy.case.submit';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-customer-privacy-submit-fixture-b';

INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES (
  'tenant-b',
  'privacy-officer',
  'service',
  'active',
  'Tenant B privacy officer fixture',
  'tx-customer-privacy-submit-fixture-b'
);

INSERT INTO crm.idempotency_records (
  tenant_id,
  idempotency_scope,
  idempotency_key,
  request_hash,
  status,
  business_transaction_id,
  expires_at
)
VALUES (
  'tenant-b',
  'customer_privacy.case.submit@1.0.0',
  'customer-privacy-submit-fixture-b',
  decode(repeat('70', 32), 'hex'),
  'completed',
  'tx-customer-privacy-submit-fixture-b',
  clock_timestamp() + interval '1 day'
);

INSERT INTO crm.outbox_events (
  tenant_id,
  event_id,
  business_transaction_id,
  aggregate_type,
  aggregate_id,
  aggregate_version,
  event_sequence,
  event_type,
  deduplication_key,
  schema_id,
  schema_version,
  descriptor_hash,
  data_class,
  payload_encoding,
  maximum_payload_size,
  retention_policy_id,
  payload_bytes,
  occurred_at
)
VALUES (
  'tenant-b',
  'event-customer-privacy-submit-fixture-b',
  'tx-customer-privacy-submit-fixture-b',
  'crm.actor',
  'privacy-officer',
  1,
  1,
  'actor.created',
  'customer-privacy-submit-fixture-b',
  'crm.actor.created.v1',
  '1.0.0',
  decode(repeat('71', 32), 'hex'),
  'internal',
  'protobuf',
  16,
  'standard',
  decode('02', 'hex'),
  clock_timestamp()
);

INSERT INTO crm.audit_records (
  tenant_id,
  audit_sequence,
  audit_record_id,
  business_transaction_id,
  actor_id,
  capability_id,
  capability_version,
  canonicalization_profile,
  previous_hash,
  record_hash,
  canonical_envelope,
  occurred_at
)
SELECT
  'tenant-b',
  audit_sequence + 1,
  'audit-customer-privacy-submit-fixture-b',
  'tx-customer-privacy-submit-fixture-b',
  'privacy-officer',
  'customer_privacy.case.submit',
  '1.0.0',
  'crm.cjson/v1',
  record_hash,
  decode(repeat('72', 32), 'hex'),
  convert_to('{"customer_privacy":"submit_fixture_b"}', 'UTF8'),
  clock_timestamp()
FROM crm.audit_records
WHERE tenant_id = 'tenant-b'
ORDER BY audit_sequence DESC
LIMIT 1;

INSERT INTO crm.business_transactions (
  tenant_id,
  business_transaction_id,
  actor_id,
  request_id,
  capability_id,
  capability_version,
  expected_outbox_events,
  expected_audit_records,
  expected_idempotency_records
)
VALUES (
  'tenant-b',
  'tx-customer-privacy-submit-fixture-b',
  'privacy-officer',
  'request-customer-privacy-submit-fixture-b',
  'customer_privacy.case.submit',
  '1.0.0',
  1,
  1,
  1
);

SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;

RESET ROLE;

SELECT 'Customer Privacy case-submit fixture PASS' AS result;

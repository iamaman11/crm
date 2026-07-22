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
)
ON CONFLICT (tenant_id, actor_id) DO UPDATE
SET actor_type = EXCLUDED.actor_type,
    status = EXCLUDED.status,
    display_name = EXCLUDED.display_name,
    last_business_transaction_id = EXCLUDED.last_business_transaction_id;

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
  0,
  0,
  0
)
ON CONFLICT (tenant_id, business_transaction_id) DO NOTHING;
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
)
ON CONFLICT (tenant_id, actor_id) DO UPDATE
SET actor_type = EXCLUDED.actor_type,
    status = EXCLUDED.status,
    display_name = EXCLUDED.display_name,
    last_business_transaction_id = EXCLUDED.last_business_transaction_id;

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
  0,
  0,
  0
)
ON CONFLICT (tenant_id, business_transaction_id) DO NOTHING;
COMMIT;

RESET ROLE;

SELECT 'Customer Privacy case-submit fixture PASS' AS result;

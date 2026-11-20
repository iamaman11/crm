-- Production-process acceptance fixture for Phase 8A.7 customer data operations.
-- Runtime catalogs and live authorization remain authoritative. These durable rows satisfy
-- capability/audit foreign keys for the public import mutations and private worker outcomes.

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
  'crm.customer-data-operations',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('c8', 32), 'hex'),
  '{}'::jsonb,
  clock_timestamp(),
  'platform'
)
ON CONFLICT (module_id, version) DO NOTHING;

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
VALUES
  ('customer_data.import.party.source.create', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsSourceService', 'CreatePartyImportSourceArtifact', decode(repeat('31', 32), 'hex'), decode(repeat('32', 32), 'hex'), 'high', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.source.chunk.append', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsSourceService', 'AppendPartyImportSourceChunk', decode(repeat('33', 32), 'hex'), decode(repeat('34', 32), 'hex'), 'high', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.source.finalize', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsSourceService', 'FinalizePartyImportSourceArtifact', decode(repeat('35', 32), 'hex'), decode(repeat('36', 32), 'hex'), 'high', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.source.job.create', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsSourceService', 'CreatePartyImportJobFromSourceArtifact', decode(repeat('37', 32), 'hex'), decode(repeat('38', 32), 'hex'), 'high', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.source.rows.validate', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsSourceService', 'ValidatePartyImportSourceBatch', decode(repeat('39', 32), 'hex'), decode(repeat('3a', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.validation.finalize', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsService', 'FinalizePartyImportValidation', decode(repeat('3b', 32), 'hex'), decode(repeat('3c', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.execution.start', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsService', 'StartPartyImportExecution', decode(repeat('3d', 32), 'hex'), decode(repeat('3e', 32), 'hex'), 'high', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.cancel', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataOperationsService', 'CancelPartyImportJob', decode(repeat('3f', 32), 'hex'), decode(repeat('40', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.internal.skip_invalid', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations_internal.v1.CustomerDataOperationsInternalService', 'CommitPartyImportInvalidSkip', decode(repeat('41', 32), 'hex'), decode(repeat('42', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.internal.record_success', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations_internal.v1.CustomerDataOperationsInternalService', 'CommitPartyImportSuccess', decode(repeat('43', 32), 'hex'), decode(repeat('44', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.internal.record_retryable_failure', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations_internal.v1.CustomerDataOperationsInternalService', 'RecordPartyImportRetryableFailure', decode(repeat('45', 32), 'hex'), decode(repeat('46', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[]),
  ('customer_data.import.party.internal.complete', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations_internal.v1.CustomerDataOperationsInternalService', 'CompletePartyImportExecution', decode(repeat('47', 32), 'hex'), decode(repeat('48', 32), 'hex'), 'medium', true, true, false, false, false, false, false, ARRAY['personal']::text[])
ON CONFLICT (capability_id, capability_version) DO NOTHING;

-- The background worker is a distinct governed actor. Persist it once using the existing
-- bootstrap actor and transaction-local execution context required by crm.require_write_context().
SET ROLE crm_app_test;
BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-import-worker-fixture';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-bootstrap-a';

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
  'crm-api-import-execution-worker',
  'service',
  'active',
  'CRM API import execution worker',
  'tx-bootstrap-a'
)
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;
RESET ROLE;

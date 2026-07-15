-- Production-process acceptance fixture for Phase 8A.8 customer-data Party exports.
-- Runtime catalogs and live authorization remain authoritative. These durable registry rows and
-- worker actors satisfy capability/audit foreign keys for the public export lifecycle and the
-- private selection/execution workers exercised by the real crm-api process.

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
  ('customer_data.export.party.create', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportService', 'CreatePartyExportJob', decode(repeat('51', 32), 'hex'), decode(repeat('52', 32), 'hex'), 'medium', true, true, false, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.execution.start', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportService', 'StartPartyExportExecution', decode(repeat('53', 32), 'hex'), decode(repeat('54', 32), 'hex'), 'high', true, true, true, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.cancel', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportService', 'CancelPartyExportJob', decode(repeat('55', 32), 'hex'), decode(repeat('56', 32), 'hex'), 'medium', true, true, false, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.selection.page.commit', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportInternalService', 'CommitPartyExportSelectionPage', decode(repeat('57', 32), 'hex'), decode(repeat('58', 32), 'hex'), 'medium', true, true, false, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.selection.finalize', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportInternalService', 'FinalizePartyExportSelection', decode(repeat('59', 32), 'hex'), decode(repeat('5a', 32), 'hex'), 'medium', true, true, false, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.execution.stage', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportInternalService', 'StagePartyExportExecution', decode(repeat('5b', 32), 'hex'), decode(repeat('5c', 32), 'hex'), 'medium', true, true, false, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.execution.outcome.commit', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportInternalService', 'CommitPartyExportExecutionOutcome', decode(repeat('5d', 32), 'hex'), decode(repeat('5e', 32), 'hex'), 'medium', true, true, false, false, false, false, true, ARRAY['personal']::text[]),
  ('customer_data.export.party.execution.complete', '1.0.0', 'crm.customer-data-operations', '0.1.0', 'crm.customer_data_operations.v1.CustomerDataExportInternalService', 'CompletePartyExportExecution', decode(repeat('5f', 32), 'hex'), decode(repeat('60', 32), 'hex'), 'high', true, true, false, false, false, false, true, ARRAY['personal']::text[])
ON CONFLICT (capability_id, capability_version) DO NOTHING;

-- Selection and execution use distinct governed service actors. Persist both once using the
-- existing bootstrap actor and transaction-local write context required by crm.require_write_context().
SET ROLE crm_app_test;
BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'actor-a';
SET LOCAL app.request_id = 'request-export-workers-fixture';
SET LOCAL app.capability_id = 'test.record.mutate';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'tx-export-workers-bootstrap-a';

INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES
  (
    'tenant-a',
    'crm-api-export-selection-worker',
    'service',
    'active',
    'CRM API export selection worker',
    'tx-export-workers-bootstrap-a'
  ),
  (
    'tenant-a',
    'crm-api-export-execution-worker',
    'service',
    'active',
    'CRM API export execution worker',
    'tx-export-workers-bootstrap-a'
  )
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;
RESET ROLE;

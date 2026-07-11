\set ON_ERROR_STOP on

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
  'crm.activities',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('ac', 32), 'hex'),
  '{}'::jsonb,
  clock_timestamp(),
  'platform'
),
(
  'crm.sales',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('ad', 32), 'hex'),
  '{}'::jsonb,
  clock_timestamp(),
  'platform'
);

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
    'activities.task.create', '1.0.0', 'crm.activities', '0.1.0',
    'crm.activities.v1.TaskService', 'CreateTask',
    decode(repeat('31', 32), 'hex'), decode(repeat('32', 32), 'hex'),
    'low', true, true, false, false, false, false, false
  ),
  (
    'activities.task.complete', '1.0.0', 'crm.activities', '0.1.0',
    'crm.activities.v1.TaskService', 'CompleteTask',
    decode(repeat('33', 32), 'hex'), decode(repeat('34', 32), 'hex'),
    'low', true, true, false, false, false, false, false
  ),
  (
    'sales.deal.create', '1.0.0', 'crm.sales', '0.1.0',
    'crm.sales.v1.DealService', 'CreateDeal',
    decode(repeat('35', 32), 'hex'), decode(repeat('36', 32), 'hex'),
    'medium', true, true, false, false, false, false, false
  ),
  (
    'sales.deal.update', '1.0.0', 'crm.sales', '0.1.0',
    'crm.sales.v1.DealService', 'UpdateDeal',
    decode(repeat('37', 32), 'hex'), decode(repeat('38', 32), 'hex'),
    'medium', true, true, false, false, false, false, false
  ),
  (
    'sales.deal.advance_stage', '1.0.0', 'crm.sales', '0.1.0',
    'crm.sales.v1.DealService', 'AdvanceStage',
    decode(repeat('39', 32), 'hex'), decode(repeat('3a', 32), 'hex'),
    'medium', true, true, false, false, false, false, false
  );

\set ON_ERROR_STOP on

INSERT INTO crm.tenants (tenant_id, status, data_region)
VALUES
  ('tenant-application-a', 'active', 'eu-central'),
  ('tenant-application-orchestration-a', 'active', 'eu-central'),
  ('tenant-suggestion-production-a', 'active', 'eu-central'),
  ('tenant-suggestion-production-b', 'active', 'eu-central')
ON CONFLICT (tenant_id) DO NOTHING;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-a';
SET LOCAL app.actor_id = 'review-fixture-bootstrap';
SET LOCAL app.request_id = 'review-fixture-actors-tenant-a';
SET LOCAL app.capability_id = 'customer_enrichment.review.seed';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'review-fixture-actors-tenant-a';
INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES
  ('tenant-a', 'reviewer-a', 'user', 'active', 'Customer Enrichment reviewer', 'review-fixture-actors-tenant-a'),
  ('tenant-a', 'customer-enrichment-provider-worker', 'service', 'active', 'Customer Enrichment provider worker', 'review-fixture-actors-tenant-a'),
  ('tenant-a', 'customer-enrichment-materialization-worker', 'service', 'active', 'Customer Enrichment materialization worker', 'review-fixture-actors-tenant-a')
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-application-a';
SET LOCAL app.actor_id = 'review-fixture-bootstrap';
SET LOCAL app.request_id = 'review-fixture-actors-application';
SET LOCAL app.capability_id = 'customer_enrichment.application.seed';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'review-fixture-actors-application';
INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES (
  'tenant-application-a',
  'application-reviewer-a',
  'user',
  'active',
  'Customer Enrichment application reviewer',
  'review-fixture-actors-application'
)
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-application-orchestration-a';
SET LOCAL app.actor_id = 'review-fixture-bootstrap';
SET LOCAL app.request_id = 'review-fixture-actors-orchestration';
SET LOCAL app.capability_id = 'customer_enrichment.application_orchestration.seed';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'review-fixture-actors-orchestration';
INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES (
  'tenant-application-orchestration-a',
  'application-orchestrator-a',
  'service',
  'active',
  'Customer Enrichment application orchestrator',
  'review-fixture-actors-orchestration'
)
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;

BEGIN;
SET LOCAL app.tenant_id = 'tenant-suggestion-production-a';
SET LOCAL app.actor_id = 'review-fixture-bootstrap';
SET LOCAL app.request_id = 'review-fixture-actors-production';
SET LOCAL app.capability_id = 'customer_enrichment.review.seed';
SET LOCAL app.capability_version = '1.0.0';
SET LOCAL app.business_transaction_id = 'review-fixture-actors-production';
INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  last_business_transaction_id
)
VALUES
  ('tenant-suggestion-production-a', 'suggestion-production-reader-a', 'user', 'active', 'Customer Enrichment production reviewer', 'review-fixture-actors-production'),
  ('tenant-suggestion-production-a', 'customer-enrichment-application-worker', 'service', 'active', 'Customer Enrichment application worker', 'review-fixture-actors-production')
ON CONFLICT (tenant_id, actor_id) DO NOTHING;
COMMIT;

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
  'crm.parties',
  '0.1.0',
  'crm.cjson/v1',
  decode(repeat('70', 32), 'hex'),
  '{"test_fixture":"customer_enrichment_review"}'::jsonb,
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
  required_permissions,
  data_classes_touched
)
VALUES
  (
    'customer_enrichment.review.seed',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.test.ReviewFixtureService',
    'SeedSuggestion',
    decode(repeat('61', 32), 'hex'),
    decode(repeat('64', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.review.seed']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.application.seed',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.test.ApplicationFixtureService',
    'SeedEvidence',
    decode(repeat('65', 32), 'hex'),
    decode(repeat('66', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.application.seed']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.application_orchestration.seed',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.test.ApplicationOrchestrationFixtureService',
    'SeedEvidence',
    decode(repeat('6b', 32), 'hex'),
    decode(repeat('6c', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.application_orchestration.seed']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.application.worker.seed',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.test.ApplicationWorkerFixtureService',
    'SeedEvidence',
    decode(repeat('6d', 32), 'hex'),
    decode(repeat('6e', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.application.worker.seed']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.suggestion.accept',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService',
    'AcceptSuggestion',
    decode(repeat('62', 32), 'hex'),
    decode(repeat('63', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.suggestion.accept']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.suggestion.reject',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService',
    'RejectSuggestion',
    decode(repeat('71', 32), 'hex'),
    decode(repeat('72', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.suggestion.reject']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.party.display_name.apply',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentSuggestionService',
    'ApplyPartyDisplayNameSuggestion',
    decode(repeat('67', 32), 'hex'),
    decode(repeat('68', 32), 'hex'),
    'high', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.party.display_name.apply']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'customer_enrichment.application.outcome.record',
    '1.0.0',
    'crm.customer-enrichment',
    '0.1.0',
    'crm.customer_enrichment.v1.CustomerEnrichmentWorkerService',
    'RecordApplicationOutcome',
    decode(repeat('69', 32), 'hex'),
    decode(repeat('6a', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['customer_enrichment.application.outcome.record']::text[],
    ARRAY['personal', 'confidential']::text[]
  ),
  (
    'parties.party.create',
    '1.0.0',
    'crm.parties',
    '0.1.0',
    'crm.parties.v1.PartyService',
    'CreateParty',
    decode(repeat('73', 32), 'hex'),
    decode(repeat('74', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['parties.party.create']::text[],
    ARRAY['personal']::text[]
  ),
  (
    'parties.party.update',
    '1.0.0',
    'crm.parties',
    '0.1.0',
    'crm.parties.v1.PartyService',
    'UpdateParty',
    decode(repeat('75', 32), 'hex'),
    decode(repeat('76', 32), 'hex'),
    'medium', true, true, false, false, false, false, false,
    ARRAY['parties.party.update']::text[],
    ARRAY['personal']::text[]
  )
ON CONFLICT (capability_id, capability_version) DO NOTHING;

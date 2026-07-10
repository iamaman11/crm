\set ON_ERROR_STOP on

\ir ../migrations/0001_platform_foundation.up.sql

SET session_replication_role = replica;

BEGIN;

INSERT INTO crm.tenants (tenant_id, status, data_region)
VALUES ('tenant-upgrade', 'active', 'eu-central');

INSERT INTO crm.actors (
  tenant_id,
  actor_id,
  actor_type,
  status,
  display_name,
  attributes,
  last_business_transaction_id
)
VALUES (
  'tenant-upgrade',
  'actor-upgrade',
  'user',
  'active',
  'Legacy Actor',
  '{"locale":"pl","source":"legacy"}'::jsonb,
  'transaction-upgrade'
);

INSERT INTO crm.records (
  tenant_id,
  record_type,
  record_id,
  version,
  owner_module_id,
  schema_id,
  schema_version,
  descriptor_hash,
  data_class,
  maximum_payload_size,
  retention_policy_id,
  payload_bytes,
  typed_projection,
  last_business_transaction_id
)
VALUES
  (
    'tenant-upgrade', 'sales.deal', 'deal-upgrade-1', 1,
    'crm.sales', 'crm.sales.deal', '1.0.0', decode(repeat('11', 32), 'hex'),
    'internal', 64, 'standard', convert_to('legacy-one', 'UTF8'),
    '{"name":"Legacy One"}'::jsonb, 'transaction-upgrade'
  ),
  (
    'tenant-upgrade', 'sales.deal', 'deal-upgrade-2', 1,
    'crm.sales', 'crm.sales.deal', '1.0.0', decode(repeat('12', 32), 'hex'),
    'internal', 64, 'standard', convert_to('legacy-two', 'UTF8'),
    '{"name":"Legacy Two"}'::jsonb, 'transaction-upgrade'
  );

INSERT INTO crm.relationships (
  tenant_id,
  relationship_type,
  source_record_type,
  source_record_id,
  target_record_type,
  target_record_id,
  version,
  attributes,
  last_business_transaction_id
)
VALUES (
  'tenant-upgrade',
  'sales.deal.related',
  'sales.deal',
  'deal-upgrade-1',
  'sales.deal',
  'deal-upgrade-2',
  1,
  '{"strength":7,"source":"legacy"}'::jsonb,
  'transaction-upgrade'
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
  schema_id,
  schema_version,
  descriptor_hash,
  data_class,
  maximum_payload_size,
  retention_policy_id,
  payload_bytes,
  occurred_at
)
VALUES
  (
    'tenant-upgrade', 'event-upgrade-1', 'transaction-upgrade',
    'sales.deal', 'deal-upgrade-1', 1, 1, 'sales.deal.created',
    'crm.sales.deal.created', '1.0.0', decode(repeat('21', 32), 'hex'),
    'internal', 64, 'standard', convert_to('event-one', 'UTF8'), clock_timestamp()
  ),
  (
    'tenant-upgrade', 'event-upgrade-2', 'transaction-upgrade',
    'sales.deal', 'deal-upgrade-2', 1, 1, 'sales.deal.created',
    'crm.sales.deal.created', '1.0.0', decode(repeat('22', 32), 'hex'),
    'internal', 64, 'standard', convert_to('event-two', 'UTF8'), clock_timestamp()
  );

INSERT INTO crm.workflow_runs (
  tenant_id,
  workflow_run_id,
  workflow_id,
  workflow_version,
  status,
  generation,
  state_schema_id,
  state_schema_version,
  state_descriptor_hash,
  state_data_class,
  maximum_state_size,
  retention_policy_id,
  state_bytes,
  last_business_transaction_id
)
VALUES (
  'tenant-upgrade', 'workflow-run-upgrade', 'workflow-upgrade', '1.0.0',
  'running', 1, 'crm.workflow.state', '1.0.0', decode(repeat('31', 32), 'hex'),
  'internal', 64, 'standard', convert_to('workflow-state', 'UTF8'),
  'transaction-upgrade'
);

INSERT INTO crm.module_state (
  tenant_id,
  module_id,
  state_key,
  version,
  schema_id,
  schema_version,
  descriptor_hash,
  data_class,
  maximum_payload_size,
  retention_policy_id,
  payload_bytes,
  last_business_transaction_id
)
VALUES (
  'tenant-upgrade', 'crm.sales', 'legacy-cursor', 1,
  'crm.sales.cursor', '1.0.0', decode(repeat('41', 32), 'hex'),
  'internal', 64, 'standard', convert_to('cursor-state', 'UTF8'),
  'transaction-upgrade'
);

INSERT INTO crm.idempotency_records (
  tenant_id,
  idempotency_scope,
  idempotency_key,
  request_hash,
  status,
  response_schema_id,
  response_schema_version,
  response_descriptor_hash,
  response_payload,
  business_transaction_id,
  expires_at
)
VALUES (
  'tenant-upgrade', 'legacy', 'legacy-request', decode(repeat('51', 32), 'hex'),
  'completed', 'crm.legacy.response', '1.0.0', decode(repeat('52', 32), 'hex'),
  convert_to('legacy-response', 'UTF8'), 'transaction-upgrade', clock_timestamp() + interval '1 day'
);

COMMIT;

SET session_replication_role = origin;

\ir ../migrations/0002_typed_payload_encoding.up.sql
\ir ../migrations/0003_relationship_payload_governance.up.sql

DO $$
DECLARE
  projection jsonb;
  relationship_projection jsonb;
  relationship_owner text;
  relationship_schema text;
  relationship_encoding text;
  relationship_maximum bigint;
  relationship_payload bytea;
  legacy_event_keys text[];
BEGIN
  SELECT typed_projection
  INTO projection
  FROM crm.actors
  WHERE tenant_id = 'tenant-upgrade' AND actor_id = 'actor-upgrade';

  IF projection <> '{"locale":"pl","source":"legacy"}'::jsonb THEN
    RAISE EXCEPTION 'actor projection was not preserved: %', projection;
  END IF;

  IF EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'crm'
      AND table_name IN ('actors', 'relationships')
      AND column_name = 'attributes'
  ) THEN
    RAISE EXCEPTION 'legacy attributes columns still exist after migration 0003';
  END IF;

  IF (SELECT count(*) FROM crm.records WHERE tenant_id = 'tenant-upgrade' AND payload_encoding = 'protobuf') <> 2 THEN
    RAISE EXCEPTION 'record payload encoding backfill failed';
  END IF;

  SELECT array_agg(deduplication_key ORDER BY event_id)
  INTO legacy_event_keys
  FROM crm.outbox_events
  WHERE tenant_id = 'tenant-upgrade' AND event_type = 'sales.deal.created';

  IF legacy_event_keys <> ARRAY['legacy:event-upgrade-1', 'legacy:event-upgrade-2'] THEN
    RAISE EXCEPTION 'collision-safe outbox deduplication backfill failed: %', legacy_event_keys;
  END IF;

  IF (SELECT count(*) FROM crm.outbox_events WHERE tenant_id = 'tenant-upgrade' AND payload_encoding = 'protobuf') <> 2 THEN
    RAISE EXCEPTION 'outbox payload encoding backfill failed';
  END IF;

  IF (SELECT state_encoding FROM crm.workflow_runs WHERE tenant_id = 'tenant-upgrade' AND workflow_run_id = 'workflow-run-upgrade') <> 'protobuf' THEN
    RAISE EXCEPTION 'workflow state encoding backfill failed';
  END IF;

  IF (SELECT payload_encoding FROM crm.module_state WHERE tenant_id = 'tenant-upgrade' AND module_id = 'crm.sales' AND state_key = 'legacy-cursor') <> 'protobuf' THEN
    RAISE EXCEPTION 'module state encoding backfill failed';
  END IF;

  IF (SELECT response_payload_encoding FROM crm.idempotency_records WHERE tenant_id = 'tenant-upgrade' AND idempotency_key = 'legacy-request') IS NOT NULL THEN
    RAISE EXCEPTION 'legacy idempotency response encoding should remain unknown/null';
  END IF;

  SELECT typed_projection, owner_module_id, schema_id, payload_encoding, maximum_payload_size, payload_bytes
  INTO relationship_projection, relationship_owner, relationship_schema, relationship_encoding, relationship_maximum, relationship_payload
  FROM crm.relationships
  WHERE tenant_id = 'tenant-upgrade'
    AND relationship_type = 'sales.deal.related'
    AND source_record_id = 'deal-upgrade-1'
    AND target_record_id = 'deal-upgrade-2';

  IF relationship_projection <> '{"strength":7,"source":"legacy"}'::jsonb THEN
    RAISE EXCEPTION 'relationship projection was not preserved: %', relationship_projection;
  END IF;
  IF relationship_owner <> 'platform'
     OR relationship_schema <> 'crm.relationship.empty.v1'
     OR relationship_encoding <> 'binary'
     OR relationship_maximum <> 0
     OR octet_length(relationship_payload) <> 0 THEN
    RAISE EXCEPTION 'relationship authoritative payload defaults are invalid';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'crm'
      AND table_name = 'relationships'
      AND column_name IN (
        'owner_module_id', 'schema_id', 'schema_version', 'descriptor_hash',
        'data_class', 'payload_encoding', 'maximum_payload_size',
        'retention_policy_id', 'payload_bytes'
      )
      AND column_default IS NOT NULL
  ) THEN
    RAISE EXCEPTION 'relationship migration defaults were not removed';
  END IF;
END;
$$;

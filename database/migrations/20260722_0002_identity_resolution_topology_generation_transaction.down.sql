BEGIN;

ALTER TABLE IF EXISTS crm.identity_resolution_topology_generations
  DROP CONSTRAINT IF EXISTS identity_resolution_topology_generation_business_transaction_fk;

COMMIT;

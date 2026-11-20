BEGIN;

ALTER TABLE crm.identity_resolution_topology_generations
  ADD CONSTRAINT identity_resolution_topology_generation_business_transaction_fk
  FOREIGN KEY (tenant_id, last_business_transaction_id)
  REFERENCES crm.business_transactions (tenant_id, business_transaction_id)
  DEFERRABLE INITIALLY DEFERRED;

COMMENT ON CONSTRAINT identity_resolution_topology_generation_business_transaction_fk
  ON crm.identity_resolution_topology_generations IS
  'Binds every authoritative Identity Resolution topology generation advance to the same tenant business transaction as the canonical redirect mutation';

COMMIT;

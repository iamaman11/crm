\set ON_ERROR_STOP on

-- Test-environment role provisioning for the Phase 7 search table.
-- Schema migrations intentionally do not reference the CI-only crm_app_test role;
-- runtime role ownership and grants remain separate from schema ownership.
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'crm_app_test') THEN
    RAISE EXCEPTION 'crm_app_test must be provisioned before search runtime grants are applied';
  END IF;
END;
$$;

GRANT SELECT, INSERT, UPDATE, DELETE
  ON TABLE crm.search_index_generations
  TO crm_app_test;

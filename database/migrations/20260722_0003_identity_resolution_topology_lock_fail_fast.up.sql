BEGIN;

-- Merge/unmerge historically lock their aggregate before the canonical relationship
-- trigger reaches the tenant topology lock. Authoritative consumers may already hold
-- the topology lock while proving that same merge-operation row. A blocking advisory
-- lock on both sides can therefore form an avoidable wait cycle. The shared primitive
-- is deliberately fail-fast: one transaction receives SQLSTATE 55P03 and retries from
-- a fresh authoritative snapshot instead of relying on deadlock detection.
CREATE OR REPLACE FUNCTION crm.lock_identity_resolution_topology(bound_tenant text)
RETURNS void
LANGUAGE plpgsql
VOLATILE
PARALLEL UNSAFE
AS $$
BEGIN
  IF bound_tenant IS NULL
     OR bound_tenant = ''
     OR bound_tenant IS DISTINCT FROM crm.current_tenant_id() THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'identity resolution topology lock tenant does not match the bound tenant context';
  END IF;

  IF NOT pg_try_advisory_xact_lock(
    hashtextextended(
      'crm.identity-resolution.canonical-redirect|' || bound_tenant,
      0
    )
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '55P03',
      MESSAGE = 'identity resolution topology is busy';
  END IF;
END;
$$;

COMMENT ON FUNCTION crm.lock_identity_resolution_topology(text) IS
  'Shared fail-fast transaction lock for one tenant Identity Resolution canonical topology';

COMMIT;

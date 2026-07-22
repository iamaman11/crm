BEGIN;

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

  PERFORM pg_advisory_xact_lock(
    hashtextextended(
      'crm.identity-resolution.canonical-redirect|' || bound_tenant,
      0
    )
  );
END;
$$;

COMMENT ON FUNCTION crm.lock_identity_resolution_topology(text) IS NULL;

COMMIT;

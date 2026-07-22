BEGIN;

CREATE OR REPLACE FUNCTION crm.lock_customer_subject(
  bound_tenant text,
  canonical_party_id text
)
RETURNS void
LANGUAGE plpgsql
VOLATILE
PARALLEL UNSAFE
AS $$
DECLARE
  lock_identity text;
BEGIN
  IF bound_tenant IS NULL
     OR bound_tenant = ''
     OR bound_tenant IS DISTINCT FROM crm.current_tenant_id() THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'customer subject lock tenant does not match the bound tenant context';
  END IF;
  IF canonical_party_id IS NULL OR canonical_party_id = '' THEN
    RAISE EXCEPTION USING
      ERRCODE = '22023',
      MESSAGE = 'customer subject lock requires a canonical Party identity';
  END IF;

  lock_identity := format(
    'crm.customer.subject-lock/v1|%s:%s|%s:%s',
    octet_length(bound_tenant),
    bound_tenant,
    octet_length(canonical_party_id),
    canonical_party_id
  );
  PERFORM pg_advisory_xact_lock(hashtextextended(lock_identity, 0));
END;
$$;

COMMENT ON FUNCTION crm.lock_customer_subject(text, text) IS
  'Shared transaction-scoped tenant plus canonical Party final lock';

COMMIT;

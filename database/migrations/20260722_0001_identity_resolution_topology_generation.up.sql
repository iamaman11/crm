BEGIN;

-- Identity Resolution owns one monotonic topology generation per tenant. Generation 1
-- represents the canonical redirect topology at migration installation time; every later
-- merge or unmerge advances it exactly once in the same transaction as the redirect edge.
CREATE TABLE crm.identity_resolution_topology_generations (
  tenant_id text PRIMARY KEY REFERENCES crm.tenants (tenant_id) ON DELETE CASCADE,
  generation bigint NOT NULL CHECK (generation > 0),
  last_business_transaction_id text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

ALTER TABLE crm.identity_resolution_topology_generations ENABLE ROW LEVEL SECURITY;
ALTER TABLE crm.identity_resolution_topology_generations FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation
  ON crm.identity_resolution_topology_generations
  USING (tenant_id = crm.current_tenant_id())
  WITH CHECK (tenant_id = crm.current_tenant_id());

CREATE TRIGGER require_write_context
BEFORE INSERT OR UPDATE OR DELETE ON crm.identity_resolution_topology_generations
FOR EACH ROW EXECUTE FUNCTION crm.require_write_context();

-- This is the single transaction-scoped lock used by canonical redirect mutations and
-- authoritative consumers that must prove one stable topology snapshot before commit.
CREATE FUNCTION crm.lock_identity_resolution_topology(bound_tenant text)
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

-- Shared final subject lock for privacy restrictions and every protected owner boundary.
-- The length-framed identity prevents ambiguous concatenation and is deliberately owned by
-- the platform schema rather than one capability-specific storage path.
CREATE FUNCTION crm.lock_customer_subject(bound_tenant text, canonical_party_id text)
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

-- Callers never read the generation table directly. The function validates transaction-local
-- tenant binding, returns baseline generation 1 for a topology that has not changed since this
-- migration, and exposes no cross-tenant existence signal.
CREATE FUNCTION crm.current_identity_resolution_generation(bound_tenant text)
RETURNS bigint
LANGUAGE plpgsql
STABLE
PARALLEL UNSAFE
SECURITY DEFINER
SET search_path = pg_catalog, crm
AS $$
DECLARE
  current_generation bigint;
BEGIN
  IF bound_tenant IS NULL
     OR bound_tenant = ''
     OR bound_tenant IS DISTINCT FROM crm.current_tenant_id() THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'identity resolution generation tenant does not match the bound tenant context';
  END IF;

  SELECT topology.generation
    INTO current_generation
    FROM crm.identity_resolution_topology_generations AS topology
   WHERE topology.tenant_id = bound_tenant;

  RETURN COALESCE(current_generation, 1);
END;
$$;

-- Reuse the exact existing topology lock from migration 0011 through the shared function.
-- The trigger remains the final database constraint for owner/type/root/cycle integrity.
CREATE OR REPLACE FUNCTION crm.enforce_identity_resolution_canonical_redirect()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
  bound_tenant text;
  cycle_exists boolean;
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.relationship_type <> 'identity_resolution.canonical_redirect' THEN
      RETURN NEW;
    END IF;
    bound_tenant := NEW.tenant_id;
  ELSIF TG_OP = 'DELETE' THEN
    IF OLD.relationship_type <> 'identity_resolution.canonical_redirect' THEN
      RETURN OLD;
    END IF;
    bound_tenant := OLD.tenant_id;
  ELSE
    RETURN NEW;
  END IF;

  PERFORM crm.lock_identity_resolution_topology(bound_tenant);

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;

  IF NEW.owner_module_id <> 'crm.identity-resolution'
     OR NEW.source_record_type <> 'parties.party'
     OR NEW.target_record_type <> 'parties.party' THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'identity resolution canonical redirects must be crm.identity-resolution-owned Party-to-Party relationships';
  END IF;

  IF NEW.source_record_id = NEW.target_record_id THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'identity resolution canonical redirect cannot target the source Party itself';
  END IF;

  IF EXISTS (
    SELECT 1
      FROM crm.relationships AS existing
     WHERE existing.tenant_id = NEW.tenant_id
       AND existing.relationship_type = 'identity_resolution.canonical_redirect'
       AND existing.source_record_type = 'parties.party'
       AND existing.source_record_id = NEW.target_record_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'identity resolution canonical redirect target is not a current canonical root';
  END IF;

  WITH RECURSIVE canonical_path(party_id) AS (
    SELECT NEW.target_record_id
    UNION
    SELECT existing.target_record_id
      FROM crm.relationships AS existing
      JOIN canonical_path AS path
        ON existing.source_record_id = path.party_id
     WHERE existing.tenant_id = NEW.tenant_id
       AND existing.relationship_type = 'identity_resolution.canonical_redirect'
       AND existing.source_record_type = 'parties.party'
       AND existing.target_record_type = 'parties.party'
  )
  SELECT EXISTS (
    SELECT 1
      FROM canonical_path
     WHERE party_id = NEW.source_record_id
  )
  INTO cycle_exists;

  IF cycle_exists THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'identity resolution canonical redirect would create a cycle';
  END IF;

  RETURN NEW;
END;
$$;

-- This trigger runs only after the canonical edge itself has passed all authoritative
-- constraints. SECURITY DEFINER is limited to the fixed table and preserves the caller's
-- transaction-local context for the platform write-context trigger.
CREATE FUNCTION crm.advance_identity_resolution_topology_generation()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, crm
AS $$
DECLARE
  bound_tenant text;
  current_generation bigint;
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.relationship_type <> 'identity_resolution.canonical_redirect' THEN
      RETURN NEW;
    END IF;
    bound_tenant := NEW.tenant_id;
  ELSIF TG_OP = 'DELETE' THEN
    IF OLD.relationship_type <> 'identity_resolution.canonical_redirect' THEN
      RETURN OLD;
    END IF;
    bound_tenant := OLD.tenant_id;
  ELSE
    RAISE EXCEPTION USING
      ERRCODE = '55000',
      MESSAGE = 'identity resolution topology generation supports only canonical redirect insert or delete';
  END IF;

  IF bound_tenant IS DISTINCT FROM crm.current_tenant_id() THEN
    RAISE EXCEPTION USING
      ERRCODE = '42501',
      MESSAGE = 'identity resolution topology generation tenant does not match the bound tenant context';
  END IF;
  IF crm.current_business_transaction_id() IS NULL THEN
    RAISE EXCEPTION USING
      ERRCODE = '28000',
      MESSAGE = 'identity resolution topology generation requires a bound business transaction';
  END IF;

  -- The BEFORE redirect guard already acquired the same tenant topology lock, so no
  -- concurrent merge/unmerge for this tenant can interleave between this read and write.
  SELECT topology.generation
    INTO current_generation
    FROM crm.identity_resolution_topology_generations AS topology
   WHERE topology.tenant_id = bound_tenant
   FOR UPDATE;

  IF FOUND THEN
    IF current_generation = 9223372036854775807 THEN
      RAISE EXCEPTION USING
        ERRCODE = '22003',
        MESSAGE = 'identity resolution topology generation exhausted its supported range';
    END IF;
    UPDATE crm.identity_resolution_topology_generations
       SET generation = current_generation + 1,
           last_business_transaction_id = crm.current_business_transaction_id(),
           updated_at = clock_timestamp()
     WHERE tenant_id = bound_tenant;
  ELSE
    INSERT INTO crm.identity_resolution_topology_generations (
      tenant_id,
      generation,
      last_business_transaction_id
    ) VALUES (
      bound_tenant,
      2,
      crm.current_business_transaction_id()
    );
  END IF;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER identity_resolution_topology_generation
AFTER INSERT OR DELETE ON crm.relationships
FOR EACH ROW
EXECUTE FUNCTION crm.advance_identity_resolution_topology_generation();

COMMENT ON TABLE crm.identity_resolution_topology_generations IS
  'Authoritative monotonic tenant generation for the current Identity Resolution canonical topology';
COMMENT ON FUNCTION crm.lock_customer_subject(text, text) IS
  'Shared transaction-scoped tenant plus canonical Party final lock';

COMMIT;

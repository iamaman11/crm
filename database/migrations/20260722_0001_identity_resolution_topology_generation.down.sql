BEGIN;

DROP TRIGGER IF EXISTS identity_resolution_topology_generation ON crm.relationships;
DROP FUNCTION IF EXISTS crm.advance_identity_resolution_topology_generation();

DROP TRIGGER IF EXISTS require_write_context ON crm.identity_resolution_topology_generations;
DROP POLICY IF EXISTS tenant_isolation ON crm.identity_resolution_topology_generations;
DROP TABLE IF EXISTS crm.identity_resolution_topology_generations;

-- Restore the exact migration-0011 implementation before removing the shared lock helper.
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

  PERFORM pg_advisory_xact_lock(
    hashtextextended(
      'crm.identity-resolution.canonical-redirect|' || bound_tenant,
      0
    )
  );

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

DROP FUNCTION IF EXISTS crm.current_identity_resolution_generation(text);
DROP FUNCTION IF EXISTS crm.lock_customer_subject(text, text);
DROP FUNCTION IF EXISTS crm.lock_identity_resolution_topology(text);

COMMIT;

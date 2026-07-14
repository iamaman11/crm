BEGIN;

-- Phase 8A.6: hard database invariants for current canonical Party redirection.
-- Immutable merge-operation records remain the lineage history. This relationship
-- type is the authoritative current-topology edge and is linked/unlinked atomically
-- with merge/unmerge mutations.

CREATE UNIQUE INDEX identity_resolution_canonical_redirect_source_uq
  ON crm.relationships (
    tenant_id,
    source_record_type,
    source_record_id
  )
  WHERE relationship_type = 'identity_resolution.canonical_redirect';

CREATE FUNCTION crm.enforce_identity_resolution_canonical_redirect()
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

  -- Serialize canonical-topology changes inside one tenant so concurrent root and
  -- cycle checks observe the topology committed by the preceding writer.
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

  -- A new merge survivor must still be a current canonical root at commit time.
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

  -- Defense in depth: follow the committed redirect chain from the proposed target.
  -- UNION (not UNION ALL) also terminates safely if pre-existing corruption exists.
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

CREATE TRIGGER identity_resolution_canonical_redirect_guard
BEFORE INSERT OR DELETE ON crm.relationships
FOR EACH ROW
EXECUTE FUNCTION crm.enforce_identity_resolution_canonical_redirect();

COMMIT;

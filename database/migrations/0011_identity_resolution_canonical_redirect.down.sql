BEGIN;

DROP TRIGGER IF EXISTS identity_resolution_canonical_redirect_guard ON crm.relationships;
DROP FUNCTION IF EXISTS crm.enforce_identity_resolution_canonical_redirect();
DROP INDEX IF EXISTS crm.identity_resolution_canonical_redirect_source_uq;

COMMIT;

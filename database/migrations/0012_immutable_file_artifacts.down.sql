BEGIN;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM crm.file_artifacts LIMIT 1) THEN
    RAISE EXCEPTION USING
      ERRCODE = '55000',
      MESSAGE = 'cannot remove immutable file artifact storage while retained artifacts exist';
  END IF;
END;
$$;

DROP TABLE IF EXISTS crm.file_artifact_chunks;
DROP TABLE IF EXISTS crm.file_artifacts;

COMMIT;

BEGIN;

DROP INDEX IF EXISTS crm.projection_documents_search_fts_idx;
DROP TABLE IF EXISTS crm.search_index_generations;

COMMIT;

BEGIN;

DROP TABLE IF EXISTS crm.metadata_transitions;
DROP TABLE IF EXISTS crm.metadata_rollback_stack;
DROP TABLE IF EXISTS crm.metadata_activation_heads;
DROP TABLE IF EXISTS crm.metadata_revision_dependencies;
DROP TABLE IF EXISTS crm.metadata_revision_documents;
DROP TABLE IF EXISTS crm.metadata_revisions_v2;

COMMIT;
